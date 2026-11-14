// BLOCKFALL TOWER-3D — BLOCKFALL wrapped around a vertical cylinder, real 3D
// with an orthographic camera. Rules: game.rs and README.md. Rust + macroquad.
#![windows_subsystem = "windows"]

mod game;

use game::{Game, MODES, PIECES, ROWS, State};
use macroquad::models::Vertex;
use macroquad::prelude::*;
use std::f32::consts::TAU;

// ---------- geometry ----------

/// Ring geometry for a given number of columns.
struct Ring {
    cols: i32,
    /// Angle of one column.
    dtheta: f32,
    r_in: f32,
    r_out: f32,
}

impl Ring {
    fn new(cols: i32) -> Ring {
        // mid radius chosen so a cell is 1 unit wide at the middle of the wall
        let r_mid = cols as f32 / TAU;
        Ring { cols, dtheta: TAU / cols as f32, r_in: r_mid - 0.5, r_out: r_mid + 0.5 }
    }

    /// Signed column distance from `focus` to `col`, the short way round.
    fn delta(&self, col: f32, focus: f32) -> f32 {
        let n = self.cols as f32;
        let mut d = (col - focus).rem_euclid(n);
        if d > n / 2.0 {
            d -= n;
        }
        d
    }

    /// Angle of board column `col` (its centre) when column `focus` faces the camera.
    fn angle(&self, col: f32, focus: f32) -> f32 {
        self.delta(col, focus) * self.dtheta
    }
}

/// World position: angle `th` (0 = facing the camera, + = to the screen
/// right), radius `r`, height `h`. World Y is up (the game's z axis).
fn pos(th: f32, r: f32, h: f32) -> Vec3 {
    vec3(r * th.sin(), h, r * th.cos())
}

fn light_dir() -> Vec3 {
    vec3(-0.45, 0.8, 0.55).normalize()
}

fn shade(c: Color, n: Vec3) -> Color {
    let f = 0.38 + 0.62 * n.dot(light_dir()).max(0.0);
    Color::new(c.r * f, c.g * f, c.b * f, c.a)
}

