// Line charts for the dashboard, drawn with Braille characters.
//
// A Braille character (U+2800..U+28FF) is a 2-wide x 4-tall grid of dots,
// and each of the 8 dots is one bit of the code point. So one terminal cell
// is 8 tiny "pixels", which is enough for a smooth line chart.
// One terminal cell can only have one foreground colour, so where two games'
// lines share a cell, the game with more dots in that cell sets the colour.
package main

import (
	"fmt"
	"math"
	"time"

	"github.com/gdamore/tcell/v2"
)

// chartMetric is one chart: a title and how to read the value from a tracker.
type chartMetric struct {
	title string
	value func(t *tracker) float64
}

var chartMetrics = []chartMetric{
	{"CPU %", func(t *tracker) float64 { return t.cpuNow }},
	{"RAM MB", func(t *tracker) float64 { return float64(t.cur.ws) / (1 << 20) }},
	{"Private MB", func(t *tracker) float64 { return float64(t.cur.private) / (1 << 20) }},
	{"Faults /s", func(t *tracker) float64 { return t.faultsPS }},
	{"Threads", func(t *tracker) float64 { return float64(t.threads) }},
	{"Handles", func(t *tracker) float64 { return float64(t.cur.handles) }},
	{"Screen KB/s", func(t *tracker) float64 { return t.ioKBps }},
	{"Pieces /s", func(t *tracker) float64 { return t.piecesPS }},
	{"Pieces/CPU-s", func(t *tracker) float64 { return t.perCPU }},
	{"Frames /s", func(t *tracker) float64 { return t.fps }},
}

// seriesLen is how many samples are kept per metric: 10 minutes at one a second.
const seriesLen = 600

// record appends the current value of every chart metric to the tracker's history.
func (t *tracker) record() {
	if t.series == nil {
		t.series = make([][]float64, len(chartMetrics))
	}
	for i, m := range chartMetrics {
		s := append(t.series[i], m.value(t))
		if len(s) > seriesLen {
			s = s[len(s)-seriesLen:]
		}
		t.series[i] = s
	}
}

// dotBit[row][col] is the bit for that dot inside a Braille cell.
var dotBit = [4][2]rune{{0x01, 0x08}, {0x02, 0x10}, {0x04, 0x20}, {0x40, 0x80}}

// niceCeil rounds v up to 1, 2, 2.5 or 5 times a power of ten, for a tidy axis.
func niceCeil(v float64) float64 {
	if v <= 0 || math.IsNaN(v) || math.IsInf(v, 0) {
		return 1
	}
	p := math.Pow(10, math.Floor(math.Log10(v)))
	for _, m := range []float64{1, 2, 2.5, 5, 10} {
		if v <= m*p*1.0000001 {
			return m * p
		}
	}
	return 10 * p
}

// num formats a value in at most about 5 characters.
func num(v float64) string {
	a := math.Abs(v)
	switch {
	case a >= 1e6:
		return fmt.Sprintf("%.1fM", v/1e6)
	case a >= 1e4:
		return fmt.Sprintf("%.0fk", v/1e3)
	case a >= 1e3:
		return fmt.Sprintf("%.1fk", v/1e3)
	case a >= 100:
		return fmt.Sprintf("%.0f", v)
	case a >= 1:
		return fmt.Sprintf("%.1f", v)
	}
	return fmt.Sprintf("%.2f", v)
}

