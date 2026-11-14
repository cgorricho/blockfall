// BLOCKFALL CUBE-3D — BLOCKFALL in a 6×6 or 9×9 well, 16 deep, real 3D. Pieces
// move in the horizontal plane, rotate about x, y and z, a full layer clears.
// Rules: game.rs and README.md. Rust + macroquad.
#![windows_subsystem = "windows"]

mod game;

use game::{Axis, Game, H, PIECES, SIZES, State};
use macroquad::models::Vertex;
use macroquad::prelude::*;

// ---------- geometry ----------

/// Game (x, y, z) -> world. World Y is up; game y points away from the
/// default camera (world -Z). A base of side `s` is centred on the origin.
fn wp(s: f32, x: f32, y: f32, z: f32) -> Vec3 {
    vec3(x - s / 2.0, z, -(y - s / 2.0))
}

/// Game axes as world vectors (for the axis gizmo).
const AX_X: Vec3 = Vec3::X;
const AX_Y: Vec3 = Vec3::new(0.0, 0.0, -1.0);
const AX_Z: Vec3 = Vec3::Y;

fn light_dir() -> Vec3 {
    vec3(-0.45, 0.85, 0.35).normalize()
}

fn shade(c: Color, n: Vec3) -> Color {
    let f = 0.40 + 0.60 * n.dot(light_dir()).max(0.0);
    Color::new(c.r * f, c.g * f, c.b * f, c.a)
}