fn to_u8(c: Color) -> [u8; 4] {
    [(c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8, (c.a * 255.0) as u8]
}

/// Arc segments per brick on the curved faces.
const SEG: usize = 6;

fn vert(p: Vec3, c: Color) -> Vertex {
    Vertex { position: p, uv: Vec2::ZERO, color: to_u8(c), normal: Vec4::ZERO }
}

/// A wedge brick: the annular sector between angles th0..th1, radii ri..ro,
/// heights h0..h1. The inner and outer faces are true cylinder sections,
/// smooth-shaded (per-vertex normals); top, bottom and sides are flat.
fn wedge(th0: f32, th1: f32, ri: f32, ro: f32, h0: f32, h1: f32, col: Color, edge: Option<Color>) {
    let p = |t, r, h| pos(t, r, h);
    let ang = |k: usize| th0 + (th1 - th0) * k as f32 / SEG as f32;
    let radial = |t: f32| vec3(t.sin(), 0.0, t.cos());
    let tan = |t: f32| vec3(t.cos(), 0.0, -t.sin());
    let mut v: Vec<Vertex> = Vec::with_capacity(8 * (SEG + 1) + 8);
    let mut ix: Vec<u16> = Vec::with_capacity(24 * SEG + 12);
    // strip between two rows of SEG+1 vertices starting at a and b
    let strip = |ix: &mut Vec<u16>, a: u16, b: u16| {
        for k in 0..SEG as u16 {
            ix.extend_from_slice(&[a + k, a + k + 1, b + k + 1, a + k, b + k + 1, b + k]);
        }
    };
    // outer and inner cylinder faces, normals follow the curve
    for (r, sign) in [(ro, 1.0), (ri, -1.0)] {
        let a = v.len() as u16;
        for k in 0..=SEG {
            let t = ang(k);
            v.push(vert(p(t, r, h0), shade(col, radial(t) * sign)));
        }
        let b = v.len() as u16;
        for k in 0..=SEG {
            let t = ang(k);
            v.push(vert(p(t, r, h1), shade(col, radial(t) * sign)));
        }
        strip(&mut ix, a, b);
    }
    // top and bottom: curved annular strips, flat normals
    for (h, n) in [(h1, Vec3::Y), (h0, -Vec3::Y)] {
        let c = shade(col, n);
        let a = v.len() as u16;
        for k in 0..=SEG {
            v.push(vert(p(ang(k), ri, h), c));
        }
        let b = v.len() as u16;
        for k in 0..=SEG {
            v.push(vert(p(ang(k), ro, h), c));
        }
        strip(&mut ix, a, b);
    }
    // the two flat sides
    for (t, n) in [(th0, -tan(th0)), (th1, tan(th1))] {
        let c = shade(col, n);
        let a = v.len() as u16;
        for q in [p(t, ri, h0), p(t, ro, h0), p(t, ro, h1), p(t, ri, h1)] {
            v.push(vert(q, c));
        }
        ix.extend_from_slice(&[a, a + 1, a + 2, a, a + 2, a + 3]);
    }
    draw_mesh(&Mesh { vertices: v, indices: ix, texture: None });
    if let Some(e) = edge {
        for t in [th0, th1] {
            draw_line_3d(p(t, ri, h0), p(t, ri, h1), e);
            draw_line_3d(p(t, ro, h0), p(t, ro, h1), e);
            for h in [h0, h1] {
                draw_line_3d(p(t, ri, h), p(t, ro, h), e);
            }
        }
        for h in [h0, h1] {
            for r in [ri, ro] {
                for k in 0..SEG {
                    draw_line_3d(p(ang(k), r, h), p(ang(k + 1), r, h), e);
                }
            }
        }
    }
}

/// Height of the bottom of board row `row`.
fn row_h(row: i32) -> f32 {
    (ROWS - 1 - row) as f32
}

fn cell(ring: &Ring, col: i32, row: i32, focus: f32, col_: Color, edge: Option<Color>) {
    let t = ring.angle(col as f32, focus);
    let h = row_h(row);
    let half = ring.dtheta / 2.0;
    wedge(t - half, t + half, ring.r_in, ring.r_out, h, h + 1.0, col_, edge);
}

// ---------- colours ----------

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

fn kind_color(kind: usize) -> Color {
    xterm(PIECES[kind].color)
}

fn darker(c: Color, f: f32) -> Color {
    Color::new(c.r * f, c.g * f, c.b * f, 1.0)
}

const BG: Color = Color::new(0.07, 0.08, 0.11, 1.0);
const PANEL_BG: Color = Color::new(0.10, 0.11, 0.15, 1.0);
const STONE: Color = Color::new(0.42, 0.44, 0.50, 1.0);
const GRID: Color = Color::new(0.25, 0.28, 0.36, 1.0);
const DIM: Color = Color::new(0.55, 0.58, 0.65, 1.0);
const TEXT: Color = Color::new(0.93, 0.94, 0.96, 1.0);
/// The ghost is a neutral outline, never the piece colour.
const GHOST: Color = Color::new(0.85, 0.87, 0.92, 0.6);

// ---------- 3D scene ----------

fn draw_tower(g: &Game, v: &View) {
    let ring = Ring::new(g.cols);
    let (focus, dt) = (v.focus, ring.dtheta);
    // plinth the tower stands on
    for c in 0..ring.cols {
        let t = ring.angle(c as f32, focus);
        wedge(t - dt / 2.0, t + dt / 2.0, ring.r_in - 0.35, ring.r_out + 0.35, -0.5, 0.0, STONE, Some(darker(STONE, 0.6)));
    }
    // optional wire frame of the empty wall (G): column lines and the top rings
    if v.frame {
        let top = ROWS as f32;
        let (ri, ro) = (ring.r_in, ring.r_out);
        for c in 0..ring.cols {
            let t = ring.angle(c as f32, focus) - dt / 2.0;
            draw_line_3d(pos(t, ro, 0.0), pos(t, ro, top), GRID);
            draw_line_3d(pos(t, ri, 0.0), pos(t, ri, top), Color { a: 0.5, ..GRID });
            for r in [ri, ro] {
                draw_line_3d(pos(t, r, top), pos(t + dt, r, top), GRID);
            }
            draw_line_3d(pos(t, ri, top), pos(t, ro, top), GRID);
        }
    }
    // locked blocks
    for row in 0..ROWS {
        for col in 0..ring.cols {
            let v = g.get(col, row);
            if v != 0 {
                let c = kind_color((v - 1) as usize);
                cell(&ring, col, row, focus, c, Some(darker(c, 0.35)));
            }
        }
    }
    if g.state == State::Playing {
        let c = kind_color(g.cur.kind);
        for (col, row) in g.cur.board_cells(g.cur.y, g.cols) {
            cell(&ring, col, row, focus, c, Some(darker(c, 0.35)));
        }
        // ghost last: a neutral wire outline
        let gy = g.ghost_y();
        if gy != g.cur.y {
            for (col, row) in g.cur.board_cells(gy, g.cols) {
                cell(&ring, col, row, focus, Color { a: 0.0, ..GHOST }, Some(GHOST));
            }
        }
    }
}

struct View {
    /// Camera elevation above horizontal, degrees.
    elev: f32,
    /// Column shown at the front; eases toward the piece for smooth turning.
    focus: f32,
    /// Wire frame of the empty wall on/off.
    frame: bool,
}

/// How fast the displayed tower catches up with the piece (1/s).
const TURN_RATE: f32 = 22.0;

impl View {
    /// Ease the displayed rotation toward the piece, the short way round.
    fn follow(&mut self, g: &Game, dt: f32) {
        let ring = Ring::new(g.cols);
        let d = ring.delta(g.cur.focus(), self.focus);
        if d.abs() < 0.002 {
            self.focus = g.cur.focus();
        } else {
            self.focus += d * (1.0 - (-TURN_RATE * dt).exp());
        }
        self.focus = self.focus.rem_euclid(g.cols as f32);
    }
}

fn set_tower_camera(g: &Game, v: &View, panel_w: f32) {
    let ring = Ring::new(g.cols);
    let (sw, sh) = (screen_width(), screen_height());
    let e = v.elev.to_radians();
    // visible height of the tower in the ortho view, plus a margin
    let fovy = (ROWS as f32 + 0.5) * e.cos() + 2.0 * (ring.r_out + 0.35) * e.sin() + 2.5;
    let px = fovy / sh; // world units per pixel
    let target = vec3(panel_w / 2.0 * px, ROWS as f32 / 2.0 - 0.25, 0.0);
    let dir = vec3(0.0, e.sin(), e.cos());
    set_camera(&Camera3D {
        position: target + dir * 60.0,
        target,
        up: Vec3::Y,
        fovy,
        aspect: Some(sw / sh),
        projection: Projection::Orthographics,
        ..Default::default()
    });
}

// ---------- 2D panel ----------

fn draw_panel(g: &Game, v: &View, x: f32) {
    let sh = screen_height();
    draw_rectangle(x - 10.0, 0.0, screen_width() - x + 10.0, sh, PANEL_BG);
    let mut y = 50.0;
    draw_text("BLOCKFALL", x, y, 34.0, TEXT);
    y += 30.0;
    draw_text("TOWER-3D", x, y, 26.0, DIM);
    draw_text(&format!("seed {}", g.seed), x + 150.0, y, 16.0, DIM);

    y += 45.0;
    draw_text("ON DECK", x, y, 22.0, DIM);
    let d = &PIECES[g.next];
    let s = 22.0;
    let c = kind_color(g.next);
    draw_rectangle_lines(x + 92.0, y - 22.0, 4.0 * s + 14.0, 2.0 * s + 14.0, 1.5, DIM);
    for &(cx, cy) in &d.cells {
        let (bx, by) = (x + 100.0 + cx as f32 * s, y - 14.0 + cy as f32 * s);
        draw_rectangle(bx, by, s - 2.0, s - 2.0, c);
        draw_rectangle_lines(bx, by, s - 2.0, s - 2.0, 2.0, darker(c, 0.4));
    }

    y += 90.0;
    for (label, val) in [("SCORE", g.score), ("LINES", g.lines), ("LEVEL", g.level()), ("PIECES", g.pieces)] {
        draw_text(label, x, y, 22.0, DIM);
        draw_text(&val.to_string(), x + 110.0, y, 26.0, TEXT);
        y += 34.0;
    }
    draw_text("RING", x, y, 22.0, DIM);
    draw_text(&format!("{} bricks", g.cols), x + 110.0, y, 24.0, TEXT);
    y += 34.0;
    draw_text("VIEW", x, y, 22.0, DIM);
    draw_text(&format!("{:.0} deg", v.elev), x + 110.0, y, 24.0, TEXT);

    y += 50.0;
    for h in [
        "<- A     turn tower CW",
        "-> D     turn tower CCW",
        "Up W X   rotate",
        "Z        rotate ccw",
        "Down S   soft drop",
        "Space    hard drop",
        "PgUp/Dn  tilt view",
        "1 2 3    ring 16/24/32",
        "G        frame on/off",
        "P        pause",
        "R        restart",
        "Q Esc    quit",
    ] {
        draw_text(h, x, y, 19.0, DIM);
        y += 24.0;
    }
}

fn draw_overlay(g: &Game, cx: f32, cy: f32) {
    let (title, sub) = match (g.state, g.paused) {
        (State::Over, _) => ("GAME OVER", "R = restart"),
        (_, true) => ("PAUSED", "P = resume"),
        _ => return,
    };
    let tw = measure_text(title, None, 56, 1.0).width;
    let sw = measure_text(sub, None, 24, 1.0).width;
    draw_rectangle(cx - tw / 2.0 - 20.0, cy - 50.0, tw + 40.0, 95.0, Color::new(0.0, 0.0, 0.0, 0.7));
    draw_text(title, cx - tw / 2.0, cy, 56.0, TEXT);
    draw_text(sub, cx - sw / 2.0, cy + 30.0, 24.0, DIM);
}

// ---------- input ----------

/// Auto-repeat for held keys: act on press, then after 170 ms every 50 ms.
#[derive(Default)]
struct Repeat {
    held: f32,
    next: f32,
}

impl Repeat {
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

/// `--seed N` and `--ring 16|24|32`.
fn parse_args() -> (u32, i32) {
    let argv: Vec<String> = std::env::args().collect();
    let val = |flag: &str| argv.iter().position(|a| a == flag).and_then(|i| argv.get(i + 1)).and_then(|s| s.parse::<u32>().ok());
    let seed = val("--seed").unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u32).unwrap_or(1)
    });
    let ring = val("--ring").map(|r| r as i32).filter(|r| MODES.contains(r)).unwrap_or(32);
    (seed, ring)
}

