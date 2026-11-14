//! Game rules: BLOCKFALL (../SPEC.md) in a 3D well.
//! Game coordinates: x and y (0..size) are horizontal, z (0..H) is up;
//! z = 0 is the floor. Pieces fall along -z.

/// Base sizes the player can pick: 6 × 6 (default) or 9 × 9.
pub const SIZES: [i32; 2] = [6, 9];
/// Storage width and depth; a game uses the first `size` of each.
pub const MAX: i32 = 9;
pub const H: i32 = 16;
/// Seconds a resting piece waits before it locks.
pub const LOCK_DELAY: f64 = 0.5;
/// Moves/rotations that may restart the lock delay, per piece.
const MAX_LOCK_RESETS: i32 = 15;

pub type Cell = (i32, i32, i32);

pub struct PieceDef {
    pub n: i32,
    pub cells: [(i32, i32); 4],
    pub color: u8,
}

/// The original seven BLOCKFALL pieces (SPEC.md), still flat: they spawn
/// lying in the horizontal x-y plane.
pub const PIECES: [PieceDef; 7] = [
    PieceDef { n: 4, cells: [(0, 1), (1, 1), (2, 1), (3, 1)], color: 175 }, // I
    PieceDef { n: 2, cells: [(0, 0), (1, 0), (0, 1), (1, 1)], color: 75 }, // O
    PieceDef { n: 3, cells: [(1, 0), (0, 1), (1, 1), (2, 1)], color: 179 }, // T
    PieceDef { n: 3, cells: [(1, 0), (2, 0), (0, 1), (1, 1)], color: 141 }, // S
    PieceDef { n: 3, cells: [(0, 0), (1, 0), (1, 1), (2, 1)], color: 73 },  // Z
    PieceDef { n: 3, cells: [(0, 0), (0, 1), (1, 1), (2, 1)], color: 107 }, // J
    PieceDef { n: 3, cells: [(2, 0), (0, 1), (1, 1), (2, 1)], color: 167 }, // L
];

