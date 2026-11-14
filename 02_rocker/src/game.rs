//! Game rules: the same pieces, rotation, kicks, PRNG and 7-bag as ../SPEC.md,
//! played in a well glued on top of the rocker.
//!
//! Two lattices share one rule set:
//! - Easy: a square grid, EASY_W×EASY_H (12×22), on a flat top.
//! - Advanced: a 45° diamond grid on a hacksaw top. Every block is a unit
//!   square turned 45°. The top has 4 teeth whose flanks are 45° and two block
//!   edges long, so one block (or an O piece) nests exactly in a V-notch.
//!
//! Cells are addressed with lattice coordinates `Pt { x: a, y: b }`:
//! - Easy: (a, b) = (column, row), row 0 at the top.
//! - Advanced: centre = a·u + b·v with u = (1,1)/√2 (up-right) and
//!   v = (−1,1)/√2 (up-left). The level k = a + b counts half-rows (√2/2 high)
//!   from the bottom; m = a − b is the horizontal position in steps of √2/2.

use std::f64::consts::{FRAC_PI_4, SQRT_2};

use crate::physics::{self, Mass, Rocker, V2, v2};

/// Board value for a cell that belongs to the base (the tips of the teeth).
pub const BASE: u8 = 8;
const KICKS: [i32; 5] = [0, -1, 1, -2, 2];
/// Speed (cells/s) a piece has when it lands after a hard drop.
pub const HARD_IMPACT: f64 = 6.0;
/// AI: seconds between actions in watch pace.
const AI_STEP: f64 = 0.10;
/// A resting piece locks after this long (seconds) unless it is moved.
pub const LOCK_DELAY: f64 = 0.5;
/// How many moves may restart the lock delay for one piece.
const MAX_LOCK_RESETS: i32 = 15;
/// How long the stack slides off before GAME OVER.
const SLIDE_TIME: f64 = 3.0;
/// Hard mode: the game ends when this many pieces have fallen off the side.
pub const MAX_LOST: i64 = 3;
/// Debris blocks collide as circles of this radius.
const DEBRIS_R: f64 = 0.47;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pt {
    pub x: i32,
    pub y: i32,
}
pub const fn p(x: i32, y: i32) -> Pt {
    Pt { x, y }
}
impl Pt {
    fn add(self, o: Pt) -> Pt {
        p(self.x + o.x, self.y + o.y)
    }
    fn times(self, k: i32) -> Pt {
        p(self.x * k, self.y * k)
    }
}

/// Diamond lattice point from (level k, horizontal position m).
fn km(k: i32, m: i32) -> Pt {
    p((k + m) / 2, (k - m) / 2)
}

pub struct PieceDef {
    pub n: i32,
    pub cells: [Pt; 4],
    pub color: u8,
}

pub const PIECES: [PieceDef; 7] = [
    PieceDef { n: 4, cells: [p(0, 1), p(1, 1), p(2, 1), p(3, 1)], color: 175 }, // I
    PieceDef { n: 2, cells: [p(0, 0), p(1, 0), p(0, 1), p(1, 1)], color: 75 }, // O
    PieceDef { n: 3, cells: [p(1, 0), p(0, 1), p(1, 1), p(2, 1)], color: 179 }, // T
    PieceDef { n: 3, cells: [p(1, 0), p(2, 0), p(0, 1), p(1, 1)], color: 141 }, // S
    PieceDef { n: 3, cells: [p(0, 0), p(1, 0), p(1, 1), p(2, 1)], color: 73 },  // Z
    PieceDef { n: 3, cells: [p(0, 0), p(0, 1), p(1, 1), p(2, 1)], color: 107 }, // J
    PieceDef { n: 3, cells: [p(2, 0), p(0, 1), p(1, 1), p(2, 1)], color: 167 }, // L
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Flat top, square blocks.
    Easy,
    /// Hacksaw top, diamond blocks.
    Advanced,
}

/// Where cells live and how they move, for one mode.
#[derive(Clone, Copy, Debug)]
pub struct Geo {
    pub mode: Mode,
    /// Levels (rows), counted from the bottom.
    pub levels: i32,
    /// Rocker radius.
    pub radius: f64,
    /// Body y of the chord (flat top of the wooden segment).
    pub chord_y: f64,
    /// Half width of the top of the base.
    pub half_w: f64,
    /// Half width of the well (walls). Hard mode: 4√2, standing on the outer
    /// peaks, so blocks reach the ends of the teeth. The 8-diamond half-rows
    /// touch both walls; the 7-diamond ones leave a half-diamond at each wall.
    pub well_half_w: f64,
    /// Body y of the top of the well.
    pub well_top: f64,
    /// Height of the teeth (0 in easy mode).
    pub teeth_h: f64,
}

/// Largest |m| over the base: 8 diamonds on odd levels (m = −7..7), 7 on even
/// levels (m = −6..6). Line clears count only these cells.
const BASE_M: i32 = 7;
/// Hard mode has no walls: pieces can move out to |m| = M_WIDE, past the edge
/// of the base, where they overhang or fall off.
const M_WIDE: i32 = 15;
/// Easy mode's well: columns × rows of square cells.
pub const EASY_W: i32 = 12;
pub const EASY_H: i32 = 22;

impl Mode {
    pub fn geo(self) -> Geo {
        match self {
            Mode::Easy => Geo {
                mode: self,
                levels: EASY_H,
                // the arc spans ±45°, so its chord (the flat top) is r·√2 wide
                radius: EASY_W as f64 / 2.0 * SQRT_2,
                chord_y: -(EASY_W as f64) / 2.0,
                half_w: EASY_W as f64 / 2.0,
                well_half_w: EASY_W as f64 / 2.0,
                well_top: (EASY_H - EASY_W / 2) as f64,
                teeth_h: 0.0,
            },
            Mode::Advanced => {
                let r = 8.0;
                let c = -r / SQRT_2;
                Geo {
                    mode: self,
                    levels: 27,
                    radius: r,
                    chord_y: c,
                    half_w: 4.0 * SQRT_2,
                    well_half_w: 4.0 * SQRT_2,
                    well_top: c + 20.0,
                    teeth_h: SQRT_2,
                }
            }
        }
    }
}