fn conf() -> Conf {
    Conf {
        window_title: "Blockfall Tower-3D".to_owned(),
        window_width: 1200,
        window_height: 860,
        window_resizable: true,
        sample_count: 4,
        ..Default::default()
    }
}

async fn run() {
    let (seed, ring) = parse_args();
    let mut g = Game::new(seed, ring);
    let mut view = View { elev: 30.0, focus: g.cur.focus(), frame: false };
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
        for (key, cols) in [(KeyCode::Key1, MODES[0]), (KeyCode::Key2, MODES[1]), (KeyCode::Key3, MODES[2])] {
            if pressed(&[key]) {
                g.set_cols(cols);
                view.focus = g.cur.focus(); // new ring: no turning animation
            }
        }
        if pressed(&[KeyCode::G]) {
            view.frame = !view.frame;
        }
        if pressed(&[KeyCode::P]) && g.state == State::Playing {
            g.paused = !g.paused;
        }
        if any(&[KeyCode::PageUp]) {
            view.elev = (view.elev + 60.0 * dt).min(85.0);
        }
        if any(&[KeyCode::PageDown]) {
            view.elev = (view.elev - 60.0 * dt).max(5.0);
        }
        let (lk, rk, dk) = ([KeyCode::Left, KeyCode::A], [KeyCode::Right, KeyCode::D], [KeyCode::Down, KeyCode::S]);
        if left.fire(pressed(&lk), any(&lk), dt) {
            g.turn_cw();
        }
        if right.fire(pressed(&rk), any(&rk), dt) {
            g.turn_ccw();
        }
        if down.fire(pressed(&dk), any(&dk), dt) {
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
        g.update(dt as f64);
        view.follow(&g, dt);

        let panel_w = 340.0;
        clear_background(BG);
        set_tower_camera(&g, &view, panel_w);
        draw_tower(&g, &view);
        set_default_camera();
        let sw = screen_width();
        draw_panel(&g, &view, sw - panel_w + 20.0);
        draw_overlay(&g, (sw - panel_w) / 2.0, screen_height() * 0.4);
        next_frame().await;
    }
}

fn main() {
    macroquad::Window::from_config(conf(), run());
}
