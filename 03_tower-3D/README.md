# BLOCKFALL TOWER-3D

BLOCKFALL wrapped around a vertical cylinder, drawn in real 3D with an orthographic camera. Rust + macroquad.

## Coordinates

| axis | meaning |
|------|---------|
| z | vertical (the cylinder axis). Pieces only ever fall along z. |
| x | radial, from the wall toward the centre, always horizontal. Pieces rotate only about this axis. |
| y | around the cylinder (the angle). The arrows turn the cylinder CW or CCW about z. |

Because pieces never move along x, the wall is **one cell thick**. Every cell is a wedge-shaped brick (an annular sector): 1 unit tall and 1 unit thick. The inner and outer faces are true cylinder sections, smooth-shaded.

## The tower: three ring sizes

The tower is **20 rows** tall. Its ring comes in three sizes (keys 1 / 2 / 3, or `--ring N`; the default is 32). Switching starts a new game.

| ring | column angle | inner / outer radius | brick width, inner / outer face |
|------|--------------|----------------------|---------------------------------|
| 16 | 22.5° | 2.05 / 3.05 | 0.80 / 1.20 |
| 24 | 15° | 3.32 / 4.32 | 0.87 / 1.13 |
| 32 | 11.25° | 4.59 / 5.59 | 0.90 / 1.10 |

- The mid radius is N / 2π, so every brick is exactly 1 unit wide at the middle of the wall.
- Columns wrap: a piece can sit across the "seam" between the last column and column 0.

## Rules

Same as [../SPEC.md](../SPEC.md) unless stated:
- **The same 7 pieces**, rotation, kicks (0, −1, +1, −2, +2, now wrapping around), PRNG and 7-bag. Same seed → same pieces.
- **Gravity on a timer:** interval = max(80, 800 − (level − 1) × 70) ms.
- **Lock delay (as in Rocker):** a piece that can't fall waits 0.5 s before it locks. A move or rotation restarts the wait, up to 15 times per piece. A piece moved off a ledge falls again. ↓ on a resting piece locks it at once; hard drop locks at once.
- **A full ring (16, 24 or 32 cells) clears**, and the rows above drop down. Scoring is 100/300/500/800 × level; soft drop +1 per row, hard drop +2.
- **No walls:** only the floor and other blocks stop a piece.
- **Ghost:** a neutral gray wire outline (never the piece colour) shows where the piece will land.
- The falling piece always stays in front of the camera. Each press moves the piece one column on the wall at once. The drawing of the tower **turns smoothly** to catch up, easing in with a time constant of about 45 ms and always taking the short way round.
- A new piece spawns in the column that's currently in front.

## Controls

| key | action |
|-----|--------|
| ← / A | turn the cylinder CW (seen from above): the front of the wall moves left |
| → / D | turn the cylinder CCW: the front of the wall moves right |
| ↑ / W / X | rotate the piece CW (about the radial axis) |
| Z | rotate CCW |
| ↓ / S | soft drop |
| Space | hard drop |
| PgUp / PgDn | tilt the camera up / down (5° – 85°) to see more of the far side |
| 1 / 2 / 3 | new game with a 16 / 24 / 32 ring |
| G | wire frame of the empty wall on/off (off by default) |
| P | pause |
| R | restart |
| Q / Esc | quit |

## Command line

- `--seed N` sets the PRNG seed.
- `--ring 16|24|32` sets the starting ring size (default 32).

## View

Orthographic camera (no perspective shrink), 30° above horizontal by default, looking at the front of the tower. One fixed light. The curved faces are smooth-shaded (6 arc segments per brick, per-vertex normals); the flat faces are flat-shaded. Block edges are outlined.

## Build and run

```
cd 03_tower-3D
cargo test --release
cargo build --release
copy target\release\blockfall-tower3d.exe ..\bin\
```
