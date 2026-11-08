// BLOCKFALL — Rust edition (crossterm). See ../SPEC.md.
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Attribute, Color, Print, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType},
};

const W: usize = 10;
const H: usize = 20;
const BOARD_X: u16 = 2;
const BOARD_Y: u16 = 2;
const PANEL_X: u16 = 27;
const LANG_NAME: &str = "[Rust]";
const AI_STEP: Duration = Duration::from_millis(50);
const TURBO_BUDGET: Duration = Duration::from_millis(15);
const FRAME: Duration = Duration::from_millis(16);
const KICKS: [i32; 5] = [0, -1, 1, -2, 2];

#[derive(Clone, Copy)]
struct Pt {
    x: i32,
    y: i32,
}
const fn p(x: i32, y: i32) -> Pt {
    Pt { x, y }
}

struct PieceDef {
    n: i32,
    cells: [Pt; 4],
    color: u8,
}

const PIECES: [PieceDef; 7] = [
    PieceDef { n: 4, cells: [p(0, 1), p(1, 1), p(2, 1), p(3, 1)], color: 14 }, // I
    PieceDef { n: 2, cells: [p(0, 0), p(1, 0), p(0, 1), p(1, 1)], color: 11 }, // O
    PieceDef { n: 3, cells: [p(1, 0), p(0, 1), p(1, 1), p(2, 1)], color: 13 }, // T
    PieceDef { n: 3, cells: [p(1, 0), p(2, 0), p(0, 1), p(1, 1)], color: 10 }, // S
    PieceDef { n: 3, cells: [p(0, 0), p(1, 0), p(1, 1), p(2, 1)], color: 9 },  // Z
    PieceDef { n: 3, cells: [p(0, 0), p(0, 1), p(1, 1), p(2, 1)], color: 12 }, // J
    PieceDef { n: 3, cells: [p(2, 0), p(0, 1), p(1, 1), p(2, 1)], color: 3 },  // L
];

const HELP: [&str; 9] = [
    "←→  move", "↑ X rotate", "Z   rotate ccw", "↓   soft drop", "SPC hard drop", "P   pause", "R   restart", "F   retro mode",
    "Q   quit",
];

/// Everything that changes between the colour and retro looks.
struct Theme {
    block: &'static str,
    ghost: &'static str,
    empty: &'static str,
    tl: &'static str,
    tr: &'static str,
    bl: &'static str,
    br: &'static str,
    horiz: &'static str,
    vert: &'static str,
    dim: u8,
    text: u8,
    empty_col: u8,
    mono: bool, // every block/ghost uses one colour
    block_mono: u8,
    ghost_mono: u8,
}

const COLOUR_THEME: Theme = Theme {
    block: "██", ghost: "░░", empty: " ·", tl: "┌", tr: "┐", bl: "└", br: "┘", horiz: "─", vert: "│",
    dim: 245, text: 15, empty_col: 240, mono: false, block_mono: 0, ghost_mono: 0,
};
const RETRO_THEME: Theme = Theme {
    block: "[]", ghost: "::", empty: " .", tl: "+", tr: "+", bl: "+", br: "+", horiz: "-", vert: "|",
    dim: 28, text: 10, empty_col: 28, mono: true, block_mono: 10, ghost_mono: 28,
};

type Board = [[u8; W]; H]; // 0 = empty, else kind + 1

struct Piece {
    kind: usize,
    cells: [Pt; 4],
    x: i32,
    y: i32,
}

#[derive(Clone, Copy)]
struct AiPlan {
    rots: i32,
    x: i32,
    ok: bool,
}

struct Game {
    board: Board,
    cur: Piece,
    next: usize,
    bag: Vec<usize>,
    rng: u32,
    seed: u32,
    score: i64,
    lines: i64,
    paused: bool,
    over: bool,
    retro: bool, // F: monochrome green "[]" rendering
    last_fall: Instant,

    ai: bool,
    turbo: bool,
    plan: AiPlan,
    rots_done: i32,
    last_ai: Instant,
    over_at: Instant,

    // running totals, never reset (lines/score are per game)
    pieces: i64,
    games: i64,
    frames: i64,
    total_lines: i64,
}

