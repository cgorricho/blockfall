// BLOCKFALL ROCKER — the BLOCKFALL pieces on a rocking base. Rust + macroquad.
// Rules: ../SPEC.md (pieces, rotation, kicks, PRNG, scoring). Physics: physics.rs.
#![windows_subsystem = "windows"]

mod game;
mod physics;

use game::{BASE, Cause, Game, Mode, PIECES, State};
use macroquad::prelude::*;
use physics::{V2, v2};

// ---------- command line ----------

struct Args {
    seed: u32,
    mode: Mode,
    ai: bool,
    bench: Option<i64>,
}

fn parse_args() -> Args {
    let mut a = Args { seed: 0, mode: Mode::Easy, ai: false, bench: None };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--advanced" | "--hard" => a.mode = Mode::Advanced,
            "--easy" => a.mode = Mode::Easy,
            "--ai" => a.ai = true,
            "--seed" if i + 1 < argv.len() => {
                i += 1;
                a.seed = argv[i].parse().unwrap_or(0);
            }
            "--bench" if i + 1 < argv.len() => {
                i += 1;
                a.bench = argv[i].parse().ok();
            }
            _ => {}
        }
        i += 1;
    }
    if a.seed == 0 {
        a.seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u32)
            .unwrap_or(1);
    }
    a
}

/// Headless: the AI places N pieces, the base settles for 1 s after each.
fn run_bench(seed: u32, mode: Mode, n: i64) {
    let t0 = std::time::Instant::now();
    let mut g = Game::new(seed, mode, true);
    let (mut slid, mut topped, mut dropped, mut worst) = (0, 0, 0, 0.0f64);
    while g.pieces < n {
        g.ai_place_now();
        g.settle(1.0);
        if g.state != State::Playing {
            match g.cause {
                Cause::SlidOff(_) | Cause::Toppled(_) => slid += 1,
                Cause::ToppedOut => topped += 1,
                Cause::LostPieces => dropped += 1,
            }
            g.reset();
        }
        worst = worst.max(g.max_tilt);
    }
    // stdout may be detached in a windows-subsystem build; also write a file
    let line = format!(
        "lang=rocker mode={:?} seed={} pieces={} lines={} games={} slid={} topped={} lost_out={} lost={} max_tilt={:.1} ms={}",
        mode,
        seed,
        g.pieces,
        g.total_lines,
        g.games,
        slid,
        topped,
        dropped,
        g.total_lost,
        worst.to_degrees(),
        t0.elapsed().as_millis()
    );
    println!("{line}");
    let _ = std::fs::write(std::env::temp_dir().join("blockfall-rocker-bench.txt"), &line);
}

// ---------- drawing ----------

fn xterm(c: u8) -> Color {
    let (r, g, b) = match c {
        175 => (215, 135, 175), // I pink
        75 => (95, 175, 255),   // O sky
        179 => (215, 175, 95),  // T sand
        141 => (175, 135, 255), // S lavender
        73 => (95, 175, 175),   // Z teal
        107 => (135, 175, 95),  // J olive
        167 => (215, 95, 95),   // L brick
        _ => (200, 200, 200),
    };
    Color::from_rgba(r, g, b, 255)
}

fn kind_color(v: u8) -> Color {
    xterm(PIECES[(v - 1) as usize].color)
}

const BG: Color = Color::new(0.07, 0.08, 0.11, 1.0);
const PANEL_BG: Color = Color::new(0.10, 0.11, 0.15, 1.0);
const WOOD: Color = Color::new(0.55, 0.36, 0.18, 1.0);
const WOOD_EDGE: Color = Color::new(0.80, 0.58, 0.32, 1.0);
const DIM: Color = Color::new(0.55, 0.58, 0.65, 1.0);
const TEXT: Color = Color::new(0.93, 0.94, 0.96, 1.0);
/// The ghost is a neutral outline, never the piece colour.
const GHOST: Color = Color::new(0.85, 0.87, 0.92, 0.6);
const DANGER: Color = Color::new(1.0, 0.33, 0.30, 1.0);
const OK: Color = Color::new(0.35, 0.85, 0.45, 1.0);

/// World → screen mapping.
struct View {
    scale: f32,
    ox: f32,
    floor: f32,
}

impl View {
    fn pt(&self, w: V2) -> Vec2 {
        vec2(self.ox + w.x as f32 * self.scale, self.floor - w.y as f32 * self.scale)
    }
    fn world_x(&self, sx: f32) -> f64 {
        ((sx - self.ox) / self.scale) as f64
    }
}

