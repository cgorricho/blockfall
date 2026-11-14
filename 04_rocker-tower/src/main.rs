// BLOCKFALL ROCKER-TOWER — tower-3D's cylinder standing on a rocking
// spherical base, with Rocker's rolling physics taken to full 3D.
// Rules: game.rs, physics: physics.rs, docs: README.md. Rust + macroquad.
#![windows_subsystem = "windows"]

mod game;
mod physics;

use game::{Cause, DEBRIS_R, Game, Geo, MODES, PIECES, ROWS, State};
use macroquad::models::Vertex;
use macroquad::prelude::*;
use std::f32::consts::TAU;

// ---------- body → world transform ----------

/// The rocker's pose as f32 for drawing: world = c + m · body.
#[derive(Clone, Copy)]
struct Xf {
    m: Mat3,
    c: Vec3,
}

impl Xf {
    fn of(g: &Game) -> Xf {
        Xf { m: Mat3::from_quat(g.rocker.q.as_quat()), c: g.rocker.c.as_vec3() }
    }
    fn p(&self, b: Vec3) -> Vec3 {
        self.c + self.m * b
    }
    fn n(&self, b: Vec3) -> Vec3 {
        self.m * b
    }
}

/// Body point at angle `th` round the axis, radius `r`, height `h`.
fn cyl(th: f32, r: f32, h: f32) -> Vec3 {
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

fn vert(p: Vec3, c: Color) -> Vertex {
    Vertex { position: p, uv: Vec2::ZERO, color: to_u8(c), normal: Vec4::ZERO }
}

/// Arc segments per brick on the curved faces.
const SEG: usize = 6;

/// A wedge brick in the body frame: the annular sector th0..th1, radii ri..ro,
/// heights h0..h1. Curved faces smooth-shaded, the rest flat (as in tower-3D).
fn wedge(x: &Xf, th0: f32, th1: f32, ri: f32, ro: f32, h0: f32, h1: f32, col: Color, edge: Option<Color>) {
    let p = |t, r, h| x.p(cyl(t, r, h));
    let ang = |k: usize| th0 + (th1 - th0) * k as f32 / SEG as f32;
    let radial = |t: f32| x.n(vec3(t.sin(), 0.0, t.cos()));
    let tan = |t: f32| x.n(vec3(t.cos(), 0.0, -t.sin()));
    let up = x.n(Vec3::Y);
    let mut v: Vec<Vertex> = Vec::with_capacity(8 * (SEG + 1) + 8);
    let mut ix: Vec<u16> = Vec::with_capacity(24 * SEG + 12);
    let strip = |ix: &mut Vec<u16>, a: u16, b: u16| {
        for k in 0..SEG as u16 {
            ix.extend_from_slice(&[a + k, a + k + 1, b + k + 1, a + k, b + k + 1, b + k]);
        }
    };
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
    for (h, n) in [(h1, up), (h0, -up)] {
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

fn cell(x: &Xf, geo: &Geo, col: i32, row: i32, c: Color, edge: Option<Color>) {
    let dt = geo.dtheta as f32;
    let t = col as f32 * dt;
    let h = geo.row_y(row) as f32;
    wedge(x, t - dt / 2.0, t + dt / 2.0, geo.r_in as f32, geo.r_out as f32, h, h + 1.0, c, edge);
}

/// A loose brick (debris), drawn as a cube with side 2·DEBRIS_R.
fn debris_box(center: Vec3, m: Mat3, col: Color) {
    let s = DEBRIS_R as f32;
    let mut v = Vec::with_capacity(24);
    let mut ix: Vec<u16> = Vec::with_capacity(36);
    let edge = darker(col, 0.35);
    for (n, u, w) in [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (-Vec3::X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (-Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (-Vec3::Z, Vec3::Y, Vec3::X),
    ] {
        let c = shade(col, m * n);
        let a = v.len() as u16;
        let corners = [n - u - w, n + u - w, n + u + w, n - u + w].map(|k| center + m * (k * s));
        for q in corners {
            v.push(vert(q, c));
        }
        ix.extend_from_slice(&[a, a + 1, a + 2, a, a + 2, a + 3]);
        for k in 0..4 {
            draw_line_3d(corners[k], corners[(k + 1) % 4], edge);
        }
    }
    draw_mesh(&Mesh { vertices: v, indices: ix, texture: None });
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
const FLOOR: Color = Color::new(0.11, 0.12, 0.16, 1.0);
const PANEL_BG: Color = Color::new(0.10, 0.11, 0.15, 1.0);
const WOOD: Color = Color::new(0.62, 0.43, 0.25, 1.0);
const WOOD2: Color = Color::new(0.48, 0.31, 0.17, 1.0);
const WOOD_TOP: Color = Color::new(0.70, 0.52, 0.33, 1.0);
const GRID: Color = Color::new(0.25, 0.28, 0.36, 1.0);
const DIM: Color = Color::new(0.55, 0.58, 0.65, 1.0);
const TEXT: Color = Color::new(0.93, 0.94, 0.96, 1.0);
/// The ghost is a neutral outline, never the piece colour.
const GHOST: Color = Color::new(0.85, 0.87, 0.92, 0.6);
const WARN: Color = Color::new(1.0, 0.35, 0.3, 1.0);
const OK: Color = Color::new(0.45, 0.85, 0.5, 1.0);
const MARK: Color = Color::new(1.0, 0.9, 0.3, 1.0);

// ---------- 3D scene ----------

/// The wooden cap: 8 two-tone sectors (so its turning and rolling show), and
/// the flat top the tower stands on.
fn draw_base(x: &Xf, geo: &Geo) {
    const RINGS: usize = 10;
    const SECTORS: usize = 8;
    const PER: usize = 6;
    let rho = geo.rho as f32;
    let cap = physics::CAP_ANGLE as f32;
    let sph = |beta: f32, phi: f32| vec3(beta.sin() * phi.sin(), -beta.cos(), beta.sin() * phi.cos());
    for s in 0..SECTORS {
        let col = if s % 2 == 0 { WOOD } else { WOOD2 };
        let mut v = Vec::with_capacity((RINGS + 1) * (PER + 1));
        let mut ix: Vec<u16> = Vec::new();
        for i in 0..=RINGS {
            let beta = cap * i as f32 / RINGS as f32;
            for k in 0..=PER {
                let phi = TAU * (s * PER + k) as f32 / (SECTORS * PER) as f32;
                let n = sph(beta, phi);
                v.push(vert(x.p(n * rho), shade(col, x.n(n))));
            }
        }
        let w = (PER + 1) as u16;
        for i in 0..RINGS as u16 {
            for k in 0..PER as u16 {
                let a = i * w + k;
                ix.extend_from_slice(&[a, a + 1, a + w + 1, a, a + w + 1, a + w]);
            }
        }
        draw_mesh(&Mesh { vertices: v, indices: ix, texture: None });
    }
    // flat top
    let n = 48;
    let r = geo.r_out as f32;
    let top = geo.top as f32;
    let c = shade(WOOD_TOP, x.n(Vec3::Y));
    let mut v = vec![vert(x.p(vec3(0.0, top, 0.0)), c)];
    let mut ix: Vec<u16> = Vec::new();
    for k in 0..=n {
        v.push(vert(x.p(cyl(TAU * k as f32 / n as f32, r, top)), c));
    }
    for k in 1..=n as u16 {
        ix.extend_from_slice(&[0, k, k + 1]);
    }
    draw_mesh(&Mesh { vertices: v, indices: ix, texture: None });
    for k in 0..n {
        let (a, b) = (TAU * k as f32 / n as f32, TAU * (k + 1) as f32 / n as f32);
        draw_line_3d(x.p(cyl(a, r, top)), x.p(cyl(b, r, top)), darker(WOOD, 0.45));
    }
}

fn draw_floor(center: Vec3) {
    let (cx, cz) = (center.x.round(), center.z.round());
    draw_plane(vec3(cx, -0.01, cz), vec2(400.0, 400.0), None, FLOOR);
    const N: i32 = 60;
    for k in -N..=N {
        let (k, n) = (k as f32, N as f32);
        let c = Color { a: 0.45, ..GRID };
        draw_line_3d(vec3(cx + k, 0.0, cz - n), vec3(cx + k, 0.0, cz + n), c);
        draw_line_3d(vec3(cx - n, 0.0, cz + k), vec3(cx + n, 0.0, cz + k), c);
    }
}

fn draw_tower(g: &Game, v: &View) {
    let x = Xf::of(g);
    let geo = g.geo;
    let dt = geo.dtheta as f32;
    draw_base(&x, &geo);
    if v.frame {
        let (ri, ro) = (geo.r_in as f32, geo.r_out as f32);
        let (bot, top) = (geo.top as f32, geo.top as f32 + ROWS as f32);
        for c in 0..geo.cols {
            let t = c as f32 * dt - dt / 2.0;
            draw_line_3d(x.p(cyl(t, ro, bot)), x.p(cyl(t, ro, top)), GRID);
            draw_line_3d(x.p(cyl(t, ri, bot)), x.p(cyl(t, ri, top)), Color { a: 0.5, ..GRID });
            for r in [ri, ro] {
                draw_line_3d(x.p(cyl(t, r, top)), x.p(cyl(t + dt, r, top)), GRID);
            }
            draw_line_3d(x.p(cyl(t, ri, top)), x.p(cyl(t, ro, top)), GRID);
        }
    }
    for row in 0..ROWS {
        for col in 0..geo.cols {
            let k = g.get(col, row);
            if k != 0 {
                let c = kind_color((k - 1) as usize);
                cell(&x, &geo, col, row, c, Some(darker(c, 0.35)));
            }
        }
    }
    for d in &g.debris {
        debris_box(d.pos.as_vec3(), Mat3::from_quat(d.rot.as_quat()), kind_color((d.kind - 1) as usize));
    }
    if g.state == State::Playing {
        let c = kind_color(g.cur.kind);
        for (col, row) in g.cur.board_cells(g.cur.y, g.cols) {
            cell(&x, &geo, col, row, c, Some(darker(c, 0.35)));
        }
        let gy = g.ghost_y();
        if gy != g.cur.y {
            for (col, row) in g.cur.board_cells(gy, g.cols) {
                cell(&x, &geo, col, row, Color { a: 0.0, ..GHOST }, Some(GHOST));
            }
        }
    }
}

struct View {
    /// Camera elevation above horizontal, degrees.
    elev: f32,
    /// Wire frame of the empty wall on/off.
    frame: bool,
    /// Physics markers (centre, COM, plumb line, contact) on/off.
    marks: bool,
    /// Where the camera looks on the floor; eases after the rolling base.
    look: Vec3,
}

impl View {
    fn follow(&mut self, g: &Game, dt: f32) {
        let c = g.rocker.contact().as_vec3();
        self.look += (c - self.look) * (1.0 - (-4.0 * dt).exp());
    }
}

fn tower_camera(g: &Game, v: &View, panel_w: f32) -> Camera3D {
    let geo = g.geo;
    let (sw, sh) = (screen_width(), screen_height());
    let e = v.elev.to_radians();
    let height = (geo.rho - geo.r_out) as f32 + ROWS as f32;
    let fovy = height * e.cos() + 2.0 * geo.rho as f32 * e.sin() + 6.0;
    let px = fovy / sh;
    let target = v.look + vec3(panel_w / 2.0 * px, height / 2.0, 0.0);
    let dir = vec3(0.0, e.sin(), e.cos());
    Camera3D {
        position: target + dir * 80.0,
        target,
        up: Vec3::Y,
        fovy,
        aspect: Some(sw / sh),
        projection: Projection::Orthographics,
        ..Default::default()
    }
}

/// World point → screen pixels for the 2D overlay.
fn to_screen(cam: &Camera3D, p: Vec3) -> Vec2 {
    let ndc = cam.matrix().project_point3(p);
    vec2((ndc.x + 1.0) / 2.0 * screen_width(), (1.0 - ndc.y) / 2.0 * screen_height())
}

/// X-ray markers over the scene: C (the sphere's centre), the centre of mass
/// with its plumb line to the floor, and the contact point P. The base comes
/// to rest when the plumb line passes through P.
fn draw_marks(g: &Game, cam: &Camera3D) {
    let rk = &g.rocker;
    let c = to_screen(cam, rk.c.as_vec3());
    let p = to_screen(cam, rk.contact().as_vec3());
    let gw = rk.to_world(rk.mass.com());
    let gs = to_screen(cam, gw.as_vec3());
    let foot = to_screen(cam, vec3(gw.x as f32, 0.0, gw.z as f32));
    // C
    draw_line(c.x - 6.0, c.y, c.x + 6.0, c.y, 1.5, DIM);
    draw_line(c.x, c.y - 6.0, c.x, c.y + 6.0, 1.5, DIM);
    draw_text("C", c.x + 7.0, c.y - 5.0, 18.0, DIM);
    // plumb line, dashed
    let d = foot - gs;
    let len = d.length();
    if len > 1.0 {
        let n = (len / 8.0) as i32;
        for k in (0..n).step_by(2) {
            let a = gs + d * (k as f32 / n as f32);
            let b = gs + d * (((k + 1) as f32 / n as f32).min(1.0));
            draw_line(a.x, a.y, b.x, b.y, 1.5, MARK);
        }
    }
    // COM: ringed dot
    draw_circle(gs.x, gs.y, 3.5, MARK);
    draw_circle_lines(gs.x, gs.y, 8.0, 1.5, MARK);
    // contact point: small triangle on the floor
    draw_triangle(vec2(p.x, p.y - 7.0), vec2(p.x - 6.0, p.y + 4.0), vec2(p.x + 6.0, p.y + 4.0), OK);
}

// ---------- 2D panel ----------

fn deg(a: f64) -> String {
    format!("{:.1} deg", a.to_degrees())
}

/// Top view of the lean: the dot is where the tower axis points, out to 45°
/// at the rim; the red ring is the 30° slide angle; the hollow ring is where
/// it would come to rest. The camera is at the bottom.
fn draw_level(g: &Game, cx: f32, cy: f32, r: f32) {
    let cap = physics::CAP_ANGLE as f32;
    let slide = physics::SLIDE_ANGLE as f32 / cap * r;
    draw_circle(cx, cy, r, Color::new(0.06, 0.07, 0.09, 1.0));
    draw_circle_lines(cx, cy, r, 1.5, GRID);
    draw_circle_lines(cx, cy, slide, 1.5, WARN);
    draw_circle_lines(cx, cy, r / 3.0, 1.0, GRID);
    draw_line(cx - r, cy, cx + r, cy, 1.0, GRID);
    draw_line(cx, cy - r, cx, cy + r, 1.0, GRID);
    if let (Some(eq), Some((dx, dz))) = (g.equilibrium(), g.rest_lean_dir()) {
        let k = (eq as f32 / cap).min(1.05) * r;
        draw_circle_lines(cx + dx as f32 * k, cy + dz as f32 * k, 6.0, 1.5, DIM);
    }
    let (lx, lz) = g.rocker.lean();
    let s = (lx.hypot(lz)).max(1e-9);
    let k = (g.rocker.tilt() as f32 / cap).min(1.05) * r;
    let col = if g.rocker.tilt() > physics::SLIDE_ANGLE { WARN } else { MARK };
    draw_circle(cx + (lx / s) as f32 * k, cy + (lz / s) as f32 * k, 5.0, col);
    let t = "camera";
    let w = measure_text(t, None, 14, 1.0).width;
    draw_text(t, cx - w / 2.0, cy + r + 14.0, 14.0, DIM);
}

fn draw_panel(g: &Game, v: &View, x: f32) {
    let sh = screen_height();
    draw_rectangle(x - 10.0, 0.0, screen_width() - x + 10.0, sh, PANEL_BG);
    let mut y = 46.0;
    draw_text("BLOCKFALL", x, y, 34.0, TEXT);
    y += 28.0;
    draw_text("ROCKER-TOWER", x, y, 24.0, DIM);
    draw_text(&format!("seed {}", g.seed), x + 190.0, y, 16.0, DIM);

    y += 40.0;
    draw_text("ON DECK", x, y, 22.0, DIM);
    let d = &PIECES[g.next];
    let s = 20.0;
    let c = kind_color(g.next);
    draw_rectangle_lines(x + 92.0, y - 22.0, 4.0 * s + 14.0, 2.0 * s + 14.0, 1.5, DIM);
    for &(cx, cy) in &d.cells {
        let (bx, by) = (x + 100.0 + cx as f32 * s, y - 14.0 + cy as f32 * s);
        draw_rectangle(bx, by, s - 2.0, s - 2.0, c);
        draw_rectangle_lines(bx, by, s - 2.0, s - 2.0, 2.0, darker(c, 0.4));
    }

    y += 64.0;
    for (label, val) in [("SCORE", g.score), ("LINES", g.lines), ("LEVEL", g.level()), ("PIECES", g.pieces)] {
        draw_text(label, x, y, 20.0, DIM);
        draw_text(&val.to_string(), x + 110.0, y, 24.0, TEXT);
        y += 28.0;
    }
    draw_text("RING", x, y, 20.0, DIM);
    draw_text(&format!("{} bricks", g.cols), x + 110.0, y, 22.0, TEXT);
    y += 34.0;

    let tilt = g.rocker.tilt();
    draw_text("TILT", x, y, 20.0, DIM);
    draw_text(&deg(tilt), x + 110.0, y, 22.0, if tilt > physics::SLIDE_ANGLE * 0.8 { WARN } else { TEXT });
    y += 26.0;
    draw_text("REST", x, y, 20.0, DIM);
    match g.equilibrium() {
        Some(eq) => draw_text(&deg(eq), x + 110.0, y, 22.0, if eq > physics::SLIDE_ANGLE { WARN } else { TEXT }),
        None => draw_text("none: top-heavy", x + 110.0, y, 20.0, WARN),
    };
    y += 26.0;
    draw_text("MAX", x, y, 20.0, DIM);
    draw_text(&deg(g.max_tilt), x + 110.0, y, 22.0, TEXT);
    y += 26.0;
    draw_text("SLIDES AT", x, y, 20.0, DIM);
    draw_text(&deg(physics::SLIDE_ANGLE), x + 110.0, y, 22.0, DIM);
    y += 16.0;
    draw_level(g, x + 150.0, y + 62.0, 58.0);
    y += 150.0;

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
        "C        markers on/off",
        "P        pause",
        "R        restart",
        "Q Esc    quit",
    ] {
        draw_text(h, x, y, 18.0, DIM);
        y += 21.0;
    }
    let _ = v;
}

fn draw_overlay(g: &Game, cx: f32, cy: f32) {
    let slid;
    let (title, sub): (&str, &str) = match (g.state, g.paused, g.cause) {
        (State::Sliding(_), _, Cause::SlidOff(a)) | (State::Over, _, Cause::SlidOff(a)) => {
            slid = format!("slid off at {:.1} deg  -  R = restart", a.to_degrees());
            ("GAME OVER", &slid)
        }
        (State::Over, _, _) => ("GAME OVER", "no room for the next piece  -  R = restart"),
        (_, true, _) => ("PAUSED", "P = resume"),
        _ => return,
    };
    let tw = measure_text(title, None, 56, 1.0).width;
    let sw = measure_text(sub, None, 24, 1.0).width;
    let w = tw.max(sw) + 40.0;
    draw_rectangle(cx - w / 2.0, cy - 50.0, w, 95.0, Color::new(0.0, 0.0, 0.0, 0.7));
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
        window_title: "Blockfall Rocker-Tower".to_owned(),
        window_width: 1200,
        window_height: 900,
        window_resizable: true,
        sample_count: 4,
        ..Default::default()
    }
}

async fn run() {
    let (seed, ring) = parse_args();
    let mut g = Game::new(seed, ring);
    let mut view = View { elev: 25.0, frame: false, marks: true, look: Vec3::ZERO };
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
            }
        }
        if pressed(&[KeyCode::G]) {
            view.frame = !view.frame;
        }
        if pressed(&[KeyCode::C]) {
            view.marks = !view.marks;
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
        let cam = tower_camera(&g, &view, panel_w);
        set_camera(&cam);
        draw_floor(view.look);
        draw_tower(&g, &view);
        set_default_camera();
        if view.marks {
            draw_marks(&g, &cam);
        }
        let sw = screen_width();
        draw_panel(&g, &view, sw - panel_w + 20.0);
        draw_overlay(&g, (sw - panel_w) / 2.0, screen_height() * 0.3);
        next_frame().await;
    }
}

fn main() {
    macroquad::Window::from_config(conf(), run());
}
