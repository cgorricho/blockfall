// BLOCKFALL resource dashboard (Go + tcell, Windows only).
//
// Finds the running games through the stats files they write to
// %TEMP%\blockfall\<go|cs|rs>.json, then samples each process once a second
// with Win32 calls: CPU time, memory counters, handles, threads and I/O.
package main

import (
	"encoding/json"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"
	"unsafe"

	"github.com/gdamore/tcell/v2"
	"golang.org/x/sys/windows"
)

// ---------- Win32 ----------

var (
	kernel32                  = windows.NewLazySystemDLL("kernel32.dll")
	procGetProcessMemoryInfo  = kernel32.NewProc("K32GetProcessMemoryInfo")
	procGetProcessHandleCount = kernel32.NewProc("GetProcessHandleCount")
	procGetProcessIoCounters  = kernel32.NewProc("GetProcessIoCounters")
)

// PROCESS_MEMORY_COUNTERS_EX
type memCounters struct {
	cb                         uint32
	PageFaultCount             uint32
	PeakWorkingSetSize         uintptr
	WorkingSetSize             uintptr
	QuotaPeakPagedPoolUsage    uintptr
	QuotaPagedPoolUsage        uintptr
	QuotaPeakNonPagedPoolUsage uintptr
	QuotaNonPagedPoolUsage     uintptr
	PagefileUsage              uintptr
	PeakPagefileUsage          uintptr
	PrivateUsage               uintptr
}

// IO_COUNTERS
type ioCounters struct {
	ReadOps, WriteOps, OtherOps, ReadBytes, WriteBytes, OtherBytes uint64
}

type procSample struct {
	t        time.Time
	cpu      time.Duration // user + kernel, total since start
	ws       uint64        // working set (RAM actually in use)
	peakWS   uint64
	private  uint64 // private bytes (commit charge)
	faults   uint32
	handles  uint32
	ioWrite  uint64
	ioReadOp uint64
}

func filetime(f windows.Filetime) time.Duration {
	return time.Duration((uint64(f.HighDateTime)<<32 | uint64(f.LowDateTime)) * 100)
}

func sampleProc(pid uint32) (procSample, error) {
	h, err := windows.OpenProcess(windows.PROCESS_QUERY_LIMITED_INFORMATION|windows.PROCESS_VM_READ, false, pid)
	if err != nil {
		return procSample{}, err
	}
	defer windows.CloseHandle(h)

	var code uint32
	if err := windows.GetExitCodeProcess(h, &code); err != nil || code != 259 /* STILL_ACTIVE */ {
		return procSample{}, fmt.Errorf("process %d has exited", pid)
	}
	var c, e, k, u windows.Filetime
	if err := windows.GetProcessTimes(h, &c, &e, &k, &u); err != nil {
		return procSample{}, err
	}
	var m memCounters
	m.cb = uint32(unsafe.Sizeof(m))
	if r, _, err := procGetProcessMemoryInfo.Call(uintptr(h), uintptr(unsafe.Pointer(&m)), uintptr(m.cb)); r == 0 {
		return procSample{}, err
	}
	var hc uint32
	procGetProcessHandleCount.Call(uintptr(h), uintptr(unsafe.Pointer(&hc)))
	var io ioCounters
	procGetProcessIoCounters.Call(uintptr(h), uintptr(unsafe.Pointer(&io)))

	return procSample{
		t: time.Now(), cpu: filetime(k) + filetime(u),
		ws: uint64(m.WorkingSetSize), peakWS: uint64(m.PeakWorkingSetSize), private: uint64(m.PrivateUsage),
		faults: m.PageFaultCount, handles: hc, ioWrite: io.WriteBytes, ioReadOp: io.ReadOps,
	}, nil
}