fn quad(a: Vec2, b: Vec2, c: Vec2, d: Vec2, col: Color) {
    draw_triangle(a, b, c, col);
    draw_triangle(a, c, d, col);
}

fn outline(pts: &[Vec2], th: f32, col: Color) {
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        draw_line(a.x, a.y, b.x, b.y, th, col);
    }
}

/// Corners of a square of side `s` centred at `c`, turned by `a` (body frame).
fn square(c: V2, s: f64, a: f64) -> [V2; 4] {
    let h = s / 2.0;
    [v2(-h, -h), v2(h, -h), v2(h, h), v2(-h, h)].map(|o| c.add(o.rot(a)))
}

/// A block of the well (square or diamond), in screen coordinates.
fn block_on_base(g: &Game, v: &View, c: game::Pt, s: f64) -> [Vec2; 4] {
    let geo = g.geo();
    square(geo.center(c), s, geo.cell_angle()).map(|b| v.pt(g.rocker.to_world(b)))
}

fn draw_block(pts: [Vec2; 4], col: Color) {
    quad(pts[0], pts[1], pts[2], pts[3], col);
    let dark = Color::new(col.r * 0.55, col.g * 0.55, col.b * 0.55, 1.0);
    outline(&pts, 1.5, dark);
}

fn draw_base(g: &Game, v: &View) {
    let geo = g.geo();
    let r = geo.radius;
    let w = |b: V2| v.pt(g.rocker.to_world(b));
    // outline: the arc from -135° to -45°, then the top surface back from right to left
    let mut body = Vec::new();
    let steps = 48;
    for i in 0..=steps {
        let a = (-135.0f64 + 90.0 * i as f64 / steps as f64).to_radians();
        body.push(v2(r * a.cos(), r * a.sin()));
    }
    // fill the segment as a fan from a point inside it
    let c = w(v2(0.0, geo.chord_y - 0.6));
    let arc: Vec<Vec2> = body.iter().map(|&b| w(b)).collect();
    for i in 0..arc.len() {
        draw_triangle(c, arc[i], arc[(i + 1) % arc.len()], WOOD);
    }
    // hacksaw teeth: one triangle per tooth segment between the profile and the chord
    let mut top = vec![v2(geo.half_w, geo.chord_y)];
    if geo.teeth_h > 0.0 {
        let step = std::f64::consts::SQRT_2; // one flank
        let n = (2.0 * geo.half_w / step).round() as i32;
        let pts: Vec<V2> = (0..=n)
            .map(|i| {
                let x = geo.half_w - i as f64 * step;
                v2(x, geo.surface(x))
            })
            .collect();
        for pair in pts.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            quad(w(v2(a.x, geo.chord_y)), w(a), w(b), w(v2(b.x, geo.chord_y)), WOOD);
        }
        top = pts;
    } else {
        top.push(v2(-geo.half_w, geo.chord_y));
    }
    // edge: arc then the top profile
    let mut edge = arc;
    edge.extend(top.iter().map(|&b| w(b)));
    outline(&edge, 2.5, WOOD_EDGE);
}

fn draw_well(g: &Game, v: &View) {
    let geo = g.geo();
    let rail = Color::new(0.35, 0.38, 0.45, 0.8);
    if geo.walls() {
        for x in [-geo.well_half_w, geo.well_half_w] {
            let a = v.pt(g.rocker.to_world(v2(x, geo.surface(x))));
            let b = v.pt(g.rocker.to_world(v2(x, geo.well_top)));
            draw_line(a.x, a.y, b.x, b.y, 2.0, rail);
        }
    }
    // faint dots at empty cell centres over the base (hard mode has open sides)
    let dot = Color::new(0.3, 0.32, 0.38, 0.6);
    for (c, val) in g.board.cells() {
        if val == 0 && geo.over_base(c) {
            let p = v.pt(g.rocker.to_world(geo.center(c)));
            draw_circle(p.x, p.y, 1.5, dot);
        }
    }
}

fn draw_cells(g: &Game, v: &View) {
    for (c, k) in g.board.cells() {
        if k != 0 && k != BASE {
            draw_block(block_on_base(g, v, c, 0.96), kind_color(k));
        }
    }
    if g.state != State::Playing {
        return;
    }
    let col = xterm(PIECES[g.cur.kind].color);
    // ghost: where it will rest; red cross-outline if it would fall off the side
    match g.landing() {
        Some(ghost) => {
            for c in g.cells_of(&g.cur.spec, ghost) {
                outline(&block_on_base(g, v, c, 0.9), 1.5, GHOST);
            }
        }
        None => {
            for c in g.cells_of(&g.cur.spec, g.cur.origin) {
                outline(&block_on_base(g, v, c, 1.15), 2.0, DANGER);
            }
        }
    }
    for c in g.cells_of(&g.cur.spec, g.cur.origin) {
        draw_block(block_on_base(g, v, c, 0.96), col);
    }
}