/// Rotation axes in game coordinates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// Rotate cells 90° inside their n×n×n box about `axis`.
/// `pos` = counter-clockwise looking down the axis toward the origin
/// (right-hand rule); otherwise clockwise.
pub fn rotate(cells: &[Cell; 4], n: i32, axis: Axis, pos: bool) -> [Cell; 4] {
    let m = n - 1;
    cells.map(|(x, y, z)| match (axis, pos) {
        (Axis::Z, true) => (m - y, x, z),
        (Axis::Z, false) => (y, m - x, z),
        (Axis::X, true) => (x, m - z, y),
        (Axis::X, false) => (x, z, m - y),
        (Axis::Y, true) => (z, y, m - x),
        (Axis::Y, false) => (m - z, y, x),
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Playing,
    Over,
}

#[derive(Clone, Copy, Debug)]
pub struct Active {
    pub kind: usize,
    pub n: i32,
    /// Cells inside the piece's n×n×n box.
    pub cells: [Cell; 4],
    /// Box origin.
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Active {
    /// Well cells with the box origin lowered to height `z`.
    pub fn cells_at(&self, z: i32) -> [Cell; 4] {
        self.cells.map(|(cx, cy, cz)| (self.x + cx, self.y + cy, z + cz))
    }
}

/// Kick offsets tried, in order, after a rotation: none, sideways by 1 and 2
/// along x and y, diagonals, then one step up.
const KICKS: [Cell; 14] = [
    (0, 0, 0),
    (-1, 0, 0),
    (1, 0, 0),
    (0, -1, 0),
    (0, 1, 0),
    (-1, -1, 0),
    (1, -1, 0),
    (-1, 1, 0),
    (1, 1, 0),
    (-2, 0, 0),
    (2, 0, 0),
    (0, -2, 0),
    (0, 2, 0),
    (0, 0, 1),
];

pub struct Game {
    /// board[z][y][x]; 0 = empty, else piece kind + 1.
    pub board: Board,
    /// Width and depth of the base (one of SIZES).
    pub size: i32,
    pub cur: Active,
    pub next: usize,
    bag: Vec<usize>,
    rng: u32,
    pub seed: u32,
    pub score: i64,
    /// Layers cleared this game.
    pub layers: i64,
    pub pieces: i64,
    pub state: State,
    pub paused: bool,
    timer: f64,
    lock_timer: Option<f64>,
    lock_resets: i32,
}

pub type Board = [[[u8; MAX as usize]; MAX as usize]; H as usize];
const EMPTY: Board = [[[0; MAX as usize]; MAX as usize]; H as usize];

impl Game {
    pub fn new(seed: u32, size: i32) -> Self {
        assert!(SIZES.contains(&size), "base size {size}");
        let seed = if seed == 0 { 0x9E37_79B9 } else { seed };
        let mut g = Game {
            board: EMPTY,
            size,
            cur: Active { kind: 0, n: 4, cells: [(0, 0, 0); 4], x: 0, y: 0, z: 0 },
            next: 0,
            bag: Vec::new(),
            rng: seed,
            seed,
            score: 0,
            layers: 0,
            pieces: 0,
            state: State::Playing,
            paused: false,
            timer: 0.0,
            lock_timer: None,
            lock_resets: 0,
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
        self.board = EMPTY;
        self.bag.clear();
        self.score = 0;
        self.layers = 0;
        self.state = State::Playing;
        self.paused = false;
        self.timer = 0.0;
        let first = self.draw7();
        self.next = self.draw7();
        self.place(first);
    }

    /// Switch base size; starts a new game.
    pub fn set_size(&mut self, size: i32) {
        assert!(SIZES.contains(&size));
        self.size = size;
        self.reset();
    }

    /// Cells in one full layer.
    pub fn layer_cells(&self) -> usize {
        (self.size * self.size) as usize
    }

    /// Level goes up every 3 layers (a layer is 81 cells, about 20 pieces).
    pub fn level(&self) -> i64 {
        self.layers / 3 + 1
    }

    /// Gravity interval in ms: slower than 2D, there's more to think about.
    pub fn interval(&self) -> f64 {
        (1500 - (self.level() - 1) * 100).max(200) as f64
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        self.board[z as usize][y as usize][x as usize]
    }

    /// Cells in layer `z` that are filled.
    pub fn layer_fill(&self, z: i32) -> usize {
        self.board[z as usize].iter().flatten().filter(|&&v| v != 0).count()
    }

    /// Inside the walls, above the floor and not overlapping a block.
    /// Above the top of the well counts as free.
    fn fits(&self, cells: &[Cell; 4], x: i32, y: i32, z: i32) -> bool {
        cells.iter().all(|&(cx, cy, cz)| {
            let (gx, gy, gz) = (x + cx, y + cy, z + cz);
            (0..self.size).contains(&gx) && (0..self.size).contains(&gy) && gz >= 0 && (gz >= H || self.get(gx, gy, gz) == 0)
        })
    }

    /// New piece lying flat in the top layer, centred. Spec row y (screen
    /// down) becomes -y (toward the viewer), so seen from the default view
    /// it looks like the 2D piece.
    fn place(&mut self, kind: usize) {
        let d = &PIECES[kind];
        let cells = d.cells.map(|(cx, cy)| (cx, d.n - 1 - cy, 0));
        self.cur = Active { kind, n: d.n, cells, x: (self.size - d.n) / 2, y: (self.size - d.n) / 2, z: H - 1 };
        self.lock_timer = None;
        self.lock_resets = 0;
        if !self.fits(&self.cur.cells, self.cur.x, self.cur.y, self.cur.z) {
            self.state = State::Over;
        }
    }

    fn lock(&mut self) {
        let mut over = false;
        for (x, y, z) in self.cur.cells_at(self.cur.z) {
            if z >= H {
                over = true; // sticks out of the top of the well
            } else {
                self.board[z as usize][y as usize][x as usize] = self.cur.kind as u8 + 1;
            }
        }
        self.pieces += 1;
        let n = self.clear_layers();
        if n > 0 {
            self.layers += n;
            self.score += [0, 100, 300, 500, 800][n.min(4) as usize] * self.level();
        }
        self.timer = 0.0;
        if over {
            self.state = State::Over;
            return;
        }
        let kind = self.next;
        self.next = self.draw7();
        self.place(kind);
    }

    /// Remove every full layer; layers above drop down. Returns how many.
    fn clear_layers(&mut self) -> i64 {
        let s = self.size as usize;
        let full = |l: &[[u8; MAX as usize]; MAX as usize]| l[..s].iter().all(|row| row[..s].iter().all(|&v| v != 0));
        let kept: Vec<_> = self.board.iter().filter(|l| !full(l)).copied().collect();
        let n = H as usize - kept.len();
        let mut b = EMPTY;
        b[..kept.len()].copy_from_slice(&kept);
        self.board = b;
        n as i64
    }

    fn active(&self) -> bool {
        self.state == State::Playing && !self.paused
    }

    /// The piece can't fall any further.
    pub fn resting(&self) -> bool {
        !self.fits(&self.cur.cells, self.cur.x, self.cur.y, self.cur.z - 1)
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

    /// Slide the piece one step in the horizontal plane.
    pub fn shift(&mut self, dx: i32, dy: i32) -> bool {
        if self.active() && self.fits(&self.cur.cells, self.cur.x + dx, self.cur.y + dy, self.cur.z) {
            self.cur.x += dx;
            self.cur.y += dy;
            self.moved();
            return true;
        }
        false
    }

    pub fn try_rotate(&mut self, axis: Axis, pos: bool) -> bool {
        if !self.active() {
            return false;
        }
        let r = rotate(&self.cur.cells, self.cur.n, axis, pos);
        for (kx, ky, kz) in KICKS {
            let (x, y, z) = (self.cur.x + kx, self.cur.y + ky, self.cur.z + kz);
            if self.fits(&r, x, y, z) {
                self.cur.cells = r;
                (self.cur.x, self.cur.y, self.cur.z) = (x, y, z);
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
        if !self.resting() {
            self.cur.z -= 1;
            self.score += 1;
            self.timer = 0.0;
            self.lock_timer = None;
        } else {
            // a soft drop on a resting piece locks it at once
            self.lock();
        }
    }

    pub fn hard_drop(&mut self) {
        if !self.active() {
            return;
        }
        let g = self.ghost_z();
        self.score += 2 * (self.cur.z - g) as i64;
        self.cur.z = g;
        self.lock();
    }

    /// Height of the box origin where the piece would land.
    pub fn ghost_z(&self) -> i32 {
        let mut z = self.cur.z;
        while self.fits(&self.cur.cells, self.cur.x, self.cur.y, z - 1) {
            z -= 1;
        }
        z
    }

    /// A gravity tick: the piece falls a layer, or starts its lock delay.
    fn gravity(&mut self) {
        if !self.resting() {
            self.cur.z -= 1;
            self.lock_timer = None;
        } else if self.lock_timer.is_none() {
            self.lock_timer = Some(0.0);
        }
    }

    /// Advance gravity and the lock delay by `dt` seconds.
    pub fn update(&mut self, dt: f64) {
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
                self.lock();
            } else {
                self.lock_timer = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: i32 = 9;
    const D: i32 = 9;

    fn empty_game() -> Game {
        let mut g = Game::new(42, 9);
        g.board = EMPTY;
        g
    }

    fn set_piece(g: &mut Game, kind: usize, x: i32, y: i32, z: i32) {
        let d = &PIECES[kind];
        g.cur = Active { kind, n: d.n, cells: d.cells.map(|(a, b)| (a, b, 0)), x, y, z };
    }

    fn sorted(mut c: [Cell; 4]) -> [Cell; 4] {
        c.sort();
        c
    }

    #[test]
    fn zero_seed_is_replaced() {
        assert_eq!(Game::new(0, 6).seed, 0x9E37_79B9);
    }

    #[test]
    fn bags_are_permutations() {
        let mut g = Game::new(7, 6);
        g.bag.clear();
        for _ in 0..20 {
            let mut seen: Vec<usize> = (0..7).map(|_| g.draw7()).collect();
            seen.sort();
            assert_eq!(seen, (0..7).collect::<Vec<_>>());
        }
    }

    #[test]
    fn same_seed_same_pieces_as_the_other_editions() {
        // seed 42 deals the same pieces as every BLOCKFALL edition, at any size
        let (mut a, mut b) = (Game::new(42, 6), Game::new(42, 9));
        for _ in 0..50 {
            assert_eq!(a.draw7(), b.draw7());
        }
    }

    #[test]
    fn z_rotation_matches_the_2d_spec() {
        // in the x-y plane, (x,y) -> (n-1-y, x) is the spec's CW turn
        let t = PIECES[2].cells.map(|(x, y)| (x, y, 0));
        let r = rotate(&t, 3, Axis::Z, true);
        assert_eq!(r, [(2, 1, 0), (1, 0, 0), (1, 1, 0), (1, 2, 0)]);
    }

    #[test]
    fn every_rotation_is_a_four_cycle_and_inverts() {
        for d in &PIECES {
            let c = d.cells.map(|(x, y)| (x, y, 1.min(d.n - 1)));
            for axis in [Axis::X, Axis::Y, Axis::Z] {
                let mut r = c;
                for _ in 0..4 {
                    r = rotate(&r, d.n, axis, true);
                    assert!(r.iter().all(|&(x, y, z)| [x, y, z].iter().all(|v| (0..d.n).contains(v))));
                }
                assert_eq!(r, c, "{axis:?}: four turns");
                assert_eq!(rotate(&rotate(&c, d.n, axis, true), d.n, axis, false), c, "{axis:?}: inverse");
            }
        }
    }

    #[test]
    fn x_and_y_rotations_stand_a_flat_piece_up() {
        let i = PIECES[0].cells.map(|(x, y)| (x, y, 0)); // flat along x
        let rx = rotate(&i, 4, Axis::X, true);
        assert!(rx.iter().all(|c| c.1 == rx[0].1), "about x: still a line along x, now raised");
        assert!(rx.iter().all(|c| c.2 == rx[0].2) && rx[0].2 == 1);
        let ry = rotate(&i, 4, Axis::Y, true);
        let zs: Vec<i32> = sorted(ry).iter().map(|c| c.2).collect();
        assert_eq!(zs, [0, 1, 2, 3], "about y: the I stands vertical");
    }

    #[test]
    fn spawns_flat_centred_at_the_top() {
        let g = Game::new(1, 9);
        assert!(g.cur.cells_at(g.cur.z).iter().all(|c| c.2 == H - 1));
        assert_eq!(g.cur.x, (W - g.cur.n) / 2);
    }

    #[test]
    fn walls_stop_moves() {
        let mut g = empty_game();
        set_piece(&mut g, 1, 0, 0, 5);
        assert!(!g.shift(-1, 0));
        assert!(!g.shift(0, -1));
        assert!(g.shift(1, 1));
        set_piece(&mut g, 1, W - 2, D - 2, 5);
        assert!(!g.shift(1, 0) && !g.shift(0, 1));
    }

    #[test]
    fn blocks_stop_moves() {
        let mut g = empty_game();
        set_piece(&mut g, 1, 3, 3, 5);
        g.board[5][3][5] = 1;
        assert!(!g.shift(1, 0));
        assert!(g.shift(-1, 0));
    }

    #[test]
    fn hard_drop_lands_on_floor_and_scores() {
        let mut g = empty_game();
        set_piece(&mut g, 1, 4, 4, H - 1);
        g.next = 2;
        g.hard_drop();
        assert_eq!((g.get(4, 4, 0), g.get(5, 5, 0)), (2, 2));
        assert_eq!(g.score, 2 * (H - 1) as i64);
        assert_eq!(g.pieces, 1);
    }

    #[test]
    fn ghost_rests_on_the_stack() {
        let mut g = empty_game();
        set_piece(&mut g, 1, 3, 3, H - 1);
        g.board[6][4][4] = 1;
        assert_eq!(g.ghost_z(), 7);
    }

    #[test]
    fn rotation_kicks_off_a_wall() {
        let mut g = empty_game();
        // vertical-plane I against the x=0 wall, then turned flat along x
        let i = rotate(&PIECES[0].cells.map(|(x, y)| (x, y, 0)), 4, Axis::Z, true); // along y, x = 2
        g.cur = Active { kind: 0, n: 4, cells: i, x: -2, y: 2, z: 5 };
        assert!(g.fits(&g.cur.cells, g.cur.x, g.cur.y, g.cur.z));
        assert!(g.try_rotate(Axis::Z, true));
        assert!(g.cur.cells_at(g.cur.z).iter().all(|&(x, ..)| (0..W).contains(&x)));
    }

    #[test]
    fn full_layer_clears_and_layers_drop() {
        let mut g = empty_game();
        for y in 0..D as usize {
            for x in 0..W as usize {
                g.board[0][y][x] = 1;
            }
        }
        g.board[0][4][3] = 0;
        g.board[0][4][4] = 0; // a 2-cell gap
        g.board[1][0][0] = 5; // a block above
        // an I lying along x, dropped into the gap row; only 2 cells fit the gap
        // so use an O instead: it covers (3,4),(4,4),(3,5),(4,5)
        g.board[0][5][3] = 0;
        g.board[0][5][4] = 0;
        set_piece(&mut g, 1, 3, 4, H - 1);
        g.next = 2;
        g.hard_drop();
        assert_eq!(g.layers, 1);
        assert_eq!(g.score, 2 * (H - 1) as i64 + 100);
        assert_eq!(g.get(0, 0, 0), 5, "the block above dropped a layer");
        assert_eq!(g.layer_fill(0), 1);
    }

    #[test]
    fn partial_layer_does_not_clear() {
        let mut g = empty_game();
        for y in 0..D as usize {
            for x in 0..W as usize {
                g.board[0][y][x] = 1;
            }
        }
        g.board[0][8][8] = 0;
        assert_eq!(g.clear_layers(), 0);
        g.board[0][8][8] = 1;
        assert_eq!(g.clear_layers(), 1);
    }

    #[test]
    fn gravity_moves_then_locks() {
        let mut g = empty_game();
        let z = g.cur.z;
        g.update(1.51);
        assert_eq!(g.cur.z, z - 1);
        g.update(1.5 * H as f64);
        assert!(g.pieces >= 1);
    }

    #[test]
    fn a_resting_piece_waits_before_locking() {
        let mut g = empty_game();
        g.cur.z = g.ghost_z();
        g.gravity();
        assert_eq!(g.pieces, 0);
        g.update(LOCK_DELAY * 0.8);
        assert_eq!(g.pieces, 0);
        assert!(g.shift(1, 0));
        g.update(LOCK_DELAY * 0.8);
        assert_eq!(g.pieces, 0, "the move restarted the delay");
        g.update(LOCK_DELAY * 0.4);
        assert_eq!(g.pieces, 1);
    }

    #[test]
    fn soft_drop_on_a_resting_piece_locks_at_once() {
        let mut g = empty_game();
        g.cur.z = g.ghost_z();
        g.soft_drop();
        assert_eq!(g.pieces, 1);
    }

    #[test]
    fn locking_above_the_rim_ends_the_game() {
        let mut g = empty_game();
        for z in 0..H as usize - 1 {
            g.board[z][4][4] = 1;
        }
        // an I standing up over the column: it rests with cells above the top
        let i = rotate(&PIECES[0].cells.map(|(x, y)| (x, y, 0)), 4, Axis::Y, true);
        let x = 4 - i[0].0;
        g.cur = Active { kind: 0, n: 4, cells: i, x, y: 4 - i[0].1, z: H };
        g.hard_drop();
        assert_eq!(g.state, State::Over);
    }

    #[test]
    fn small_base_has_walls_at_six() {
        let mut g = Game::new(3, 6);
        g.board = EMPTY;
        set_piece(&mut g, 1, 4, 4, 5); // O at x,y = 4..5: against both far walls
        assert!(!g.shift(1, 0) && !g.shift(0, 1));
        assert_eq!(g.cur.x, 4);
        assert!(Game::new(3, 6).cur.cells_at(H - 1).iter().all(|&(x, y, _)| (0..6).contains(&x) && (0..6).contains(&y)));
    }

    #[test]
    fn small_base_clears_a_six_by_six_layer() {
        let mut g = Game::new(3, 6);
        g.board = EMPTY;
        for y in 0..6 {
            for x in 0..6 {
                g.board[0][y][x] = 1;
            }
        }
        assert_eq!(g.layer_cells(), 36);
        assert_eq!(g.clear_layers(), 1, "36 cells fill a 6 x 6 layer");
        g.set_size(9);
        for y in 0..6 {
            for x in 0..6 {
                g.board[0][y][x] = 1;
            }
        }
        assert_eq!(g.clear_layers(), 0, "but not a 9 x 9 one");
    }

    #[test]
    fn switching_size_restarts() {
        let mut g = Game::new(5, 6);
        g.hard_drop();
        g.set_size(9);
        assert_eq!((g.size, g.score, g.pieces > 0, g.layer_fill(0)), (9, 0, true, 0));
    }

    #[test]
    fn blocked_spawn_ends_game() {
        let mut g = empty_game();
        g.board[(H - 1) as usize][4][4] = 1;
        g.place(1);
        assert_eq!(g.state, State::Over);
        g.hard_drop();
        assert_eq!(g.state, State::Over);
    }
}