impl Geo {
    pub fn diamond(&self) -> bool {
        self.mode == Mode::Advanced
    }
    /// Number of storage slots in a row (the board is levels × slots).
    pub fn slots(&self) -> usize {
        if self.diamond() { (M_WIDE + 1) as usize } else { EASY_W as usize }
    }
    /// Storage index (level, slot) of a lattice point, or None outside the well.
    pub fn index(&self, c: Pt) -> Option<(usize, usize)> {
        if self.diamond() {
            let (k, m) = (c.x + c.y, c.x - c.y);
            if k < 0 || k >= self.levels || m.abs() > M_WIDE {
                return None;
            }
            Some((k as usize, (m + M_WIDE).div_euclid(2) as usize))
        } else {
            if c.x < 0 || c.x >= EASY_W || c.y < 0 || c.y >= self.levels {
                return None;
            }
            Some(((self.levels - 1 - c.y) as usize, c.x as usize))
        }
    }
    /// Lattice point of a storage index, or None for the unused last slot of even diamond levels.
    pub fn point(&self, level: usize, slot: usize) -> Option<Pt> {
        let k = level as i32;
        if self.diamond() {
            // m has the parity of k (odd levels m = −15..15, even levels m = −14..14)
            let m = 2 * slot as i32 - M_WIDE + (k - M_WIDE).rem_euclid(2);
            if m.abs() > M_WIDE {
                return None;
            }
            Some(p((k + m) / 2, (k - m) / 2))
        } else {
            Some(p(slot as i32, self.levels - 1 - k))
        }
    }
    /// Centre of a cell in the rocker's body frame.
    pub fn center(&self, c: Pt) -> V2 {
        if self.diamond() {
            let (k, m) = ((c.x + c.y) as f64, (c.x - c.y) as f64);
            v2(m / SQRT_2, self.chord_y + SQRT_2 / 2.0 + k / SQRT_2)
        } else {
            v2(c.x as f64 - (EASY_W as f64 - 1.0) / 2.0, self.chord_y + (self.levels - 1 - c.y) as f64 + 0.5)
        }
    }
    /// Whether a cell lies over the base (hard mode: |m| ≤ 7). Only these
    /// count for line clears and for the AI's heights.
    pub fn over_base(&self, c: Pt) -> bool {
        !self.diamond() || (c.x - c.y).abs() <= BASE_M
    }
    /// Easy mode has walls; hard mode has none.
    pub fn walls(&self) -> bool {
        !self.diamond()
    }
    /// How the blocks are turned relative to the base.
    pub fn cell_angle(&self) -> f64 {
        if self.diamond() { FRAC_PI_4 } else { 0.0 }
    }
    /// Maps a SPEC.md piece cell (x right, y down) into the lattice.
    /// Diamond: the piece is turned 45° counter-clockwise (x → up-right).
    fn piece_cell(&self, c: Pt) -> Pt {
        if self.diamond() { p(c.x, -c.y) } else { c }
    }
    pub fn left(&self) -> Pt {
        if self.diamond() { p(-1, 1) } else { p(-1, 0) }
    }
    pub fn right(&self) -> Pt {
        if self.diamond() { p(1, -1) } else { p(1, 0) }
    }
    pub fn down(&self) -> Pt {
        if self.diamond() { p(-1, -1) } else { p(0, 1) }
    }
    fn spawn(&self, n: i32) -> Pt {
        if self.diamond() {
            // centre the piece's box (k = 0, m = n−1 in box coordinates) at m = 0,
            // with its highest possible level at the top of the well
            let (k, m) = (self.levels - 1 - (n - 1), -(n - 1));
            p((k + m) / 2, (k - m) / 2)
        } else {
            p((EASY_W - n) / 2, 0)
        }
    }
    /// Lowest levels never clear: the level that holds the teeth tips.
    fn base_levels(&self) -> i32 {
        if self.diamond() { 1 } else { 0 }
    }
    /// Levels removed together by one line clear.
    fn band(&self) -> i32 {
        if self.diamond() { 2 } else { 1 }
    }
    /// Height of the top surface of the base at body x (chord + teeth).
    pub fn surface(&self, x: f64) -> f64 {
        if !self.diamond() {
            return self.chord_y;
        }
        let period = 2.0 * SQRT_2;
        let t = (x + self.half_w).rem_euclid(period);
        self.chord_y + (t - SQRT_2).abs()
    }
}

#[derive(Clone)]
pub struct Board {
    pub geo: Geo,
    v: Vec<u8>, // levels × slots, 0 empty, 1..=7 piece kind + 1, BASE
}

impl Board {
    fn new(geo: Geo) -> Board {
        let mut b = Board { geo, v: vec![0; geo.levels as usize * geo.slots()] };
        if geo.diamond() {
            // the tips of the three inner teeth poke into level 0; at m = ±8 the
            // end peaks fill the inner half of a level-0 cell (the outer half is air)
            for m in [-8, -4, 0, 4, 8] {
                b.set(km(0, m), BASE);
            }
        }
        b
    }