fn draw_debris(g: &Game, v: &View) {
    for d in &g.debris {
        draw_block(square(d.pos, 0.96, d.angle).map(|w| v.pt(w)), kind_color(d.kind));
    }
}

/// Centre of curvature, centre of mass and the plumb line to the floor.
fn draw_physics_markers(g: &Game, v: &View) {
    let c = v.pt(g.rocker.center());
    draw_circle_lines(c.x, c.y, 5.0, 1.5, DIM);
    draw_text("C", c.x + 7.0, c.y - 5.0, 18.0, DIM);

    let com = v.pt(g.rocker.to_world(g.rocker.mass.com()));
    let stable = g.rocker.mass.com().y < 0.0;
    let col = if stable { OK } else { DANGER };
    let mut y = com.y;
    while y < v.floor {
        let y2 = (y + 6.0).min(v.floor);
        draw_line(com.x, y, com.x, y2, 1.5, Color::new(col.r, col.g, col.b, 0.7));
        y += 12.0;
    }
    draw_circle(com.x, com.y, 6.0, col);
    draw_circle(com.x, com.y, 3.0, BG);
    let p = v.pt(g.rocker.contact());
    draw_triangle(vec2(p.x, p.y), vec2(p.x - 7.0, p.y + 10.0), vec2(p.x + 7.0, p.y + 10.0), WOOD_EDGE);
}

fn draw_floor(v: &View, w: f32) {
    draw_rectangle(0.0, v.floor, w, screen_height() - v.floor, Color::new(0.12, 0.13, 0.17, 1.0));
    draw_line(0.0, v.floor, w, v.floor, 2.0, Color::new(0.4, 0.42, 0.5, 1.0));
    let mut x = -20.0;
    while x < w {
        draw_line(x, v.floor + 18.0, x + 18.0, v.floor, 1.0, Color::new(0.25, 0.27, 0.33, 1.0));
        x += 18.0;
    }
}

/// Semicircular tilt gauge: needle = tilt now, tick = resting tilt, red = slide zone.
fn draw_gauge(g: &Game, cx: f32, cy: f32, r: f32) {
    let limit = g.break_angle() as f32;
    let span = 60f32.to_radians(); // gauge shows ±60°
    let to_pt = |a: f32, rr: f32| vec2(cx + rr * (-a).sin(), cy - rr * (-a).cos());
    let steps = 40;
    for i in 0..steps {
        let a0 = -span + 2.0 * span * i as f32 / steps as f32;
        let a1 = -span + 2.0 * span * (i + 1) as f32 / steps as f32;
        let col = if a0.abs() >= limit - 1e-3 && a1.abs() >= limit - 1e-3 { DANGER } else { DIM };
        let (p, q) = (to_pt(a0, r), to_pt(a1, r));
        draw_line(p.x, p.y, q.x, q.y, 4.0, col);
    }
    if let Some(eq) = g.equilibrium() {
        let a = (eq as f32).clamp(-span, span);
        let (p, q) = (to_pt(a, r - 10.0), to_pt(a, r + 6.0));
        draw_line(p.x, p.y, q.x, q.y, 3.0, OK);
    }
    let a = (g.rocker.theta as f32).clamp(-span, span);
    let tip = to_pt(a, r - 4.0);
    let col = if g.rocker.theta.abs() > g.break_angle() * 0.75 { DANGER } else { TEXT };
    draw_line(cx, cy, tip.x, tip.y, 3.0, col);
    draw_circle(cx, cy, 5.0, col);
}

