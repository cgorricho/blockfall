// BLOCKFALL — Go edition (raw console + ANSI, like the C# and Rust editions). See ../SPEC.md.
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"time"
)

const (
	W           = 10
	H           = 20
	boardX      = 2
	boardY      = 2
	panelX      = 27
	langName    = "[Go]"
	langLabel   = "Go"
	langKey     = "go"
	aiStep      = 50 * time.Millisecond
	turboBudget = 15 * time.Millisecond
	frameTime   = 16 * time.Millisecond
)

type pt struct{ x, y int }

type pieceDef struct {
	n     int
	cells []pt
	color int
}

var pieces = []pieceDef{
	{4, []pt{{0, 1}, {1, 1}, {2, 1}, {3, 1}}, 14}, // I
	{2, []pt{{0, 0}, {1, 0}, {0, 1}, {1, 1}}, 11}, // O
	{3, []pt{{1, 0}, {0, 1}, {1, 1}, {2, 1}}, 13}, // T
	{3, []pt{{1, 0}, {2, 0}, {0, 1}, {1, 1}}, 10}, // S
	{3, []pt{{0, 0}, {1, 0}, {1, 1}, {2, 1}}, 9},  // Z
	{3, []pt{{0, 0}, {0, 1}, {1, 1}, {2, 1}}, 12}, // J
	{3, []pt{{2, 0}, {0, 1}, {1, 1}, {2, 1}}, 3},  // L
}

var helpLines = []string{"←→  move", "↑ X rotate", "Z   rotate ccw", "↓   soft drop", "SPC hard drop", "P   pause", "R   restart", "F   retro mode", "Q   quit"}

type board [H][W]int // 0 = empty, else kind+1

type piece struct {
	kind  int
	cells []pt
	x, y  int
}

type aiPlan struct {
	rots, x int
	ok      bool
}

type game struct {
	board    board
	cur      piece
	next     int
	bag      []int
	rng      uint32
	seed     uint32
	score    int64
	lines    int64
	paused   bool
	over     bool
	retro    bool // F: monochrome green "[]" rendering
	lastFall time.Time

	ai       bool
	turbo    bool
	plan     aiPlan
	rotsDone int
	lastAI   time.Time
	overAt   time.Time

	pieces, games, frames, totalLines int64 // never reset (lines/score are per game)
}

func (g *game) level() int64 { return g.lines/10 + 1 }

func (g *game) interval() time.Duration {
	ms := 800 - (g.level()-1)*70
	if ms < 80 {
		ms = 80
	}
	return time.Duration(ms) * time.Millisecond
}

// ---------- shared PRNG + 7-bag ----------

func (g *game) nextRand() uint32 {
	x := g.rng
	x ^= x << 13
	x ^= x >> 17
	x ^= x << 5
	g.rng = x
	return x
}

func (g *game) draw7() int {
	if len(g.bag) == 0 {
		a := []int{0, 1, 2, 3, 4, 5, 6}
		for i := 6; i >= 1; i-- {
			j := int(g.nextRand() % uint32(i+1))
			a[i], a[j] = a[j], a[i]
		}
		g.bag = a
	}
	k := g.bag[0]
	g.bag = g.bag[1:]
	return k
}

// ---------- core rules ----------

func (g *game) reset() {
	g.board = board{}
	g.bag = nil
	g.score, g.lines = 0, 0
	g.paused, g.over = false, false
	g.next = g.draw7()
	g.spawn()
}

func (g *game) spawn() {
	d := pieces[g.next]
	c := make([]pt, len(d.cells))
	copy(c, d.cells)
	g.cur = piece{kind: g.next, cells: c, x: (W - d.n) / 2, y: 0}
	g.next = g.draw7()
	g.lastFall = time.Now()
	if !fitsOn(&g.board, g.cur.cells, g.cur.x, g.cur.y) {
		g.over = true
		g.games++
		g.overAt = time.Now()
		return
	}
	if g.ai {
		g.plan = g.findPlan()
		g.rotsDone = 0
		g.lastAI = time.Now()
	}
}

func fitsOn(b *board, cells []pt, x, y int) bool {
	for _, c := range cells {
		bx, by := x+c.x, y+c.y
		if bx < 0 || bx >= W || by < 0 || by >= H || b[by][bx] != 0 {
			return false
		}
	}
	return true
}

func rotate(cells []pt, n int, cw bool) []pt {
	out := make([]pt, len(cells))
	for i, c := range cells {
		if cw {
			out[i] = pt{n - 1 - c.y, c.x}
		} else {
			out[i] = pt{c.y, n - 1 - c.x}
		}
	}
	return out
}