fn to_u8(c: Color) -> [u8; 4] {
    [(c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8, (c.a * 255.0) as u8]
}

fn vert(p: Vec3, c: Color) -> Vertex {
    Vertex { position: p, uv: Vec2::ZERO, color: to_u8(c), normal: Vec4::ZERO }
}

/// A flat-shaded box between corners `a` and `b`, with outlined edges.
fn cuboid(a: Vec3, b: Vec3, col: Color, edge: Option<Color>) {
    let (lo, hi) = (a.min(b), a.max(b));
    let c = |i: usize| vec3(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
    // each face: 4 corner indices and its normal
    let faces: [([usize; 4], Vec3); 6] = [
        ([0, 2, 6, 4], -Vec3::X),
        ([1, 5, 7, 3], Vec3::X),
        ([0, 4, 5, 1], -Vec3::Y),
        ([2, 3, 7, 6], Vec3::Y),
        ([0, 1, 3, 2], -Vec3::Z),
        ([4, 6, 7, 5], Vec3::Z),
    ];
    let mut v = Vec::with_capacity(24);
    let mut ix: Vec<u16> = Vec::with_capacity(36);
    for (q, n) in faces {
        let s = shade(col, n);
        let base = v.len() as u16;
        for i in q {
            v.push(vert(c(i), s));
        }
        ix.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    draw_mesh(&Mesh { vertices: v, indices: ix, texture: None });
    if let Some(e) = edge {
        for (i, j) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
            draw_line_3d(c(i), c(j), e);
        }
    }
}

/// One unit cube of the well at game cell (x, y, z).
fn cube(s: f32, x: i32, y: i32, z: i32, col: Color, edge: Option<Color>) {
    let (x, y, z) = (x as f32, y as f32, z as f32);
    cuboid(wp(s, x, y, z), wp(s, x + 1.0, y + 1.0, z + 1.0), col, edge);
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

/// Colours for "colour by layer" (L): one per height, repeating.
const LAYER_COLORS: [u8; 7] = [107, 141, 75, 167, 73, 179, 175];

fn darker(c: Color, f: f32) -> Color {
    Color::new(c.r * f, c.g * f, c.b * f, 1.0)
}

const BG: Color = Color::new(0.07, 0.08, 0.11, 1.0);
const PANEL_BG: Color = Color::new(0.10, 0.11, 0.15, 1.0);
const GRID: Color = Color::new(0.24, 0.27, 0.35, 1.0);
const RIM: Color = Color::new(0.50, 0.54, 0.62, 1.0);
const DIM: Color = Color::new(0.55, 0.58, 0.65, 1.0);
const TEXT: Color = Color::new(0.93, 0.94, 0.96, 1.0);
/// The ghost is a neutral outline, never the piece colour.
const GHOST: Color = Color::new(0.85, 0.87, 0.92, 0.6);

// ---------- view ----------

/// The base only ever turns in quarter turns about z (A / D), so every side
/// can be seen; it never shifts or tilts.
struct View {
    /// Quarter turns about z. The camera yaw is YAW0 + 90° × turns, so the
    /// view always looks across a corner.
    turns: i32,
    /// Displayed yaw in degrees; eases toward the target.
    yaw: f32,
    ortho: bool,
    grid: bool,
    layer_colors: bool,
}

/// Camera yaw about the vertical at turn 0, degrees (0 = straight at the y = 0 wall).
const YAW0: f32 = 28.0;
/// Camera elevation above horizontal, degrees.
const ELEV: f32 = 38.0;
/// How fast the displayed turn catches up (1/s).
const TURN_RATE: f32 = 14.0;

impl View {
    fn new() -> View {
        View { turns: 0, yaw: YAW0, ortho: true, grid: true, layer_colors: false }
    }

    fn yaw_target(&self) -> f32 {
        YAW0 + 90.0 * self.turns as f32
    }

    fn follow(&mut self, dt: f32) {
        let d = self.yaw_target() - self.yaw;
        self.yaw = if d.abs() < 0.01 { self.yaw_target() } else { self.yaw + d * (1.0 - (-TURN_RATE * dt).exp()) };
    }

    /// Camera direction (from the target toward the eye).
    fn eye_dir(&self) -> Vec3 {
        let (p, e) = (self.yaw.to_radians(), ELEV.to_radians());
        vec3(e.cos() * p.sin(), e.sin(), e.cos() * p.cos())
    }

    /// Screen right and screen up as world vectors.
    fn basis(&self) -> (Vec3, Vec3) {
        let f = -self.eye_dir();
        let right = f.cross(Vec3::Y).normalize();
        (right, right.cross(f))
    }
}

/// The game steps (dx, dy) for → and ↑ after `turns` quarter turns: → goes
/// toward the screen's right, ↑ away from you. ← and ↓ are the opposites.
fn arrow_steps(turns: i32) -> ((i32, i32), (i32, i32)) {
    // with yaw φ the screen's right is game (cos φ, sin φ) and "away" is
    // (-sin φ, cos φ); the YAW0 offset is ignored, so these are exact.
    match turns.rem_euclid(4) {
        0 => ((1, 0), (0, 1)),
        1 => ((0, 1), (-1, 0)),
        2 => ((-1, 0), (0, -1)),
        _ => ((0, -1), (1, 0)),
    }
}

fn set_well_camera(v: &View, panel_w: f32) {
    let (sw, sh) = (screen_width(), screen_height());
    let e = ELEV.to_radians();
    // visible height of the well in the view, plus a margin
    // sized for the 9 x 9 base, so both sizes are drawn to the same scale
    let diag = (2.0 * (SIZES[1] * SIZES[1]) as f32).sqrt();
    let fovy = H as f32 * e.cos() + diag * e.sin() + 3.0;
    let px = fovy / sh; // world units per pixel at the target
    let (right, _) = v.basis();
    let target = vec3(0.0, H as f32 / 2.0 - 0.6, 0.0) + right * (panel_w / 2.0 * px);
    let (projection, fov, dist) = if v.ortho {
        (Projection::Orthographics, fovy, 60.0)
    } else {
        // back off far enough that the near rim, which looks bigger, still fits
        let a = 30f32.to_radians();
        (Projection::Perspective, a, 1.15 * fovy / (2.0 * (a / 2.0).tan()))
    };
    set_camera(&Camera3D {
        position: target + v.eye_dir() * dist,
        target,
        up: Vec3::Y,
        fovy: fov,
        aspect: Some(sw / sh),
        projection,
        ..Default::default()
    });
}

// ---------- 3D scene ----------

/// The well: floor grid, grids on the two walls facing you from the back,
/// and the outline of the rim and corners.
fn draw_well(v: &View, size: i32) {
    let (w, d, h) = (size as f32, size as f32, H as f32);
    let wp = |x, y, z| wp(w, x, y, z);
    let eye = v.eye_dir();
    // the floor slab
    cuboid(wp(-0.3, -0.3, -0.4), wp(w + 0.3, d + 0.3, 0.0), Color::new(0.20, 0.22, 0.28, 1.0), Some(GRID));
    if v.grid {
        for i in 0..=size {
            draw_line_3d(wp(i as f32, 0.0, 0.01), wp(i as f32, d, 0.01), GRID);
        }
        for j in 0..=size {
            draw_line_3d(wp(0.0, j as f32, 0.01), wp(w, j as f32, 0.01), GRID);
        }
        // back walls: those whose outward normal points away from the eye
        let back = |n: Vec3| n.dot(eye) < 0.0;
        let faint = Color { a: 0.55, ..GRID };
        for (xw, n) in [(0.0, -AX_X), (w, AX_X)] {
            if back(n) {
                for j in 0..=size {
                    draw_line_3d(wp(xw, j as f32, 0.0), wp(xw, j as f32, h), faint);
                }
                for k in 0..=H {
                    draw_line_3d(wp(xw, 0.0, k as f32), wp(xw, d, k as f32), faint);
                }
            }
        }
        for (yw, n) in [(0.0, -AX_Y), (d, AX_Y)] {
            if back(n) {
                for i in 0..=size {
                    draw_line_3d(wp(i as f32, yw, 0.0), wp(i as f32, yw, h), faint);
                }
                for k in 0..=H {
                    draw_line_3d(wp(0.0, yw, k as f32), wp(w, yw, k as f32), faint);
                }
            }
        }
    }
    // vertical corners and the rim
    for (x, y) in [(0.0, 0.0), (w, 0.0), (w, d), (0.0, d)] {
        draw_line_3d(wp(x, y, 0.0), wp(x, y, h), RIM);
    }
    let rim = [(0.0, 0.0), (w, 0.0), (w, d), (0.0, d), (0.0, 0.0)];
    for p in rim.windows(2) {
        draw_line_3d(wp(p[0].0, p[0].1, h), wp(p[1].0, p[1].1, h), RIM);
    }
}

fn draw_blocks(g: &Game, v: &View) {
    let s = g.size as f32;
    let wp = |x, y, z| wp(s, x, y, z);
    let cube = |x, y, z, c, e| cube(s, x, y, z, c, e);
    for z in 0..H {
        let lc = xterm(LAYER_COLORS[z as usize % LAYER_COLORS.len()]);
        for y in 0..g.size {
            for x in 0..g.size {
                let k = g.get(x, y, z);
                if k != 0 {
                    let c = if v.layer_colors { lc } else { kind_color((k - 1) as usize) };
                    cube(x, y, z, c, Some(darker(c, 0.35)));
                }
            }
        }
    }
    if g.state != State::Playing {
        return;
    }
    let c = kind_color(g.cur.kind);
    for (x, y, z) in g.cur.cells_at(g.cur.z) {
        if z < H + 4 {
            cube(x, y, z, c, Some(darker(c, 0.35)));
        }
    }
    // the projection: where the piece will land, a neutral wire outline, drawn last;
    // plus a drop line under each cell that has nothing of the piece below it
    let gz = g.ghost_z();
    if gz != g.cur.z {
        let cells = g.cur.cells_at(g.cur.z);
        for (x, y, z) in cells {
            if !cells.contains(&(x, y, z - 1)) {
                let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                draw_line_3d(wp(cx, cy, z as f32), wp(cx, cy, (z - g.cur.z + gz) as f32 + 1.0), Color { a: 0.45, ..GHOST });
            }
        }
        for (x, y, z) in g.cur.cells_at(gz) {
            cube(x, y, z, Color { a: 0.0, ..GHOST }, Some(GHOST));
        }
    }
}

// ---------- 2D panel ----------

fn draw_gizmo(v: &View, cx: f32, cy: f32) {
    let (right, up) = v.basis();
    let len = 38.0;
    draw_circle_lines(cx, cy, len + 8.0, 1.0, GRID);
    let axes = [("X", AX_X, Color::new(1.0, 0.35, 0.35, 1.0)), ("Y", AX_Y, Color::new(0.35, 0.95, 0.4, 1.0)), ("Z", AX_Z, Color::new(0.4, 0.6, 1.0, 1.0))];
    for (name, a, col) in axes {
        let (sx, sy) = (a.dot(right) * len, -a.dot(up) * len);
        draw_line(cx, cy, cx + sx, cy + sy, 3.0, col);
        draw_text(name, cx + sx * 1.25 - 5.0, cy + sy * 1.25 + 6.0, 20.0, col);
    }
}

fn draw_panel(g: &Game, v: &View, x: f32) {
    let sh = screen_height();
    draw_rectangle(x - 10.0, 0.0, screen_width() - x + 10.0, sh, PANEL_BG);
    let mut y = 46.0;
    draw_text("BLOCKFALL", x, y, 34.0, TEXT);
    y += 28.0;
    draw_text("CUBE-3D", x, y, 26.0, DIM);
    draw_text(&format!("seed {}", g.seed), x + 150.0, y, 16.0, DIM);

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
    draw_gizmo(v, x + 250.0, y + 6.0);

    y += 70.0;
    draw_text("BASE", x, y, 22.0, DIM);
    draw_text(&format!("{0} x {0}", g.size), x + 110.0, y, 24.0, TEXT);
    y += 30.0;
    for (label, val) in [("SCORE", g.score), ("LAYERS", g.layers), ("LEVEL", g.level()), ("PIECES", g.pieces)] {
        draw_text(label, x, y, 22.0, DIM);
        draw_text(&val.to_string(), x + 110.0, y, 26.0, TEXT);
        y += 30.0;
    }

    // how full each layer is, top layer at the top
    y += 6.0;
    draw_text("FILL", x, y, 18.0, DIM);
    let (bx, bw, bh) = (x + 50.0, 150.0, 9.0);
    for z in (0..H).rev() {
        let by = y - 10.0 + (H - 1 - z) as f32 * (bh + 2.0);
        let f = g.layer_fill(z) as f32 / g.layer_cells() as f32;
        draw_rectangle(bx, by, bw, bh, Color::new(0.16, 0.18, 0.23, 1.0));
        if f > 0.0 {
            let c = if f > 0.9 { Color::new(0.4, 0.95, 0.5, 1.0) } else { Color::new(0.45, 0.55, 0.75, 1.0) };
            draw_rectangle(bx, by, bw * f, bh, c);
        }
        if z % 4 == 0 {
            draw_text(&format!("{}", z + 1), bx + bw + 8.0, by + bh, 14.0, DIM);
        }
    }
    y += H as f32 * (bh + 2.0) + 18.0;

    for h in [
        "Arrows    move piece (x / y)",
        "X Y Z     rotate about x/y/z",
        "  +Shift  rotate the other way",
        "S         soft drop",
        "Space     hard drop",
        "A / D     turn the base 90 deg",
        "6 / 9     new game, 6x6 / 9x9",
        "V         ortho/perspective",
        "G  L      grid / layer colours",
        "P R Q     pause restart quit",
    ] {
        draw_text(h, x, y, 18.0, DIM);
        y += 21.0;
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

/// Auto-repeat for held keys: act on press, then after 170 ms every 60 ms.
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
            self.next += 0.06;
            return true;
        }
        false
    }
}

/// `--seed N` and `--size 6|9`.
fn parse_args() -> (u32, i32) {
    let argv: Vec<String> = std::env::args().collect();
    let val = |flag: &str| argv.iter().position(|a| a == flag).and_then(|i| argv.get(i + 1)).and_then(|s| s.parse::<u32>().ok());
    let seed = val("--seed").unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u32).unwrap_or(1)
    });
    let size = val("--size").map(|v| v as i32).filter(|v| SIZES.contains(v)).unwrap_or(SIZES[0]);
    (seed, size)
}

