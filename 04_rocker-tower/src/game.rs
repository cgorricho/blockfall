//! Game rules: BLOCKFALL (../SPEC.md) wrapped around a cylinder (as in
//! tower-3D), standing on a rocking spherical base (physics.rs).
//! Board columns wrap modulo `cols` (16, 24 or 32); rows run top (0) to bottom (ROWS-1).
//! The board lives in the tower's own frame: pieces fall along the tower axis
//! however far it leans.

use crate::physics::{self, Mass, Rocker};
use macroquad::math::{DQuat, DVec3};
use std::f64::consts::{SQRT_2, TAU};

/// Storage width; a game uses the first `cols` columns.
pub const MAX_COLS: usize = 32;
/// Ring sizes (columns around) the player can choose.
pub const MODES: [i32; 3] = [16, 24, 32];
/// Seconds a resting piece waits before it locks.
pub const LOCK_DELAY: f64 = 0.5;
/// Moves/rotations that may restart the lock delay, per piece.
const MAX_LOCK_RESETS: i32 = 15;
pub const ROWS: i32 = 20;
/// Speed (cells/s) a piece has when it lands after a hard drop (as in Rocker).
pub const HARD_IMPACT: f64 = 6.0;
/// How fast the tower is twisted to bring the piece's column to the front (1/s).
pub const TURN_RATE: f64 = 22.0;
/// How long the stack slides off before GAME OVER.
const SLIDE_TIME: f64 = 3.0;
/// Debris bricks collide as spheres of this radius.
pub const DEBRIS_R: f64 = 0.47;
/// Longest physics/rules step; longer updates are split.
const MAX_STEP: f64 = 1.0 / 60.0;

/// Tower and base geometry for one ring size. Body frame: origin at the
/// centre of the base's sphere, +Y up the tower axis, column c centred at
/// angle c*dtheta, at (r sin a, y, r cos a).
#[derive(Clone, Copy, Debug)]
pub struct Geo {
    pub cols: i32,
    pub dtheta: f64,
    pub r_mid: f64,
    pub r_in: f64,
    pub r_out: f64,
    /// Radius of the spherical underside; its ±45° cap has a flat top of radius r_out.
    pub rho: f64,
    /// Body height of the flat top the tower stands on (= −ρ·cos 45°).
    pub top: f64,
}

impl Geo {
    pub fn new(cols: i32) -> Geo {
        // mid radius chosen so a cell is 1 unit wide at the middle of the wall
        let r_mid = cols as f64 / TAU;
        let r_out = r_mid + 0.5;
        Geo { cols, dtheta: TAU / cols as f64, r_mid, r_in: r_mid - 0.5, r_out, rho: r_out * SQRT_2, top: -r_out }
    }
    /// Body height of the bottom of board row `row`.
    pub fn row_y(&self, row: i32) -> f64 {
        self.top + (ROWS - 1 - row) as f64
    }
    pub fn cell_center(&self, col: i32, row: i32) -> DVec3 {
        let a = col as f64 * self.dtheta;
        DVec3::new(self.r_mid * a.sin(), self.row_y(row) + 0.5, self.r_mid * a.cos())
    }
}

/// The wooden cap: as heavy as BASE_RINGS full rings.
pub fn base_mass(g: &Geo) -> Mass {
    physics::cap_mass(g.rho, physics::BASE_RINGS * g.cols as f64)
}

/// A brick flying off after the stack slid.
pub struct Debris {
    pub pos: DVec3,
    pub vel: DVec3,
    pub rot: DQuat,
    pub spin: DVec3,
    pub kind: u8,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cause {
    None,
    /// The tilt passed the friction angle (radians at the moment it broke loose).
    SlidOff(f64),
    /// A new piece had no room.
    Blocked,
}

pub struct PieceDef {
    pub n: i32,
    pub cells: [(i32, i32); 4],
    pub color: u8,
}

pub const PIECES: [PieceDef; 7] = [
    PieceDef { n: 4, cells: [(0, 1), (1, 1), (2, 1), (3, 1)], color: 175 }, // I
    PieceDef { n: 2, cells: [(0, 0), (1, 0), (0, 1), (1, 1)], color: 75 }, // O
    PieceDef { n: 3, cells: [(1, 0), (0, 1), (1, 1), (2, 1)], color: 179 }, // T
    PieceDef { n: 3, cells: [(1, 0), (2, 0), (0, 1), (1, 1)], color: 141 }, // S
    PieceDef { n: 3, cells: [(0, 0), (1, 0), (1, 1), (2, 1)], color: 73 },  // Z
    PieceDef { n: 3, cells: [(0, 0), (0, 1), (1, 1), (2, 1)], color: 107 }, // J
    PieceDef { n: 3, cells: [(2, 0), (0, 1), (1, 1), (2, 1)], color: 167 }, // L
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Playing,
    /// The stack broke loose; seconds since.
    Sliding(f64),
    Over,
}

#[derive(Clone, Copy, Debug)]
pub struct Active {
    pub kind: usize,
    pub n: i32,
    pub cells: [(i32, i32); 4],
    /// Column of the piece box's left edge (always kept in 0..cols).
    pub x: i32,
    pub y: i32,
}

impl Active {
    /// Board cells as (column, row), columns wrapped.
    pub fn board_cells(&self, y: i32, cols: i32) -> [(i32, i32); 4] {
        self.cells.map(|(cx, cy)| ((self.x + cx).rem_euclid(cols), y + cy))
    }
    /// The column (as a float) the camera centres on: the middle of the piece box.
    pub fn focus(&self) -> f64 {
        self.x as f64 + (self.n - 1) as f64 / 2.0
    }
}

pub struct Game {
    /// board[row][col]; 0 = empty, else piece kind + 1.
    pub board: [[u8; MAX_COLS]; ROWS as usize],
    /// Columns around the ring (one of MODES).
    pub cols: i32,
    pub cur: Active,
    pub next: usize,
    bag: Vec<usize>,
    rng: u32,
    pub seed: u32,
    pub score: i64,
    pub lines: i64,
    pub pieces: i64,
    pub state: State,
    pub paused: bool,
    pub cause: Cause,
    timer: f64,
    lock_timer: Option<f64>,
    lock_resets: i32,