fn fits_on(b: &Board, cells: &[Pt; 4], x: i32, y: i32) -> bool {
    cells.iter().all(|c| {
        let (bx, by) = (x + c.x, y + c.y);
        bx >= 0 && bx < W as i32 && by >= 0 && by < H as i32 && b[by as usize][bx as usize] == 0
    })
}

fn rotate(cells: &[Pt; 4], n: i32, cw: bool) -> [Pt; 4] {
    cells.map(|c| if cw { p(n - 1 - c.y, c.x) } else { p(c.y, n - 1 - c.x) })
}

/// Rotates and applies the wall-kick rule; None if nothing fits.
fn kick_rotate(b: &Board, cells: &[Pt; 4], n: i32, x: i32, y: i32, cw: bool) -> Option<([Pt; 4], i32)> {
    let r = rotate(cells, n, cw);
    KICKS.iter().find(|&&k| fits_on(b, &r, x + k, y)).map(|&k| (r, x + k))
}

/// Removes full rows and returns how many were cleared.
fn clear_lines(b: &mut Board) -> i64 {
    let mut cleared = 0;
    let mut y = H as i32 - 1;
    while y >= 0 {
        if b[y as usize].iter().all(|&v| v != 0) {
            cleared += 1;
            for yy in (1..=y as usize).rev() {
                b[yy] = b[yy - 1];
            }
            b[0] = [0; W];
            // re-check same row
        } else {
            y -= 1;
        }
    }
    cleared
}

fn evaluate(b: &Board, cleared: i64) -> i64 {
    let (mut agg, mut holes, mut bump) = (0i64, 0i64, 0i64);
    let mut hts = [0i64; W];
    for x in 0..W {
        let top = (0..H).find(|&y| b[y][x] != 0).unwrap_or(H);
        hts[x] = (H - top) as i64;
        agg += hts[x];
        holes += (top + 1..H).filter(|&y| b[y][x] == 0).count() as i64;
    }
    for x in 0..W - 1 {
        bump += (hts[x] - hts[x + 1]).abs();
    }
    -510066 * agg + 760666 * cleared - 356630 * holes - 184483 * bump
}

impl Game {
    fn new(seed: u32, ai: bool) -> Self {
        let seed = if seed == 0 { 0x9E37_79B9 } else { seed };
        let now = Instant::now();
        let mut g = Game {
            board: [[0; W]; H],
            cur: Piece { kind: 0, cells: PIECES[0].cells, x: 0, y: 0 },
            next: 0,
            bag: Vec::new(),
            rng: seed,
            seed,
            score: 0,
            lines: 0,
            paused: false,
            over: false,
            retro: false,
            last_fall: now,
            ai,
            turbo: false,
            plan: AiPlan { rots: 0, x: 0, ok: false },
            rots_done: 0,
            last_ai: now,
            over_at: now,
            pieces: 0,
            games: 0,
            frames: 0,
            total_lines: 0,
        };
        g.reset();
        g
    }

    fn level(&self) -> i64 {
        self.lines / 10 + 1
    }

    fn interval(&self) -> Duration {
        let ms = 800 - (self.level() - 1) * 70;
        Duration::from_millis(ms.max(80) as u64)
    }

    // ---------- shared PRNG + 7-bag ----------

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

    // ---------- core rules ----------

    fn reset(&mut self) {
        self.board = [[0; W]; H];
        self.bag.clear();
        self.score = 0;
        self.lines = 0;
        self.paused = false;
        self.over = false;
        self.next = self.draw7();
        self.spawn();
    }

    fn spawn(&mut self) {
        let d = &PIECES[self.next];
        self.cur = Piece { kind: self.next, cells: d.cells, x: (W as i32 - d.n) / 2, y: 0 };
        self.next = self.draw7();
        self.last_fall = Instant::now();
        if !fits_on(&self.board, &self.cur.cells, self.cur.x, self.cur.y) {
            self.over = true;
            self.games += 1;
            self.over_at = Instant::now();
            return;
        }
        if self.ai {
            self.plan = self.find_plan();
            self.rots_done = 0;
            self.last_ai = Instant::now();
        }
    }

