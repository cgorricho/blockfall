# BLOCKFALL — shared spec

All three implementations (Go, C#, Rust) follow this spec exactly so they look and play the same.

## Board
- 12 columns x 22 rows. Each cell is drawn as 2 characters.
- Empty cell: ` ·` (dim gray, 256-color 240). Locked/active block: `██` in the piece colour. Ghost: `::` in neutral gray (245), never the piece colour.
- Border: box-drawing `┌ ─ ┐ │ └ ┘`, gray (245).

### Retro mode (toggle with F; the default)
A monochrome, ASCII-only look on a black background (256-color 0 everywhere, including the rest of the terminal):
- Block: `[]` bright green (10) for every piece, including the ON DECK preview. Ghost: `::` dark green (28). Empty: ` .` dark green (28).
- Border: `+ - |` dark green (28). Title, labels and values: bright green (10). Language tag and key help: dark green (28).
- Overlays: same text, reversed (black on green).
- The mode survives restarts (R); the game starts in retro mode.

## Pieces (n = box size; cells as (x,y) in spawn orientation)
| id | n | cells | colour (256-palette) |
|----|---|-------|----------------------|
| I | 4 | (0,1)(1,1)(2,1)(3,1) | 175 pink |
| O | 2 | (0,0)(1,0)(0,1)(1,1) | 75 sky |
| T | 3 | (1,0)(0,1)(1,1)(2,1) | 179 sand |
| S | 3 | (1,0)(2,0)(0,1)(1,1) | 141 lavender |
| Z | 3 | (0,0)(1,0)(1,1)(2,1) | 73 teal |
| J | 3 | (0,0)(0,1)(1,1)(2,1) | 107 olive |
| L | 3 | (2,0)(0,1)(1,1)(2,1) | 167 brick |

The palette is BLOCKFALL's own, a set of muted tones. The GUI editions use the same indices (RGB from the xterm-256 cube). Keep it that way: no piece should get the cyan/yellow/purple/green/red/blue/orange colouring other falling-block games use.

- Rotate clockwise: (x,y) -> (n-1-y, x). Counter-clockwise: (x,y) -> (y, n-1-x).
- Wall kicks: try x offsets 0, -1, +1, -2, +2 (first that fits wins).
- Spawn: x = (12 - n) / 2, y = 0. If spawn collides -> game over.
- Randomiser: 7-bag (shuffle all 7, deal, repeat), using the shared PRNG below so every language deals the same pieces for the same seed.

## Shared PRNG (identical in every language)
- State: unsigned 32-bit. Seed from `--seed N`; without it, from the clock. A seed of 0 becomes `0x9E3779B9`.
- `next()`: `x ^= x << 13; x ^= x >> 17; x ^= x << 5; return x` (all 32-bit wrapping).
- Bag refill: `a = [0,1,2,3,4,5,6]`; for `i = 6` down to `1`: `j = next() % (i + 1)`, swap `a[i]`, `a[j]`. Deal from the front.
- Restart (R or AI auto-restart) clears the bag but keeps the PRNG state (no reseed).

## Command line
| flag | effect |
|------|--------|
| `--ai` | computer player (see AI mode) |
| `--turbo` | start the AI in turbo pace (implies nothing without `--ai`) |
| `--seed N` | PRNG seed (1..4294967295) |
| `--bench N` | headless: AI places N pieces flat-out, no UI, prints one line and exits |

`--bench` output, exactly: `lang=<go|cs|rs> seed=<S> pieces=<N> lines=<L> score=<S> games=<G> ms=<elapsed>`
(everything except `lang` and `ms` must match across languages for the same seed and N).
`pieces`, `games` and `lines` here are running totals that never reset (the on-screen LINES and SCORE are per game); `score` is the current game's score.

## AI mode (`--ai`)
- Gravity is off: the AI moves the piece at the spawn row, then hard-drops it. This makes play independent of timing, so all languages play the same game.
- Player movement keys are ignored. P, R, F, Q still work. **T** toggles watch/turbo pace.
- On every spawn the AI picks a plan `(rotations r, target x)`:
  - For `r = 0..3`: from the spawn position apply `r` clockwise rotations using the normal kick rules (skip `r` if a rotation fails). Then for `tx = -3 .. W-1` (ascending): slide one column at a time from the current x to `tx` (skip `tx` if a step is blocked), drop to the lowest fitting row, lock on a copy of the board, clear full lines, and score it.
  - Score (64-bit integer): `-510066*aggregateHeight + 760666*linesCleared - 356630*holes - 184483*bumpiness`.
    Column height = `H - (row of its topmost block)` (0 if empty). Holes = empty cells below the topmost block of each column. Bumpiness = sum of |height difference| between neighbouring columns.
  - Highest score wins; ties keep the first one found.
- Executing the plan, one action at a time: rotate CW until `r` rotations are done; then move one column toward `tx`; when there, hard drop (+2 per row as usual). If a move is blocked, hard drop.
- Pace: **watch** = one action every 50 ms. **turbo** = each ~16 ms frame spends ~15 ms placing whole pieces back to back, then renders once.
- Game over: counted in `games`. Watch restarts after 1 s, turbo immediately.
- Screen extras: row 0 col 30 shows `AI WATCH` or `AI TURBO` (help colour); row 27 shows a 10th help line `T   ai turbo`.

## Frame loop (all modes)
Each iteration (~16 ms): handle all pending keys, update the game (gravity or AI), and redraw only if something changed. The frame counter counts redraws.

## Stats file (for the dashboard)
Every 500 ms each game writes `%TEMP%\blockfall\<go|cs|rs>.json` (write to `.tmp`, then rename over) and deletes it on exit:
`{"lang":"Go","pid":1234,"ai":true,"turbo":false,"seed":42,"pieces":0,"lines":0,"score":0,"games":0,"frames":0,"ts":<unix ms>}`
`lang` is `Go`, `C#` or `Rust`. `pieces`, `lines`, `games`, `frames` are running totals; `score` is the current game's.

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
| F | toggle retro mode (any time) |
| Q / Esc | quit |

## Screen layout (0-based terminal cells)
- Row 0: title ` B L O C K F A L L ` starting at col 2, bold white, followed by the language tag in gray (e.g. `[Go]`).
- Board border top-left at (col 2, row 2); interior starts at (col 3, row 3); board is 24 chars wide.
- The whole layout fits in 44 columns x 28 rows (so three games fit side by side).
- Board border from col 2 to col 27, rows 2 to 25.
- Side panel at col 30:
  - rows 3–8 the ON DECK box: a 10-wide frame in the border characters, ` ON DECK` set into its top edge (bright), the next piece inside on rows 4–7 from col 31 (2 chars per cell)
  - row 10 `SCORE`, row 11 value
  - row 12 `LINES`, row 13 value
  - row 14 `LEVEL`, row 15 value
  - rows 18–26 key help (9 lines, gray):
    `←→  move`, `↑ X rotate`, `Z   rotate ccw`, `↓   soft drop`, `SPC hard drop`, `P   pause`, `R   restart`, `F   retro mode`, `Q   quit`
- Centered overlay inside the board on row 13: ` PAUSED ` or ` GAME OVER ` (inverse), row 14 ` R = restart ` for game over.
- Input is polled every ~16 ms; see "Frame loop".
