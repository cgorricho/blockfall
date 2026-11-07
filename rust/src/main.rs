// BLOCKFALL — Rust edition (crossterm). See ../SPEC.md.
use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Attribute, Color, Print, SetAttribute, SetForegroundColor},
    terminal::{self, ClearType},
};
use rand::seq::SliceRandom;

const W: usize = 10;
const H: usize = 20;
const BOARD_X: u16 = 2;
const BOARD_Y: u16 = 2;
const PANEL_X: u16 = 27;
const LANG_NAME: &str = "[Rust]";

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

const HELP: [&str; 8] = [
    "←→  move", "↑ X rotate", "Z   rotate ccw", "↓   soft drop", "SPC hard drop", "P   pause", "R   restart", "Q   quit",
];

struct Piece {
    kind: usize,
    cells: [Pt; 4],
    x: i32,
    y: i32,
}

struct Game {
    board: [[u8; W]; H], // 0 = empty, else kind + 1
    cur: Piece,
    next: usize,
    bag: Vec<usize>,
    score: u32,
    lines: u32,
    paused: bool,
    over: bool,
    last_fall: Instant,
}

impl Game {
    fn new() -> Self {
        let mut g = Game {
            board: [[0; W]; H],
            cur: Piece { kind: 0, cells: PIECES[0].cells, x: 0, y: 0 },
            next: 0,
            bag: Vec::new(),
            score: 0,
            lines: 0,
            paused: false,
            over: false,
            last_fall: Instant::now(),
        };
        g.reset();
        g
    }

    fn level(&self) -> u32 {
        self.lines / 10 + 1
    }

    fn interval(&self) -> Duration {
        let ms = 800i64 - (self.level() as i64 - 1) * 70;
        Duration::from_millis(ms.max(80) as u64)
    }