    /// Whether obstacle cell `o` is in the way of a block in column `mover_m`.
    /// The end half-tips (level 0, m = ±8) are solid only on their inner half,
    /// so a block dropping past them on the outside (|m| > 8) is not stopped.
    fn blocks(&self, mover_m: i32, o: Pt, vertical: bool) -> bool {
        let (k, m) = (o.x + o.y, o.x - o.y);
        // open sky above the well, and open space past the side limit
        if self.free(o) || (self.geo.diamond() && (k >= self.geo.levels || m.abs() > M_WIDE)) {
            return false;
        }
        let end_tip = self.geo.diamond() && k == 0 && m.abs() == BASE_M + 1;
        if end_tip && vertical && mover_m.signum() == m.signum() && mover_m.abs() > BASE_M + 1 {
            return false;
        }
        true
    }
    pub fn get(&self, c: Pt) -> Option<u8> {
        self.geo.index(c).map(|(l, s)| self.v[l * self.geo.slots() + s])
    }
    fn set(&mut self, c: Pt, val: u8) {
        let (l, s) = self.geo.index(c).expect("cell inside the well");
        self.v[l * self.geo.slots() + s] = val;
    }
    /// Whether a piece cell may be at `c`. In hard mode the space beside the
    /// base, below its top (k < 0, |m| > 7), is open air a piece falls through.
    fn free(&self, c: Pt) -> bool {
        if self.geo.diamond() {
            let (k, m) = (c.x + c.y, c.x - c.y);
            if k < 0 {
                return m.abs() > BASE_M && m.abs() <= M_WIDE && k >= -4;
            }
        }
        self.get(c) == Some(0)
    }
    /// Every cell of the well with its lattice point.
    pub fn cells(&self) -> impl Iterator<Item = (Pt, u8)> + '_ {
        let g = self.geo;
        (0..g.levels as usize).flat_map(move |l| {
            (0..g.slots()).filter_map(move |s| g.point(l, s).map(|c| (c, self.v[l * g.slots() + s])))
        })
    }
    fn level(&self, l: i32) -> impl Iterator<Item = u8> + '_ {
        let g = self.geo;
        let l = l as usize;
        (0..g.slots())
            .filter(move |&s| g.point(l, s).is_some_and(|c| g.over_base(c)))
            .map(move |s| self.v[l * g.slots() + s])
    }
    fn level_full(&self, l: i32) -> bool {
        self.level(l).all(|v| v != 0) && !self.level(l).any(|v| v == BASE)
    }
    /// Removes full bands and returns how many. Levels above drop by the band
    /// height, which keeps the diamond parity (band = 2 levels).
    fn clear_lines(&mut self) -> i64 {
        let g = self.geo;
        let (band, n) = (g.band(), g.slots());
        let mut cleared = 0;
        let mut l = g.base_levels();
        while l + band <= g.levels {
            if (l..l + band).all(|ll| self.level_full(ll)) {
                cleared += 1;
                let from = (l + band) as usize * n;
                self.v.copy_within(from.., l as usize * n);
                let len = self.v.len();
                self.v[len - band as usize * n..].fill(0);
            } else {
                l += 1;
            }
        }
        cleared
    }
    fn fits(&self, cells: &[Pt; 4], origin: Pt) -> bool {
        cells.iter().all(|&c| self.free(origin.add(c)))
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Playing,
    /// The stack broke loose; seconds since.
    Sliding(f64),
    Over,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cause {
    SlidOff(f64),
    /// Hard mode: the rocker rolled onto the end of its arc and tipped over.
    Toppled(f64),
    ToppedOut,
    /// Hard mode: MAX_LOST pieces fell off the side.
    LostPieces,
}

#[derive(Clone, Copy)]
pub struct Piece {
    pub kind: usize,
    /// SPEC.md cells (x right, y down) in the current rotation.
    pub spec: [Pt; 4],
    pub origin: Pt,
}

/// A block flying off after the stack slid.
pub struct Debris {
    pub pos: V2,
    pub vel: V2,
    pub angle: f64,
    pub spin: f64,
    pub kind: u8,
}

#[derive(Clone, Copy)]
struct AiPlan {
    rots: i32,
    h: i32,
    ok: bool,
}

pub struct Game {
    pub board: Board,
    pub mode: Mode,
    pub cur: Piece,
    pub next: usize,
    bag: Vec<usize>,
    rng: u32,
    pub seed: u32,
    pub score: i64,
    pub lines: i64,
    pub paused: bool,
    pub state: State,
    pub cause: Cause,
    pub rocker: Rocker,
    pub debris: Vec<Debris>,
    /// World x range debris must stay in (the visible play area).
    pub arena: (f64, f64),
    fall_timer: f64,
    /// Seconds the current piece has been resting (None: still falling).
    lock_timer: Option<f64>,
    lock_resets: i32,
    base_mass: Mass,

    pub ai: bool,
    plan: AiPlan,
    rots_done: i32,
    replanned: bool,
    ai_timer: f64,
    over_timer: f64,

    // running totals
    pub pieces: i64,
    pub games: i64,
    pub total_lines: i64,
    /// Pieces that fell off the side (this game / all games).
    pub lost: i64,
    pub total_lost: i64,
    pub max_tilt: f64,
}

fn rotate(cells: &[Pt; 4], n: i32, cw: bool) -> [Pt; 4] {
    cells.map(|c| if cw { p(n - 1 - c.y, c.x) } else { p(c.y, n - 1 - c.x) })
}

/// Wooden segment plus, in advanced mode, the teeth (density 1, like blocks).
fn base_mass(geo: &Geo) -> Mass {
    let mut m = physics::segment_mass(geo.radius);
    if geo.diamond() {
        let g = *geo;
        m = m.plus(physics::profile_mass(-g.half_w, g.half_w, g.chord_y, g.chord_y + g.teeth_h, move |x| g.surface(x)));
    }
    m
}

/// Tilt at which the stack comes off: it slides at 30° on the flat top; on the
/// hacksaw (sticky blocks) it holds until the rocker topples at 45°.
pub fn break_angle(g: &Geo) -> f64 {
    if g.diamond() { physics::TOPPLE_ANGLE } else { physics::SLIDE_ANGLE }
}

/// Mass of the base plus every block on the board.
fn board_mass(base: Mass, b: &Board) -> Mass {
    b.cells()
        .filter(|&(_, v)| v != 0 && v != BASE)
        .fold(base, |m, (c, _)| m.plus(Mass::cell(b.geo.center(c))))
}

/// SPEC.md's board score (per lattice column) plus a balance term. `peak` is
/// the largest tilt the placement would cause (simulated), `mass` the load after.
fn evaluate(b: &Board, cleared: i64, mass: &Mass, peak: f64) -> i64 {
    let g = b.geo;
    // columns: x in easy mode, m in diamond mode
    let ncol = if g.diamond() { (2 * M_WIDE + 1) as usize } else { EASY_W as usize };
    let col = |c: Pt| if g.diamond() { (c.x - c.y + M_WIDE) as usize } else { c.x as usize };
    let level = |c: Pt| if g.diamond() { c.x + c.y } else { g.levels - 1 - c.y };
    let mut top = vec![-1i32; ncol];
    for (c, v) in b.cells() {
        if v != 0 {
            top[col(c)] = top[col(c)].max(level(c));
        }
    }
    let mut holes = 0i64;
    for (c, v) in b.cells() {
        if v != 0 {
            continue;
        }
        let sealed = if g.diamond() {
            // a diamond falling into a cell sweeps past both cells diagonally
            // above it (up-left (0,+1), up-right (+1,0)) and sticks to the first
            // face it touches: once either is filled, the cell can't be reached
            [p(c.x, c.y + 1), p(c.x + 1, c.y)].iter().any(|&u| b.get(u).is_some_and(|x| x != 0))
        } else {
            level(c) < top[col(c)]
        };
        if sealed {
            holes += 1;
        }
    }
    // diamond levels are half as tall: count heights in whole rows
    let div = if g.diamond() { 2 } else { 1 };
    // heights only over the base (hard mode: columns m = −7..7)
    let over = if g.diamond() { &top[(M_WIDE - BASE_M) as usize..=(M_WIDE + BASE_M) as usize] } else { &top[..] };
    let hts: Vec<i64> = over.iter().map(|&t| ((t + 1) / div) as i64).collect();
    let agg: i64 = hts.iter().sum();
    let bump: i64 = hts.windows(2).map(|w| (w[0] - w[1]).abs()).sum();
    // hard mode: cells hanging past the edge of the base add torque and can't clear
    let overhang = b.cells().filter(|&(c, v)| v != 0 && !g.over_base(c)).count() as i64;
    let classic = -510066 * agg + 760666 * cleared - 356630 * holes - 184483 * bump - 600_000 * overhang;
    // Balance: only care once the swing gets large; never risk the slide angle
    // or a top-heavy stack. A strong balance term makes it stack badly instead.
    let peak_deg = peak.to_degrees();
    let soft = (peak_deg - 10.0).max(0.0);
    let hard = (peak_deg - (break_angle(&g).to_degrees() - 5.0)).max(0.0);
    let mut balance = -(soft * soft * 200_000.0 + hard * hard * 5_000_000.0) as i64;
    if Rocker::equilibrium(mass).is_none() {
        balance -= 1_000_000_000_000;
    }
    classic + balance
}

impl Game {
    pub fn new(seed: u32, mode: Mode, ai: bool) -> Self {
        let seed = if seed == 0 { 0x9E37_79B9 } else { seed };
        let geo = mode.geo();
        let base = base_mass(&geo);
        let mut g = Game {
            board: Board::new(geo),
            mode,
            cur: Piece { kind: 0, spec: PIECES[0].cells, origin: p(0, 0) },
            next: 0,
            bag: Vec::new(),
            rng: seed,
            seed,
            score: 0,
            lines: 0,
            paused: false,
            state: State::Playing,
            cause: Cause::ToppedOut,
            rocker: Rocker::new(base, geo.radius),
            debris: Vec::new(),
            arena: (-20.0, 20.0),
            fall_timer: 0.0,
            lock_timer: None,
            lock_resets: 0,
            base_mass: base,
            ai,
            plan: AiPlan { rots: 0, h: 0, ok: false },
            rots_done: 0,
            replanned: false,
            ai_timer: 0.0,
            over_timer: 0.0,
            pieces: 0,
            games: 0,
            total_lines: 0,
            lost: 0,
            total_lost: 0,
            max_tilt: 0.0,
        };
        g.reset();
        g
    }

    pub fn geo(&self) -> Geo {
        self.board.geo
    }

    pub fn level(&self) -> i64 {
        self.lines / 10 + 1
    }

    pub fn interval(&self) -> f64 {
        (800 - (self.level() - 1) * 70).max(80) as f64 / 1000.0
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
            for i in (1..=6usize).rev() {
                let j = (self.next_rand() % (i as u32 + 1)) as usize;
                a.swap(i, j);
            }
            self.bag = a;
        }
        self.bag.remove(0)
    }

    /// New game. Keeps the PRNG state (as in SPEC.md), so restarts deal on.
    pub fn reset(&mut self) {
        let geo = self.mode.geo();
        self.board = Board::new(geo);
        self.base_mass = base_mass(&geo);
        self.rocker = Rocker::new(self.base_mass, geo.radius);
        self.debris.clear();
        self.bag.clear();
        self.score = 0;
        self.lines = 0;
        self.lost = 0;
        self.paused = false;
        self.state = State::Playing;
        self.max_tilt = 0.0;
        self.next = self.draw7();
        self.spawn();
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.reset();
    }

    fn game_over(&mut self, cause: Cause) {
        self.cause = cause;
        self.games += 1;
        self.over_timer = 0.0;
        match cause {
            Cause::ToppedOut | Cause::LostPieces => self.state = State::Over,
            Cause::SlidOff(_) | Cause::Toppled(_) => self.state = State::Sliding(0.0),
        }
    }

    /// Lattice cells of a piece placed at `origin`.
    pub fn cells_of(&self, spec: &[Pt; 4], origin: Pt) -> [Pt; 4] {
        let g = self.geo();
        spec.map(|c| origin.add(g.piece_cell(c)))
    }

    fn piece_cells(&self, spec: &[Pt; 4]) -> [Pt; 4] {
        let g = self.geo();
        spec.map(|c| g.piece_cell(c))
    }

    fn fits(&self, spec: &[Pt; 4], origin: Pt) -> bool {
        self.board.fits(&self.piece_cells(spec), origin)
    }

    fn spawn(&mut self) {
        let d = &PIECES[self.next];
        self.cur = Piece { kind: self.next, spec: d.cells, origin: self.geo().spawn(d.n) };
        self.next = self.draw7();
        self.fall_timer = 0.0;
        self.lock_timer = None;
        self.lock_resets = 0;
        if !self.fits(&self.cur.spec, self.cur.origin) {
            self.game_over(Cause::ToppedOut);
            return;
        }
        if self.ai {
            self.plan = self.find_plan();
            self.rots_done = 0;
            self.replanned = false;
            self.ai_timer = 0.0;
        }
    }

    /// Whether a piece can move one step sideways (`d` = left or right). A
    /// diamond moving sideways also sweeps through the two diamonds touching
    /// its faces on that side, so those must be free too.
    fn can_shift(&self, spec: &[Pt; 4], origin: Pt, d: Pt) -> bool {
        if !self.fits(spec, origin.add(d)) {
            return false;
        }
        if !self.geo().diamond() {
            return true;
        }
        let s = (d.x - d.y).signum(); // +1 right, -1 left
        let cells = self.cells_of(spec, origin);
        cells.iter().all(|&q| {
            let (k, m) = (q.x + q.y, q.x - q.y);
            [km(k + 1, m + s), km(k - 1, m + s)].iter().all(|&o| cells.contains(&o) || !self.board.blocks(m, o, false))
        })
    }

    fn try_shift(&mut self, d: Pt) -> bool {
        let o = self.cur.origin.add(d);
        if self.can_shift(&self.cur.spec, self.cur.origin, d) {
            self.cur.origin = o;
            true
        } else {
            false
        }
    }

    pub fn move_left(&mut self) -> bool {
        self.push(-1)
    }
    pub fn move_right(&mut self) -> bool {
        self.push(1)
    }

    /// Whether the current piece is resting on something.
    pub fn resting(&self) -> bool {
        self.fall_step(&self.cur.spec, self.cur.origin).is_none()
    }

    /// ← / → for one step (`s` = −1 left, +1 right). A resting diamond is first
    /// pushed one step diagonally down the slope that way, if it fits; otherwise
    /// (and always for falling pieces and squares) it moves straight sideways.
    fn push(&mut self, s: i32) -> bool {
        let g = self.geo();
        if g.diamond() && self.resting() {
            let diag = self.cur.origin.add(if s > 0 { p(0, -1) } else { p(-1, 0) });
            if self.fits(&self.cur.spec, diag) {
                self.cur.origin = diag;
                if self.falls_off(&self.cur.spec, diag) {
                    self.lose_piece();
                } else {
                    self.moved();
                }
                return true;
            }
        }
        let ok = self.try_shift(if s > 0 { g.right() } else { g.left() });
        if ok {
            self.moved();
        }
        ok
    }

    /// After a successful move or rotation: a resting piece gets its lock delay
    /// restarted (a limited number of times); a piece that can fall again falls.
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

    /// Rotation with SPEC.md's kicks (0, −1, +1, −2, +2 steps sideways).
    fn kick_rotate(&self, spec: &[Pt; 4], kind: usize, origin: Pt, cw: bool) -> Option<([Pt; 4], Pt)> {
        let r = rotate(spec, PIECES[kind].n, cw);
        let right = self.geo().right();
        KICKS.iter().map(|&k| origin.add(right.times(k))).find(|&o| self.fits(&r, o)).map(|o| (r, o))
    }

    pub fn try_rotate(&mut self, cw: bool) {
        if let Some((spec, o)) = self.kick_rotate(&self.cur.spec, self.cur.kind, self.cur.origin, cw) {
            self.cur.spec = spec;
            self.cur.origin = o;
            self.moved();
        }
    }

    /// One step of falling. None = the piece rests and must lock.
    ///
    /// Diamonds (hard mode) fall straight down, which sweeps each block through
    /// the cell below it and the two cells below its lower faces. Then:
    /// - a lower face flush against something (block, tooth, base) holds: friction;
    /// - resting only on a point (a tooth tip, another block's corner), the piece
    ///   tips off that point toward the side its centre of mass is on (the side
    ///   the base leans when it is balanced exactly), one half-step diagonally.
    fn fall_step(&self, spec: &[Pt; 4], origin: Pt) -> Option<Pt> {
        let g = self.geo();
        let down = origin.add(g.down());
        if !g.diamond() {
            return self.fits(spec, down).then_some(down);
        }
        let cells = self.cells_of(spec, origin);
        let b = &self.board;
        let km_of = |q: Pt| (q.x + q.y, q.x - q.y);
        let face = cells.iter().any(|&q| {
            let (k, m) = km_of(q);
            [km(k - 1, m - 1), km(k - 1, m + 1)].iter().any(|&o| !cells.contains(&o) && b.blocks(m, o, true))
        });
        if face {
            return None;
        }
        let points: Vec<Pt> = cells
            .iter()
            .copied()
            .filter(|&q| {
                let (k, m) = km_of(q);
                let o = km(k - 2, m);
                !cells.contains(&o) && b.blocks(m, o, true)
            })
            .collect();
        if points.is_empty() {
            return self.fits(spec, down).then_some(down);
        }
        // tip off the point(s): compare the centre of mass with the contact, in the world
        let r = &self.rocker;
        let world_x = |v: V2| v.rot(r.theta).x;
        let com = cells.iter().map(|&q| world_x(g.center(q))).sum::<f64>() / 4.0;
        let contact = points.iter().map(|&q| world_x(g.center(q).add(v2(0.0, -SQRT_2 / 2.0)))).sum::<f64>()
            / points.len() as f64;
        let right_first = if (com - contact).abs() > 1e-9 { com > contact } else { r.theta < 0.0 };
        let (dl, dr) = (origin.add(p(-1, 0)), origin.add(p(0, -1)));
        let order = if right_first { [dr, dl] } else { [dl, dr] };
        order.into_iter().find(|&o| self.fits(spec, o))
    }

    /// Whether a piece at `origin` has dropped below the top of the base beside
    /// it, i.e. it is falling off the side (hard mode).
    fn falls_off(&self, spec: &[Pt; 4], origin: Pt) -> bool {
        self.geo().diamond() && self.cells_of(spec, origin).iter().any(|c| c.x + c.y < 0)
    }

    /// Where a piece would come to rest, or None if it falls off the side.
    fn drop_path(&self, spec: &[Pt; 4], mut o: Pt) -> Option<Pt> {
        while let Some(n) = self.fall_step(spec, o) {
            o = n;
            if self.falls_off(spec, o) {
                return None;
            }
        }
        Some(o)
    }

    /// Where the current piece would come to rest (None: it would fall off).
    pub fn landing(&self) -> Option<Pt> {
        self.drop_path(&self.cur.spec, self.cur.origin)
    }

    /// The current piece went over the edge: it tumbles to the floor and is lost.
    fn lose_piece(&mut self) {
        let g = self.geo();
        let fall = v2(0.0, -4.0).rot(self.rocker.theta);
        for c in self.cells_of(&self.cur.spec, self.cur.origin) {
            let b = g.center(c);
            self.debris.push(Debris {
                pos: self.rocker.to_world(b),
                vel: self.rocker.velocity_of(b).add(fall),
                angle: self.rocker.theta + g.cell_angle(),
                spin: 0.0,
                kind: self.cur.kind as u8 + 1,
            });
        }
        self.lost += 1;
        self.total_lost += 1;
        self.pieces += 1;
        if self.lost >= MAX_LOST {
            self.game_over(Cause::LostPieces);
            return;
        }
        self.spawn();
    }

    /// Moves the current piece one fall step; true if it is still in play.
    fn step_down(&mut self) -> bool {
        match self.fall_step(&self.cur.spec, self.cur.origin) {
            Some(o) => {
                self.cur.origin = o;
                if self.falls_off(&self.cur.spec, o) {
                    self.lose_piece();
                    return false;
                }
                true
            }
            None => false,
        }
    }

    /// Locks the current piece. `speed` is how fast it was falling (cells/s),
    /// which sets how hard it hits the base.
    fn lock(&mut self, speed: f64) {
        let g = self.geo();
        let mut add = Mass::default();
        for c in self.cells_of(&self.cur.spec, self.cur.origin) {
            self.board.set(c, self.cur.kind as u8 + 1);
            add = add.plus(Mass::cell(g.center(c)));
        }
        self.rocker.land(add, v2(0.0, -speed));
        let cleared = self.board.clear_lines();
        if cleared > 0 {
            self.rocker.set_mass(board_mass(self.base_mass, &self.board));
        }
        const PTS: [i64; 5] = [0, 100, 300, 500, 800];
        self.score += PTS[(cleared as usize).min(4)] * self.level();
        self.lines += cleared;
        self.total_lines += cleared;
        self.pieces += 1;
        self.spawn();
    }

    /// A gravity tick: the piece falls a step, or starts its lock delay.
    pub fn gravity(&mut self) {
        let before = self.pieces;
        if self.step_down() {
            self.lock_timer = None;
        } else if self.pieces == before && self.lock_timer.is_none() {
            self.lock_timer = Some(0.0);
        }
        self.fall_timer = 0.0;
    }

    fn lock_resting(&mut self) {
        let speed = 1.0 / self.interval();
        self.lock(speed);
    }

    /// ↓: one step down; on a resting piece it locks it at once.
    pub fn soft_drop(&mut self) {
        let before = self.pieces;
        if self.step_down() || self.pieces != before {
            self.score += 1;
            self.fall_timer = 0.0;
            self.lock_timer = None;
        } else {
            self.lock_resting();
        }
    }

    pub fn hard_drop(&mut self) {
        let before = self.pieces;
        while self.step_down() {
            self.score += 2;
        }
        if self.pieces == before {
            self.lock(HARD_IMPACT);
        }
    }

    /// Resting tilt the current stack would settle at (None = top-heavy).
    pub fn equilibrium(&self) -> Option<f64> {
        Rocker::equilibrium(&self.rocker.mass)
    }

    /// Advances physics, gravity, the AI and the slide-off animation.
    pub fn update(&mut self, dt: f64) {
        if self.paused {
            return;
        }
        self.rocker.step(dt);
        self.max_tilt = self.max_tilt.max(self.rocker.theta.abs());
        match self.state {
            State::Playing => {
                self.step_debris(dt);
                if self.must_break() {
                    self.break_loose();
                    return;
                }
                if self.ai {
                    self.ai_timer += dt;
                    while self.ai_timer >= AI_STEP && self.state == State::Playing {
                        self.ai_timer -= AI_STEP;
                        self.ai_action();
                    }
                } else {
                    self.fall_timer += dt;
                    if self.fall_timer >= self.interval() {
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
            State::Sliding(t) => {
                self.step_debris(dt);
                let t = t + dt;
                self.state = if t >= SLIDE_TIME { State::Over } else { State::Sliding(t) };
            }
            State::Over => {
                // the physics keeps going after game over: past the limit the
                // stack still comes off, whatever ended the game
                if self.has_blocks() && self.must_break() {
                    self.release_stack();
                }
                self.step_debris(dt);
                self.over_timer += dt;
                if self.ai && self.over_timer > 1.5 {
                    self.reset();
                }
            }
        }
    }

    /// Headless: runs only the physics for `secs`, checking for a slide every 10 ms.
    pub fn settle(&mut self, secs: f64) {
        let mut t = 0.0;
        while t < secs && self.state == State::Playing {
            self.rocker.step(0.01);
            self.max_tilt = self.max_tilt.max(self.rocker.theta.abs());
            if self.must_break() {
                self.break_loose();
            }
            t += 0.01;
        }
    }

    /// The tilt passed the friction angle: every block breaks loose. Each block
    /// starts with the velocity of its spot on the swinging base, plus a push
    /// down the slope; from then on the blocks fall, hit each other, the base
    /// and the floor, and pile up (see step_debris).
    fn break_loose(&mut self) {
        let theta = self.rocker.theta;
        self.release_stack();
        let cause = if self.geo().diamond() { Cause::Toppled(theta.abs()) } else { Cause::SlidOff(theta.abs()) };
        self.game_over(cause);
    }

    pub fn break_angle(&self) -> f64 {
        break_angle(&self.geo())
    }

    /// Whether the stack has to come off now: tilted past the limit, or a
    /// block hanging past the edge has swung down into the floor.
    fn must_break(&self) -> bool {
        if self.rocker.theta.abs() > self.break_angle() {
            return true;
        }
        let g = self.geo();
        // lowest corner of a block, turned with the base (half a diagonal below its centre at worst)
        self.board
            .cells()
            .filter(|&(_, v)| v != 0 && v != BASE)
            .any(|(c, _)| {
                let ctr = self.rocker.to_world(g.center(c));
                let a = g.cell_angle() + self.rocker.theta;
                let reach = 0.5 * (a.sin().abs() + a.cos().abs());
                ctr.y - reach < 0.0
            })
    }

    fn has_blocks(&self) -> bool {
        self.board.cells().any(|(_, v)| v != 0 && v != BASE)
    }

    /// Turns every block on the base into debris.
    fn release_stack(&mut self) {
        let theta = self.rocker.theta;
        let g = self.geo();
        let downhill = v2(1.0, 0.0).rot(theta).scale(if theta > 0.0 { -1.0 } else { 1.0 });
        let cells: Vec<(Pt, u8)> = self.board.cells().filter(|&(_, v)| v != 0 && v != BASE).collect();
        for (c, v) in cells {
            let b = g.center(c);
            let vel = self.rocker.velocity_of(b).add(downhill.scale(2.0));
            self.debris.push(Debris {
                pos: self.rocker.to_world(b),
                vel,
                angle: theta + g.cell_angle(),
                spin: 0.0,
                kind: v,
            });
            self.board.set(c, 0);
        }
        self.rocker.set_mass(self.base_mass);
        let (lo, hi) = (self.arena.0 + DEBRIS_R, self.arena.1 - DEBRIS_R);
        for d in &mut self.debris {
            d.pos.x = d.pos.x.clamp(lo, hi);
        }
    }

    /// Debris: discs of radius DEBRIS_R with gravity, the floor, the side walls
    /// of the play area, the (moving) base, and each other. Position-based
    /// contacts, solved in substeps.
    fn step_debris(&mut self, dt: f64) {
        if self.debris.is_empty() {
            return;
        }
        const SUB: usize = 8;
        let h = dt / SUB as f64;
        let r = DEBRIS_R;
        let g = self.geo();
        for _ in 0..SUB {
            for d in &mut self.debris {
                d.vel.y -= physics::G * h;
                d.pos = d.pos.add(d.vel.scale(h));
                d.angle += d.spin * h;
            }
            // pairs
            let n = self.debris.len();
            for i in 0..n {
                for j in i + 1..n {
                    let (a, b) = (self.debris[i].pos, self.debris[j].pos);
                    let d = b.sub(a);
                    let dist2 = d.len2();
                    if dist2 >= 4.0 * r * r || dist2 < 1e-12 {
                        continue;
                    }
                    let dist = dist2.sqrt();
                    let nrm = d.scale(1.0 / dist);
                    let push = nrm.scale((2.0 * r - dist) / 2.0);
                    self.debris[i].pos = self.debris[i].pos.sub(push);
                    self.debris[j].pos = self.debris[j].pos.add(push);
                    let rel = self.debris[j].vel.sub(self.debris[i].vel).dot(nrm);
                    if rel < 0.0 {
                        let imp = nrm.scale(-rel * 0.6);
                        self.debris[i].vel = self.debris[i].vel.sub(imp);
                        self.debris[j].vel = self.debris[j].vel.add(imp);
                        let tang = self.debris[j].vel.sub(self.debris[i].vel);
                        let spin = (tang.x * nrm.y - tang.y * nrm.x) * 0.3;
                        self.debris[i].spin += spin;
                        self.debris[j].spin -= spin;
                    }
                }
            }
            for d in &mut self.debris {
                // base
                let b = self.rocker.to_body(d.pos);
                let inside_arc = b.len() < g.radius + r;
                let top = g.surface(b.x);
                if inside_arc && b.x.abs() < g.half_w + r && b.y < top + r && b.y > top - 2.0 {
                    let (nb, depth) = (v2(0.0, 1.0), top + r - b.y);
                    let (radial, rdepth) = (b.scale(1.0 / b.len().max(1e-9)), g.radius + r - b.len());
                    let (nb, depth) = if rdepth < depth && b.y < g.chord_y { (radial, rdepth) } else { (nb, depth) };
                    let nw = nb.rot(self.rocker.theta);
                    d.pos = d.pos.add(nw.scale(depth));
                    let vb = self.rocker.velocity_of(self.rocker.to_body(d.pos));
                    let rel = d.vel.sub(vb);
                    let vn = rel.dot(nw);
                    if vn < 0.0 {
                        let tang = rel.sub(nw.scale(vn));
                        d.vel = vb.add(tang.scale(0.97)).add(nw.scale(-vn * 0.1));
                    }
                }
                // floor
                if d.pos.y < r {
                    d.pos.y = r;
                    if d.vel.y < 0.0 {
                        d.vel.y = -d.vel.y * 0.15;
                    }
                    d.vel.x *= 0.9;
                    d.spin *= 0.85;
                    // come to rest lying flat
                    let flat = (d.angle / std::f64::consts::FRAC_PI_2).round() * std::f64::consts::FRAC_PI_2;
                    d.angle += (flat - d.angle) * 0.05;
                }
                // side walls of the play area
                let (lo, hi) = (self.arena.0 + r, self.arena.1 - r);
                if d.pos.x < lo {
                    d.pos.x = lo;
                    d.vel.x = d.vel.x.abs() * 0.3;
                }
                if d.pos.x > hi {
                    d.pos.x = hi;
                    d.vel.x = -d.vel.x.abs() * 0.3;
                }
                d.spin = d.spin.clamp(-8.0, 8.0);
            }
        }
    }

    // ---------- AI (SPEC.md planner + a balance term) ----------

    fn find_plan(&self) -> AiPlan {
        let mut best = AiPlan { rots: 0, h: 0, ok: false };
        let mut best_score = 0i64;
        let g = self.geo();
        let (mut spec, mut origin) = (self.cur.spec, self.cur.origin);
        for r in 0..4 {
            if r > 0 {
                match self.kick_rotate(&spec, self.cur.kind, origin, true) {
                    Some((s, o)) => {
                        spec = s;
                        origin = o;
                    }
                    None => break,
                }
            }
            for h in -12i32..=12 {
                // slide sideways one step at a time
                let step = if h > 0 { g.right() } else { g.left() };
                let mut o = origin;
                let mut ok = true;
                for _ in 0..h.abs() {
                    if !self.can_shift(&spec, o, step) {
                        ok = false;
                        break;
                    }
                    o = o.add(step);
                }
                if !ok {
                    continue;
                }
                let Some(o) = self.drop_path(&spec, o) else {
                    continue; // would fall off the side
                };
                let mut b = self.board.clone();
                let mut add = Mass::default();
                for c in self.cells_of(&spec, o) {
                    b.set(c, self.cur.kind as u8 + 1);
                    add = add.plus(Mass::cell(g.center(c)));
                }
                let cleared = b.clear_lines();
                let after = board_mass(self.base_mass, &b);
                let peak = self.rocker.peak_after(add, HARD_IMPACT, after, 3.0);
                let s = evaluate(&b, cleared, &after, peak);
                if !best.ok || s > best_score {
                    best = AiPlan { rots: r, h, ok: true };
                    best_score = s;
                }
            }
        }
        best
    }

    fn ai_action(&mut self) {
        if !self.plan.ok {
            self.hard_drop();
            return;
        }
        if self.rots_done < self.plan.rots {
            self.try_rotate(true);
            self.rots_done += 1;
        } else if self.plan.h != 0 {
            let moved = if self.plan.h > 0 { self.move_right() } else { self.move_left() };
            self.plan.h -= self.plan.h.signum();
            if !moved {
                self.hard_drop();
            }
        } else {
            // the base has rocked since the plan was made, and which way a piece
            // slides depends on the tilt: plan again if it would now fall off
            if self.landing().is_none() && !self.replanned {
                self.replanned = true;
                self.plan = self.find_plan();
                self.rots_done = 0;
                return;
            }
            self.hard_drop();
        }
    }

    /// Bench helper: executes the whole plan for the current piece at once.
    pub fn ai_place_now(&mut self) {
        let start = self.pieces;
        while self.pieces == start && self.state == State::Playing {
            self.ai_action();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(g: &mut Game, cells: &[Pt]) {
        for &c in cells {
            g.board.set(c, 1);
        }
        g.rocker.set_mass(board_mass(g.base_mass, &g.board));
    }


    #[test]
    fn bags_are_permutations() {
        let mut g = Game::new(42, Mode::Easy, false);
        let mut seq = vec![g.cur.kind, g.next];
        for _ in 0..12 {
            seq.push(g.draw7());
        }
        for bag in seq.chunks(7) {
            let mut b = bag.to_vec();
            b.sort();
            assert_eq!(b, (0..7).collect::<Vec<_>>());
        }
    }

    #[test]
    fn index_and_point_round_trip() {
        for mode in [Mode::Easy, Mode::Advanced] {
            let geo = mode.geo();
            let mut count = 0;
            for l in 0..geo.levels as usize {
                for s in 0..geo.slots() {
                    if let Some(c) = geo.point(l, s) {
                        assert_eq!(geo.index(c), Some((l, s)), "{mode:?} {c:?}");
                        count += 1;
                    }
                }
            }
            let want = if mode == Mode::Easy { (EASY_W * EASY_H) as usize } else { 14 * 15 + 13 * 16 };
            assert_eq!(count, want, "{mode:?}");
        }
    }

    #[test]
    fn diamonds_fit_the_well_and_the_notches() {
        let geo = Mode::Advanced.geo();
        let h = SQRT_2 / 2.0; // half diagonal of a diamond
        let b = Board::new(geo);
        for (c, v) in b.cells() {
            let ctr = geo.center(c);
            // only cells whose centre is over the base can touch the teeth
            if v == BASE || ctr.x.abs() >= geo.half_w - 1e-9 {
                continue;
            }
            // the bottom corner never dips into the teeth
            assert!(ctr.y - h >= geo.surface(ctr.x) - 1e-9, "{c:?} overlaps a tooth");
            // and the side corners neither
            for sx in [-h, h] {
                assert!(ctr.y >= geo.surface(ctr.x + sx) - 1e-9, "{c:?} side corner in a tooth");
            }
        }
        // level 0: diamonds sit exactly in the four V-notches
        let valleys: Vec<f64> = b
            .cells()
            .filter(|&(c, v)| c.x + c.y == 0 && v == 0 && geo.over_base(c))
            .map(|(c, _)| geo.center(c).x)
            .collect();
        let want = [-3.0 * SQRT_2, -SQRT_2, SQRT_2, 3.0 * SQRT_2];
        assert_eq!(valleys.len(), 4);
        // level 1: a diamond fits on each outer flank, its outer corner on the end peak
        for m in [-7, 7] {
            let c = p((1 + m) / 2, (1 - m) / 2);
            assert_eq!(b.get(c), Some(0), "outer flank m={m} reachable");
            let ctr = geo.center(c);
            assert!((ctr.x.abs() + h - geo.half_w).abs() < 1e-9);
            assert!((ctr.y - (geo.chord_y + geo.teeth_h)).abs() < 1e-9, "corner at peak height");
        }
        for (a, w) in valleys.iter().zip(want) {
            assert!((a - w).abs() < 1e-9);
        }
    }

    #[test]
    fn upright_with_empty_board() {
        for mode in [Mode::Easy, Mode::Advanced] {
            let g = Game::new(1, mode, false);
            assert!(g.equilibrium().unwrap().abs() < 1e-6, "{mode:?}");
        }
    }

    #[test]
    fn o_piece_drops_into_a_notch() {
        let mut g = Game::new(1, Mode::Advanced, false);
        g.cur = Piece { kind: 1, spec: PIECES[1].cells, origin: g.geo().spawn(2) };
        // spawned with its bottom cell at m = 0 (above the middle peak); one step
        // left puts it over the notch at m = −2
        assert!(g.move_left());
        let o = g.landing().expect("rests on the base");
        let cells = g.cells_of(&g.cur.spec, o);
        let mut ks: Vec<i32> = cells.iter().map(|c| c.x + c.y).collect();
        ks.sort();
        // a 2×2 diamond resting in a V: bottom cell at level 0, top at level 2
        assert_eq!(ks, vec![0, 1, 1, 2]);
        let bottom = cells.into_iter().min_by_key(|c| c.x + c.y).unwrap();
        assert_eq!(bottom.x - bottom.y, -2);
    }

    /// Drops an O piece straight down with its bottom block in column `m`.
    fn drop_o(g: &mut Game, m: i32) -> Option<Pt> {
        g.cur = Piece { kind: 1, spec: PIECES[1].cells, origin: p(0, 0) };
        // O cells: bottom (k, m), sides (k+1, m±1), top (k+2, m)
        let bottom = g.cells_of(&g.cur.spec, p(0, 0)).into_iter().min_by_key(|c| c.x + c.y).unwrap();
        let t = km(20, m);
        g.cur.origin = p(t.x - bottom.x, t.y - bottom.y);
        assert!(g.fits(&g.cur.spec, g.cur.origin));
        g.landing().map(|o| g.cells_of(&g.cur.spec, o).into_iter().min_by_key(|c| c.x + c.y).unwrap())
    }

    #[test]
    fn piece_balanced_on_a_tooth_tip_tips_the_way_the_base_leans() {
        for (theta, side) in [(-0.1, 1), (0.1, -1)] {
            let mut g = Game::new(1, Mode::Advanced, false);
            g.rocker.theta = theta;
            // the O's bottom corner lands exactly on the middle tip (m = 0)
            let b = drop_o(&mut g, 0).expect("stays on the base");
            // it tips half a step and then rests flat on that flank
            assert_eq!((b.x + b.y, b.x - b.y), (1, side), "theta {theta}");
        }
    }

    #[test]
    fn a_resting_piece_waits_before_locking() {
        let mut g = Game::new(1, Mode::Easy, false);
        g.cur.origin = g.landing().unwrap(); // resting on the floor of the well
        let first = g.pieces;
        g.gravity(); // touches down: lock delay starts
        assert_eq!(g.pieces, first, "not locked yet");
        g.update(LOCK_DELAY * 0.8);
        assert_eq!(g.pieces, first, "still waiting");
        // a move restarts the wait
        assert!(g.move_left());
        g.update(LOCK_DELAY * 0.8);
        assert_eq!(g.pieces, first, "move restarted the delay");
        g.update(LOCK_DELAY * 0.4);
        assert_eq!(g.pieces, first + 1, "locked after the delay");
    }

    #[test]
    fn down_on_a_resting_piece_locks_it_at_once() {
        let mut g = Game::new(1, Mode::Easy, false);
        g.cur.origin = g.landing().unwrap();
        let first = g.pieces;
        g.soft_drop();
        assert_eq!(g.pieces, first + 1);
    }

    #[test]
    fn push_a_resting_piece_down_the_slope_into_a_notch() {
        // an O resting on the middle of a flank (faces hold) is pushed → down the
        // slope into the notch at m = −2; pushing ← instead goes uphill: blocked
        // diagonally, so it moves straight sideways instead
        let mut g = Game::new(1, Mode::Advanced, false);
        drop_o(&mut g, -3).unwrap();
        g.cur.origin = g.landing().unwrap();
        assert!(g.resting());
        assert!(g.move_right());
        let b = g.cells_of(&g.cur.spec, g.cur.origin).into_iter().min_by_key(|c| c.x + c.y).unwrap();
        assert_eq!((b.x + b.y, b.x - b.y), (0, -2), "slid into the notch");
        assert!(g.resting());
    }

    #[test]
    fn push_into_a_slot_the_piece_could_not_fall_into() {
        // like the screenshot: an S whose left blocks catch on a higher shelf
        // before its right block reaches the slot; one push → drops it in
        let mut g = Game::new(1, Mode::Advanced, false);
        // a shelf on the left: blocks over the left notch and flank
        fill(&mut g, &[km(0, -6), km(1, -5), km(1, -7), km(2, -6), km(2, -4), km(3, -5)]);
        g.cur = Piece { kind: 3, spec: PIECES[3].cells, origin: g.geo().spawn(3) };
        for _ in 0..2 {
            g.move_left();
        }
        let rest = g.landing().unwrap();
        g.cur.origin = rest;
        let low = |g: &Game| g.cells_of(&g.cur.spec, g.cur.origin).iter().map(|c| c.x + c.y).min().unwrap();
        let before = low(&g);
        let moved = g.move_right();
        assert!(moved);
        // the push may let it fall further on the next ticks
        let after_push = g.landing().unwrap();
        g.cur.origin = after_push;
        assert!(low(&g) < before, "got lower: {before} -> {}", low(&g));
    }

    #[test]
    fn face_contact_holds_even_on_a_slope() {
        // a block falling onto the middle of a flank stops there: no sliding
        let mut g = Game::new(1, Mode::Advanced, false);
        let b = drop_o(&mut g, -3).expect("stays");
        assert_eq!((b.x + b.y, b.x - b.y), (1, -3));
    }

    #[test]
    fn rows_easy_and_bands_diamond_clear() {
        // easy: a full bottom row clears
        let mut g = Game::new(1, Mode::Easy, false);
        let row: Vec<Pt> = (0..EASY_W).map(|x| p(x, EASY_H - 1)).collect();
        fill(&mut g, &row);
        assert_eq!(g.board.clear_lines(), 1);
        // diamond: level 0 (teeth) never clears; levels 1 + 2 full clear as one band
        let mut g = Game::new(1, Mode::Advanced, false);
        let cells: Vec<Pt> = g.board.cells().filter(|&(c, v)| v == 0 && c.x + c.y <= 2).map(|(c, _)| c).collect();
        fill(&mut g, &cells);
        let marker = km(3, 1);
        g.board.set(marker, 5);
        assert_eq!(g.board.clear_lines(), 1);
        assert_eq!(g.board.get(km(1, 1)), Some(5), "level 3 dropped to level 1");
        assert!(g.board.cells().filter(|&(c, _)| c.x + c.y == 0).all(|(_, v)| v != 0), "level 0 kept");
    }

    #[test]
    fn piece_moved_past_the_edge_falls_off() {
        for right in [false, true] {
            let mut g = Game::new(1, Mode::Advanced, false);
            for _ in 0..12 {
                if right { g.move_right() } else { g.move_left() };
            }
            assert!(g.landing().is_none(), "ghost says it falls off");
            let before = g.board.cells().filter(|&(_, v)| v != 0).count();
            g.hard_drop();
            assert_eq!(g.lost, 1);
            assert_eq!(g.debris.len(), 4);
            assert_eq!(g.board.cells().filter(|&(_, v)| v != 0).count(), before, "nothing locked");
            assert_eq!(g.state, State::Playing, "losing one piece is not game over");
            for _ in 0..300 {
                g.update(0.01);
            }
            let cx = g.rocker.center().x;
            for d in &g.debris {
                assert!(d.pos.y < 1.0, "on the floor");
                assert_eq!(d.pos.x > cx, right, "fell on its own side");
            }
        }
    }

    #[test]
    fn third_lost_piece_ends_the_game() {
        let mut g = Game::new(1, Mode::Advanced, false);
        for n in 1..=MAX_LOST {
            for _ in 0..12 {
                g.move_right();
            }
            g.hard_drop();
            assert_eq!(g.lost, n);
            let over = n == MAX_LOST;
            assert_eq!(g.state == State::Over, over, "after {n} lost");
        }
        assert_eq!(g.cause, Cause::LostPieces);
        assert_eq!(g.games, 1);
        g.reset();
        assert_eq!((g.lost, g.state), (0, State::Playing), "restart clears the count");
    }

    #[test]
    fn lands_on_a_diagonal_and_overhangs_the_edge() {
        // your screenshot: a cyan I lying up the slope from the left valley; an O
        // dropped over its lower end comes to rest on it with half a block out
        let mut g = Game::new(1, Mode::Advanced, false);
        fill(&mut g, &[km(0, -6), km(1, -5), km(2, -4), km(3, -3)]);
        let b = drop_o(&mut g, -7).expect("rests on the cyan, not lost");
        assert_eq!((b.x + b.y, b.x - b.y), (1, -7));
        // its left block hangs past the end of the base
        let o = g.landing().unwrap();
        assert!(g.cells_of(&g.cur.spec, o).iter().any(|c| (c.x - c.y).abs() > 7));
    }

    #[test]
    fn balanced_on_the_end_peak_it_tips_off_outward_or_onto_the_flank() {
        // the O's bottom corner lands exactly on the left end peak (m = −8)
        let mut g = Game::new(1, Mode::Advanced, false);
        g.rocker.theta = 0.1; // leaning left: tips outward and falls off
        assert!(drop_o(&mut g, -8).is_none(), "falls off");
        let mut g = Game::new(1, Mode::Advanced, false);
        g.rocker.theta = -0.1; // leaning right: tips inward onto the outer flank
        let b = drop_o(&mut g, -8).expect("stays");
        assert_eq!((b.x + b.y, b.x - b.y), (1, -7));
    }

    #[test]
    fn falls_past_the_end_peak_on_the_outside() {
        // dropped just outside the end peak, it grazes it and falls off
        let mut g = Game::new(1, Mode::Advanced, false);
        assert!(drop_o(&mut g, -9).is_none());
    }

    #[test]
    fn lands_flush_next_to_the_edge() {
        // an O piece put over the outer right valley nests in it with no gap
        let mut g = Game::new(1, Mode::Advanced, false);
        g.cur = Piece { kind: 1, spec: PIECES[1].cells, origin: g.geo().spawn(2) };
        for _ in 0..3 {
            assert!(g.move_right());
        }
        let o = g.landing().expect("rests on the base");
        let cells = g.cells_of(&g.cur.spec, o);
        let bottom = cells.into_iter().min_by_key(|c| c.x + c.y).unwrap();
        assert_eq!((bottom.x + bottom.y, bottom.x - bottom.y), (0, 6), "in the outer valley");
    }

    fn slide_off(right: bool, mode: Mode) -> Game {
        let mut g = Game::new(3, mode, false);
        g.arena = (-30.0, 30.0);
        let mut cells = vec![];
        for (c, v) in g.board.cells() {
            let x = g.geo().center(c).x;
            let side = if right { x > 1.0 } else { x < -1.0 };
            let lvl = g.geo().center(c).y - g.geo().chord_y;
            if v == 0 && side && lvl < 10.0 {
                cells.push(c);
            }
        }
        fill(&mut g, &cells);
        for _ in 0..600 {
            g.update(0.01);
        }
        g
    }

    #[test]
    fn heavy_side_slides_off_and_ends_the_game() {
        for mode in [Mode::Easy, Mode::Advanced] {
            for right in [false, true] {
                let g = slide_off(right, mode);
                match (mode, g.cause) {
                    (Mode::Easy, Cause::SlidOff(a)) => assert!(a > physics::SLIDE_ANGLE),
                    (Mode::Advanced, Cause::Toppled(_)) => {}
                    other => panic!("unexpected {other:?}"),
                }
                assert_eq!(g.state, State::Over);
                assert!(g.board.cells().all(|(_, v)| v == 0 || v == BASE), "board emptied");
                // the heap ends up on the floor, on the side it slid to (in hard
                // mode a block can stay wedged in a V-notch and ride the base)
                let cx = g.rocker.center().x;
                let half = g.geo().half_w;
                let on_floor: Vec<&Debris> = g.debris.iter().filter(|d| (d.pos.x - cx).abs() > half).collect();
                assert!(on_floor.len() * 10 >= g.debris.len() * 8, "{mode:?}: most blocks leave the base");
                // a few can be flung back across by the empty base swinging back
                let wrong = on_floor.iter().filter(|d| (d.pos.x > cx) != right).count();
                assert!(wrong * 10 <= on_floor.len() * 2, "{mode:?} right={right}: {wrong}/{} on the wrong side", on_floor.len());
            }
        }
    }

    #[test]
    fn stack_still_comes_off_after_game_over() {
        // the game ended some other way, then the base rolls past the limit
        let mut g = Game::new(3, Mode::Easy, false);
        let cells: Vec<Pt> = g.board.cells().filter(|&(c, _)| c.x < 4 && c.y >= 10).map(|(c, _)| c).collect();
        fill(&mut g, &cells);
        g.state = State::Over;
        g.cause = Cause::ToppedOut;
        g.games = 1;
        for _ in 0..600 {
            g.update(0.01);
        }
        assert!(!g.has_blocks(), "blocks came off");
        assert_eq!(g.debris.len(), cells.len());
        assert_eq!((g.games, g.cause), (1, Cause::ToppedOut), "same game over, counted once");
    }

    #[test]
    fn hard_mode_holds_past_30_degrees() {
        // a load that rests between 30° and 45°: easy mode would have slid off
        let mut g = Game::new(3, Mode::Advanced, false);
        let mut cells: Vec<(f64, Pt)> = g
            .board
            .cells()
            .filter(|&(c, v)| v == 0 && g.geo().over_base(c) && g.geo().center(c).x < -2.0)
            .map(|(c, _)| (g.geo().center(c).x, c))
            .collect();
        cells.sort_by(|a, b| a.0.total_cmp(&b.0));
        // add blocks from the far left until the resting tilt is past 30°
        let mut placed = vec![];
        for (_, c) in cells {
            placed.push(c);
            fill(&mut g, &[c]);
            let eq = g.equilibrium().unwrap().abs();
            if eq > physics::SLIDE_ANGLE + 0.05 {
                break;
            }
        }
        let eq = g.equilibrium().unwrap().abs();
        assert!(eq > physics::SLIDE_ANGLE && eq < physics::TOPPLE_ANGLE, "rest {:.1}°", eq.to_degrees());
        // let it settle gently from its resting angle: no impact swing
        g.rocker.theta = g.equilibrium().unwrap();
        for _ in 0..300 {
            g.update(0.01);
        }
        assert_eq!(g.state, State::Playing, "still holding at {:.1}°", g.rocker.theta.to_degrees());
        assert!(g.rocker.theta.abs() > physics::SLIDE_ANGLE);
    }

    #[test]
    fn overhang_hitting_the_floor_topples_it() {
        // blocks hanging far past the left edge reach the floor before 45°
        let mut g = Game::new(3, Mode::Advanced, false);
        let cells: Vec<Pt> = g
            .board
            .cells()
            .filter(|&(c, v)| {
                let ctr = g.geo().center(c);
                v == 0 && ctr.x < -6.0 && ctr.y < g.geo().chord_y + 3.0
            })
            .map(|(c, _)| c)
            .collect();
        fill(&mut g, &cells);
        let mut floor_hit_before_45 = false;
        for _ in 0..600 {
            let before = g.state;
            g.update(0.01);
            if before == State::Playing && g.state != State::Playing {
                floor_hit_before_45 = g.max_tilt < physics::TOPPLE_ANGLE;
                break;
            }
        }
        assert!(matches!(g.cause, Cause::Toppled(_)), "{:?}", g.cause);
        assert!(floor_hit_before_45, "came off at {:.1}°", g.max_tilt.to_degrees());
    }

    #[test]
    fn slide_off_is_mirror_symmetric() {
        let (l, r) = (slide_off(false, Mode::Easy), slide_off(true, Mode::Easy));
        let spread = |g: &Game| {
            let cx = g.rocker.center().x;
            let xs: Vec<f64> = g.debris.iter().map(|d| (d.pos.x - cx).abs()).collect();
            let mean = xs.iter().sum::<f64>() / xs.len() as f64;
            let top = g.debris.iter().map(|d| d.pos.y).fold(0.0, f64::max);
            (mean, top)
        };
        let ((ml, tl), (mr, tr)) = (spread(&l), spread(&r));
        assert!((ml - mr).abs() < 0.5, "mean distance {ml:.2} vs {mr:.2}");
        assert!((tl - tr).abs() < 1.0, "heap height {tl:.2} vs {tr:.2}");
    }

    #[test]
    fn debris_stays_in_the_arena() {
        let mut g = Game::new(3, Mode::Easy, false);
        g.arena = (-9.0, 9.0);
        let cells: Vec<Pt> = g.board.cells().filter(|&(c, _)| c.x < 4 && c.y >= 10).map(|(c, _)| c).collect();
        fill(&mut g, &cells);
        for _ in 0..600 {
            g.update(0.01);
            for d in &g.debris {
                assert!(d.pos.x >= -9.0 && d.pos.x <= 9.0, "escaped to {}", d.pos.x);
            }
        }
        assert!(!g.debris.is_empty());
    }

    #[test]
    fn ai_keeps_it_balanced_for_a_while() {
        for mode in [Mode::Easy, Mode::Advanced] {
            let mut g = Game::new(42, mode, true);
            for _ in 0..200 {
                g.ai_place_now();
                g.settle(1.0);
                if g.state != State::Playing {
                    break;
                }
            }
            // the diamond grid is much harder for this planner (see README)
            let want = if mode == Mode::Easy { 100 } else { 30 };
            assert!(g.pieces >= want, "{mode:?}: AI lost after {} pieces ({:?})", g.pieces, g.cause);
        }
    }
}
