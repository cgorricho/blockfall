# BLOCKFALL ROCKER-TOWER

[tower-3D](../03_tower-3D/README.md)'s cylinder, standing on a **rocking spherical base** with [Rocker](../02_rocker/README.md)'s rolling physics carried over to full 3D. Rust + macroquad.

```powershell
& 'C:\Users\carlos\Documents\Projects\blockfall\bin\blockfall-rocker-tower.exe'
& 'C:\Users\carlos\Documents\Projects\blockfall\bin\blockfall-rocker-tower.exe' --ring 16 --seed 42
```

## How it plays

- **The tower:** the same as tower-3D. It's 20 rows tall and 16, 24 or 32 bricks round. It has the same pieces, rotation, wrapping kicks, PRNG, 7-bag, gravity, lock delay and scoring. A full ring clears.
- **The base:** the tower stands on the flat top of a wooden spherical cap. Its underside is part of a sphere, spanning ±45° from its lowest point. The top disc is exactly as wide as the tower.
- **Rocking:** the whole thing (base plus every locked brick) is one rigid body that rolls on the floor without slipping. It can tip in **any direction**. As it rolls, the base moves across the floor.
- **Weight:** pile bricks on one side and the tower leans that way. When it settles, the centre of mass sits straight above the contact point.
- **Height:** a tall stack is top-heavy. Once the centre of mass rises above the sphere's centre C, upright is no longer stable. The tower then tips over even if it's perfectly balanced.
- **Pieces fall along the tower's own axis,** however far it leans. The board is in the tower's frame, so the rules are exactly tower-3D's.
- **Turning is physical:** ← / → move the piece one column on the wall, as before. The **tower itself then twists** on its contact point to bring that column to the front. Its lean turns with it. The camera never moves round.
- **Impacts:** a landing piece pushes the tower. Its momentum about the contact point is kept through the impact. A **hard drop** lands at 6 cells/s, which hits much harder than a piece that settles at gravity speed.
- **Game over:**
  - the tilt passes **30°** (the friction angle), and the stack breaks loose and slides off, or
  - a new piece has no room.
- **After a slide:** the bricks tumble as debris. They hit each other, the base and the floor, and settle on the floor round the base. The empty base rocks back upright. The physics keeps running after game over: if the base still tips past 30° with bricks on it, they come off.

## Controls

| key | action |
|-----|--------|
| ← / A | piece one column right on the wall; the tower twists CW (seen from above) |
| → / D | piece one column left; the tower twists CCW |
| ↑ / W / X | rotate CW |
| Z | rotate CCW |
| ↓ / S | soft drop; on a resting piece, lock it now |
| Space | hard drop |
| PgUp / PgDn | tilt the camera up / down |
| 1 / 2 / 3 | new game with a 16 / 24 / 32 ring |
| G | wire frame of the empty wall |
| C | physics markers on/off |
| P | pause (the physics stops too) |
| R | restart |
| Q / Esc | quit |

## On screen

- **C** is the centre of the base's sphere. It always stays straight above the contact point.
- The **ringed dot** is the centre of mass of base plus bricks. Its dashed **plumb line** drops to the floor. The green triangle is the **contact point**. The markers are drawn on top of everything, so they can be seen through the tower.
- **TILT** is the angle now. **REST** is the angle it will settle at, or "top-heavy" when there's no stable rest. **MAX** is the largest tilt this game.
- **The bubble level** is a top view, with the camera at the bottom. The dot shows which way and how far the tower leans, out to 45° at the rim. The red ring is the 30° slide angle. The hollow ring is where it will come to rest.
- **The base** has two-tone wooden sectors so its twisting and rolling are visible.

## Physics (`src/physics.rs`)

**Units:** cell = 1, g = 49 cells/s² (as in Rocker), and one brick has mass 1.

**Body frame:**
- The origin is at the sphere's centre C, with +Y up the tower axis.
- The cap is the part of a ball of radius ρ below y = −ρ·cos 45°.
- ρ = r_out·√2, so the cap's top disc has the tower's outer radius. For a 32 ring, ρ ≈ 7.9.

**Base mass:** the cap is uniform wood, as heavy as **3 full rings** (96 for a 32 ring). Its inertia tensor is integrated in thin discs, and a test checks the centroid against the closed-form formula.

**Rolling:** C stays at height ρ, straight above the contact point P, and the body point at P doesn't move:

    v_C = ω × ρŷ        v_G = ω × d,   d = G − P

Taking moments about G and eliminating the contact force gives the rolling-ball equation for an off-centre mass:

    I_P α = −ω × (I_G ω) − m d × (ω × (ω × (G − C)) − g) + τ
    I_P   = I_G + m(|d|²·1 − d dᵀ)

- **State:** orientation (a quaternion), the position of C and the angular velocity ω.
- **Integration:** RK2 midpoint at 480 Hz.
- **Damping τ:** rolling resistance −0.45·I_P·ω, as in Rocker. On top of that, pivot friction against spin about the vertical.
- **Rim:** the cap ends at 45°. If the tilt ever reaches 45° (only possible after game over), the rim stops it. This is a simplification: the body doesn't pivot onto the rim edge.
- **Landing:** angular momentum about P (I_P·ω, plus the falling piece's own) is kept through the impact.
- **Line clears:** the mass changes and ω is kept, as in Rocker.
- **Twist:** a rotation about the vertical through C. It keeps C, P and the height of the centre of mass, so it never adds or removes energy.

**Debris:** spheres of radius 0.47, drawn as cubes, with gravity and position-based contacts in 8 substeps. They collide with each other, the moving base (its top disc and spherical underside) and the floor. A circular arena keeps them on screen. When they come to rest they turn to lie flat. Debris doesn't push back on the base, the same simplification as Rocker.

## Tests

43 tests: `cargo test --release`.

**Physics:**
- The cap's centroid, volume and symmetry match the formulas.
- The empty base settles upright from leans in several directions.
- Energy never grows with damping, and is conserved without it.
- A load in one plane rocks exactly like Rocker's 1-DOF model (the two are integrated side by side, and C rolls −ρ·Δθ).
- An off-centre load settles at the predicted angle, with its centre of mass over the contact point.
- A tall balanced tower is unstable.
- The tilt never passes the rim.
- A landing tips the tower toward the piece.
- A twist keeps the energy, the contact point and the tilt.

**Game:** tower-3D's 22 rule tests, plus:
- a lopsided pile slides off and ends the game,
- debris comes to rest on the floor inside the arena and the base rights itself,
- a low balanced stack stands,
- a hard drop hits harder than a gentle lock,
- a line clear takes the weight off,
- the tower twists so the piece faces the camera,
- restart and pause behave correctly.

## Build

```powershell
cd 04_rocker-tower
cargo test --release
cargo build --release
copy target\release\blockfall-rocker-tower.exe ..\bin\
```
`..\build.ps1` builds it with everything else.
