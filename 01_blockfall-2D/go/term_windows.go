package main

import (
	"os"

	"golang.org/x/sys/windows"
)

// enterTerminal switches the console to raw VT mode (same approach as the C#
// and Rust editions): keys arrive as bytes / escape sequences on stdin and
// output is plain ANSI text. The returned function restores everything.
func enterTerminal() (func(), error) {
	in := windows.Handle(os.Stdin.Fd())
	out := windows.Handle(os.Stdout.Fd())
	var inMode, outMode uint32
	if err := windows.GetConsoleMode(in, &inMode); err != nil {
		return nil, err
	}
	if err := windows.GetConsoleMode(out, &outMode); err != nil {
		return nil, err
	}
	raw := inMode&^(windows.ENABLE_ECHO_INPUT|windows.ENABLE_LINE_INPUT|windows.ENABLE_PROCESSED_INPUT) |
		windows.ENABLE_VIRTUAL_TERMINAL_INPUT
	if err := windows.SetConsoleMode(in, raw); err != nil {
		return nil, err
	}
	if err := windows.SetConsoleMode(out, outMode|windows.ENABLE_VIRTUAL_TERMINAL_PROCESSING); err != nil {
		_ = windows.SetConsoleMode(in, inMode)
		return nil, err
	}
	os.Stdout.WriteString("\x1b[?1049h\x1b[?25l\x1b[2J")
	return func() {
		os.Stdout.WriteString("\x1b[0m\x1b[?25h\x1b[?1049l")
		_ = windows.SetConsoleMode(in, inMode)
		_ = windows.SetConsoleMode(out, outMode)
	}, nil
}