    fn try_move(&mut self, dx: i32, dy: i32) -> bool {
        if fits_on(&self.board, &self.cur.cells, self.cur.x + dx, self.cur.y + dy) {
            self.cur.x += dx;
            self.cur.y += dy;
            true
        } else {
            false
        }
    }

    fn try_rotate(&mut self, cw: bool) {
        let n = PIECES[self.cur.kind].n;
        if let Some((cells, x)) = kick_rotate(&self.board, &self.cur.cells, n, self.cur.x, self.cur.y, cw) {
            self.cur.cells = cells;
            self.cur.x = x;
        }
    }

    fn lock(&mut self) {
        for c in self.cur.cells {
            self.board[(self.cur.y + c.y) as usize][(self.cur.x + c.x) as usize] = self.cur.kind as u8 + 1;
        }
        let cleared = clear_lines(&mut self.board);
        const PTS: [i64; 5] = [0, 100, 300, 500, 800];
        self.score += PTS[cleared as usize] * self.level();
        self.lines += cleared;
        self.total_lines += cleared;
        self.pieces += 1;
        self.spawn();
    }

    fn gravity(&mut self) {
        if !self.try_move(0, 1) {
            self.lock();
        }
        self.last_fall = Instant::now();
    }

    fn hard_drop(&mut self) {
        while self.try_move(0, 1) {
            self.score += 2;
        }
        self.lock();
    }

    fn ghost_y(&self) -> i32 {
        let mut y = self.cur.y;
        while fits_on(&self.board, &self.cur.cells, self.cur.x, y + 1) {
            y += 1;
        }
        y
    }

    // ---------- AI ----------

    fn find_plan(&self) -> AiPlan {
        let mut best = AiPlan { rots: 0, x: 0, ok: false };
        let mut best_score = 0i64;
        let n = PIECES[self.cur.kind].n;
        'rot: for r in 0..4 {
            let (mut cells, mut x, y) = (self.cur.cells, self.cur.x, self.cur.y);
            for _ in 0..r {
                match kick_rotate(&self.board, &cells, n, x, y, true) {
                    Some((c, nx)) => {
                        cells = c;
                        x = nx;
                    }
                    None => continue 'rot,
                }
            }
            'target: for tx in -3..W as i32 {
                let mut px = x;
                while px != tx {
                    let step = if tx < px { -1 } else { 1 };
                    if !fits_on(&self.board, &cells, px + step, y) {
                        continue 'target;
                    }
                    px += step;
                }
                let mut py = y;
                while fits_on(&self.board, &cells, px, py + 1) {
                    py += 1;
                }
                let mut b = self.board;
                for c in cells {
                    b[(py + c.y) as usize][(px + c.x) as usize] = 1;
                }
                let cleared = clear_lines(&mut b);
                let s = evaluate(&b, cleared);
                if !best.ok || s > best_score {
                    best = AiPlan { rots: r, x: tx, ok: true };
                    best_score = s;
                }
            }
        }
        best
    }

    /// Performs one step of the current plan.
    fn ai_action(&mut self) {
        let p = self.plan;
        if !p.ok {
            self.hard_drop();
        } else if self.rots_done < p.rots {
            self.try_rotate(true);
            self.rots_done += 1;
        } else if self.cur.x < p.x {
            if !self.try_move(1, 0) {
                self.hard_drop();
            }
        } else if self.cur.x > p.x {
            if !self.try_move(-1, 0) {
                self.hard_drop();
            }
        } else {
            self.hard_drop();
        }
    }

    /// Executes the whole plan for the current piece at once.
    fn ai_place_now(&mut self) {
        let start = self.pieces;
        while self.pieces == start && !self.over {
            self.ai_action();
        }
    }

    /// Advances the AI; returns true if anything changed.
    fn ai_update(&mut self) -> bool {
        if self.paused {
            return false;
        }
        if self.over {
            if self.turbo || self.over_at.elapsed() >= Duration::from_secs(1) {
                self.reset();
                return true;
            }
            return false;
        }
        if self.turbo {
            let start = Instant::now();
            while start.elapsed() < TURBO_BUDGET && !self.over {
                self.ai_place_now();
            }
            return true;
        }
        if self.last_ai.elapsed() >= AI_STEP {
            self.ai_action();
            self.last_ai = Instant::now();
            return true;
        }
        false
    }

    // ---------- stats file ----------

    fn write_stats(&self) {
        let path = stats_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let json = format!(
            "{{\"lang\":\"Rust\",\"pid\":{},\"ai\":{},\"turbo\":{},\"seed\":{},\"pieces\":{},\"lines\":{},\"score\":{},\"games\":{},\"frames\":{},\"ts\":{}}}",
            std::process::id(), self.ai, self.turbo, self.seed, self.pieces, self.total_lines, self.score, self.games, self.frames, ts
        );
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }

    /// Returns true when the game should quit.
    fn handle_key(&mut self, k: KeyEvent) -> bool {
        let ch = match k.code {
            KeyCode::Char(c) => Some(c.to_ascii_lowercase()),
            _ => None,
        };
        if k.code == KeyCode::Esc || ch == Some('q') || (ch == Some('c') && k.modifiers.contains(KeyModifiers::CONTROL)) {
            return true;
        }
        match ch {
            Some('r') => {
                self.reset();
                return false;
            }
            Some('f') => {
                self.retro = !self.retro;
                return false;
            }
            Some('p') => {
                if !self.over {
                    self.paused = !self.paused;
                }
                return false;
            }
            Some('t') => {
                if self.ai {
                    self.turbo = !self.turbo;
                }
                return false;
            }
            _ => {}
        }
        if self.over || self.paused || self.ai {
            return false;
        }
        match (k.code, ch) {
            (KeyCode::Left, _) | (_, Some('a')) => {
                self.try_move(-1, 0);
            }
            (KeyCode::Right, _) | (_, Some('d')) => {
                self.try_move(1, 0);
            }
            (KeyCode::Down, _) | (_, Some('s')) => {
                if self.try_move(0, 1) {
                    self.score += 1;
                    self.last_fall = Instant::now();
                }
            }
            (KeyCode::Up, _) | (_, Some('w')) | (_, Some('x')) => self.try_rotate(true),
            (_, Some('z')) => self.try_rotate(false),
            (_, Some(' ')) => self.hard_drop(),
            _ => {}
        }
        false
    }
}

