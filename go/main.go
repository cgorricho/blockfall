// BLOCKFALL — Go edition (tcell). See ../SPEC.md.
package main

import (
	"fmt"
	"math/rand"
	"os"
	"time"

	"github.com/gdamore/tcell/v2"
)

const (
	W        = 10
	H        = 20
	boardX   = 2
	boardY   = 2
	panelX   = 27
	langName = "[Go]"
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

var helpLines = []string{"←→  move", "↑ X rotate", "Z   rotate ccw", "↓   soft drop", "SPC hard drop", "P   pause", "R   restart", "Q   quit"}

type piece struct {
	kind  int
	cells []pt
	x, y  int
}

type game struct {
	board    [H][W]int // 0 = empty, else color+1... we store kind+1
	cur      piece
	next     int
	bag      []int
	score    int
	lines    int
	paused   bool
	over     bool
	lastFall time.Time
	rng      *rand.Rand
}

func (g *game) level() int { return g.lines/10 + 1 }

func (g *game) interval() time.Duration {
	ms := 800 - (g.level()-1)*70
	if ms < 80 {
		ms = 80
	}
	return time.Duration(ms) * time.Millisecond
}

func (g *game) draw7() int {
	if len(g.bag) == 0 {
		g.bag = g.rng.Perm(7)
	}
	k := g.bag[0]
	g.bag = g.bag[1:]
	return k
}

func newPiece(kind int) piece {
	d := pieces[kind]
	c := make([]pt, len(d.cells))
	copy(c, d.cells)
	return piece{kind: kind, cells: c, x: (W - d.n) / 2, y: 0}
}

func (g *game) reset() {
	g.board = [H][W]int{}
	g.bag = nil
	g.score, g.lines = 0, 0
	g.paused, g.over = false, false
	g.next = g.draw7()
	g.spawn()
}

func (g *game) spawn() {
	g.cur = newPiece(g.next)
	g.next = g.draw7()
	g.lastFall = time.Now()
	if !g.fits(g.cur.cells, g.cur.x, g.cur.y) {
		g.over = true
	}
}

func (g *game) fits(cells []pt, x, y int) bool {
	for _, c := range cells {
		bx, by := x+c.x, y+c.y
		if bx < 0 || bx >= W || by < 0 || by >= H || g.board[by][bx] != 0 {
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

func (g *game) tryMove(dx, dy int) bool {
	if g.fits(g.cur.cells, g.cur.x+dx, g.cur.y+dy) {
		g.cur.x += dx
		g.cur.y += dy
		return true
	}
	return false
}

func (g *game) tryRotate(cw bool) {
	r := rotate(g.cur.cells, pieces[g.cur.kind].n, cw)
	for _, k := range []int{0, -1, 1, -2, 2} {
		if g.fits(r, g.cur.x+k, g.cur.y) {
			g.cur.cells = r
			g.cur.x += k
			return
		}
	}
}

func (g *game) lock() {
	for _, c := range g.cur.cells {
		g.board[g.cur.y+c.y][g.cur.x+c.x] = g.cur.kind + 1
	}
	cleared := 0
	for y := H - 1; y >= 0; y-- {
		full := true
		for x := 0; x < W; x++ {
			if g.board[y][x] == 0 {
				full = false
				break
			}
		}
		if full {
			cleared++
			for yy := y; yy > 0; yy-- {
				g.board[yy] = g.board[yy-1]
			}
			g.board[0] = [W]int{}
			y++ // re-check same row
		}
	}
	pts := []int{0, 100, 300, 500, 800}
	g.score += pts[cleared] * g.level()
	g.lines += cleared
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
	for g.fits(g.cur.cells, g.cur.x, y+1) {
		y++
	}
	return y
}

// ---------- rendering ----------

func pc(n int) tcell.Color { return tcell.PaletteColor(n) }

func put(s tcell.Screen, x, y int, str string, st tcell.Style) {
	for _, r := range str {
		s.SetContent(x, y, r, nil, st)
		x++
	}
}

func (g *game) render(s tcell.Screen) {
	s.Clear()
	base := tcell.StyleDefault
	gray := base.Foreground(pc(245))
	white := base.Foreground(pc(15)).Bold(true)

	put(s, 2, 0, " B L O C K F A L L ", white)
	put(s, 22, 0, langName, gray)

	// border
	put(s, boardX, boardY, "┌", gray)
	put(s, boardX+1+W*2, boardY, "┐", gray)
	put(s, boardX, boardY+H+1, "└", gray)
	put(s, boardX+1+W*2, boardY+H+1, "┘", gray)
	for i := 0; i < W*2; i++ {
		put(s, boardX+1+i, boardY, "─", gray)
		put(s, boardX+1+i, boardY+H+1, "─", gray)
	}
	for y := 0; y < H; y++ {
		put(s, boardX, boardY+1+y, "│", gray)
		put(s, boardX+1+W*2, boardY+1+y, "│", gray)
	}

	cell := func(x, y int, txt string, color int) {
		put(s, boardX+1+x*2, boardY+1+y, txt, base.Foreground(pc(color)))
	}
	for y := 0; y < H; y++ {
		for x := 0; x < W; x++ {
			if k := g.board[y][x]; k != 0 {
				cell(x, y, "██", pieces[k-1].color)
			} else {
				cell(x, y, " ·", 240)
			}
		}
	}
	if !g.over {
		col := pieces[g.cur.kind].color
		gy := g.ghostY()
		for _, c := range g.cur.cells {
			cell(g.cur.x+c.x, gy+c.y, "░░", col)
		}
		for _, c := range g.cur.cells {
			cell(g.cur.x+c.x, g.cur.y+c.y, "██", col)
		}
	}

	// side panel
	put(s, panelX, 3, "NEXT", white)
	nd := pieces[g.next]
	for _, c := range nd.cells {
		put(s, panelX+c.x*2, 4+c.y, "██", base.Foreground(pc(nd.color)))
	}
	put(s, panelX, 9, "SCORE", white)
	put(s, panelX, 10, fmt.Sprint(g.score), base)
	put(s, panelX, 12, "LINES", white)
	put(s, panelX, 13, fmt.Sprint(g.lines), base)
	put(s, panelX, 15, "LEVEL", white)
	put(s, panelX, 16, fmt.Sprint(g.level()), base)
	for i, h := range helpLines {
		put(s, panelX, 18+i, h, gray)
	}

	center := func(row int, txt string) {
		x := boardX + 1 + (W*2-len([]rune(txt)))/2
		put(s, x, row, txt, base.Reverse(true).Bold(true))
	}
	if g.over {
		center(12, " GAME OVER ")
		center(13, " R = restart ")
	} else if g.paused {
		center(12, " PAUSED ")
	}
	s.Show()
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

	events := make(chan tcell.Event, 32)
	go func() {
		for {
			ev := s.PollEvent()
			if ev == nil {
				return
			}
			events <- ev
		}
	}()

	g := &game{rng: rand.New(rand.NewSource(time.Now().UnixNano()))}
	g.reset()
	ticker := time.NewTicker(16 * time.Millisecond)
	defer ticker.Stop()

	for {
		select {
		case ev := <-events:
			switch e := ev.(type) {
			case *tcell.EventResize:
				s.Sync()
			case *tcell.EventKey:
				if g.handleKey(e) {
					return
				}
			}
		case <-ticker.C:
			if !g.over && !g.paused && time.Since(g.lastFall) >= g.interval() {
				g.gravity()
			}
		}
		g.render(s)
	}
}

// handleKey returns true when the game should quit.
func (g *game) handleKey(e *tcell.EventKey) bool {
	r := e.Rune()
	if r >= 'A' && r <= 'Z' {
		r += 'a' - 'A'
	}
	switch {
	case e.Key() == tcell.KeyEscape || e.Key() == tcell.KeyCtrlC || r == 'q':
		return true
	case r == 'r':
		g.reset()
		return false
	case r == 'p':
		if !g.over {
			g.paused = !g.paused
		}
		return false
	}
	if g.over || g.paused {
		return false
	}
	switch {
	case e.Key() == tcell.KeyLeft || r == 'a':
		g.tryMove(-1, 0)
	case e.Key() == tcell.KeyRight || r == 'd':
		g.tryMove(1, 0)
	case e.Key() == tcell.KeyDown || r == 's':
		if g.tryMove(0, 1) {
			g.score++
			g.lastFall = time.Now()
		}
	case e.Key() == tcell.KeyUp || r == 'w' || r == 'x':
		g.tryRotate(true)
	case r == 'z':
		g.tryRotate(false)
	case r == ' ':
		g.hardDrop()
	}
	return false
}
