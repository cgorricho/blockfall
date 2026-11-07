# BLOCKFALL — shared spec

All three implementations (Go, C#, Rust) follow this spec exactly so they look and play the same.

## Board
- 10 columns x 20 rows. Each cell is drawn as 2 characters.
- Empty cell: ` ·` (dim gray, 256-color 240). Locked/active block: `██` in the piece colour. Ghost: `░░` in the piece colour.
- Border: box-drawing `┌ ─ ┐ │ └ ┘`, gray (245).

## Pieces (n = box size; cells as (x,y) in spawn orientation)
| id | n | cells | colour (256-palette) |
|----|---|-------|----------------------|
| I | 4 | (0,1)(1,1)(2,1)(3,1) | 14 |
| O | 2 | (0,0)(1,0)(0,1)(1,1) | 11 |
| T | 3 | (1,0)(0,1)(1,1)(2,1) | 13 |
| S | 3 | (1,0)(2,0)(0,1)(1,1) | 10 |
| Z | 3 | (0,0)(1,0)(1,1)(2,1) | 9 |
| J | 3 | (0,0)(0,1)(1,1)(2,1) | 12 |
| L | 3 | (2,0)(0,1)(1,1)(2,1) | 3 |

- Rotate clockwise: (x,y) -> (n-1-y, x). Counter-clockwise: (x,y) -> (y, n-1-x).
- Wall kicks: try x offsets 0, -1, +1, -2, +2 (first that fits wins).
- Spawn: x = (10 - n) / 2, y = 0. If spawn collides -> game over.
- Randomiser: 7-bag (shuffle all 7, deal, repeat).

## Timing & scoring
- Level = lines / 10 + 1.
- Gravity interval (ms) = max(80, 800 - (level - 1) * 70).
- On a gravity tick, if the piece cannot move down it locks immediately.
- Line clears: 1/2/3/4 lines = 100/300/500/800 x level.
- Soft drop: +1 per cell and resets the gravity timer. Hard drop: +2 per cell, then locks.

## Controls
| key | action |
|-----|--------|
| ← / A | left |
| → / D | right |
| ↓ / S | soft drop |
| ↑ / W / X | rotate CW |
| Z | rotate CCW |
| Space | hard drop |
| P | pause |
| R | restart (any time) |
| Q / Esc | quit |

## Screen layout (0-based terminal cells)
- Row 0: title ` B L O C K F A L L ` starting at col 2, bold white, followed by the language tag in gray (e.g. `[Go]`).
- Board border top-left at (col 2, row 2); interior starts at (col 3, row 3); board is 20 chars wide.
- Side panel at col 27:
  - row 3  `NEXT`, rows 4–7 next-piece preview (4x4 box, 2 chars per cell)
  - row 9  `SCORE`, row 10 value
  - row 12 `LINES`, row 13 value
  - row 15 `LEVEL`, row 16 value
  - rows 18–25 key help (8 lines, gray):
    `←→  move`, `↑ X rotate`, `Z   rotate ccw`, `↓   soft drop`, `SPC hard drop`, `P   pause`, `R   restart`, `Q   quit`
- Centered overlay inside the board on row 12: ` PAUSED ` or ` GAME OVER ` (inverse), row 13 ` R = restart ` for game over.
- Input is polled every ~16 ms; the screen is redrawn after every event/tick.