fn stats_path() -> PathBuf {
    std::env::temp_dir().join("blockfall").join("rs.json")
}

// ---------- rendering ----------

#[derive(Clone, Copy, PartialEq)]
struct Style {
    fg: Option<u8>,
    bold: bool,
    reverse: bool,
    bg: Option<u8>,
}
const PLAIN: Style = Style { fg: None, bold: false, reverse: false, bg: None };

struct Screen {
    w: usize,
    h: usize,
    ch: Vec<char>,
    st: Vec<Style>,
    fill: Style,
    // background the whole terminal was last cleared to (outer None = never cleared)
    last_bg: Option<Option<u8>>,
}

fn queue_style(out: &mut impl Write, s: Style) -> io::Result<()> {
    queue!(out, SetAttribute(Attribute::Reset))?;
    if let Some(c) = s.fg {
        queue!(out, SetForegroundColor(Color::AnsiValue(c)))?;
    }
    if let Some(c) = s.bg {
        queue!(out, SetBackgroundColor(Color::AnsiValue(c)))?;
    }
    if s.bold {
        queue!(out, SetAttribute(Attribute::Bold))?;
    }
    if s.reverse {
        queue!(out, SetAttribute(Attribute::Reverse))?;
    }
    Ok(())
}

impl Screen {
    fn new(w: usize, h: usize) -> Self {
        Screen { w, h, ch: vec![' '; w * h], st: vec![PLAIN; w * h], fill: PLAIN, last_bg: None }
    }
    fn clear(&mut self, fill: Style) {
        self.fill = fill;
        self.ch.fill(' ');
        self.st.fill(fill);
    }
    fn put(&mut self, x: u16, y: u16, s: &str, style: Style) {
        let (mut x, y) = (x as usize, y as usize);
        for c in s.chars() {
            if x < self.w && y < self.h {
                self.ch[y * self.w + x] = c;
                self.st[y * self.w + x] = style;
            }
            x += 1;
        }
    }
    fn flush(&mut self, out: &mut impl Write) -> io::Result<()> {
        if self.last_bg != Some(self.fill.bg) {
            // Background changed (mode toggle): repaint the whole terminal, not just our buffer.
            queue_style(out, self.fill)?;
            queue!(out, terminal::Clear(ClearType::All))?;
            self.last_bg = Some(self.fill.bg);
        }
        let mut cur: Option<Style> = None;
        for y in 0..self.h {
            queue!(out, cursor::MoveTo(0, y as u16))?;
            for x in 0..self.w {
                let s = self.st[y * self.w + x];
                if cur != Some(s) {
                    queue_style(out, s)?;
                    cur = Some(s);
                }
                queue!(out, Print(self.ch[y * self.w + x]))?;
            }
        }
        queue!(out, SetAttribute(Attribute::Reset))?;
        out.flush()
    }
}

