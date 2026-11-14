# BLOCKFALL ROCKER — session handoff (2026-11-11)

Read this first in a new session, then [README.md](README.md), which has the full rules and the physics write-up.

## What it is

A spin-off of BLOCKFALL in `02_rocker/`, written in Rust with macroquad and running as a GUI window.
- **Pieces:** the same as BLOCKFALL, dealt in the same order (PRNG and 7-bag from `../SPEC.md`).
- **The base:** they're stacked on a rocking base that rolls on the floor without slipping.
- **Physics:** a 1-DOF Lagrangian with RK2 at 480 Hz (`src/physics.rs`).

The binary is `bin\blockfall-rocker.exe`. The user plays it themselves and reports issues with screenshots.

## Status

- **Working:** all **33 tests pass** (`cd 02_rocker; cargo test --release`). The release build is copied into `bin\`.
- **Committed** on `main` in `5346a3c` (with `LANGUAGE-COMPARISON.md` and the README/build changes). For the whole repo's state, see [../HANDOFF.md](../HANDOFF.md).
- **Commit only when the user asks.**

## Files

| file | what |
|------|------|
| `src/physics.rs` | `V2`, `Mass` (mass accumulator), `segment_mass(r)`, `profile_mass`, `Rocker` (step, land/impulse, `peak_after` for the AI, equilibrium). Constants: `G = 49`, `BASE_MASS = 30` (wood scales with R²), `DAMPING = 0.45`, `SLIDE_ANGLE = 30°`, `TOPPLE_ANGLE = 45°` |
| `src/game.rs` | Everything else: `Geo` (per-mode lattice), `Board`, `Game` (rules, falling, lock delay, push, lost pieces, debris, break-loose), the AI, and most of the tests |
| `src/main.rs` | Command line (`--hard`/`--advanced`, `--easy`, `--ai`, `--seed N`, `--bench N`), drawing, panel, input with key auto-repeat |
| `README.md` | Rules, controls, physics, AI notes, list of tests |

## Design decisions agreed with the user

1. **GUI, not a terminal game.** Pieces are **glued to the base grid**. The arc is **90° (±45°)**. **Line clears are kept.**
2. **Easy mode:** flat top, square 12×22 grid, R = 6√2 (was 10×20, R = 5√2). The stack **slides off at 30°**. The walls stay.
3. **Hard mode:** a 45° hacksaw top. **4 full teeth**, each flank 2 block-edges long, so the top is 8√2 ≈ 11.3 wide with a peak at each end. R = 8.
   - **Diamond blocks:** blocks are unit squares turned 45°, and pieces spawn already turned.
   - **Lattice:** in `Pt{x:a,y:b}`, level k = a+b (half-rows of √2/2) and horizontal position m = a−b. Over the base, |m| ≤ 7 (`BASE_M`). Pieces can move out to |m| ≤ 15 (`M_WIDE`).
   - **No walls.** Pieces can overhang the edge, or fall off it and are **lost**. **3 lost pieces = game over.**
   - **Landing rules (the user's):**
     - A piece falls straight down. Falling sweeps the cell below and the two cells below its faces.
     - **Face contact holds** (friction), even on a slope.
     - **Point contact tips:** a piece resting on a tooth tip or a block's corner tips half a step diagonally toward its centre of mass. If it's balanced exactly, it tips the way the base leans.
     - **The end peaks are half-tips** (BASE cells at level 0, m = ±8, solid only on the inner half; see `Board::blocks`).
     - Sideways moves also sweep the face cells on that side.
   - **Lock delay** of 0.5 s (both modes). A move or rotation restarts it, up to 15 times. ↓ on a resting piece locks it at once. Hard drop locks at once.
   - **Push along a slope:** ← or → on a *resting* diamond first tries one diagonal step down that way, then a plain sideways step.
   - **Line clear = a band:** two neighbouring half-rows that are both full over the base (8 + 7 = 15 cells). Overhangs in those half-rows go with it. Level 0 (the tooth tips) never clears.
   - **No sliding:** the stack **topples** at 45°, or earlier if an overhanging block swings down into the floor (`Game::must_break`).
4. **Physics keeps running after game over.** Past the limit, the stack still comes off (`release_stack` in the `State::Over` branch).
5. **Debris:** blocks become discs that collide with each other, the moving base, the floor and the edges of the play area (`Game::arena`, set from the view each frame). Debris doesn't push back on the base, which is a known simplification.

## Known limits / open threads

- **The hard-mode AI is weak:** about 50 bands per 1,500 pieces, and it loses every ~75 pieces. It doesn't use pushes or the lock delay. It re-plans just before dropping if the piece would fall off.
  - Bench: `.\bin\blockfall-rocker.exe --bench 1500 --seed 42 --hard`.
  - In easy mode it plays well: about 556 lines per 1,500 pieces.
- **Debris flung across:** after a break, the light empty base swings back hard and can fling a few blocks still on it across to the other side.
- **Future (user's plan):** other **curvature profiles** for the base. The rolling model in `physics.rs` assumes a circular arc (`center()`, `contact()`, `i_contact`, `segment_mass`), so that part needs a general rolling-contact model.
- **Not done yet:** easy mode could also lose its walls, but only if the user wants that.

## Working notes (Windows)

- **cargo** isn't on Git Bash's PATH by default. Use `export PATH="$PATH:$HOME/.cargo/bin"`. In PowerShell, add `$env:USERPROFILE\.cargo\bin`.
- **Copying into `bin\` fails while the user's game is running** (the exe is locked). Check with `tasklist | grep -i blockfall-rocker`, then copy `02_rocker\target\release\blockfall-rocker.exe` to `bin\`.
- **Screenshots of the window:** start the exe with `Start-Process -PassThru`, then `SetForegroundWindow` + `GetWindowRect` + `Graphics.CopyFromScreen` (via `Add-Type`). Save to the scratchpad.
- **Testing visual scenarios:** a scratch copy of the crate with a `ROCKER_PILE` env var hack in `Game::reset` pre-fills one side. This was never part of the real code.
- **Edits:** `src/game.rs` is large. Precise Python string replacements (each asserting exactly one match) worked well.
- **When giving run commands, use the full path**, e.g.
  `& 'C:\Users\carlos\Documents\Projects\blockfall\bin\blockfall-rocker.exe' --hard`
