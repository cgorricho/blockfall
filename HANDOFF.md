# BLOCKFALL — project handoff (2026-11-11)

Read this first in a new session. Each project has its own docs: [README.md](README.md) (overview), [SPEC.md](SPEC.md) (shared rules), [01_blockfall-2D/README.md](01_blockfall-2D/README.md), [02_rocker/HANDOFF.md](02_rocker/HANDOFF.md), [03_tower-3D/README.md](03_tower-3D/README.md), [04_rocker-tower/README.md](04_rocker-tower/README.md).

## What it is

An original falling-block game, built several ways:

| folder | what | binary |
|--------|------|--------|
| `01_blockfall-2D/go/`, `csharp/`, `rust/` | The same terminal game three times, to [SPEC.md](SPEC.md). Same seed = same game (`--bench` must match; `build.ps1` checks it) | `bin\blockfall-{go,cs,rs}.exe` |
| `01_blockfall-2D/dashboard/` | Go + tcell. Live resource use of the three games; **Tab** switches to Braille line charts (`chart.go`) | `bin\blockfall-dash.exe` |
| `02_rocker/` | Spin-off: Rust + macroquad GUI, pieces on a rocking base with rolling physics | `bin\blockfall-rocker.exe` |
| `03_tower-3D/` | Spin-off: Rust + macroquad, the board wrapped round a 3D cylinder | `bin\blockfall-tower3d.exe` |
| `04_rocker-tower/` | Spin-off: the tower-3D cylinder on a rocking spherical base, a full 3D rolling rigid body (Rocker's physics in 3D) | `bin\blockfall-rocker-tower.exe` |

Scripts: `build.ps1` at the root builds every project into the shared root `bin\`. In `01_blockfall-2D\`: `play-ai.ps1` (three AI games side by side + dashboard), `measure.ps1` (CPU/IO over a window), `disasm.ps1` (assembly listings).

Research docs stay at the root: [ASSEMBLY.md](ASSEMBLY.md) + `01_blockfall-2D/{go,rust,csharp}/asm/` (commented x86-64 listings and walkthroughs of the PRNG, 7-bag shuffle and AI score); [LANGUAGE-COMPARISON.md](LANGUAGE-COMPARISON.md) (Rust ~2× Go, ~3.6× C# on `--bench 50000`).

## Status

- Work on `main`, pushed to `origin` (github.com/cgorricho/blockfall). `rocker` and `tower-3d` were merged into `main` on 2026-11-11 (fast-forward). The two branches still exist locally and on `origin`, with nothing that `main` lacks.
- 2026-11-11: the original game moved from the root into `01_blockfall-2D/`. Then the project folders got number prefixes in build order: `01_blockfall-2D`, `02_rocker`, `03_tower-3D`, `04_rocker-tower`.
- 2026-11-11: new `04_rocker-tower/` (43 tests pass, built into `bin\`), committed on `main`. Not yet played by the user.
- Dashboard charts: tests pass (`cd 01_blockfall-2D\dashboard; go test ./...`), but they were **not yet checked by eye with real games running**.

## Open threads / ideas

- Look at the chart view live (`01_blockfall-2D\play-ai.ps1`, then Tab in the dashboard). Overlapping lines show one colour per cell — a known limit.
- The C# listing only covers the 19 methods `--bench` runs; drawing code isn't captured (it only runs interactively).
- Rocker-tower: no AI or `--bench`. The twist is applied directly, not by a torque. The rim at 45° only stops the tilt. Debris doesn't push the base. Tuning knobs: `BASE_RINGS` and `PIVOT_DAMPING` in `04_rocker-tower/src/physics.rs`.
- Rocker: weak hard-mode AI, debris flung after a break, other base curvature profiles (see [02_rocker/HANDOFF.md](02_rocker/HANDOFF.md)).

## Working notes (Windows)

- **Close running games before `build.ps1`:** Windows locks the `.exe` files in `bin\` and the copy fails. Check with `Get-Process blockfall*`.
- **Toolchains:** Go 1.27, .NET 8 SDK, Rust stable `x86_64-pc-windows-gnu`, GNU `objdump` from WinLibs (needed by `disasm.ps1`). cargo may be missing from Git Bash's PATH: `export PATH="$PATH:$HOME/.cargo/bin"`.
- `bin\*.exe~` files are old copies left by rebuilds; safe to delete.
- **Commit only when the user asks.** Give run commands with full paths, e.g. `& 'C:\Users\carlos\Documents\Projects\blockfall\01_blockfall-2D\play-ai.ps1'`.
