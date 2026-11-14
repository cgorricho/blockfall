# IP guardrails

BLOCKFALL uses the free rules of falling-block puzzles (pieces fall, rotate, full lines clear), but none of another game's look. Copyright doesn't protect game rules. It does protect a game's look: *Tetris Holding v. Xio Interactive* (D.N.J. 2012) found that the exact 10×20 board, the style and colours of the pieces, and the look of the ghost piece, the next-piece preview, garbage lines, the colour flash on lock and the board-fill at game over were all protected. Keep it that way in every edition:

- **Board:** never 10×20. The terminal and Rocker easy wells are 12×22. The towers are rings of 16/24/32 columns, 20 rows tall.
- **Colours:** BLOCKFALL's own muted palette (I pink, O sky, T sand, S lavender, Z teal, J olive, L brick; indices in SPEC.md). Never the cyan/yellow/purple/green/red/blue/orange mapping.
- **Ghost:** a neutral outline (`::` in gray, or a gray wire outline in the GUI editions), never a faded copy of the piece.
- **Preview:** the framed "ON DECK" box, not a bare "NEXT" panel.
- **Effects:** no colour flash when a piece locks, no board-fill animation at game over, and no garbage lines styled like another game's.
- **Words and sound:** no "Tetris", "-tris" names or "Tetrimino" in code, repos, docs, commits or store text. Don't call features "Guideline". No Korobeiniki or similar melodies.
- **Default look:** the terminal editions start in retro mode.

The seven 4-square piece shapes are the remaining shared element. They're widely used in other games, but the court also counted piece *shape* as part of the protected look. Switching to pentominoes would remove even that.

None of this is legal advice. It reflects US case law and app-store rules as of November 2026.
