# BLOCKFALL CUBE-3D

BLOCKFALL in a 3D well, 16 deep, on a **6 × 6 base (the default) or a 9 × 9 base**. The pieces fall from the sky. The piece slides parallel to the base (along x and y) and rotates about all three axes. **The base never slides or tilts.** It only turns a quarter turn about z when you ask, so you can see every side. A layer clears when its whole grid is covered. Rust + macroquad, real 3D, orthographic camera by default.

## Coordinates

| axis | meaning |
|------|---------|
| x | horizontal, 6 or 9 cells (red on the axis gizmo) |
| y | horizontal, 6 or 9 cells (green) |
| z | vertical, 16 cells (blue). z = 0 is the floor; pieces fall along −z. |

## Rules

Same as [../SPEC.md](../SPEC.md) unless stated:
- **The original 7 pieces** (I O T S Z J L), with the same PRNG and 7-bag, so seed 42 deals the same pieces as every other edition. A new piece spawns **lying flat** in the top layer, centred.
- **Rotation about x, y or z:** 90° inside the piece's n × n × n box (n = 4 for I, 2 for O, 3 for the rest). The z rotation is the spec's 2D turn. Kicks try, in order: no shift, then ±1 along x and y, the four diagonals, ±2 along x and y, and finally one cell up. In 3D, S and Z are the same piece flipped over.
- **Walls:** all four sides and the floor stop a piece. The space above the rim is open, so you can rotate near the top.
- **Projection:** a neutral gray wire outline (never the piece colour) shows where the piece will land, with a gray drop line under each cell.
- **Gravity on a timer:** interval = max(200, 1500 − (level − 1) × 100) ms. This is slower than 2D because there's more to look at.
- **Lock delay (as in Rocker / Tower-3D):** a piece that can't fall waits 0.5 s before it locks. A move or rotation restarts the wait, up to 15 times per piece. S on a resting piece locks it at once.
- **A full layer clears** (36 cells on 6 × 6, 81 on 9 × 9), and the layers above drop down. Scoring is 100 / 300 / 500 / 800 × level for 1 / 2 / 3 / 4 layers at once. Soft drop gives +1 per layer, hard drop +2.
- **Level** = layers / 3 + 1, at both sizes. A layer takes about 9 pieces on 6 × 6 and about 20 on 9 × 9.
- **Base size:** 6 or 9 starts a new game at that size. The seed and bag carry on, as with R.
- **Game over:** a new piece can't spawn, or a piece locks while sticking out above the rim.

## Controls

| key | action |
|-----|--------|
| ← → ↑ ↓ | move the piece parallel to the base, following the screen (↑ = away from you) |
| X / Y / Z | rotate about the x / y / z axis (see the gizmo) |
| Shift + X / Y / Z | rotate the other way |
| S | soft drop |
| Space | hard drop |
| A / D | turn the base 90° about z, one way or the other (animated) |
| 6 / 9 | new game on a 6 × 6 / 9 × 9 base |
| V | orthographic / perspective |
| G | grid on the floor and back walls, on/off |
| L | colour locked blocks by layer instead of by piece |
| P | pause |
| R | restart |
| Q / Esc | quit |

The camera looks across a corner from 38° above horizontal. A / D swing it to the next corner. The arrows always follow the screen, so → still moves the piece to the right after a turn. Both base sizes are drawn to the same scale.

## Screen

On the right are the framed ON DECK preview, the axis gizmo, base size, score, layers, level and pieces. Under them, a **FILL** meter shows how full each of the 16 layers is; it turns green above 90 %.

## Command line

- `--seed N` sets the PRNG seed.
- `--size 6|9` sets the starting base size (default 6).

## Build and run

```
cd 05_cube-3D
cargo test --release
cargo build --release
copy target\release\blockfall-cube3d.exe ..\bin\
```
