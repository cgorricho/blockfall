# BLOCKFALL — Player's Manual

One game, three builds: **Go**, **C#** and **Rust**. They look and play the same, so pick any one.

---

## 1. Before you start

- **Use Windows Terminal.** The old blue console window can show the colours or border lines wrong.
- **Make the window at least 60 columns × 28 rows.** If it's smaller, part of the board gets cut off.
- The C# build needs the **.NET 8 runtime**. It's already on this PC because the SDK was installed. The Go and Rust builds run on their own.

---

## 2. Starting the game

Every build is a single `.exe` in `C:\Users\carlos\Documents\Projects\blockfall\bin\`.

### Go edition
```bash
& 'C:\Users\carlos\Documents\Projects\blockfall\bin\blockfall-go.exe'
```

### C# edition
```bash
& 'C:\Users\carlos\Documents\Projects\blockfall\bin\blockfall-cs.exe'
```

### Rust edition
```bash
& 'C:\Users\carlos\Documents\Projects\blockfall\bin\blockfall-rs.exe'
```

You can also open the `bin` folder in Explorer and double-click one. Windows opens it in a terminal window, which closes again when you quit.

The game starts right away. The top line shows which language you're running: `[Go]`, `[C#]` or `[Rust]`.

---

## 3. The screen

```
   B L O C K F A L L  [Go]

  ┌────────────────────┐
  │ · · · · · · · · · ·│   NEXT          <- the piece that comes after this one
  │ · · · ·████ · · · ·│   ████
  │ · · ·████ · · · · ·│     ████
  │ · · · · · · · · · ·│
  │        ...         │   SCORE
  │ · · · · · · · · · ·│   0
  │ · · · ·░░░░ · · · ·│   LINES         <- ░░ = where the piece will land
  │ · · ·░░░░ · · · · ·│   0
  └────────────────────┘   LEVEL 1
```

| Symbol | Meaning |
|--------|---------|
| `██` | A block: the falling piece, or a piece that has already landed |
| `░░` | Shadow: where the falling piece will land if you drop it |
| ` ·` | An empty cell |

The well is **10 columns wide and 20 rows tall**.

### Retro mode (press F)

Press **F** to switch to a plain text look: green characters on a black screen, with every block drawn as `[]`. Press **F** again to go back to colour.

```
  +--------------------+
  | . . . . . . . . . .|   NEXT
  | . . . .[][] . . . .|   [][][][]
  | . . . .[][] . . . .|
  | . . . . . . . . . .|
  |        ...         |
  | . . . .:::: . . . .|   <- :: = where the piece will land
  |[][][] . . .[][][][]|
  +--------------------+
```

| Symbol | Meaning |
|--------|---------|
| `[]` | A block |
| `::` | Shadow: where the falling piece will land |
| ` .` | An empty cell |

For example, the long piece is `[][][][]` and the square is two rows of `[][]`. Your score, the board and the current piece all stay the same when you switch, and the game keeps the mode when you restart with **R**.

---

## 4. Controls

| Key | Action |
|-----|--------|
| **←** or **A** | Move left |
| **→** or **D** | Move right |
| **↑**, **W** or **X** | Rotate clockwise |
| **Z** | Rotate counter-clockwise |
| **↓** or **S** | Soft drop: move down one row (+1 point) |
| **Space** | Hard drop: drop to the bottom at once (+2 points per row) |
| **P** | Pause / resume |
| **R** | Restart (works at any time) |
| **F** | Switch between colour mode and retro mode (works at any time) |
| **Q** or **Esc** | Quit |

**Rotating next to a wall:** if a rotation would push the piece into a wall or another block, the game tries shifting it up to 2 columns left or right so the rotation still works.

---

## 5. How to play

1. Pieces made of 4 blocks fall from the top, one at a time.
2. Move and rotate each piece while it falls.
3. When a piece can't fall any further it **locks** in place, and the next piece appears.
4. Fill a whole row from wall to wall and it **clears**. Everything above it drops down one row.
5. The game ends when a new piece has no room to appear at the top.

The 7 pieces come in shuffled bags: you get each of the 7 once before any repeats. So you never wait long for the piece you need.

---

## 6. Scoring and speed

| Rows cleared at once | Points |
|----------------------|--------|
| 1 | 100 × level |
| 2 | 300 × level |
| 3 | 500 × level |
| 4 | 800 × level |

Extra points:
- Soft drop: **+1** per row
- Hard drop: **+2** per row

**Levels:** you go up one level for every **10 rows** cleared. Pieces start by dropping one row every 0.8 seconds. Each level makes that 0.07 seconds faster, down to a fastest speed of 0.08 seconds per row (level 11 and above).

---

## 7. Tips

- **Watch the shadow (`░░`).** It shows exactly where the piece will land, so you can line it up and hard-drop.
- **Check NEXT** to plan where the following piece will go.
- **Keep the surface flat**, and leave one column open at the side for the long 4-block piece. It clears 4 rows at once for the biggest score.
- **Clearing several rows at once pays off.** 4 rows together gives 800 points; 4 single rows give only 400.

---

## 8. Watch the computer play all three

```bash
& 'C:\Users\carlos\Documents\Projects\blockfall\play-ai.ps1' -Seed 42
```

Two Windows Terminal windows open:

1. **The games.** All three versions sit side by side, each played by the same computer player. With the same seed they get the same pieces and make the same moves, so the boards stay in step.
2. **The dashboard.** It shows how hard each game makes your PC work (CPU, RAM, threads, and so on), updated every second. The best value in each row is in green.

In any game window:

| Key | Action |
|-----|--------|
| **T** | Switch between watchable speed and turbo (as fast as possible) |
| **F** | Retro look |
| **P** | Pause |
| **Q** | Quit that game |

Turbo is where the language differences really show: watch **Pieces /s** and **Pieces per CPU-second** on the dashboard. Press **T** in all three panes to compare them fairly.

In the dashboard, **R** resets the averages and peaks, and **Q** quits.

Press **Tab** in the dashboard to see **line charts** of every metric over about the last minute, with one coloured line per game: Go cyan, C# magenta, Rust orange. **←/→** turns the page (ten charts on two pages in the standard window), and **Tab** goes back to the table. Where two lines cross in the same spot, only one colour can show.

Leave out `-Seed 42` to get a random game each time. Add `-Turbo` to start at full speed.

---

## 9. Troubleshooting

| Problem | Fix |
|---------|-----|
| The board is cut off or looks scrambled | Make the window bigger (at least 60×28) and restart with **R** |
| Boxes or `?` where the lines and blocks should be | Run it in Windows Terminal, not the old console window |
| `blockfall-cs.exe` says .NET is missing | Install the .NET 8 runtime: `winget install Microsoft.DotNet.Runtime.8` |
| You changed the code and want new builds | Run `& 'C:\Users\carlos\Documents\Projects\blockfall\build.ps1'` |
| The three boards are cut off | Make the game window wider (each pane needs 44 columns) or shrink the font with **Ctrl −** |
| The dashboard says "not running" | That game isn't open. Start it with `play-ai.ps1`, or wait a second after it starts |

Have fun! 🧱