fn draw_panel(g: &Game, x: f32) {
    draw_rectangle(x - 10.0, 0.0, screen_width() - x + 10.0, screen_height(), PANEL_BG);
    let mut y = 50.0;
    draw_text("BLOCKFALL ROCKER", x, y, 32.0, TEXT);
    draw_text(&format!("seed {}", g.seed), x + 270.0, y - 26.0, 16.0, DIM);
    y += 26.0;
    let mode = match g.mode {
        Mode::Easy => "EASY  (flat top)",
        Mode::Advanced => "HARD  (45° hacksaw)",
    };
    draw_text(mode, x, y, 20.0, DIM);
    if g.ai {
        draw_text("AI PLAYING", x + 270.0, y, 20.0, OK);
    }

    y += 40.0;
    draw_text("ON DECK", x, y, 22.0, DIM);
    let d = &PIECES[g.next];
    let s = 20.0f64;
    let angle = g.geo().cell_angle();
    let (cx, cy) = (x as f64 + 140.0, y as f64 + 45.0);
    draw_rectangle_lines(cx as f32 - 45.0, cy as f32 - 45.0, 90.0, 90.0, 1.5, DIM);
    let mid = (d.n as f64 - 1.0) / 2.0;
    for c in d.cells {
        // screen y grows downward: turning counter-clockwise on screen is -angle
        let off = v2(c.x as f64 - mid, c.y as f64 - mid).rot(-angle).scale(s);
        let pts = square(v2(cx + off.x, cy + off.y), s - 2.0, -angle).map(|p| vec2(p.x as f32, p.y as f32));
        quad(pts[0], pts[1], pts[2], pts[3], xterm(d.color));
    }

    y += 110.0;
    let mut stats = vec![
        ("SCORE", g.score.to_string(), TEXT),
        ("LINES", g.lines.to_string(), TEXT),
        ("LEVEL", g.level().to_string(), TEXT),
    ];
    if !g.geo().walls() {
        let col = if g.lost > 0 { DANGER } else { TEXT };
        stats.push(("LOST", format!("{} / {}", g.lost, game::MAX_LOST), col));
    }
    for (label, val, col) in stats {
        draw_text(label, x, y, 22.0, DIM);
        draw_text(&val, x + 110.0, y, 26.0, col);
        y += 32.0;
    }

    y += 20.0;
    draw_text("TILT", x, y, 22.0, DIM);
    let side = |t: f64| if t > 0.0 { "left down" } else { "right down" };
    let tilt = g.rocker.theta;
    let now = if tilt.to_degrees().abs() < 0.05 {
        "level".to_string()
    } else {
        format!("{:.1}°  {}", tilt.to_degrees().abs(), side(tilt))
    };
    draw_text(&now, x + 110.0, y, 24.0, TEXT);
    y += 28.0;
    let rest = match g.equilibrium() {
        Some(e) if e.to_degrees().abs() < 0.05 => "rests level".to_string(),
        Some(e) => format!("rests at {:.1}° {}", e.to_degrees().abs(), side(e)),
        None => "TOP-HEAVY: will tip".to_string(),
    };
    let rest_col = if g.equilibrium().map_or(true, |e| e.abs() > g.break_angle()) { DANGER } else { OK };
    draw_text(&rest, x + 110.0, y, 20.0, rest_col);
    y += 95.0;
    draw_gauge(g, x + 150.0, y, 75.0);
    let limit_text = if g.geo().walls() { "slides off at" } else { "topples at" };
    draw_text(&format!("{limit_text} ±{:.0}°", g.break_angle().to_degrees()), x + 85.0, y + 25.0, 18.0, DANGER);

    y += 60.0;
    let margin = -g.rocker.mass.com().y; // how far the COM is below C
    draw_text("BALANCE", x, y, 22.0, DIM);
    draw_text(&format!("COM {:.2} below C", margin), x + 110.0, y, 20.0, if margin > 0.0 { OK } else { DANGER });
    y += 22.0;
    draw_text(&format!("load {} blocks", g.board.cells().filter(|&(_, v)| v != 0 && v != BASE).count()), x + 110.0, y, 18.0, DIM);

    y += 40.0;
    let help = [
        "<- ->   move",
        "UP / X  rotate     Z  rotate ccw",
        "DOWN    soft drop  SPACE  hard drop",
        "P pause   R restart   M easy/hard",
        "I  AI on/off   Q / Esc  quit",
    ];
    for h in help {
        draw_text(h, x, y, 18.0, DIM);
        y += 22.0;
    }
}

fn draw_overlay(g: &Game, cx: f32, cy: f32) {
    let (title, sub) = match g.state {
        State::Over => {
            let why = match g.cause {
                Cause::SlidOff(a) => format!("the stack slid off at {:.0}° tilt", a.to_degrees()),
                Cause::Toppled(_) => "the rocker toppled over".to_string(),
                Cause::ToppedOut if g.geo().walls() => "the well is full".to_string(),
                Cause::ToppedOut => "no room for the next piece".to_string(),
                Cause::LostPieces => format!("{} pieces fell off the side", game::MAX_LOST),
            };
            ("GAME OVER", format!("{why}  -  R restart"))
        }
        _ if g.paused => ("PAUSED", "P to continue".to_string()),
        _ => return,
    };
    let tw = measure_text(title, None, 56, 1.0).width;
    let sw = measure_text(&sub, None, 24, 1.0).width;
    let w = tw.max(sw) + 60.0;
    draw_rectangle(cx - w / 2.0, cy - 60.0, w, 100.0, Color::new(0.0, 0.0, 0.0, 0.75));
    draw_text(title, cx - tw / 2.0, cy, 56.0, TEXT);
    draw_text(&sub, cx - sw / 2.0, cy + 30.0, 24.0, DIM);
}