fn conf() -> Conf {
    Conf {
        window_title: "Blockfall Cube-3D".to_owned(),
        window_width: 1200,
        window_height: 860,
        window_resizable: true,
        sample_count: 4,
        ..Default::default()
    }
}

async fn run() {
    let (seed, size) = parse_args();
    let mut g = Game::new(seed, size);
    let mut view = View::new();
    let mut reps: [Repeat; 5] = Default::default();
    loop {
        let dt = get_frame_time().min(0.05);
        let down = |ks: &[KeyCode]| ks.iter().any(|&k| is_key_down(k));
        let pressed = |ks: &[KeyCode]| ks.iter().any(|&k| is_key_pressed(k));
        let shift = down(&[KeyCode::LeftShift, KeyCode::RightShift]);

        if pressed(&[KeyCode::Q, KeyCode::Escape]) {
            break;
        }
        if pressed(&[KeyCode::R]) {
            g.reset();
        }
        if pressed(&[KeyCode::P]) && g.state == State::Playing {
            g.paused = !g.paused;
        }
        if pressed(&[KeyCode::G]) {
            view.grid = !view.grid;
        }
        if pressed(&[KeyCode::L]) {
            view.layer_colors = !view.layer_colors;
        }
        if pressed(&[KeyCode::V]) {
            view.ortho = !view.ortho;
        }
        for (key, size) in [(KeyCode::Key6, SIZES[0]), (KeyCode::Key9, SIZES[1])] {
            if pressed(&[key]) {
                g.set_size(size);
            }
        }
        // the base turns a quarter about z, either way
        if pressed(&[KeyCode::A]) {
            view.turns -= 1;
        }
        if pressed(&[KeyCode::D]) {
            view.turns += 1;
        }
        // the arrows slide the piece parallel to the base, following the screen
        let (r, a) = arrow_steps(view.turns);
        let moves = [(KeyCode::Right, r), (KeyCode::Left, (-r.0, -r.1)), (KeyCode::Up, a), (KeyCode::Down, (-a.0, -a.1))];
        for (i, (k, (dx, dy))) in moves.into_iter().enumerate() {
            if reps[i].fire(is_key_pressed(k), is_key_down(k), dt) {
                g.shift(dx, dy);
            }
        }
        if reps[4].fire(pressed(&[KeyCode::S]), down(&[KeyCode::S]), dt) {
            g.soft_drop();
        }
        for (k, axis) in [(KeyCode::X, Axis::X), (KeyCode::Y, Axis::Y), (KeyCode::Z, Axis::Z)] {
            if is_key_pressed(k) {
                g.try_rotate(axis, !shift);
            }
        }
        if pressed(&[KeyCode::Space]) {
            g.hard_drop();
        }
        g.update(dt as f64);
        view.follow(dt);

        let panel_w = 340.0;
        clear_background(BG);
        set_well_camera(&view, panel_w);
        draw_well(&view, g.size);
        draw_blocks(&g, &view);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The arrows must match the camera after any number of quarter turns:
    /// → along the screen's right, ↑ away from the eye.
    #[test]
    fn arrows_follow_the_view() {
        for turns in -4..8 {
            let v = View { turns, yaw: YAW0 + 90.0 * turns as f32, ..View::new() };
            let (right, _) = v.basis();
            let away = vec3(-v.eye_dir().x, 0.0, -v.eye_dir().z).normalize();
            let (r, a) = arrow_steps(turns);
            let world = |(dx, dy): (i32, i32)| AX_X * dx as f32 + AX_Y * dy as f32;
            assert!(world(r).dot(right) > 0.7, "turns {turns}: right");
            assert!(world(a).dot(away) > 0.7, "turns {turns}: away");
        }
    }
}