fn render(g: &mut Game, s: &mut Screen, out: &mut impl Write) -> io::Result<()> {
    g.frames += 1;
    let th = if g.retro { &RETRO_THEME } else { &COLOUR_THEME };
    let bg = if g.retro { Some(0) } else { None };
    let fg = |c: u8| Style { fg: Some(c), bold: false, reverse: false, bg };
    let block_col = |kind: usize| if th.mono { th.block_mono } else { PIECES[kind].color };

    let plain = Style { fg: if g.retro { Some(10) } else { None }, bold: false, reverse: false, bg };
    s.clear(plain);
    let gray = fg(th.dim);
    let white = Style { bold: true, ..fg(th.text) };
    let (bw, bh) = (W as u16, H as u16);

    s.put(2, 0, " B L O C K F A L L ", white);
    s.put(22, 0, LANG_NAME, gray);
    if g.ai {
        s.put(30, 0, if g.turbo { "AI TURBO" } else { "AI WATCH" }, gray);
    }

    s.put(BOARD_X, BOARD_Y, th.tl, gray);
    s.put(BOARD_X + 1 + bw * 2, BOARD_Y, th.tr, gray);
    s.put(BOARD_X, BOARD_Y + bh + 1, th.bl, gray);
    s.put(BOARD_X + 1 + bw * 2, BOARD_Y + bh + 1, th.br, gray);
    for i in 0..bw * 2 {
        s.put(BOARD_X + 1 + i, BOARD_Y, th.horiz, gray);
        s.put(BOARD_X + 1 + i, BOARD_Y + bh + 1, th.horiz, gray);
    }
    for y in 0..bh {
        s.put(BOARD_X, BOARD_Y + 1 + y, th.vert, gray);
        s.put(BOARD_X + 1 + bw * 2, BOARD_Y + 1 + y, th.vert, gray);
    }

    let cell = |s: &mut Screen, x: i32, y: i32, txt: &str, color: u8| {
        s.put(BOARD_X + 1 + (x as u16) * 2, BOARD_Y + 1 + y as u16, txt, fg(color));
    };
    for y in 0..H {
        for x in 0..W {
            match g.board[y][x] {
                0 => cell(s, x as i32, y as i32, th.empty, th.empty_col),
                k => cell(s, x as i32, y as i32, th.block, block_col(k as usize - 1)),
            }
        }
    }
    if !g.over {
        let col = block_col(g.cur.kind);
        let ghost_col = if th.mono { th.ghost_mono } else { col };
        let gy = g.ghost_y();
        for c in g.cur.cells {
            cell(s, g.cur.x + c.x, gy + c.y, th.ghost, ghost_col);
        }
        for c in g.cur.cells {
            cell(s, g.cur.x + c.x, g.cur.y + c.y, th.block, col);
        }
    }

    s.put(PANEL_X, 3, "NEXT", white);
    for c in PIECES[g.next].cells {
        s.put(PANEL_X + c.x as u16 * 2, 4 + c.y as u16, th.block, fg(block_col(g.next)));
    }
    s.put(PANEL_X, 9, "SCORE", white);
    s.put(PANEL_X, 10, &g.score.to_string(), plain);
    s.put(PANEL_X, 12, "LINES", white);
    s.put(PANEL_X, 13, &g.lines.to_string(), plain);
    s.put(PANEL_X, 15, "LEVEL", white);
    s.put(PANEL_X, 16, &g.level().to_string(), plain);
    for (i, h) in HELP.iter().enumerate() {
        s.put(PANEL_X, 18 + i as u16, h, gray);
    }
    if g.ai {
        s.put(PANEL_X, 27, "T   ai turbo", gray);
    }

    let inv = Style { bold: true, reverse: true, ..plain };
    let center = |s: &mut Screen, row: u16, txt: &str| {
        let x = BOARD_X + 1 + (bw * 2 - txt.chars().count() as u16) / 2;
        s.put(x, row, txt, inv);
    };
    if g.over {
        center(s, 12, " GAME OVER ");
        center(s, 13, " R = restart ");
    } else if g.paused {
        center(s, 12, " PAUSED ");
    }
    s.flush(out)
}

