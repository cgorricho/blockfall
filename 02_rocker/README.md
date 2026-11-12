# BLOCKFALL ROCKER

The BLOCKFALL pieces, dealt in the same order, stacked on a **rocking base**. It's written in Rust with [macroquad](https://macroquad.rs) and runs as a window, not in the terminal.

```powershell
.\bin\blockfall-rocker.exe                # easy mode, random seed
.\bin\blockfall-rocker.exe --hard         # 45° hacksaw top, diamond blocks
.\bin\blockfall-rocker.exe --ai --seed 42 # watch the computer play
```

## How it plays

- The 10×20 well sits on a wooden rocker. The well's floor is the rocker's flat top. The underside is a circular arc: the lower half of the bottom half of a circle, ±45° from the bottom.
- The pieces, rotation, wall kicks, PRNG, 7-bag, scoring and levels are the same as in [../SPEC.md](../SPEC.md). Seed 42 deals the same pieces as the other editions.
- Pieces fall and lock **in the well's own grid**, like in BLOCKFALL. Once locked, they're glued to the base, and the whole thing rocks as one rigid body.
- After each piece the base rocks and settles at a new angle. Pile weight on one side and it leans that way.
- **Game over** comes several ways:
  - **Easy mode:** the tilt goes past **30°** (the friction angle), and the stack breaks loose and slides off.
  - **Hard mode:** the blocks grip the teeth and each other, so the stack never slides. It **topples** when the base rolls to **45°** (the end of its arc hits the floor), or sooner if a block hanging past the edge swings down into the floor.
  - **Both modes:** a new piece has no room to spawn.
  - **Hard mode:** the third piece lost off the side.
