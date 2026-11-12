# BLOCKFALL — project handoff (2026-11-11)

Read this first in a new session. Each sub-project has its own docs: [README.md](README.md) (overview), [SPEC.md](SPEC.md) (shared rules), [rocker/HANDOFF.md](rocker/HANDOFF.md), [tower-3D/README.md](tower-3D/README.md).

## What it is

An original falling-block game, built several ways:

| folder | what | binary |
|--------|------|--------|
| `go/`, `csharp/`, `rust/` | The same terminal game three times, to [SPEC.md](SPEC.md). Same seed = same game (`--bench` must match; `build.ps1` checks it) | `bin\blockfall-{go,cs,rs}.exe` |
| `dashboard/` | Go + tcell. Live resource use of the three games; **Tab** switches to Braille line charts (`chart.go`) | `bin\blockfall-dash.exe` |
| `rocker/` | Spin-off: Rust + macroquad GUI, pieces on a rocking base with rolling physics | `bin\blockfall-rocker.exe` |
| `tower-3D/` | Spin-off: Rust + macroquad, the board wrapped round a 3D cylinder | `bin\blockfall-tower3d.exe` |

Scripts: `build.ps1` (build all into `bin\`), `play-ai.ps1` (three AI games side by side + dashboard), `measure.ps1` (CPU/IO over a window), `disasm.ps1` (assembly listings).

Docs for learning: [ASSEMBLY.md](ASSEMBLY.md) + `go/asm/`, `rust/asm/`, `csharp/asm/` (commented x86-64 listings and walkthroughs of the PRNG, 7-bag shuffle and AI score); [LANGUAGE-COMPARISON.md](LANGUAGE-COMPARISON.md) (Rust ~2× Go, ~3.6× C# on `--bench 50000`).

## Status

- Branch `main`, last commit `97ac611` (tower-3D). Nothing pushed is assumed — check `git status` / `git log origin/main..` if a remote matters.
- **Uncommitted:** `csharp/asm/blockfall-cs.asm` and `rust/asm/blockfall-rs.asm` were regenerated (re-run of `disasm.ps1`). The diff is only renamed inline markers and JIT addresses, no change to the game code. Commit them or `git checkout` them; the READMEs don't depend on the difference.
- Dashboard charts: tests pass (`cd dashboard; go test ./...`), but they were **not yet checked by eye with real games running**.

## Open threads / ideas

- Look at the chart view live (`.\play-ai.ps1`, then Tab in the dashboard). Overlapping lines show one colour per cell — a known limit.
- The C# listing only covers the 19 methods `--bench` runs; drawing code isn't captured (it only runs interactively).
- Rocker: weak hard-mode AI, debris flung after a break, other base curvature profiles (see [rocker/HANDOFF.md](rocker/HANDOFF.md)).

## Working notes (Windows)

- **Close running games before `build.ps1`:** Windows locks the `.exe` files in `bin\` and the copy fails. Check with `Get-Process blockfall*`.
- **Toolchains:** Go 1.27, .NET 8 SDK, Rust stable `x86_64-pc-windows-gnu`, GNU `objdump` from WinLibs (needed by `disasm.ps1`). cargo may be missing from Git Bash's PATH: `export PATH="$PATH:$HOME/.cargo/bin"`.
- `bin\*.exe~` files are old copies left by rebuilds; safe to delete.
- **Commit only when the user asks.** Give run commands with full paths, e.g. `& 'C:\Users\carlos\Documents\Projects\blockfall\play-ai.ps1'`.