    pub geo: Geo,
    base: Mass,
    pub rocker: Rocker,
    pub debris: Vec<Debris>,
    /// Centre and radius of the floor area debris stays in.
    pub arena: (DVec3, f64),
    /// Largest tilt this game (radians).
    pub max_tilt: f64,
}

fn rotate(cells: &[(i32, i32); 4], n: i32, cw: bool) -> [(i32, i32); 4] {
    cells.map(|(x, y)| if cw { (n - 1 - y, x) } else { (y, n - 1 - x) })
}

impl Game {
    pub fn new(seed: u32, cols: i32) -> Self {
        assert!(MODES.contains(&cols), "ring size {cols}");
        let seed = if seed == 0 { 0x9E37_79B9 } else { seed };
        let geo = Geo::new(cols);
        let base = base_mass(&geo);
        let mut g = Game {
            board: [[0; MAX_COLS]; ROWS as usize],
            cols,
            cur: Active { kind: 0, n: 4, cells: PIECES[0].cells, x: 0, y: 0 },
            next: 0,
            bag: Vec::new(),
            rng: seed,
            seed,
            score: 0,
            lines: 0,
            pieces: 0,
            state: State::Playing,
            paused: false,
            cause: Cause::None,
            timer: 0.0,
            lock_timer: None,
            lock_resets: 0,
            geo,
            base,
            rocker: Rocker::new(base, geo.rho),
            debris: Vec::new(),
            arena: (DVec3::ZERO, 0.0),
            max_tilt: 0.0,
        };
        g.reset();
        g
    }

