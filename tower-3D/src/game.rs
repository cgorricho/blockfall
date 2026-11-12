//! Game rules: BLOCKFALL (../SPEC.md) wrapped around a cylinder.
//! Board columns wrap modulo `cols` (16, 24 or 32); rows run top (0) to bottom (ROWS-1).

/// Storage width; a game uses the first `cols` columns.
pub const MAX_COLS: usize = 32;
/// Ring sizes (columns around) the player can choose.
pub const MODES: [i32; 3] = [16, 24, 32];
/// Seconds a resting piece waits before it locks.
pub const LOCK_DELAY: f64 = 0.5;
/// Moves/rotations that may restart the lock delay, per piece.
const MAX_LOCK_RESETS: i32 = 15;
pub const ROWS: i32 = 20;

pub struct PieceDef {
    pub n: i32,
    pub cells: [(i32, i32); 4],
    pub color: u8,
}

pub const PIECES: [PieceDef; 7] = [
    PieceDef { n: 4, cells: [(0, 1), (1, 1), (2, 1), (3, 1)], color: 14 }, // I
    PieceDef { n: 2, cells: [(0, 0), (1, 0), (0, 1), (1, 1)], color: 11 }, // O
    PieceDef { n: 3, cells: [(1, 0), (0, 1), (1, 1), (2, 1)], color: 13 }, // T
    PieceDef { n: 3, cells: [(1, 0), (2, 0), (0, 1), (1, 1)], color: 10 }, // S
    PieceDef { n: 3, cells: [(0, 0), (1, 0), (1, 1), (2, 1)], color: 9 },  // Z
    PieceDef { n: 3, cells: [(0, 0), (0, 1), (1, 1), (2, 1)], color: 12 }, // J
    PieceDef { n: 3, cells: [(2, 0), (0, 1), (1, 1), (2, 1)], color: 3 },  // L
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Playing,
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
    pub fn focus(&self) -> f32 {
        self.x as f32 + (self.n - 1) as f32 / 2.0
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
    timer: f64,
    lock_timer: Option<f64>,
    lock_resets: i32,
}

fn rotate(cells: &[(i32, i32); 4], n: i32, cw: bool) -> [(i32, i32); 4] {
    cells.map(|(x, y)| if cw { (n - 1 - y, x) } else { (y, n - 1 - x) })
}

impl Game {
    pub fn new(seed: u32, cols: i32) -> Self {
        assert!(MODES.contains(&cols), "ring size {cols}");
        let seed = if seed == 0 { 0x9E37_79B9 } else { seed };
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
        self.board = [[0; MAX_COLS]; ROWS as usize];
        self.bag.clear();
        self.score = 0;
        self.lines = 0;
        self.state = State::Playing;
        self.paused = false;
        self.timer = 0.0;
        let first = self.draw7();
        self.next = self.draw7();
        self.place(first, 0);
    }

    /// Switch ring size; starts a new game.
    pub fn set_cols(&mut self, cols: i32) {
        assert!(MODES.contains(&cols));
        self.cols = cols;
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
        }
    }

    fn spawn(&mut self) {
        let front = self.cur.x + (self.cur.n - 1) / 2;
        let kind = self.next;
        self.next = self.draw7();
        self.place(kind, front);
    }

    fn lock(&mut self) {
        for (c, r) in self.cur.board_cells(self.cur.y, self.cols) {
            self.board[r as usize][c as usize] = self.cur.kind as u8 + 1;
        }
        self.pieces += 1;
        let n = self.clear_rings();
        if n > 0 {
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
            self.lock();
        }
    }

    pub fn hard_drop(&mut self) {
        if !self.active() {
            return;
        }
        let g = self.ghost_y();
        self.score += 2 * (g - self.cur.y) as i64;
        self.cur.y = g;
        self.lock();
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
}
