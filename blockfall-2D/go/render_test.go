package main

import (
	"strings"
	"testing"
)

// dump renders the game into a 44x28 buffer and returns its text.
func dump(g *game) (string, *screen) {
	s := newScreen(44, 28)
	g.render(s)
	var b strings.Builder
	for y := 0; y < s.h; y++ {
		b.WriteString(string(s.ch[y*s.w : (y+1)*s.w]))
		b.WriteByte('\n')
	}
	return b.String(), s
}

func TestRetroToggle(t *testing.T) {
	g := &game{rng: 1, seed: 1}
	g.reset()
	g.next = 0          // I piece in NEXT
	g.board[H-1][0] = 2 // one locked O-block

	colour, _ := dump(g)
	if !strings.Contains(colour, "██") || strings.Contains(colour, "[]") {
		t.Fatal("colour mode should use ██ blocks")
	}

	g.handleKey(key{code: keyRune, r: 'f'})
	if !g.retro {
		t.Fatal("F did not enable retro mode")
	}
	retro, s := dump(g)
	if strings.ContainsAny(retro, "█░┌─│·") {
		t.Fatal("retro mode should be ASCII-only")
	}
	if !strings.Contains(retro, "[][][][]") {
		t.Fatal("NEXT should show the line piece as [][][][]")
	}
	for i, st := range s.st {
		if st.bg != 0 || (st.fg != 10 && st.fg != 28) {
			t.Fatalf("cell %d not green-on-black: %+v", i, st)
		}
	}

	g.handleKey(key{code: keyRune, r: 'F'})
	if g.retro {
		t.Fatal("second F should return to colour mode")
	}
}

func TestParseKeys(t *testing.T) {
	got := parseKeys([]byte("\x1b[Aa \x1b[D\x03\x1b"))
	want := []key{{code: keyUp}, {keyRune, 'a'}, {keyRune, ' '}, {code: keyLeft}, {code: keyCtrlC}, {code: keyEsc}}
	if len(got) != len(want) {
		t.Fatalf("got %v, want %v", got, want)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("key %d: got %v, want %v", i, got[i], want[i])
		}
	}
}

func TestFlushIsOneANSIString(t *testing.T) {
	g := &game{rng: 1, seed: 1}
	g.reset()
	s := newScreen(44, 28)
	g.render(s)
	var b strings.Builder
	s.flush(&b)
	out := b.String()
	if !strings.HasPrefix(out, "\x1b[0m\x1b[2J") || !strings.Contains(out, "B L O C K F A L L") {
		t.Fatalf("unexpected frame start: %q", out[:40])
	}
}

func TestPRNGBag(t *testing.T) {
	g := &game{rng: 42}
	seen := map[int]bool{}
	for i := 0; i < 7; i++ {
		seen[g.draw7()] = true
	}
	if len(seen) != 7 {
		t.Fatalf("a bag must deal all 7 pieces, got %v", seen)
	}
}

func TestAIPlaysWell(t *testing.T) {
	g := &game{ai: true, rng: 42, seed: 42}
	g.reset()
	for g.pieces < 2000 {
		if g.over {
			g.reset()
		}
		g.aiPlaceNow()
	}
	t.Logf("2000 pieces: totalLines=%d games=%d", g.totalLines, g.games)
	// 2000 pieces = 8000 cells = 800 full rows if nothing were left over
	if g.totalLines < 700 {
		t.Fatalf("AI cleared only %d lines in 2000 pieces", g.totalLines)
	}
}