func exePath(pid uint32) string {
	h, err := windows.OpenProcess(windows.PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
	if err != nil {
		return ""
	}
	defer windows.CloseHandle(h)
	buf := make([]uint16, windows.MAX_PATH)
	n := uint32(len(buf))
	if windows.QueryFullProcessImageName(h, 0, &buf[0], &n) != nil {
		return ""
	}
	return windows.UTF16ToString(buf[:n])
}

// threadCounts returns pid -> thread count for every process (one snapshot).
func threadCounts() map[uint32]uint32 {
	out := map[uint32]uint32{}
	snap, err := windows.CreateToolhelp32Snapshot(windows.TH32CS_SNAPPROCESS, 0)
	if err != nil {
		return out
	}
	defer windows.CloseHandle(snap)
	var pe windows.ProcessEntry32
	pe.Size = uint32(unsafe.Sizeof(pe))
	for err = windows.Process32First(snap, &pe); err == nil; err = windows.Process32Next(snap, &pe) {
		out[pe.ProcessID] = pe.Threads
	}
	return out
}

// ---------- per-game tracking ----------

type statsFile struct {
	Lang   string `json:"lang"`
	Pid    uint32 `json:"pid"`
	AI     bool   `json:"ai"`
	Turbo  bool   `json:"turbo"`
	Seed   uint32 `json:"seed"`
	Pieces int64  `json:"pieces"`
	Lines  int64  `json:"lines"`
	Score  int64  `json:"score"`
	Games  int64  `json:"games"`
	Frames int64  `json:"frames"`
	TS     int64  `json:"ts"`
}

const historyLen = 12

type tracker struct {
	key, label string
	color      int

	live      bool
	pid       uint32
	exeSize   int64
	st, prevS statsFile
	cur, prev procSample
	hasPrev   bool
	threads   uint32

	cpuNow, cpuPeak, cpuSum float64
	cpuN                    int
	history                 []float64
	faultsPS, ioKBps        float64
	piecesPS, fps, perCPU   float64
	series                  [][]float64 // per chart metric, oldest first (see chart.go)
}

func (t *tracker) resetStats() {
	t.cpuPeak, t.cpuSum, t.cpuN = 0, 0, 0
	t.history = nil
	t.series = nil
	t.hasPrev = false
}

func statsDir() string { return filepath.Join(os.TempDir(), "blockfall") }

func (t *tracker) update(threads map[uint32]uint32) {
	data, err := os.ReadFile(filepath.Join(statsDir(), t.key+".json"))
	var st statsFile
	if err != nil || json.Unmarshal(data, &st) != nil || time.Since(time.UnixMilli(st.TS)) > 3*time.Second {
		t.live = false
		t.resetStats()
		return
	}
	if st.Pid != t.pid {
		t.pid = st.Pid
		t.resetStats()
		t.exeSize = 0
		if p := exePath(st.Pid); p != "" {
			if fi, err := os.Stat(p); err == nil {
				t.exeSize = fi.Size()
			}
		}
	}
	s, err := sampleProc(st.Pid)
	if err != nil {
		t.live = false
		t.resetStats()
		return
	}
	t.live = true
	t.threads = threads[st.Pid]
	t.prev, t.prevS = t.cur, t.st
	t.cur, t.st = s, st
	if t.hasPrev {
		dt := t.cur.t.Sub(t.prev.t).Seconds()
		cpuDelta := (t.cur.cpu - t.prev.cpu).Seconds()
		t.cpuNow = cpuDelta / dt * 100
		t.cpuSum += t.cpuNow
		t.cpuN++
		t.cpuPeak = math.Max(t.cpuPeak, t.cpuNow)
		t.history = append(t.history, t.cpuNow)
		if len(t.history) > historyLen {
			t.history = t.history[1:]
		}
		t.faultsPS = float64(t.cur.faults-t.prev.faults) / dt
		t.ioKBps = float64(t.cur.ioWrite-t.prev.ioWrite) / 1024 / dt
		pieces := float64(t.st.Pieces - t.prevS.Pieces)
		t.piecesPS = pieces / dt
		t.fps = float64(t.st.Frames-t.prevS.Frames) / dt
		t.perCPU = 0
		if cpuDelta > 0.001 {
			t.perCPU = pieces / cpuDelta
		}
		t.record()
	}
	t.hasPrev = true
}

// ---------- rendering ----------

const (
	labelW = 26
	colW   = 14
)

func pc(n int) tcell.Color { return tcell.PaletteColor(n) }

func put(s tcell.Screen, x, y int, str string, st tcell.Style) {
	for _, r := range str {
		s.SetContent(x, y, r, nil, st)
		x++
	}
}

func mb(b uint64) string { return fmt.Sprintf("%.1f MB", float64(b)/(1<<20)) }

func size(b int64) string {
	if b >= 1<<20 {
		return fmt.Sprintf("%.1f MB", float64(b)/(1<<20))
	}
	return fmt.Sprintf("%d KB", b/1024)
}

func spark(h []float64) string {
	const bars = "▁▂▃▄▅▆▇█"
	r := []rune(bars)
	var b strings.Builder
	for i := 0; i < historyLen-len(h); i++ {
		b.WriteRune(' ')
	}
	for _, v := range h {
		i := int(v / 100 * float64(len(r)-1))
		if i < 0 {
			i = 0
		}
		if i >= len(r) {
			i = len(r) - 1
		}
		b.WriteRune(r[i])
	}
	return b.String()
}

// row: label, text per tracker, numeric value for "best" highlighting, direction (-1 lower better, +1 higher better, 0 none)
type row struct {
	label  string
	text   func(t *tracker) string
	value  func(t *tracker) float64
	better int
}

func section(name string) row { return row{label: name} }

var rows = []row{
	{"Mode", func(t *tracker) string {
		if !t.st.AI {
			return "human"
		}
		if t.st.Turbo {
			return "AI turbo"
		}
		return "AI watch"
	}, nil, 0},
	{"PID", func(t *tracker) string { return fmt.Sprint(t.pid) }, nil, 0},
	{"Program size (.exe)", func(t *tracker) string { return size(t.exeSize) }, func(t *tracker) float64 { return float64(t.exeSize) }, -1},
	section("CPU   (100% = one core)"),
	{"CPU now", func(t *tracker) string { return fmt.Sprintf("%.1f %%", t.cpuNow) }, func(t *tracker) float64 { return t.cpuNow }, -1},
	{"CPU average", func(t *tracker) string { return fmt.Sprintf("%.1f %%", avg(t)) }, avg, -1},
	{"CPU peak", func(t *tracker) string { return fmt.Sprintf("%.1f %%", t.cpuPeak) }, func(t *tracker) float64 { return t.cpuPeak }, -1},
	{"CPU last 12 s", func(t *tracker) string { return spark(t.history) }, nil, 0},
	section("MEMORY"),
	{"RAM in use (working set)", func(t *tracker) string { return mb(t.cur.ws) }, func(t *tracker) float64 { return float64(t.cur.ws) }, -1},
	{"RAM peak", func(t *tracker) string { return mb(t.cur.peakWS) }, func(t *tracker) float64 { return float64(t.cur.peakWS) }, -1},
	{"Private bytes", func(t *tracker) string { return mb(t.cur.private) }, func(t *tracker) float64 { return float64(t.cur.private) }, -1},
	{"Page faults /s", func(t *tracker) string { return fmt.Sprintf("%.0f", t.faultsPS) }, func(t *tracker) float64 { return t.faultsPS }, -1},
	section("OPERATING SYSTEM"),
	{"Threads", func(t *tracker) string { return fmt.Sprint(t.threads) }, func(t *tracker) float64 { return float64(t.threads) }, -1},
	{"Handles", func(t *tracker) string { return fmt.Sprint(t.cur.handles) }, func(t *tracker) float64 { return float64(t.cur.handles) }, -1},
	{"Screen output KB/s", func(t *tracker) string { return fmt.Sprintf("%.1f", t.ioKBps) }, func(t *tracker) float64 { return t.ioKBps }, -1},
	section("GAME"),
	{"Pieces /s", func(t *tracker) string { return fmt.Sprintf("%.0f", t.piecesPS) }, func(t *tracker) float64 { return t.piecesPS }, +1},
	{"Pieces per CPU-second", func(t *tracker) string { return fmt.Sprintf("%.0f", t.perCPU) }, func(t *tracker) float64 { return t.perCPU }, +1},
	{"Frames drawn /s", func(t *tracker) string { return fmt.Sprintf("%.0f", t.fps) }, nil, 0},
	{"Pieces total", func(t *tracker) string { return fmt.Sprint(t.st.Pieces) }, nil, 0},
	{"Lines total", func(t *tracker) string { return fmt.Sprint(t.st.Lines) }, nil, 0},
	{"Games over", func(t *tracker) string { return fmt.Sprint(t.st.Games) }, nil, 0},
}

func avg(t *tracker) float64 {
	if t.cpuN == 0 {
		return 0
	}
	return t.cpuSum / float64(t.cpuN)
}

func draw(s tcell.Screen, ts []*tracker, self *tracker, started time.Time) {
	s.Clear()
	base := tcell.StyleDefault
	dim := base.Foreground(pc(245))
	bold := base.Foreground(pc(15)).Bold(true)
	best := base.Foreground(pc(10)).Bold(true)
	worst := base.Foreground(pc(9)).Bold(true)

	put(s, 1, 0, "BLOCKFALL · resource dashboard", bold)
	put(s, 1, 1, fmt.Sprintf("updates every second · running %s · Tab charts · R reset · Q quit", time.Since(started).Round(time.Second)), dim)

	y := 3
	for i, t := range ts {
		put(s, labelW+i*colW, y, t.label, base.Foreground(pc(t.color)).Bold(true))
	}
	y++
	for _, r := range rows {
		if r.text == nil {
			y++
			put(s, 1, y, r.label, dim)
			y++
			continue
		}
		put(s, 1, y, r.label, base)
		// best (green) and worst (red) among the live games; ties share the colour,
		// and nothing is coloured when fewer than two games run or all values are equal
		var vals [3]float64
		var ok [3]bool
		lo, hi, live := math.Inf(1), math.Inf(-1), 0
		for i, t := range ts {
			if !t.live || !t.hasPrev || r.value == nil || r.better == 0 {
				continue
			}
			vals[i], ok[i] = r.value(t), true
			lo, hi = math.Min(lo, vals[i]), math.Max(hi, vals[i])
			live++
		}
		bestVal, worstVal := lo, hi
		if r.better > 0 {
			bestVal, worstVal = hi, lo
		}
		rank := live > 1 && hi > lo
		for i, t := range ts {
			txt, st := "—", dim
			if t.live {
				txt, st = r.text(t), base
				if rank && ok[i] {
					switch vals[i] {
					case bestVal:
						st = best
					case worstVal:
						st = worst
					}
				}
			} else if r.label == "Mode" {
				txt = "not running"
			}
			put(s, labelW+i*colW, y, txt, st)
		}
		y++
	}

	y++
	put(s, 1, y, "green", best)
	put(s, 7, y, "= best   ", dim)
	put(s, 16, y, "red", worst)
	put(s, 20, y, "= worst · C# also loads the shared .NET 8 runtime", dim)
	y++
	put(s, 1, y, "terminal drawing itself is done by Windows Terminal, not counted above", dim)
	y++
	if self.live {
		put(s, 1, y, fmt.Sprintf("this dashboard: CPU %.1f %% · RAM %s · machine has %d logical CPUs",
			self.cpuNow, mb(self.cur.ws), runtime.NumCPU()), dim)
	}
	s.Show()
}

// selfTracker samples the dashboard itself (no stats file).
func (t *tracker) updateSelf() {
	s, err := sampleProc(uint32(os.Getpid()))
	if err != nil {
		return
	}
	t.prev, t.cur = t.cur, s
	if t.hasPrev {
		t.cpuNow = (t.cur.cpu - t.prev.cpu).Seconds() / t.cur.t.Sub(t.prev.t).Seconds() * 100
	}
	t.hasPrev, t.live = true, true
}

func main() {
	s, err := tcell.NewScreen()
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	if err := s.Init(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	defer s.Fini()
	s.HideCursor()

	ts := []*tracker{
		{key: "go", label: "Go", color: 14},
		{key: "cs", label: "C#", color: 13},
		{key: "rs", label: "Rust", color: 208},
	}
	self := &tracker{}
	started := time.Now()

	events := make(chan tcell.Event, 8)
	go func() {
		for {
			ev := s.PollEvent()
			if ev == nil {
				return
			}
			events <- ev
		}
	}()

	charts, page := false, 0 // Tab switches between the table and the charts
	redraw := func() {
		if charts {
			page = drawCharts(s, ts, page, started)
		} else {
			draw(s, ts, self, started)
		}
	}
	refresh := func() {
		threads := threadCounts()
		for _, t := range ts {
			t.update(threads)
		}
		self.updateSelf()
		redraw()
	}
	refresh()
	ticker := time.NewTicker(time.Second)
	defer ticker.Stop()
	for {
		select {
		case ev := <-events:
			switch e := ev.(type) {
			case *tcell.EventResize:
				s.Sync()
				redraw()
			case *tcell.EventKey:
				switch {
				case e.Key() == tcell.KeyEscape || e.Key() == tcell.KeyCtrlC || e.Rune() == 'q' || e.Rune() == 'Q':
					return
				case e.Rune() == 'r' || e.Rune() == 'R':
					for _, t := range ts {
						t.resetStats()
					}
					started = time.Now()
					redraw()
				case e.Key() == tcell.KeyTab || e.Rune() == 'g' || e.Rune() == 'G':
					charts = !charts
					redraw()
				case charts && e.Key() == tcell.KeyRight:
					page++
					redraw()
				case charts && e.Key() == tcell.KeyLeft:
					page--
					redraw()
				}
			}
		case <-ticker.C:
			refresh()
		}
	}
}
