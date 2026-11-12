# BLOCKFALL 2D

The original BLOCKFALL: a falling-block puzzle game for the terminal, built three times with the same behaviour, plus a live dashboard that compares the three while they run.

| folder | language | terminal lib | binary |
|--------|----------|--------------|--------|
| [go/](go/) | Go | none (raw console + ANSI, via `golang.org/x/sys`) | `bin\blockfall-go.exe` |
| [csharp/](csharp/) | C# / .NET 8 | none (System.Console + ANSI) | `bin\blockfall-cs.exe` |
| [rust/](rust/) | Rust | [crossterm](https://github.com/crossterm-rs/crossterm) | `bin\blockfall-rs.exe` |
| [dashboard/](dashboard/) | Go | tcell + Win32 process APIs | `bin\blockfall-dash.exe` |

All three games follow [SPEC.md](../SPEC.md) (rules, layout, controls), which lives in the repo root because the spin-offs use it too. The same seed gives the same game in every edition.

How to play, step by step: [MANUAL.md](MANUAL.md).

## Build

The repo-wide build script builds everything, these editions included, into the shared `bin\` folder at the repo root:
```powershell
& 'C:\Users\carlos\Documents\Projects\blockfall\build.ps1'
```
It ends by running `--bench 10000 --seed 42` in all three editions. They must print the same `lines`, `score` and `games`.

## Play
```powershell
& 'C:\Users\carlos\Documents\Projects\blockfall\bin\blockfall-go.exe'
& 'C:\Users\carlos\Documents\Projects\blockfall\bin\blockfall-cs.exe'
& 'C:\Users\carlos\Documents\Projects\blockfall\bin\blockfall-rs.exe'
```
Use **Windows Terminal** for correct colours and box characters. The window needs to be at least 60×28.

Controls: ←/→ move · ↑/X rotate · Z rotate ccw · ↓ soft drop · Space hard drop · P pause · R restart · F retro mode (green `[]` on black) · T turbo (AI mode) · Q/Esc quit

## Computer players + resource dashboard
```powershell
& 'C:\Users\carlos\Documents\Projects\blockfall\01_blockfall-2D\play-ai.ps1'            # random seed
& 'C:\Users\carlos\Documents\Projects\blockfall\01_blockfall-2D\play-ai.ps1' -Seed 42   # repeatable
& 'C:\Users\carlos\Documents\Projects\blockfall\01_blockfall-2D\play-ai.ps1' -Turbo     # start flat-out
```
This opens one Windows Terminal window with the three games side by side, each played by the same AI with the same seed, so they play the same game. A second window opens the dashboard. Press **T** in a game to switch between watchable and turbo pace.

The dashboard samples each game process once a second:
- **CPU:** now, average, peak, and a 12-second trend (100% = one core).
- **Memory:** RAM in use (working set), RAM peak, private bytes, page faults per second.
- **OS:** threads, handles, and screen output in KB/s.
- **Game:** pieces per second, pieces per CPU-second (efficiency), frames per second, and totals.

The best value in each row is shown in green, the worst in red. Press **Tab** (or **G**) to switch to **line charts**: one chart per metric, three coloured lines over roughly the last minute. **←/→** pages through the charts, and Tab goes back to the table. The charts use Braille characters, which give 2×4 dots per terminal cell (see [dashboard/chart.go](dashboard/chart.go)).

To measure CPU and I/O over a fixed window while the games run:
```powershell
& 'C:\Users\carlos\Documents\Projects\blockfall\01_blockfall-2D\measure.ps1'              # 30 seconds
& 'C:\Users\carlos\Documents\Projects\blockfall\01_blockfall-2D\measure.ps1' -Seconds 60
```

## Headless benchmark
Just the game logic and AI, with no screen involved:
```powershell
& 'C:\Users\carlos\Documents\Projects\blockfall\bin\blockfall-rs.exe' --bench 10000 --seed 42
```
Results and analysis: [LANGUAGE-COMPARISON.md](../LANGUAGE-COMPARISON.md).

## Assembly listings
```powershell
& 'C:\Users\carlos\Documents\Projects\blockfall\01_blockfall-2D\disasm.ps1'
```
This writes the machine code of each edition as commented x86-64 assembly into `go\asm\`, `rust\asm\` and `csharp\asm\`, each with a README that walks through the same functions. It needs GNU `objdump` (WinLibs). Start with [ASSEMBLY.md](../ASSEMBLY.md).

## How the screen is drawn
All three games build each frame in memory and write it to the terminal as a single block of text. That keeps the CPU comparison about the language, not a terminal library. The Go version used tcell at first, which drew through the older Windows console interface. That cost about 11% of a core at the watchable pace, against about 1–2% now.

## Tests
```powershell
cd 'C:\Users\carlos\Documents\Projects\blockfall\01_blockfall-2D\go'; go test ./...
cd 'C:\Users\carlos\Documents\Projects\blockfall\01_blockfall-2D\dashboard'; go test ./...
```