- **The physics keeps running after game over.** If the base then rolls past the limit, the stack still comes off.
- **Lock delay:** a piece that comes to rest locks after 0.5 s. A successful move or rotation restarts that wait, up to 15 times per piece. A hard drop locks at once.
- Full rows clear as usual. That's also how you take weight off.
- **When the stack slides off,** the blocks break apart. They tumble, hit each other, the base and the floor, and end up in a heap. The heap stays inside the play area, on either side.
- **Easy mode:** the top of the base is flat and the blocks are squares, on a 10×20 grid.
- **Hard mode** (`--hard`, or M in the game): the top of the base is a 45° hacksaw.
  - **The teeth:** 4 teeth with 45° flanks on both sides, each flank two block-edges long. The top is 8√2 ≈ 11.3 wide and ends on a peak at each side. The arc radius is 8.
  - **Diamond blocks:** because the notches are 90° V's, the blocks are diamonds, unit squares turned 45°. A piece spawns already turned and falls straight down. One left/right step moves it one diamond width (√2).
  - **How pieces land:** a piece falls straight down and stops at the first thing it touches.
    - **Face contact holds.** If a block's lower face lands flush on something (another block, a tooth flank, a notch), friction holds it there, even on a 45° slope.
    - **Point contact tips.** If the piece only rests on a point (a tooth tip, or another block's corner), it tips off that point toward the side its centre of mass is on. If it's balanced exactly, it tips the way the base leans.
    - **End peaks:** each end peak is only half a tooth. A piece balanced on one can tip outward and fall off, or inward onto the flank.
    - **Sideways moves** are blocked by any block touching the moving piece's faces on that side. A diamond can't squeeze between two corners.
    - **Pushing down a slope:** when a piece is resting, ← or → first tries to slide it one step diagonally down the slope that way, and only then a plain sideways step. That's how you get a piece into a slot it can't fall into straight.
  - **No walls:** pieces can move past the edge of the base. A piece can come to rest hanging over the edge, which adds torque. A piece that drops past the edge, or slides off the outer slope, tumbles to the floor and is **lost**. The panel counts lost pieces. The **third lost piece ends the game**. The ghost turns into a red outline when the piece would fall off.
  - **Rows:** the diamonds over the base stack in half-rows that alternate 8 and 7 diamonds (the bottom 8 lie on the outer flanks).
  - **Line clears:** a **band**, two neighbouring half-rows, clears when both are full across the width of the base. Any overhang in those rows goes with it. It counts as one line. The bottom half-row holds the tooth tips and never clears.
  - **Sealed cells:** a block falling into an empty cell passes both cells diagonally above it, and sticks to the first face it touches. So once **either** of those two cells is filled, the empty cell can never be filled.

### Controls

| key | action |
|-----|--------|
| ← → (A D) | move |
| ↑ / W / X | rotate clockwise |
| Z | rotate counter-clockwise |
| ↓ (S) | soft drop; on a resting piece, lock it now |
| Space | hard drop (it lands harder: see impacts below) |
| P | pause |
| R | restart |
| M | switch easy / hard (restarts) |
| I | computer player on / off (restarts) |
| Q / Esc | quit |

### On screen

- **C** is the centre of the arc's circle.
- The ringed dot is the **centre of mass** of base plus pieces. Its dashed plumb line drops to the floor. The base comes to rest when that line passes through the contact point (the small triangle).
- **TILT** shows the angle now and the angle the stack will **rest at**.
- The gauge shows the same: the needle is the tilt now, the green tick is the resting angle, and the red arcs are the slide zone.
- **BALANCE** shows how far the centre of mass is below C. If it rises above C, the stack is top-heavy and has no resting angle: it will tip over even when perfectly centred.

## The physics ([src/physics.rs](src/physics.rs))

The base plus every locked cell is one rigid body that **rolls without slipping** on a flat floor. Its only degree of freedom is the tilt θ.

- **Units:** a cell is 1 unit (think 20 cm), so gravity is 49 cells/s². A cell has mass 1.
- **Geometry:**
  - Easy mode: R = 5√2 ≈ 7.07, which makes the flat top exactly 10 cells wide. The wood weighs 30.
  - Hard mode: R = 8, for the 11.3-wide toothed top. The wood has the same density, so it weighs 38.4, plus 8 for the teeth (density 1, like the blocks).
  - Mass, centre of mass and moment of inertia are integrated numerically. A test checks the segment against the textbook centroid formula.
- **Equation of motion** (from the Lagrangian of a rolling body with an offset centre of mass):

  `I_P(θ)·θ'' + ½·I_P'(θ)·θ'² + m·g·x_com(θ) + damping = 0`

  - `I_P` is the moment of inertia about the contact point.
  - `x_com` is the horizontal distance from the contact point to the centre of mass.
  - It's integrated with RK2 at 480 Hz.
- **Equilibrium:** the base rests where the centre of mass is directly above the contact point, so tan θ = x_com / y_com. That rest is stable only while the centre of mass is below C. Tall stacks therefore become unstable even when balanced.
- **Impacts:** a landing piece conserves angular momentum about the contact point. A hard drop lands at 6 cells/s, a normal landing at the gravity speed. So hard-dropping on an edge kicks the base harder.
- **Damping:** the base is lightly damped (rolling resistance), so it overshoots. A line clear that suddenly shifts the weight can swing it well past its new resting angle. That's often what slides the stack off, not the resting angle itself.
- **Line clears** remove mass instantly, and the base keeps its angular velocity.
- **The slide-off:** the blocks are simulated as discs with gravity, contact with each other, the moving base, the floor, and the edges of the play area. Each block starts with the velocity of its spot on the swinging base plus a push down the slope.
- **Simplification:** debris doesn't push back on the base. After unloading, the light base swings back hard and can fling a few blocks that are still on it (in hard mode, blocks wedged in a notch) across to the other side.
- **Why hard mode doesn't slide:** a block that holds on a 45° face needs a lot of friction. With that much grip, and the stack interlocked with 45° teeth, nothing slides at 30°. The stack stays on until the rocker itself tips over.

All the tuning constants are at the top of `physics.rs` (`BASE_MASS`, `DAMPING`, `SLIDE_ANGLE`, `G`) and `game.rs` (`HARD_IMPACT`). Other curvature profiles will need changes to the geometry part of `physics.rs`: the arc, `R`, `CHORD_Y` and `segment_mass`. The rolling model assumes a circular arc.

## Computer player

The computer player uses SPEC.md's planner and board score, plus a balance term. For every placement it simulates the next 3 s of rocking and penalises big swings. A strong balance term made it stack badly and top out, so the penalty only kicks in above 10° and gets steep near 25°.

Headless benchmark, with no window. The base settles for 1 s after each piece:

```powershell
.\bin\blockfall-rocker.exe --bench 3000 --seed 42 [--hard]
```
The result line is printed and also written to `%TEMP%\blockfall-rocker-bench.txt`. Typical results:
- **Easy mode:** about one lost game per 500 pieces.
- **Hard mode:** about one lost game per 75 pieces, with around 50 bands per 1,500 pieces. Because faces hold, cells get sealed easily, and the AI builds wide overhanging stacks. The planner counts holes using the diamond rule (sealed cells), but it still stacks poorly on the diamond grid. Treat it as a demo there, not a strong player.

## Tests

```powershell
cd 02_rocker; cargo test --release
```
There are 33 tests.
- **Physics:**
  - the segment centroid
  - settling upright
  - energy never increasing
  - the predicted resting angle
  - a one-sided pile tipping over
  - a tall tower being unstable
  - the impact direction
- **Hard-mode geometry:**
  - diamonds stay inside the walls and out of the teeth
  - the 4 notches
  - an O piece nesting in a V
  - slides going toward the lean
- **Line clears:** rows in easy mode, bands in hard mode, and the teeth row never clearing.
- **Coming off:** hard mode holds past 30°; an overhang hitting the floor topples the stack; the stack still comes off after game over; slide-off to both sides in both modes, mirror symmetry, and debris staying in the play area.
- **Lock delay and pushing:** a resting piece waits before locking; a move restarts the wait; ↓ locks at once; a push slides a piece down a flank into a notch, or into a slot it couldn't fall into.
- **Landing and falling off:** faces hold on a slope; a piece on a tooth tip tips toward the lean; a piece lands on a diagonal and overhangs the edge; a piece on the end peak tips outward (lost) or inward; a piece passing outside the end peak falls off; a piece moved past the edge is lost; the third lost piece ends the game.
- **AI:** survival.