// drawChart draws metric mi in the box (x0, y0, w, h):
// a title row with each game's latest value, the plot, and a time axis row.
func drawChart(s tcell.Screen, x0, y0, w, h, mi int, ts []*tracker) {
	base := tcell.StyleDefault
	dim := base.Foreground(pc(245))
	m := chartMetrics[mi]

	put(s, x0, y0, m.title, base.Foreground(pc(15)).Bold(true))
	lx := x0 + len([]rune(m.title)) + 2
	for _, t := range ts {
		if !t.live || t.series == nil || len(t.series[mi]) == 0 {
			continue
		}
		vals := t.series[mi]
		txt := t.label + " " + num(vals[len(vals)-1])
		if lx+len([]rune(txt)) > x0+w {
			break
		}
		put(s, lx, y0, txt, base.Foreground(pc(t.color)))
		lx += len([]rune(txt)) + 2
	}

	const axisW = 7 // "123.4k" + the axis line
	pw, ph := w-axisW-1, h-2
	if pw < 4 || ph < 2 {
		return
	}
	W, H := pw*2, ph*4 // plot size in dots

	// visible samples per game: the newest W (one dot column per second)
	visible := make([][]float64, len(ts))
	top := 0.0
	for i, t := range ts {
		if !t.live || t.series == nil {
			continue
		}
		v := t.series[mi]
		if len(v) > W {
			v = v[len(v)-W:]
		}
		visible[i] = v
		for _, x := range v {
			top = math.Max(top, x)
		}
	}
	top = niceCeil(top)

	// y axis: top value, middle value, zero
	for r := 0; r < ph; r++ {
		ch, label := '│', ""
		switch {
		case r == 0:
			ch, label = '┤', num(top)
		case r == ph-1:
			ch, label = '┤', "0"
		case ph >= 5 && r == (ph-1)/2:
			ch, label = '┤', num(top/2)
		}
		put(s, x0+axisW-1-len([]rune(label)), y0+1+r, label, dim)
		s.SetContent(x0+axisW-1, y0+1+r, ch, nil, dim)
	}

	// plot every game's line into one dot grid, remembering who drew each dot
	type cell struct {
		bits  rune
		count [3]int
	}
	grid := make([]cell, pw*ph)
	for si, vals := range visible {
		prevY := -1
		for i, v := range vals {
			dx := W - len(vals) + i
			dy := H - 1 - int(math.Round(v/top*float64(H-1)))
			dy = min(max(dy, 0), H-1)
			lo, hi := dy, dy
			if prevY >= 0 { // join to the previous point with a vertical run
				lo, hi = min(prevY, dy), max(prevY, dy)
			}
			for yy := lo; yy <= hi; yy++ {
				c := &grid[(yy/4)*pw+dx/2]
				c.bits |= dotBit[yy%4][dx%2]
				c.count[si]++
			}
			prevY = dy
		}
	}
	for i, c := range grid {
		if c.bits == 0 {
			continue
		}
		owner := 0
		for si := range ts {
			if c.count[si] >= c.count[owner] && c.count[si] > 0 {
				owner = si
			}
		}
		s.SetContent(x0+axisW+i%pw, y0+1+i/pw, 0x2800+c.bits, nil, base.Foreground(pc(ts[owner].color)))
	}

	// time axis
	put(s, x0+axisW, y0+1+ph, fmt.Sprintf("-%ds", W), dim)
	put(s, x0+axisW+pw-3, y0+1+ph, "now", dim)
}

// drawCharts lays the charts out in a grid that fits the window
// (charts at least 40 columns by 10 rows) and returns the page actually shown.
func drawCharts(s tcell.Screen, ts []*tracker, page int, started time.Time) int {
	s.Clear()
	w, h := s.Size()
	cols := max(1, w/40)
	rowsN := max(1, (h-3)/10)
	per := cols * rowsN
	pages := (len(chartMetrics) + per - 1) / per
	page = ((page % pages) + pages) % pages

	bold := tcell.StyleDefault.Foreground(pc(15)).Bold(true)
	dim := tcell.StyleDefault.Foreground(pc(245))
	put(s, 1, 0, "BLOCKFALL · resource charts", bold)
	x := 30
	for _, t := range ts {
		put(s, x, 0, "━━ "+t.label, tcell.StyleDefault.Foreground(pc(t.color)).Bold(true))
		x += len([]rune(t.label)) + 5
	}
	put(s, 1, 1, fmt.Sprintf("page %d/%d · ←/→ page · Tab table · R reset · Q quit · running %s",
		page+1, pages, time.Since(started).Round(time.Second)), dim)

	cw, chh := w/cols, (h-3)/rowsN
	for k := 0; k < per; k++ {
		mi := page*per + k
		if mi >= len(chartMetrics) {
			break
		}
		drawChart(s, (k%cols)*cw+1, 3+(k/cols)*chh, cw-1, chh-1, mi, ts)
	}
	s.Show()
	return page
}