    fn draw7(&mut self) -> usize {
        if self.bag.is_empty() {
            let mut v: Vec<usize> = (0..7).collect();
            v.shuffle(&mut rand::thread_rng());
            self.bag = v;
        }
        self.bag.remove(0)
    }

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
        if !self.fits(&self.cur.cells, self.cur.x, self.cur.y) {
            self.over = true;
        }
    }

    fn fits(&self, cells: &[Pt; 4], x: i32, y: i32) -> bool {
        cells.iter().all(|c| {
            let (bx, by) = (x + c.x, y + c.y);
            bx >= 0 && bx < W as i32 && by >= 0 && by < H as i32 && self.board[by as usize][bx as usize] == 0
        })
    }

    fn try_move(&mut self, dx: i32, dy: i32) -> bool {
        if self.fits(&self.cur.cells, self.cur.x + dx, self.cur.y + dy) {
            self.cur.x += dx;
            self.cur.y += dy;
            true
        } else {
            false
        }
    }

    fn try_rotate(&mut self, cw: bool) {
        let n = PIECES[self.cur.kind].n;
        let r = self.cur.cells.map(|c| if cw { p(n - 1 - c.y, c.x) } else { p(c.y, n - 1 - c.x) });
        for k in [0, -1, 1, -2, 2] {
            if self.fits(&r, self.cur.x + k, self.cur.y) {
                self.cur.cells = r;
                self.cur.x += k;
                return;
            }
        }
    }

    fn lock(&mut self) {
        for c in self.cur.cells {
            self.board[(self.cur.y + c.y) as usize][(self.cur.x + c.x) as usize] = self.cur.kind as u8 + 1;
        }
        let mut cleared = 0usize;
        let mut y = H as i32 - 1;
        while y >= 0 {
            if self.board[y as usize].iter().all(|&v| v != 0) {
                cleared += 1;
                for yy in (1..=y as usize).rev() {
                    self.board[yy] = self.board[yy - 1];
                }
                self.board[0] = [0; W];
                // re-check same row
            } else {
                y -= 1;
            }
        }
        const PTS: [u32; 5] = [0, 100, 300, 500, 800];
        self.score += PTS[cleared] * self.level();
        self.lines += cleared as u32;
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
        while self.fits(&self.cur.cells, self.cur.x, y + 1) {
            y += 1;
        }
        y
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
        if ch == Some('r') {
            self.reset();
            return false;
        }
        if ch == Some('p') {
            if !self.over {
                self.paused = !self.paused;
            }
            return false;
        }
        if self.over || self.paused {
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

// ---------- rendering ----------

#[derive(Clone, Copy, PartialEq)]
struct Style {
    fg: Option<u8>,
    bold: bool,
    reverse: bool,
}
const fn fg(c: u8) -> Style {
    Style { fg: Some(c), bold: false, reverse: false }
}
const PLAIN: Style = Style { fg: None, bold: false, reverse: false };

struct Screen {
    w: usize,
    h: usize,
    ch: Vec<char>,
    st: Vec<Style>,
}

impl Screen {
    fn new(w: usize, h: usize) -> Self {
        Screen { w, h, ch: vec![' '; w * h], st: vec![PLAIN; w * h] }
    }
    fn clear(&mut self) {
        self.ch.fill(' ');
        self.st.fill(PLAIN);
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
    fn flush(&self, out: &mut impl Write) -> io::Result<()> {
        let mut cur: Option<Style> = None;
        for y in 0..self.h {
            queue!(out, cursor::MoveTo(0, y as u16))?;
            for x in 0..self.w {
                let s = self.st[y * self.w + x];
                if cur != Some(s) {
                    queue!(out, SetAttribute(Attribute::Reset))?;
                    if let Some(c) = s.fg {
                        queue!(out, SetForegroundColor(Color::AnsiValue(c)))?;
                    }
                    if s.bold {
                        queue!(out, SetAttribute(Attribute::Bold))?;
                    }
                    if s.reverse {
                        queue!(out, SetAttribute(Attribute::Reverse))?;
                    }
                    cur = Some(s);
                }
                queue!(out, Print(self.ch[y * self.w + x]))?;
            }
        }
        queue!(out, SetAttribute(Attribute::Reset))?;
        out.flush()
    }
}

fn render(g: &Game, s: &mut Screen, out: &mut impl Write) -> io::Result<()> {
    s.clear();
    let gray = fg(245);
    let white = Style { fg: Some(15), bold: true, reverse: false };
    let (bw, bh) = (W as u16, H as u16);

    s.put(2, 0, " B L O C K F A L L ", white);
    s.put(22, 0, LANG_NAME, gray);

    s.put(BOARD_X, BOARD_Y, "┌", gray);
    s.put(BOARD_X + 1 + bw * 2, BOARD_Y, "┐", gray);
    s.put(BOARD_X, BOARD_Y + bh + 1, "└", gray);
    s.put(BOARD_X + 1 + bw * 2, BOARD_Y + bh + 1, "┘", gray);
    for i in 0..bw * 2 {
        s.put(BOARD_X + 1 + i, BOARD_Y, "─", gray);
        s.put(BOARD_X + 1 + i, BOARD_Y + bh + 1, "─", gray);
    }
    for y in 0..bh {
        s.put(BOARD_X, BOARD_Y + 1 + y, "│", gray);
        s.put(BOARD_X + 1 + bw * 2, BOARD_Y + 1 + y, "│", gray);
    }

    let cell = |s: &mut Screen, x: i32, y: i32, txt: &str, color: u8| {
        s.put(BOARD_X + 1 + (x as u16) * 2, BOARD_Y + 1 + y as u16, txt, fg(color));
    };
    for y in 0..H {
        for x in 0..W {
            match g.board[y][x] {
                0 => cell(s, x as i32, y as i32, " ·", 240),
                k => cell(s, x as i32, y as i32, "██", PIECES[k as usize - 1].color),
            }
        }
    }
    if !g.over {
        let col = PIECES[g.cur.kind].color;
        let gy = g.ghost_y();
        for c in g.cur.cells {
            cell(s, g.cur.x + c.x, gy + c.y, "░░", col);
        }
        for c in g.cur.cells {
            cell(s, g.cur.x + c.x, g.cur.y + c.y, "██", col);
        }
    }

    s.put(PANEL_X, 3, "NEXT", white);
    let nd = &PIECES[g.next];
    for c in nd.cells {
        s.put(PANEL_X + c.x as u16 * 2, 4 + c.y as u16, "██", fg(nd.color));
    }
    s.put(PANEL_X, 9, "SCORE", white);
    s.put(PANEL_X, 10, &g.score.to_string(), PLAIN);
    s.put(PANEL_X, 12, "LINES", white);
    s.put(PANEL_X, 13, &g.lines.to_string(), PLAIN);
    s.put(PANEL_X, 15, "LEVEL", white);
    s.put(PANEL_X, 16, &g.level().to_string(), PLAIN);
    for (i, h) in HELP.iter().enumerate() {
        s.put(PANEL_X, 18 + i as u16, h, gray);
    }

    let inv = Style { fg: None, bold: true, reverse: true };
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

fn run(out: &mut impl Write) -> io::Result<()> {
    let mut g = Game::new();
    let mut s = Screen::new(60, 28);
    render(&g, &mut s, out)?;
    loop {
        let mut dirty = false;
        while event::poll(Duration::from_millis(16))? {
            match event::read()? {
                // Windows reports both press and release; act on press/repeat only.
                Event::Key(k) if k.kind != KeyEventKind::Release => {
                    if g.handle_key(k) {
                        return Ok(());
                    }
                    dirty = true;
                }
                Event::Resize(..) => {
                    queue!(out, terminal::Clear(ClearType::All))?;
                    dirty = true;
                }
                _ => {}
            }
        }
        if !g.over && !g.paused && g.last_fall.elapsed() >= g.interval() {
            g.gravity();
            dirty = true;
        }
        if dirty {
            render(&g, &mut s, out)?;
        }
    }
}

fn main() -> io::Result<()> {
    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide, terminal::Clear(ClearType::All))?;
    let res = run(&mut out);
    execute!(out, SetAttribute(Attribute::Reset), cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    res
}
