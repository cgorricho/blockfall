package main

import (
	"math"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/gdamore/tcell/v2"
)

func TestNiceCeil(t *testing.T) {
	for _, c := range []struct{ in, want float64 }{
		{0, 1}, {0.7, 1}, {1, 1}, {1.2, 2}, {2.2, 2.5}, {3, 5}, {7, 10}, {100, 100}, {101, 200}, {4200, 5000},
	} {
		if got := niceCeil(c.in); got != c.want {
			t.Errorf("niceCeil(%v) = %v, want %v", c.in, got, c.want)
		}
	}
}

// fakeTrackers returns three live trackers with 200 s of synthetic history.
func fakeTrackers() []*tracker {
	ts := []*tracker{
		{key: "go", label: "Go", color: 14, live: true},
		{key: "cs", label: "C#", color: 13, live: true},
		{key: "rs", label: "Rust", color: 208, live: true},
	}
	for i := 0; i < 200; i++ {
		for k, t := range ts {
			t.cpuNow = 2 + float64(k)*1.5 + math.Sin(float64(i)/6+float64(k))
			t.cur.ws = uint64(8+k*12) << 20
			t.piecesPS = 300 + 50*math.Cos(float64(i)/9)
			t.record()
		}
	}
	return ts
}

// TestChartsRender draws the charts on a simulated 80x37 screen and checks the
// layout. With BLOCKFALL_DUMP=1 it prints the screen, to eyeball it.
func TestChartsRender(t *testing.T) {
	s := tcell.NewSimulationScreen("UTF-8")
	if err := s.Init(); err != nil {
		t.Fatal(err)
	}
	defer s.Fini()
	s.SetSize(80, 37)
	ts := fakeTrackers()

	for _, page := range []int{0, 1} {
		got := drawCharts(s, ts, page, time.Now())
		if got != page {
			t.Fatalf("page %d shown as %d", page, got)
		}
		cells, w, h := s.GetContents()
		var b strings.Builder
		braille := 0
		for y := 0; y < h; y++ {
			for x := 0; x < w; x++ {
				r := ' '
				if rs := cells[y*w+x].Runes; len(rs) > 0 {
					r = rs[0]
				}
				if r >= 0x2800 && r <= 0x28FF {
					braille++
				}
				b.WriteRune(r)
			}
			b.WriteByte('\n')
		}
		out := b.String()
		// 2 x 3 charts per page at 80x37: page 0 = first six metrics, page 1 = the rest
		if page == 0 && !(strings.Contains(out, "CPU %") && strings.Contains(out, "Handles") && !strings.Contains(out, "Pieces /s")) {
			t.Errorf("page 0 should hold CPU %% .. Handles")
		}
		if page == 1 && !strings.Contains(out, "Pieces /s") {
			t.Errorf("page 1 should hold Pieces /s")
		}
		if page == 0 && braille == 0 {
			t.Errorf("page 0 drew no chart lines")
		}
		if os.Getenv("BLOCKFALL_DUMP") != "" {
			t.Logf("page %d:\n%s", page, out)
		}
	}
	if got := drawCharts(s, ts, 2, time.Now()); got != 0 {
		t.Errorf("page 2 should wrap to 0, got %d", got)
	}
}