// kickRotate rotates cells and applies the wall-kick rule; returns the new cells and x.
func kickRotate(b *board, cells []pt, n, x, y int, cw bool) ([]pt, int, bool) {
	r := rotate(cells, n, cw)
	for _, k := range []int{0, -1, 1, -2, 2} {
		if fitsOn(b, r, x+k, y) {
			return r, x + k, true
		}
	}
	return cells, x, false
}

func (g *game) tryMove(dx, dy int) bool {
	if fitsOn(&g.board, g.cur.cells, g.cur.x+dx, g.cur.y+dy) {
		g.cur.x += dx
		g.cur.y += dy
		return true
	}
	return false
}

func (g *game) tryRotate(cw bool) {
	if c, x, ok := kickRotate(&g.board, g.cur.cells, pieces[g.cur.kind].n, g.cur.x, g.cur.y, cw); ok {
		g.cur.cells, g.cur.x = c, x
	}
}

// clearLines removes full rows and returns how many were cleared.
func clearLines(b *board) int {
	cleared := 0
	for y := H - 1; y >= 0; y-- {
		full := true
		for x := 0; x < W; x++ {
			if b[y][x] == 0 {
				full = false
				break
			}
		}
		if full {
			cleared++
			for yy := y; yy > 0; yy-- {
				b[yy] = b[yy-1]
			}
			b[0] = [W]int{}
			y++ // re-check same row
		}
	}
	return cleared
}

func (g *game) lock() {
	for _, c := range g.cur.cells {
		g.board[g.cur.y+c.y][g.cur.x+c.x] = g.cur.kind + 1
	}
	cleared := clearLines(&g.board)
	pts := []int64{0, 100, 300, 500, 800}
	g.score += pts[cleared] * g.level()
	g.lines += int64(cleared)
	g.totalLines += int64(cleared)
	g.pieces++
	g.spawn()
}

func (g *game) gravity() {
	if !g.tryMove(0, 1) {
		g.lock()
	}
	g.lastFall = time.Now()
}

func (g *game) hardDrop() {
	for g.tryMove(0, 1) {
		g.score += 2
	}
	g.lock()
}

func (g *game) ghostY() int {
	y := g.cur.y
	for fitsOn(&g.board, g.cur.cells, g.cur.x, y+1) {
		y++
	}
	return y
}

// ---------- AI ----------

func evaluate(b *board, cleared int) int64 {
	var agg, holes, bump int64
	var hts [W]int64
	for x := 0; x < W; x++ {
		top := H
		for y := 0; y < H; y++ {
			if b[y][x] != 0 {
				top = y
				break
			}
		}
		hts[x] = int64(H - top)
		agg += hts[x]
		for y := top + 1; y < H; y++ {
			if b[y][x] == 0 {
				holes++
			}
		}
	}
	for x := 0; x < W-1; x++ {
		d := hts[x] - hts[x+1]
		if d < 0 {
			d = -d
		}
		bump += d
	}
	return -510066*agg + 760666*int64(cleared) - 356630*holes - 184483*bump
}

func (g *game) findPlan() aiPlan {
	var best aiPlan
	var bestScore int64
	n := pieces[g.cur.kind].n
	for r := 0; r < 4; r++ {
		cells, x, y := g.cur.cells, g.cur.x, g.cur.y
		ok := true
		for i := 0; i < r && ok; i++ {
			cells, x, ok = kickRotate(&g.board, cells, n, x, y, true)
		}
		if !ok {
			continue
		}
		for tx := -3; tx < W; tx++ {
			px, valid := x, true
			for px != tx {
				step := 1
				if tx < px {
					step = -1
				}
				if !fitsOn(&g.board, cells, px+step, y) {
					valid = false
					break
				}
				px += step
			}
			if !valid {
				continue
			}
			py := y
			for fitsOn(&g.board, cells, px, py+1) {
				py++
			}
			b := g.board
			for _, c := range cells {
				b[py+c.y][px+c.x] = 1
			}
			s := evaluate(&b, clearLines(&b))
			if !best.ok || s > bestScore {
				best, bestScore = aiPlan{r, tx, true}, s
			}
		}
	}
	return best
}

// aiAction performs one step of the current plan.
func (g *game) aiAction() {
	p := g.plan
	switch {
	case !p.ok:
		g.hardDrop()
	case g.rotsDone < p.rots:
		g.tryRotate(true)
		g.rotsDone++
	case g.cur.x < p.x:
		if !g.tryMove(1, 0) {
			g.hardDrop()
		}
	case g.cur.x > p.x:
		if !g.tryMove(-1, 0) {
			g.hardDrop()
		}
	default:
		g.hardDrop()
	}
}

// aiPlaceNow executes the whole plan for the current piece at once.
func (g *game) aiPlaceNow() {
	start := g.pieces
	for g.pieces == start && !g.over {
		g.aiAction()
	}
}

