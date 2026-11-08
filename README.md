# BLOCKFALL ×3

An original falling-block puzzle game for the terminal, built three times with the same behaviour:

| folder | language | terminal lib | binary |
|--------|----------|--------------|--------|
| `go/` | Go | none (raw console + ANSI, via `golang.org/x/sys`) | `bin\blockfall-go.exe` |
| `csharp/` | C# / .NET 8 | none (System.Console + ANSI) | `bin\blockfall-cs.exe` |
| `rust/` | Rust | [crossterm](https://github.com/crossterm-rs/crossterm) | `bin\blockfall-rs.exe` |
| `dashboard/` | Go | tcell + Win32 process APIs | `bin\blockfall-dash.exe` |

The rules, layout and controls are in [SPEC.md](SPEC.md). All three versions follow it.

## Toolchains (installed with winget)
```powershell
winget install -e --id GoLang.Go
winget install -e --id Microsoft.DotNet.SDK.8
winget install -e --id Rustlang.Rustup
rustup default stable-x86_64-pc-windows-gnu   # self-contained linker, no Visual Studio needed
```

## Build & play
```powershell
.\build.ps1
.\bin\blockfall-go.exe
.\bin\blockfall-cs.exe
.\bin\blockfall-rs.exe
```
## Computer players + resource dashboard
```powershell
.\play-ai.ps1            # random seed
.\play-ai.ps1 -Seed 42   # repeatable
.\play-ai.ps1 -Turbo     # start flat-out
```
This opens one Windows Terminal window with the three games side by side, each played by the same AI with the same seed, so they play the same game. A second window opens the dashboard (`dashboard/`, Go). Press **T** in a game to switch between watchable and turbo pace.

The dashboard samples each game process once a second:
- **CPU:** now, average, peak, and a 12-second trend (100% = one core).
- **Memory:** RAM in use (working set), RAM peak, private bytes, page faults per second.
- **OS:** threads, handles, and screen output in KB/s.
- **Game:** pieces per second, pieces per CPU-second (efficiency), frames per second, and totals.

The best value in each row is shown in green, the worst in red.

Headless benchmark, with no screen involved, just the game logic and AI:
```powershell
.\bin\blockfall-rs.exe --bench 10000 --seed 42
```
All three versions must print the same `lines`, `score` and `games` for the same seed. `build.ps1` checks this.

All three games draw the screen the same way: they build each frame in memory and write it to the terminal as a single block of text. That keeps the CPU comparison about the language, not a terminal library. The Go version used tcell at first, which drew through the older Windows console interface. That cost about 11% of a core at the watchable pace, against about 1–2% now.

Use **Windows Terminal** for correct colours and box characters. The window needs to be at least 60×28.

Controls: ←/→ move · ↑/X rotate · Z rotate ccw · ↓ soft drop · Space hard drop · P pause · R restart · F retro mode (green `[]` on black) · Q/Esc quit
