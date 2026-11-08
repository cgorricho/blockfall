package main

import (
	"fmt"
	"io"
	"strings"
)

// style is a cell's look; colours are 256-palette indexes, -1 = terminal default.
type style struct {
	fg, bg        int
	bold, reverse bool
}

var plainStyle = style{fg: -1, bg: -1}

// screen is an off-screen cell buffer flushed as one ANSI string per frame
// (the same technique the C# and Rust editions use).
type screen struct {
	w, h      int
	ch        []rune
	st        []style
	fill      style
	lastBg    int  // background the whole terminal was last cleared to
	hasLastBg bool // false until the first flush
}

func newScreen(w, h int) *screen {
	s := &screen{w: w, h: h, ch: make([]rune, w*h), st: make([]style, w*h)}
	s.clear(plainStyle)
	return s
}

func (s *screen) clear(fill style) {
	s.fill = fill
	for i := range s.ch {
		s.ch[i] = ' '
		s.st[i] = fill
	}
}

func (s *screen) put(x, y int, str string, st style) {
	for _, r := range str {
		if x >= 0 && x < s.w && y >= 0 && y < s.h {
			s.ch[y*s.w+x] = r
			s.st[y*s.w+x] = st
		}
		x++
	}
}

func appendSGR(b *strings.Builder, st style) {
	b.WriteString("\x1b[0m")
	if st.fg >= 0 {
		fmt.Fprintf(b, "\x1b[38;5;%dm", st.fg)
	}
	if st.bg >= 0 {
		fmt.Fprintf(b, "\x1b[48;5;%dm", st.bg)
	}
	if st.bold {
		b.WriteString("\x1b[1m")
	}
	if st.reverse {
		b.WriteString("\x1b[7m")
	}
}

func (s *screen) flush(w io.Writer) {
	var b strings.Builder
	if !s.hasLastBg || s.lastBg != s.fill.bg {
		// Background changed (mode toggle): repaint the whole terminal, not just our buffer.
		appendSGR(&b, s.fill)
		b.WriteString("\x1b[2J")
		s.lastBg, s.hasLastBg = s.fill.bg, true
	}
	b.WriteString("\x1b[H")
	var cur style
	have := false
	for y := 0; y < s.h; y++ {
		fmt.Fprintf(&b, "\x1b[%d;1H", y+1)
		for x := 0; x < s.w; x++ {
			st := s.st[y*s.w+x]
			if !have || cur != st {
				appendSGR(&b, st)
				cur, have = st, true
			}
			b.WriteRune(s.ch[y*s.w+x])
		}
	}
	b.WriteString("\x1b[0m")
	io.WriteString(w, b.String())
}

// ---------- keyboard ----------

type keyCode int

const (
	keyRune keyCode = iota
	keyLeft
	keyRight
	keyUp
	keyDown
	keyEsc
	keyCtrlC
)

type key struct {
	code keyCode
	r    rune
}

// parseKeys turns raw VT input bytes into keys.
func parseKeys(buf []byte) []key {
	var out []key
	for i := 0; i < len(buf); i++ {
		b := buf[i]
		switch {
		case b == 0x1b && i+2 < len(buf) && buf[i+1] == '[':
			// CSI sequence: ESC [ ... final letter
			j := i + 2
			for j < len(buf) && !(buf[j] >= 0x40 && buf[j] <= 0x7e) {
				j++
			}
			if j < len(buf) {
				switch buf[j] {
				case 'A':
					out = append(out, key{code: keyUp})
				case 'B':
					out = append(out, key{code: keyDown})
				case 'C':
					out = append(out, key{code: keyRight})
				case 'D':
					out = append(out, key{code: keyLeft})
				}
			}
			i = j
		case b == 0x1b:
			out = append(out, key{code: keyEsc})
		case b == 0x03:
			out = append(out, key{code: keyCtrlC})
		case b >= 0x20 && b < 0x7f:
			out = append(out, key{code: keyRune, r: rune(b)})
		}
	}
	return out
}