// ---------- input ----------

/// Auto-repeat for held keys: act on press, then after 170 ms every 50 ms.
#[derive(Default)]
struct Repeat {
    held: f32,
    next: f32,
}

impl Repeat {
    /// `pressed` catches taps shorter than a frame (down and up between two frames).
    fn fire(&mut self, pressed: bool, down: bool, dt: f32) -> bool {
        if pressed {
            self.held = 1e-4;
            self.next = 0.17;
            return true;
        }
        if !down || self.held == 0.0 {
            self.held = 0.0;
            return false;
        }
        self.held += dt;
        if self.held >= self.next {
            self.next += 0.05;
            return true;
        }
        false
    }
}

fn conf() -> Conf {
    Conf {
        window_title: "Blockfall Rocker".to_owned(),
        window_width: 1280,
        window_height: 820,
        window_resizable: true,
        sample_count: 4,
        ..Default::default()
    }
}

async fn run(args: Args) {
    let mut g = Game::new(args.seed, args.mode, args.ai);
    let (mut left, mut right, mut down) = (Repeat::default(), Repeat::default(), Repeat::default());
    loop {
        let dt = get_frame_time().min(0.05);
        let any = |ks: &[KeyCode]| ks.iter().any(|&k| is_key_down(k));
        let pressed = |ks: &[KeyCode]| ks.iter().any(|&k| is_key_pressed(k));

        if pressed(&[KeyCode::Q, KeyCode::Escape]) {
            break;
        }
        if pressed(&[KeyCode::R]) {
            g.reset();
        }
        if pressed(&[KeyCode::M]) {
            let m = if g.mode == Mode::Easy { Mode::Advanced } else { Mode::Easy };
            g.set_mode(m);
        }
        if pressed(&[KeyCode::I]) {
            g.ai = !g.ai;
            g.reset();
        }
        if pressed(&[KeyCode::P]) && g.state == State::Playing {
            g.paused = !g.paused;
        }
        let human = !g.ai && !g.paused && g.state == State::Playing;
        let (lk, rk, dk) = ([KeyCode::Left, KeyCode::A], [KeyCode::Right, KeyCode::D], [KeyCode::Down, KeyCode::S]);
        let l = left.fire(pressed(&lk), any(&lk), dt);
        let r = right.fire(pressed(&rk), any(&rk), dt);
        let d = down.fire(pressed(&dk), any(&dk), dt);
        if human {
            if l {
                g.move_left();
            }
            if r {
                g.move_right();
            }
            if d {
                g.soft_drop();
            }
            if pressed(&[KeyCode::Up, KeyCode::W, KeyCode::X]) {
                g.try_rotate(true);
            }
            if pressed(&[KeyCode::Z]) {
                g.try_rotate(false);
            }
            if pressed(&[KeyCode::Space]) {
                g.hard_drop();
            }
        }

        // layout: play area left, panel right
        let (sw, sh) = (screen_width(), screen_height());
        let panel_w = 420.0;
        let play_w = (sw - panel_w).max(300.0);
        let scale = (play_w / 34.0).min((sh - 40.0) / 23.5);
        let view = View { scale, ox: play_w / 2.0, floor: sh - 30.0 };
        // debris bounce off the edges of what is visible
        g.arena = (view.world_x(0.0), view.world_x(play_w));
        g.update(dt as f64);

        clear_background(BG);
        draw_floor(&view, play_w);
        draw_well(&g, &view);
        draw_base(&g, &view);
        draw_cells(&g, &view);
        draw_debris(&g, &view);
        draw_physics_markers(&g, &view);
        draw_panel(&g, play_w + 10.0);
        draw_overlay(&g, play_w / 2.0, sh * 0.35);
        next_frame().await;
    }
}

fn main() {
    let args = parse_args();
    if let Some(n) = args.bench {
        run_bench(args.seed, args.mode, n);
        return;
    }
    macroquad::Window::from_config(conf(), run(args));
}