// aiUpdate advances the AI; returns true if anything changed.
func (g *game) aiUpdate() bool {
	if g.paused {
		return false
	}
	if g.over {
		if g.turbo || time.Since(g.overAt) >= time.Second {
			g.reset()
			return true
		}
		return false
	}
	if g.turbo {
		start := time.Now()
		for time.Since(start) < turboBudget && !g.over {
			g.aiPlaceNow()
		}
		return true
	}
	if time.Since(g.lastAI) >= aiStep {
		g.aiAction()
		g.lastAI = time.Now()
		return true
	}
	return false
}

// ---------- stats file ----------

type statsFile struct {
	Lang   string `json:"lang"`
	Pid    int    `json:"pid"`
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

func statsPath() string { return filepath.Join(os.TempDir(), "blockfall", langKey+".json") }

func (g *game) writeStats() {
	p := statsPath()
	_ = os.MkdirAll(filepath.Dir(p), 0o755)
	data, _ := json.Marshal(statsFile{langLabel, os.Getpid(), g.ai, g.turbo, g.seed,
		g.pieces, g.totalLines, g.score, g.games, g.frames, time.Now().UnixMilli()})
	if os.WriteFile(p+".tmp", data, 0o644) == nil {
		_ = os.Rename(p+".tmp", p)
	}
}

// ---------- rendering ----------

// theme holds everything that changes between the colour and retro looks.
type theme struct {
	block, ghost, empty         string
	tl, tr, bl, br, horiz, vert string
	dim, text, emptyCol         int
	mono                        bool // every block/ghost uses one colour
	blockMono, ghostMono        int
}

var colourTheme = theme{"██", "░░", " ·", "┌", "┐", "└", "┘", "─", "│", 245, 15, 240, false, 0, 0}
var retroTheme = theme{"[]", "::", " .", "+", "+", "+", "+", "-", "|", 28, 10, 28, true, 10, 28}

// render draws the game into the screen buffer (the caller flushes it).
func (g *game) render(s *screen) {
	g.frames++
	th := colourTheme
	bg := -1
	if g.retro {
		th, bg = retroTheme, 0
	}
	fg := func(c int) style { return style{fg: c, bg: bg} }
	plain := style{fg: -1, bg: bg}
	if g.retro {
		plain.fg = 10
	}
	s.clear(plain)
	gray := fg(th.dim)
	white := fg(th.text)
	white.bold = true
	blockCol := func(kind int) int {
		if th.mono {
			return th.blockMono
		}
		return pieces[kind].color
	}

	s.put(2, 0, " B L O C K F A L L ", white)
	s.put(22, 0, langName, gray)
	if g.ai {
		mode := "AI WATCH"
		if g.turbo {
			mode = "AI TURBO"
		}
		s.put(30, 0, mode, gray)
	}

	// border
	s.put(boardX, boardY, th.tl, gray)
	s.put(boardX+1+W*2, boardY, th.tr, gray)
	s.put(boardX, boardY+H+1, th.bl, gray)
	s.put(boardX+1+W*2, boardY+H+1, th.br, gray)
	for i := 0; i < W*2; i++ {
		s.put(boardX+1+i, boardY, th.horiz, gray)
		s.put(boardX+1+i, boardY+H+1, th.horiz, gray)
	}
	for y := 0; y < H; y++ {
		s.put(boardX, boardY+1+y, th.vert, gray)
		s.put(boardX+1+W*2, boardY+1+y, th.vert, gray)
	}

	cell := func(x, y int, txt string, color int) {
		s.put(boardX+1+x*2, boardY+1+y, txt, fg(color))
	}
	for y := 0; y < H; y++ {
		for x := 0; x < W; x++ {
			if k := g.board[y][x]; k != 0 {
				cell(x, y, th.block, blockCol(k-1))
			} else {
				cell(x, y, th.empty, th.emptyCol)
			}
		}
	}
	if !g.over {
		col := blockCol(g.cur.kind)
		ghostCol := col
		if th.mono {
			ghostCol = th.ghostMono
		}
		gy := g.ghostY()
		for _, c := range g.cur.cells {
			cell(g.cur.x+c.x, gy+c.y, th.ghost, ghostCol)
		}
		for _, c := range g.cur.cells {
			cell(g.cur.x+c.x, g.cur.y+c.y, th.block, col)
		}
	}

	// side panel
	s.put(panelX, 3, "NEXT", white)
	for _, c := range pieces[g.next].cells {
		s.put(panelX+c.x*2, 4+c.y, th.block, fg(blockCol(g.next)))
	}
	s.put(panelX, 9, "SCORE", white)
	s.put(panelX, 10, fmt.Sprint(g.score), plain)
	s.put(panelX, 12, "LINES", white)
	s.put(panelX, 13, fmt.Sprint(g.lines), plain)
	s.put(panelX, 15, "LEVEL", white)
	s.put(panelX, 16, fmt.Sprint(g.level()), plain)
	for i, h := range helpLines {
		s.put(panelX, 18+i, h, gray)
	}
	if g.ai {
		s.put(panelX, 27, "T   ai turbo", gray)
	}

	inv := plain
	inv.bold, inv.reverse = true, true
	center := func(row int, txt string) {
		s.put(boardX+1+(W*2-len([]rune(txt)))/2, row, txt, inv)
	}
	if g.over {
		center(12, " GAME OVER ")
		center(13, " R = restart ")
	} else if g.paused {
		center(12, " PAUSED ")
	}
}

// ---------- main ----------

func runBench(g *game, n int64) {
	start := time.Now()
	for g.pieces < n {
		if g.over {
			g.reset()
		}
		g.aiPlaceNow()
	}
	fmt.Printf("lang=%s seed=%d pieces=%d lines=%d score=%d games=%d ms=%d\n",
		langKey, g.seed, g.pieces, g.totalLines, g.score, g.games, time.Since(start).Milliseconds())
}

func main() {
	aiFlag := flag.Bool("ai", false, "computer player")
	turboFlag := flag.Bool("turbo", false, "start the AI in turbo pace")
	seedFlag := flag.Uint64("seed", 0, "PRNG seed (1..4294967295)")
	benchFlag := flag.Int64("bench", 0, "headless: AI places N pieces and prints a summary")
	flag.Parse()

	seed := uint32(*seedFlag)
	if *seedFlag == 0 {
		seed = uint32(time.Now().UnixNano())
	}
	if seed == 0 {
		seed = 0x9E3779B9
	}

	if *benchFlag > 0 {
		g := &game{ai: true, rng: seed, seed: seed}
		g.reset()
		runBench(g, *benchFlag)
		return
	}

	restore, err := enterTerminal()
	if err != nil {
		fmt.Fprintln(os.Stderr, "needs a Windows console:", err)
		os.Exit(1)
	}
	defer restore()
	defer os.Remove(statsPath())

	// Keyboard: raw VT bytes from stdin, parsed on a reader goroutine.
	keys := make(chan key, 32)
	go func() {
		buf := make([]byte, 64)
		for {
			n, err := os.Stdin.Read(buf)
			if err != nil {
				return
			}
			for _, k := range parseKeys(buf[:n]) {
				keys <- k
			}
		}
	}()

	// ai must be set before the first spawn so it gets a plan
	g := &game{ai: *aiFlag, turbo: *aiFlag && *turboFlag, rng: seed, seed: seed}
	g.reset()
	s := newScreen(44, 28)
	g.render(s)
	s.flush(os.Stdout)
	g.writeStats()
	lastStats := time.Now()

	ticker := time.NewTicker(frameTime)
	defer ticker.Stop()

	for {
		dirty := false
		select {
		case k := <-keys:
			if g.handleKey(k) {
				return
			}
			dirty = true
		case <-ticker.C:
			if g.ai {
				dirty = g.aiUpdate()
			} else if !g.over && !g.paused && time.Since(g.lastFall) >= g.interval() {
				g.gravity()
				dirty = true
			}
			if time.Since(lastStats) >= 500*time.Millisecond {
				g.writeStats()
				lastStats = time.Now()
			}
		}
		if dirty {
			g.render(s)
			s.flush(os.Stdout)
		}
	}
}

// handleKey returns true when the game should quit.
func (g *game) handleKey(k key) bool {
	r := k.r
	if r >= 'A' && r <= 'Z' {
		r += 'a' - 'A'
	}
	switch {
	case k.code == keyEsc || k.code == keyCtrlC || r == 'q':
		return true
	case r == 'r':
		g.reset()
		return false
	case r == 'f':
		g.retro = !g.retro
		return false
	case r == 'p':
		if !g.over {
			g.paused = !g.paused
		}
		return false
	case r == 't':
		if g.ai {
			g.turbo = !g.turbo
		}
		return false
	}
	if g.over || g.paused || g.ai {
		return false
	}
	switch {
	case k.code == keyLeft || r == 'a':
		g.tryMove(-1, 0)
	case k.code == keyRight || r == 'd':
		g.tryMove(1, 0)
	case k.code == keyDown || r == 's':
		if g.tryMove(0, 1) {
			g.score++
			g.lastFall = time.Now()
		}
	case k.code == keyUp || r == 'w' || r == 'x':
		g.tryRotate(true)
	case r == 'z':
		g.tryRotate(false)
	case r == ' ':
		g.hardDrop()
	}
	return false
}