    fn next_rand(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    fn draw7(&mut self) -> usize {
        if self.bag.is_empty() {
            let mut a: Vec<usize> = (0..7).collect();
            for i in (1..7).rev() {
                let j = (self.next_rand() % (i as u32 + 1)) as usize;
                a.swap(i, j);
            }
            self.bag = a;
        }
        self.bag.remove(0)
    }

    /// Restart: clears the bag but keeps the PRNG state (as in SPEC.md).
    pub fn reset(&mut self) {
        self.board = [[0; MAX_COLS]; ROWS as usize];
        self.bag.clear();
        self.score = 0;
        self.lines = 0;
        self.state = State::Playing;
        self.cause = Cause::None;
        self.paused = false;
        self.timer = 0.0;
        self.rocker = Rocker::new(self.base, self.geo.rho);
        self.debris.clear();
        self.max_tilt = 0.0;
        let first = self.draw7();
        self.next = self.draw7();
        self.place(first, 0);
        // start with the first piece's column already facing the camera
        let e = self.front_error();
        self.rocker.twist(-e);
    }

    /// Switch ring size; starts a new game.
    pub fn set_cols(&mut self, cols: i32) {
        assert!(MODES.contains(&cols));
        self.cols = cols;
        self.geo = Geo::new(cols);
        self.base = base_mass(&self.geo);
        self.reset();
    }

    pub fn wrap(&self, c: i32) -> i32 {
        c.rem_euclid(self.cols)
    }

    pub fn level(&self) -> i64 {
        self.lines / 10 + 1
    }

    /// Gravity interval in ms.
    pub fn interval(&self) -> f64 {
        (800 - (self.level() - 1) * 70).max(80) as f64
    }

    pub fn get(&self, col: i32, row: i32) -> u8 {
        self.board[row as usize][self.wrap(col) as usize]
    }

    fn fits(&self, cells: &[(i32, i32); 4], x: i32, y: i32) -> bool {
        cells.iter().all(|&(cx, cy)| {
            let r = y + cy;
            (0..ROWS).contains(&r) && self.board[r as usize][self.wrap(x + cx) as usize] == 0
        })
    }

    /// Put piece `kind` in the box whose centre is at column `front`, top row.
    fn place(&mut self, kind: usize, front: i32) {
        let d = &PIECES[kind];
        self.cur = Active { kind, n: d.n, cells: d.cells, x: self.wrap(front - (d.n - 1) / 2), y: 0 };
        self.lock_timer = None;
        self.lock_resets = 0;
        if !self.fits(&self.cur.cells, self.cur.x, 0) {
            self.state = State::Over;
            self.cause = Cause::Blocked;
        }
    }

    fn spawn(&mut self) {
        let front = self.cur.x + (self.cur.n - 1) / 2;
        let kind = self.next;
        self.next = self.draw7();
        self.place(kind, front);
    }

    /// Locks the current piece. `speed` is how fast it was falling (cells/s),
    /// which sets how hard it hits the tower.
    fn lock(&mut self, speed: f64) {
        let mut add = Mass::default();
        for (c, r) in self.cur.board_cells(self.cur.y, self.cols) {
            self.board[r as usize][c as usize] = self.cur.kind as u8 + 1;
            add = add.plus(Mass::cell(self.geo.cell_center(c, r)));
        }
        self.rocker.land(add, speed);
        self.pieces += 1;
        let n = self.clear_rings();
        if n > 0 {
            self.rocker.set_mass(self.board_mass());
            self.lines += n;
            self.score += [0, 100, 300, 500, 800][n as usize] * self.level();
        }
        self.timer = 0.0;
        self.spawn();
    }

    /// Remove every full ring; rows above drop down. Returns how many.
    fn clear_rings(&mut self) -> i64 {
        let w = self.cols as usize;
        let kept: Vec<[u8; MAX_COLS]> =
            self.board.iter().filter(|row| row[..w].iter().any(|&v| v == 0)).copied().collect();
        let n = ROWS as usize - kept.len();
        let mut b = [[0; MAX_COLS]; ROWS as usize];
        b[n..].copy_from_slice(&kept);
        self.board = b;
        n as i64
    }

    fn active(&self) -> bool {
        self.state == State::Playing && !self.paused
    }

    /// The piece can't fall any further.
    pub fn resting(&self) -> bool {
        !self.fits(&self.cur.cells, self.cur.x, self.cur.y + 1)
    }

    /// After a successful move or rotation: a resting piece gets its lock delay
    /// restarted (up to MAX_LOCK_RESETS times); a piece that can fall again falls.
    fn moved(&mut self) {
        if self.lock_timer.is_some() {
            if !self.resting() {
                self.lock_timer = None;
            } else if self.lock_resets < MAX_LOCK_RESETS {
                self.lock_timer = Some(0.0);
                self.lock_resets += 1;
            }
        }
    }

    /// Move the piece `d` columns around the cylinder (relative to the wall).
    pub fn shift(&mut self, d: i32) -> bool {
        if self.active() && self.fits(&self.cur.cells, self.cur.x + d, self.cur.y) {
            self.cur.x = self.wrap(self.cur.x + d);
            self.moved();
            return true;
        }
        false
    }

    /// ←: the cylinder turns CW seen from above, so its front moves left and
    /// the piece ends up one column further right on the wall.
    pub fn turn_cw(&mut self) -> bool {
        self.shift(1)
    }

    /// →: the cylinder turns CCW, its front moves right.
    pub fn turn_ccw(&mut self) -> bool {
        self.shift(-1)
    }

    pub fn try_rotate(&mut self, cw: bool) -> bool {
        if !self.active() {
            return false;
        }
        let r = rotate(&self.cur.cells, self.cur.n, cw);
        for k in [0, -1, 1, -2, 2] {
            if self.fits(&r, self.cur.x + k, self.cur.y) {
                self.cur.cells = r;
                self.cur.x = self.wrap(self.cur.x + k);
                self.moved();
                return true;
            }
        }
        false
    }

    pub fn soft_drop(&mut self) {
        if !self.active() {
            return;
        }
        if self.fits(&self.cur.cells, self.cur.x, self.cur.y + 1) {
            self.cur.y += 1;
            self.score += 1;
            self.timer = 0.0;
            self.lock_timer = None;
        } else {
            // ↓ on a resting piece locks it at once
            self.lock_resting();
        }
    }

    pub fn hard_drop(&mut self) {
        if !self.active() {
            return;
        }
        let g = self.ghost_y();
        self.score += 2 * (g - self.cur.y) as i64;
        self.cur.y = g;
        self.lock(HARD_IMPACT);
    }

    /// Row where the piece would land.
    pub fn ghost_y(&self) -> i32 {
        let mut y = self.cur.y;
        while self.fits(&self.cur.cells, self.cur.x, y + 1) {
            y += 1;
        }
        y
    }

    /// A gravity tick: the piece falls a row, or starts its lock delay.
    fn gravity(&mut self) {
        if !self.resting() {
            self.cur.y += 1;
            self.lock_timer = None;
        } else if self.lock_timer.is_none() {
            self.lock_timer = Some(0.0);
        }
    }

    /// A resting piece locks at the gravity speed (one row per interval).
    fn lock_resting(&mut self) {
        self.lock(1000.0 / self.interval());
    }

    /// Mass of the base plus every brick on the board.
    pub fn board_mass(&self) -> Mass {
        let mut m = self.base;
        for row in 0..ROWS {
            for col in 0..self.cols {
                if self.get(col, row) != 0 {
                    m = m.plus(Mass::cell(self.geo.cell_center(col, row)));
                }
            }
        }
        m
    }

    pub fn has_blocks(&self) -> bool {
        self.board.iter().any(|row| row[..self.cols as usize].iter().any(|&v| v != 0))
    }

    /// Resting tilt the stack would settle at (None = top-heavy, no rest).
    pub fn equilibrium(&self) -> Option<f64> {
        Rocker::equilibrium(&self.rocker.mass)
    }

    /// Horizontal world direction (x, z) of the resting lean, if there is one.
    pub fn rest_lean_dir(&self) -> Option<(f64, f64)> {
        let c = self.rocker.mass.com();
        let d = self.rocker.q * DVec3::new(c.x, 0.0, c.z);
        let l = d.x.hypot(d.z);
        (l > 1e-9).then(|| (d.x / l, d.z / l))
    }

    /// Angle (radians, + = to the screen right) between the camera-facing
    /// direction and the column the piece is centred on.
    pub fn front_error(&self) -> f64 {
        let a = self.cur.focus() * self.geo.dtheta;
        let d = self.rocker.q * DVec3::new(a.sin(), 0.0, a.cos());
        d.x.atan2(d.z)
    }

    /// The player twists the tower so the piece's column faces the camera,
    /// easing in (time constant 1/TURN_RATE), the short way round.
    fn steer(&mut self, dt: f64) {
        let e = self.front_error();
        self.rocker.twist(-e * (1.0 - (-TURN_RATE * dt).exp()));
    }

    /// Advances physics, the twist, gravity, the lock delay and the debris.
    pub fn update(&mut self, dt: f64) {
        if self.paused {
            return;
        }
        let mut left = dt;
        while left > 1e-12 {
            let h = left.min(MAX_STEP);
            self.update_step(h);
            left -= h;
        }
    }

    fn update_step(&mut self, dt: f64) {
        self.rocker.step(dt);
        self.max_tilt = self.max_tilt.max(self.rocker.tilt());
        self.step_debris(dt);
        match self.state {
            State::Playing => {
                self.steer(dt);
                if self.rocker.tilt() > physics::SLIDE_ANGLE {
                    self.cause = Cause::SlidOff(self.rocker.tilt());
                    self.release_stack();
                    self.state = State::Sliding(0.0);
                    return;
                }
                self.rules(dt);
            }
            State::Sliding(t) => {
                let t = t + dt;
                self.state = if t >= SLIDE_TIME { State::Over } else { State::Sliding(t) };
            }
            State::Over => {
                // the physics keeps going after game over: past the friction
                // angle the stack still comes off, whatever ended the game
                if self.has_blocks() && self.rocker.tilt() > physics::SLIDE_ANGLE {
                    self.release_stack();
                }
            }
        }
    }

    /// Every brick breaks loose. Each starts with the velocity of its spot on
    /// the moving tower plus a push down the slope of the tilted top; from then
    /// on the bricks fall, hit each other, the base and the floor (step_debris).
    fn release_stack(&mut self) {
        let axis = self.rocker.axis();
        let mut downhill = -DVec3::Y - axis * (-DVec3::Y).dot(axis);
        if downhill.length_squared() > 1e-12 {
            downhill = downhill.normalize();
        }
        for row in 0..ROWS {
            for col in 0..self.cols {
                let v = self.get(col, row);
                if v == 0 {
                    continue;
                }
                let b = self.geo.cell_center(col, row);
                self.debris.push(Debris {
                    pos: self.rocker.to_world(b),
                    vel: self.rocker.velocity_of(b) + downhill * 2.0,
                    rot: self.rocker.q * DQuat::from_rotation_y(col as f64 * self.geo.dtheta),
                    spin: DVec3::ZERO,
                    kind: v,
                });
            }
        }
        self.board = [[0; MAX_COLS]; ROWS as usize];
        self.rocker.set_mass(self.base);
        self.arena = (self.rocker.contact(), self.geo.rho + 4.5);
    }

    /// Debris: spheres of radius DEBRIS_R with gravity, the floor, the edge of
    /// the arena, the (moving) base, and each other. Position-based contacts,
    /// solved in substeps. Debris doesn't push back on the base.
    fn step_debris(&mut self, dt: f64) {
        if self.debris.is_empty() {
            return;
        }
        const SUB: usize = 8;
        let h = dt / SUB as f64;
        let r = DEBRIS_R;
        let geo = self.geo;
        let rk = self.rocker;
        let rim = geo.rho * physics::CAP_ANGLE.sin();
        for _ in 0..SUB {
            for d in &mut self.debris {
                d.vel.y -= physics::G * h;
                d.pos += d.vel * h;
                let s = d.spin * h;
                if s.length_squared() > 1e-18 {
                    d.rot = (DQuat::from_scaled_axis(s) * d.rot).normalize();
                }
            }
            let n = self.debris.len();
            for i in 0..n {
                for j in i + 1..n {
                    let dv = self.debris[j].pos - self.debris[i].pos;
                    let dist2 = dv.length_squared();
                    if dist2 >= 4.0 * r * r || dist2 < 1e-12 {
                        continue;
                    }
                    let dist = dist2.sqrt();
                    let nrm = dv / dist;
                    let push = nrm * ((2.0 * r - dist) / 2.0);
                    self.debris[i].pos -= push;
                    self.debris[j].pos += push;
                    let rel = (self.debris[j].vel - self.debris[i].vel).dot(nrm);
                    if rel < 0.0 {
                        let imp = nrm * (-rel * 0.6);
                        self.debris[i].vel -= imp;
                        self.debris[j].vel += imp;
                        let tang = self.debris[j].vel - self.debris[i].vel;
                        let spin = nrm.cross(tang) * 0.3;
                        self.debris[i].spin += spin;
                        self.debris[j].spin -= spin;
                    }
                }
            }
            for d in &mut self.debris {
                // the base: the flat top (over the disc) or the spherical underside
                let b = rk.to_body(d.pos);
                let hr = b.x.hypot(b.z);
                let len = b.length();
                if len < geo.rho + r && b.y < geo.top + r {
                    let up = (geo.top + r - b.y, DVec3::Y);
                    let radial = (geo.rho + r - len, b / len.max(1e-9));
                    let (depth, nb) = if (b.y < geo.top && radial.0 < up.0) || hr > rim { radial } else { up };
                    let nw = rk.q * nb;
                    d.pos += nw * depth;
                    let vb = rk.velocity_of(rk.to_body(d.pos));
                    let rel = d.vel - vb;
                    let vn = rel.dot(nw);
                    if vn < 0.0 {
                        let tang = rel - nw * vn;
                        d.vel = vb + tang * 0.97 + nw * (-vn * 0.1);
                    }
                }
                // floor
                if d.pos.y < r {
                    d.pos.y = r;
                    if d.vel.y < 0.0 {
                        d.vel.y = -d.vel.y * 0.15;
                    }
                    d.vel.x *= 0.9;
                    d.vel.z *= 0.9;
                    d.spin *= 0.85;
                    // come to rest lying flat: turn the most upright axis to vertical
                    let axes = [d.rot * DVec3::X, d.rot * DVec3::Y, d.rot * DVec3::Z];
                    let a = axes.into_iter().max_by(|p, q| p.y.abs().total_cmp(&q.y.abs())).unwrap();
                    let to = if a.y >= 0.0 { DVec3::Y } else { -DVec3::Y };
                    let arc = DQuat::from_rotation_arc(a, to);
                    d.rot = (DQuat::IDENTITY.slerp(arc, 0.05) * d.rot).normalize();
                }
                // edge of the arena
                let (c, rad) = self.arena;
                let off = DVec3::new(d.pos.x - c.x, 0.0, d.pos.z - c.z);
                let l = off.length();
                if l > rad - r {
                    let n = off / l;
                    d.pos -= n * (l - (rad - r));
                    let vn = d.vel.dot(n);
                    if vn > 0.0 {
                        d.vel -= n * (vn * 1.3);
                    }
                }
                if d.spin.length() > 8.0 {
                    d.spin = d.spin.normalize() * 8.0;
                }
            }
        }
    }

    /// Gravity and the lock delay.
    fn rules(&mut self, dt: f64) {
        if !self.active() {
            return;
        }

        self.timer += dt * 1000.0;
        while self.timer >= self.interval() {
            self.timer -= self.interval();
            self.gravity();
        }
        if let Some(t) = self.lock_timer {
            let t = t + dt;
            if t < LOCK_DELAY {
                self.lock_timer = Some(t);
            } else if self.resting() {
                self.lock_resting();
            } else {
                self.lock_timer = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLS: i32 = 32;

    fn wrap(c: i32) -> i32 {
        c.rem_euclid(COLS)
    }

    fn empty_game() -> Game {
        let mut g = Game::new(42, COLS);
        g.board = [[0; MAX_COLS]; ROWS as usize];
        g
    }

    #[test]
    fn zero_seed_is_replaced() {
        assert_eq!(Game::new(0, 16).seed, 0x9E37_79B9);
    }

    #[test]
    fn bags_are_permutations() {
        let mut g = Game::new(7, 24);
        g.bag.clear();
        for _ in 0..20 {
            let mut seen: Vec<usize> = (0..7).map(|_| g.draw7()).collect();
            seen.sort();
            assert_eq!(seen, (0..7).collect::<Vec<_>>());
        }
    }

    #[test]
    fn same_seed_same_pieces() {
        let (mut a, mut b) = (Game::new(123, 16), Game::new(123, 16));
        for _ in 0..50 {
            assert_eq!(a.draw7(), b.draw7());
        }
    }

    #[test]
    fn rotation_matches_spec() {
        // T rotated CW: (x,y) -> (n-1-y, x)
        let r = rotate(&PIECES[2].cells, 3, true);
        assert_eq!(r, [(2, 1), (1, 0), (1, 1), (1, 2)]);
        // CW then CCW is the identity
        assert_eq!(rotate(&r, 3, false), PIECES[2].cells);
    }

    #[test]
    fn left_arrow_moves_piece_right_on_the_wall() {
        let mut g = empty_game();
        let x = g.cur.x;
        assert!(g.turn_cw());
        assert_eq!(g.cur.x, wrap(x + 1));
        assert!(g.turn_ccw());
        assert_eq!(g.cur.x, x);
    }

    #[test]
    fn columns_wrap_around_the_seam() {
        let mut g = empty_game();
        g.cur.x = 0;
        assert!(g.turn_ccw());
        assert_eq!(g.cur.x, COLS - 1);
        let cols: Vec<i32> = g.cur.board_cells(g.cur.y, COLS).iter().map(|c| c.0).collect();
        assert!(cols.contains(&(COLS - 1)) && cols.contains(&0), "piece straddles the seam: {cols:?}");
        // a full turn comes back to the start
        for _ in 0..COLS {
            assert!(g.turn_cw());
        }
        assert_eq!(g.cur.x, COLS - 1);
    }

    #[test]
    fn blocks_stop_sideways_moves_across_the_seam() {
        let mut g = empty_game();
        g.cur = Active { kind: 1, n: 2, cells: PIECES[1].cells, x: 0, y: 0 }; // O at cols 0,1
        g.board[0][(COLS - 1) as usize] = 3;
        assert!(!g.turn_ccw());
        assert_eq!(g.cur.x, 0);
    }

    #[test]
    fn hard_drop_lands_on_floor_and_scores() {
        let mut g = empty_game();
        g.cur = Active { kind: 1, n: 2, cells: PIECES[1].cells, x: 5, y: 0 };
        g.next = 2;
        g.hard_drop();
        assert_eq!(g.get(5, ROWS - 1), 2);
        assert_eq!(g.get(6, ROWS - 2), 2);
        assert_eq!(g.score, 2 * (ROWS - 2) as i64);
        assert_eq!(g.pieces, 1);
    }

    #[test]
    fn ghost_rests_on_stack() {
        let mut g = empty_game();
        g.cur = Active { kind: 1, n: 2, cells: PIECES[1].cells, x: 3, y: 0 };
        g.board[10][4] = 1;
        assert_eq!(g.ghost_y(), 8);
    }

    #[test]
    fn full_ring_clears_and_rows_drop() {
        let mut g = empty_game();
        let bottom = (ROWS - 1) as usize;
        for c in 0..COLS as usize {
            g.board[bottom][c] = 1;
        }
        g.board[bottom][3] = 0; // gap
        g.board[bottom - 1][7] = 5; // a block above
        // vertical I (rotated) fills the gap
        g.cur = Active { kind: 0, n: 4, cells: rotate(&PIECES[0].cells, 4, true), x: 1, y: 0 };
        // rotated I occupies column x+2 = 3
        g.next = 1;
        g.hard_drop();
        assert_eq!(g.lines, 1);
        assert_eq!(g.score, 2 * (ROWS - 4) as i64 + 100);
        // block that was above the cleared ring dropped one row
        assert_eq!(g.get(7, ROWS - 1), 5);
        // the I's three upper cells dropped too
        assert_eq!(g.get(3, ROWS - 1), 1);
        assert_eq!(g.get(3, ROWS - 3), 1);
        assert_eq!(g.get(3, ROWS - 4), 0);
    }

    #[test]
    fn every_mode_clears_exactly_its_full_ring() {
        for cols in MODES {
            let mut g = Game::new(5, cols);
            g.board = [[0; MAX_COLS]; ROWS as usize];
            let bottom = (ROWS - 1) as usize;
            for c in 0..cols as usize - 1 {
                g.board[bottom][c] = 1;
            }
            assert_eq!(g.clear_rings(), 0, "{cols}: one short");
            g.board[bottom][cols as usize - 1] = 1;
            assert_eq!(g.clear_rings(), 1, "{cols}: full ring");
        }
    }

    #[test]
    fn every_mode_wraps_at_its_size() {
        for cols in MODES {
            let mut g = Game::new(5, cols);
            g.board = [[0; MAX_COLS]; ROWS as usize];
            g.cur.x = 0;
            assert!(g.turn_ccw());
            assert_eq!(g.cur.x, cols - 1);
            for _ in 0..cols {
                assert!(g.turn_cw());
            }
            assert_eq!(g.cur.x, cols - 1);
        }
    }

    #[test]
    fn switching_mode_restarts() {
        let mut g = Game::new(5, 32);
        g.hard_drop();
        g.set_cols(16);
        assert_eq!((g.cols, g.score, g.board[(ROWS - 1) as usize][0..32].iter().any(|&v| v != 0)), (16, 0, false));
    }

    #[test]
    fn partial_ring_does_not_clear() {
        let mut g = empty_game();
        for c in 0..(COLS - 1) as usize {
            g.board[(ROWS - 1) as usize][c] = 1;
        }
        assert_eq!(g.clear_rings(), 0);
    }

    #[test]
    fn rotation_kicks_wrap_around() {
        let mut g = empty_game();
        // vertical I at column 0 next to a block in column 1; rotating to
        // horizontal needs a kick and may land across the seam.
        g.cur = Active { kind: 0, n: 4, cells: rotate(&PIECES[0].cells, 4, true), x: COLS - 2, y: 5 };
        assert!(g.try_rotate(true));
        assert!(g.cur.board_cells(g.cur.y, COLS).iter().all(|&(c, r)| (0..COLS).contains(&c) && (0..ROWS).contains(&r)));
    }

    #[test]
    fn gravity_moves_then_locks() {
        let mut g = empty_game();
        let y = g.cur.y;
        g.update(0.81);
        assert_eq!(g.cur.y, y + 1);
        g.update(0.8 * ROWS as f64);
        assert!(g.pieces >= 1);
    }

    #[test]
    fn a_resting_piece_waits_before_locking() {
        let mut g = empty_game();
        g.cur.y = g.ghost_y();
        g.gravity(); // touches down: lock delay starts
        assert_eq!(g.pieces, 0, "not locked yet");
        g.update(LOCK_DELAY * 0.8);
        assert_eq!(g.pieces, 0, "still waiting");
        assert!(g.turn_cw());
        g.update(LOCK_DELAY * 0.8);
        assert_eq!(g.pieces, 0, "the move restarted the delay");
        g.update(LOCK_DELAY * 0.4);
        assert_eq!(g.pieces, 1, "locked after the delay");
    }

    #[test]
    fn lock_delay_restarts_are_limited() {
        let mut g = empty_game();
        g.cur.y = g.ghost_y();
        g.gravity();
        for i in 0..40 {
            g.update(LOCK_DELAY * 0.3);
            if g.pieces > 0 {
                assert!(i > MAX_LOCK_RESETS as usize, "locked after {i} moves");
                return;
            }
            if i % 2 == 0 { g.turn_cw(); } else { g.turn_ccw(); }
        }
        panic!("never locked");
    }

    #[test]
    fn down_on_a_resting_piece_locks_at_once() {
        let mut g = empty_game();
        g.cur.y = g.ghost_y();
        g.soft_drop();
        assert_eq!(g.pieces, 1);
    }

    #[test]
    fn a_piece_moved_off_a_ledge_falls_again() {
        let mut g = empty_game();
        g.cur = Active { kind: 1, n: 2, cells: PIECES[1].cells, x: 3, y: 0 };
        g.board[10][3] = 1;
        g.board[10][4] = 1;
        g.cur.y = g.ghost_y();
        g.gravity();
        assert!(g.lock_timer.is_some());
        assert!(g.turn_cw() && g.turn_cw()); // off the ledge
        assert!(g.lock_timer.is_none());
        g.update(LOCK_DELAY * 2.0);
        assert_eq!(g.pieces, 0);
    }

    #[test]
    fn new_piece_spawns_at_front() {
        let mut g = empty_game();
        for _ in 0..5 {
            g.turn_cw();
        }
        let front = g.cur.x + (g.cur.n - 1) / 2;
        g.hard_drop();
        assert_eq!(wrap(g.cur.x + (g.cur.n - 1) / 2), wrap(front));
    }

    #[test]
    fn blocked_spawn_ends_game() {
        let mut g = empty_game();
        for r in 0..3 {
            for c in 0..COLS as usize {
                g.board[r][c] = 1;
            }
            g.board[r][0] = 0;
        }
        g.place(1, 5);
        assert_eq!(g.state, State::Over);
        g.hard_drop(); // ignored
        assert_eq!(g.state, State::Over);
    }
    // ---------- physics coupling ----------

    /// Fills the bottom `rows` rows of columns lo..hi and syncs the mass.
    fn pile(g: &mut Game, lo: i32, hi: i32, rows: i32) {
        for r in 0..rows {
            for c in lo..hi {
                g.board[(ROWS - 1 - r) as usize][g.wrap(c) as usize] = 3;
            }
        }
        g.rocker.set_mass(g.board_mass());
    }

    #[test]
    fn base_top_is_the_tower_floor_and_the_rim_is_its_outer_wall() {
        for cols in MODES {
            let geo = Geo::new(cols);
            assert!((geo.rho * physics::CAP_ANGLE.sin() - geo.r_out).abs() < 1e-12);
            assert!((geo.cell_center(0, ROWS - 1).y - (geo.top + 0.5)).abs() < 1e-12);
        }
    }

    #[test]
    fn a_lopsided_stack_slides_off_and_ends_the_game() {
        let mut g = empty_game();
        pile(&mut g, 0, COLS / 2, 9);
        g.paused = false;
        let mut slid = false;
        for _ in 0..600 {
            g.update(1.0 / 60.0);
            if let Cause::SlidOff(a) = g.cause {
                assert!(a > physics::SLIDE_ANGLE);
                slid = true;
                break;
            }
        }
        assert!(slid, "max tilt {:.1}°", g.max_tilt.to_degrees());
        assert!(matches!(g.state, State::Sliding(_)));
        assert!(!g.has_blocks());
        assert_eq!(g.debris.len(), 9 * COLS as usize / 2);
        assert!((g.rocker.mass.m - g.base.m).abs() < 1e-9);
        g.update(4.0);
        assert_eq!(g.state, State::Over);
    }

    #[test]
    fn debris_comes_to_rest_on_the_floor_inside_the_arena() {
        let mut g = empty_game();
        pile(&mut g, 0, COLS / 2, 9);
        g.update(25.0);
        assert!(matches!(g.cause, Cause::SlidOff(_)));
        let (c, rad) = g.arena;
        for d in &g.debris {
            assert!(d.pos.y > DEBRIS_R - 0.05, "below floor {}", d.pos.y);
            assert!(d.vel.length() < 0.5, "still moving {}", d.vel.length());
            assert!((d.pos.x - c.x).hypot(d.pos.z - c.z) <= rad + 1e-6);
        }
        // the empty base rocks back upright
        assert!(g.rocker.tilt() < 0.02, "tilt {}", g.rocker.tilt().to_degrees());
    }

    #[test]
    fn a_balanced_low_stack_stands() {
        let mut g = empty_game();
        pile(&mut g, 0, COLS, 4);
        g.board[(ROWS - 1) as usize][0] = 0; // keep it from clearing
        g.rocker.set_mass(g.board_mass());
        g.paused = true; // no new pieces, just the physics
        g.paused = false;
        g.rocker.q = DQuat::from_rotation_x(0.1);
        for _ in 0..600 {
            g.rocker.step(1.0 / 60.0);
        }
        assert!(g.rocker.tilt() < 0.02);
    }

    #[test]
    fn hard_drop_on_one_side_tips_the_tower_that_way() {
        let mut g = empty_game();
        g.cur = Active { kind: 1, n: 2, cells: PIECES[1].cells, x: 7, y: 0 };
        g.next = 2;
        g.hard_drop();
        // the O sits around column 8 (angle π/2 in body x for 32 columns)
        let d = g.geo.cell_center(8, ROWS - 1);
        let dw = g.rocker.q * DVec3::new(d.x, 0.0, d.z);
        g.rocker.step(0.15);
        let (lx, lz) = g.rocker.lean();
        assert!(lx * dw.x + lz * dw.z > 0.0, "leans toward the piece");
    }

    #[test]
    fn a_hard_drop_hits_harder_than_a_gentle_lock() {
        let mut a = empty_game();
        a.cur = Active { kind: 1, n: 2, cells: PIECES[1].cells, x: 7, y: 0 };
        a.next = 2;
        let mut b = Game::new(42, COLS);
        b.board = a.board;
        b.cur = a.cur;
        b.next = 2;
        a.hard_drop();
        b.cur.y = b.ghost_y();
        b.soft_drop();
        assert!(a.rocker.w.length() > b.rocker.w.length() * 3.0);
    }

    #[test]
    fn line_clear_takes_the_weight_off() {
        let mut g = empty_game();
        let bottom = (ROWS - 1) as usize;
        for c in 0..COLS as usize {
            g.board[bottom][c] = 1;
        }
        g.board[bottom][3] = 0;
        g.rocker.set_mass(g.board_mass());
        g.cur = Active { kind: 0, n: 4, cells: rotate(&PIECES[0].cells, 4, true), x: 1, y: 0 };
        g.next = 1;
        g.hard_drop();
        assert_eq!(g.lines, 1);
        assert!((g.rocker.mass.m - (g.base.m + 3.0)).abs() < 1e-9, "mass {}", g.rocker.mass.m);
    }

    #[test]
    fn the_tower_turns_so_the_piece_faces_the_camera() {
        let mut g = empty_game();
        assert!(g.front_error().abs() < 1e-9, "starts facing the camera");
        for _ in 0..5 {
            g.turn_cw();
        }
        assert!((g.front_error() - 5.0 * g.geo.dtheta).abs() < 1e-9);
        g.update(0.5);
        assert!(g.front_error().abs() < 1e-3, "error {}", g.front_error());
        // the tower itself turned 5 columns about the vertical
        let (a0, d) = (g.cur.focus() * g.geo.dtheta, g.rocker.q * DVec3::Z);
        assert!((d.x.atan2(d.z) + a0).abs() < 0.01, "column 0 at {}", d.x.atan2(d.z));
    }

    #[test]
    fn restart_resets_the_physics() {
        let mut g = empty_game();
        pile(&mut g, 0, COLS / 2, 9);
        g.update(6.0);
        g.reset();
        assert_eq!(g.state, State::Playing);
        assert!(g.debris.is_empty() && g.rocker.tilt() == 0.0 && g.rocker.w == DVec3::ZERO);
        assert!((g.rocker.mass.m - g.base.m).abs() < 1e-9);
    }

    #[test]
    fn pause_freezes_the_physics() {
        let mut g = empty_game();
        g.rocker.q = DQuat::from_rotation_x(0.2);
        g.paused = true;
        g.update(1.0);
        assert!((g.rocker.tilt() - 0.2).abs() < 1e-12);
    }
}