// ---------- main ----------

struct Args {
    ai: bool,
    turbo: bool,
    seed: u32,
    bench: i64,
}

fn parse_args() -> Args {
    let mut a = Args { ai: false, turbo: false, seed: 0, bench: 0 };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        match argv[i].trim_start_matches('-') {
            "ai" => a.ai = true,
            "turbo" => a.turbo = true,
            "seed" if i + 1 < argv.len() => {
                i += 1;
                a.seed = argv[i].parse::<u64>().unwrap_or(0) as u32;
            }
            "bench" if i + 1 < argv.len() => {
                i += 1;
                a.bench = argv[i].parse().unwrap_or(0);
            }
            _ => {}
        }
        i += 1;
    }
    if a.seed == 0 {
        a.seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u32).unwrap_or(1);
    }
    a
}

fn run_bench(seed: u32, n: i64) {
    let mut g = Game::new(seed, true);
    let start = Instant::now();
    while g.pieces < n {
        if g.over {
            g.reset();
        }
        g.ai_place_now();
    }
    println!(
        "lang=rs seed={} pieces={} lines={} score={} games={} ms={}",
        g.seed, g.pieces, g.total_lines, g.score, g.games, start.elapsed().as_millis()
    );
}

fn run(g: &mut Game, out: &mut impl Write) -> io::Result<()> {
    let mut s = Screen::new(44, 28);
    render(g, &mut s, out)?;
    g.write_stats();
    let mut last_stats = Instant::now();
    loop {
        let mut dirty = false;
        let timeout = if g.ai && g.turbo && !g.paused { Duration::ZERO } else { FRAME };
        if event::poll(timeout)? {
            loop {
                match event::read()? {
                    // Windows reports both press and release; act on press/repeat only.
                    Event::Key(k) if k.kind != KeyEventKind::Release => {
                        if g.handle_key(k) {
                            return Ok(());
                        }
                        dirty = true;
                    }
                    Event::Resize(..) => {
                        s.last_bg = None; // force a full repaint in the current background
                        dirty = true;
                    }
                    _ => {}
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
        if g.ai {
            dirty |= g.ai_update();
        } else if !g.over && !g.paused && g.last_fall.elapsed() >= g.interval() {
            g.gravity();
            dirty = true;
        }
        if dirty {
            render(g, &mut s, out)?;
        }
        if last_stats.elapsed() >= Duration::from_millis(500) {
            g.write_stats();
            last_stats = Instant::now();
        }
    }
}

fn main() -> io::Result<()> {
    let args = parse_args();
    if args.bench > 0 {
        run_bench(args.seed, args.bench);
        return Ok(());
    }

    let mut g = Game::new(args.seed, args.ai);
    g.turbo = args.ai && args.turbo;

    // Collect each frame in one 64 KB buffer and hand it to the console in a single flush
    // (like the Go and C# editions). Plain io::stdout() is line-buffered with a 1 KB buffer,
    // so a ~6 KB frame without newlines would reach Windows as many small writes.
    let mut out = io::BufWriter::with_capacity(64 * 1024, io::stdout());
    terminal::enable_raw_mode()?;
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide, terminal::Clear(ClearType::All))?;
    let res = run(&mut g, &mut out);
    execute!(out, SetAttribute(Attribute::Reset), cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    let _ = std::fs::remove_file(stats_path());
    res
}
