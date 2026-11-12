//! Rocker physics: one rigid body (the curved base plus every locked cell)
//! rolling without slipping on a flat floor. One degree of freedom: the tilt θ.
//!
//! Units: 1 = one cell (think 20 cm), seconds, mass 1 = one cell of blocks.
//!
//! Body frame: origin at the centre of curvature C, x right, y up.
//! The base is the circular segment below its chord. The arc spans ±45° from
//! the bottom, so the chord is R·√2 wide and sits R/√2 below C.
//! Easy mode: R = 5√2 → a 10-wide top. Hard mode: R = 8 → an 8√2 ≈ 11.3-wide top.
//!
//! World frame: floor is y = 0. Rolling without slipping puts C at
//! (x0 - R·θ, R) and the contact point P straight below it at (x0 - R·θ, 0).
//! θ > 0 is counter-clockwise (left side down).
//!
//! Equation of motion (Lagrange, rolling disc with offset centre of mass):
//!   U(θ)  = m·g·(rc.y)                 rc = COM rotated by θ, relative to C
//!   I(θ)  = I_com + m·|COM − P|²       moment of inertia about the contact point
//!   I·θ'' + ½·I'(θ)·θ'² + U'(θ) + damping = 0
//!   U'(θ) = m·g·rc.x                   (gravity torque about P)
//!   I'(θ) = 2·m·R·rc.x
//! Equilibrium: rc.x = 0, i.e. the COM is straight above the contact point.
//! It is stable only while the COM is below C (closer to the floor than R).

/// Gravity in cells/s². 9.81 m/s² with 20 cm cells.
pub const G: f64 = 49.0;
/// Mass of the wooden segment of the easy-mode rocker (R = 5√2), in cells.
/// Other radii keep the same wood density, so mass grows with R².
pub const BASE_MASS: f64 = 30.0;
/// Rolling resistance: damping torque = -DAMPING · I · ω.
pub const DAMPING: f64 = 0.45;
/// Friction angle: above this tilt the glued stack breaks loose and slides off.
pub const SLIDE_ANGLE: f64 = 30.0 * std::f64::consts::PI / 180.0;
/// Hard mode: blocks grip the teeth and each other, so the stack never slides;
/// at this tilt the end of the 45° arc meets the floor and the rocker topples.
pub const TOPPLE_ANGLE: f64 = std::f64::consts::FRAC_PI_4;
/// Integration step.
pub const DT: f64 = 1.0 / 480.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f64,
    pub y: f64,
}

pub const fn v2(x: f64, y: f64) -> V2 {
    V2 { x, y }
}

impl V2 {
    pub fn rot(self, a: f64) -> V2 {
        let (s, c) = a.sin_cos();
        v2(self.x * c - self.y * s, self.x * s + self.y * c)
    }
    pub fn add(self, o: V2) -> V2 {
        v2(self.x + o.x, self.y + o.y)
    }
    pub fn sub(self, o: V2) -> V2 {
        v2(self.x - o.x, self.y - o.y)
    }
    pub fn scale(self, k: f64) -> V2 {
        v2(self.x * k, self.y * k)
    }
    pub fn dot(self, o: V2) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn len2(self) -> f64 {
        self.x * self.x + self.y * self.y
    }
    pub fn len(self) -> f64 {
        self.len2().sqrt()
    }
}

/// Mass accumulator in the body frame: total mass, first moments, and the
/// moment of inertia about the body origin C. Adding two of these combines bodies.
#[derive(Clone, Copy, Debug, Default)]
pub struct Mass {
    pub m: f64,
    pub mx: f64,
    pub my: f64,
    pub i0: f64,
}

impl Mass {
    /// A piece of mass m centred at p with its own moment of inertia i_own (about p).
    pub fn at(m: f64, p: V2, i_own: f64) -> Mass {
        Mass { m, mx: m * p.x, my: m * p.y, i0: i_own + m * p.len2() }
    }
    /// One unit cell (unit square, mass 1, any orientation) centred at p.
    pub fn cell(p: V2) -> Mass {
        Mass::at(1.0, p, 1.0 / 6.0)
    }
    pub fn plus(self, o: Mass) -> Mass {
        Mass { m: self.m + o.m, mx: self.mx + o.mx, my: self.my + o.my, i0: self.i0 + o.i0 }
    }
    pub fn scaled(self, k: f64) -> Mass {
        Mass { m: self.m * k, mx: self.mx * k, my: self.my * k, i0: self.i0 * k }
    }
    pub fn com(&self) -> V2 {
        v2(self.mx / self.m, self.my / self.m)
    }
    pub fn i_com(&self) -> f64 {
        self.i0 - self.m * self.com().len2()
    }
}

/// Integrates density 1 over the region where `inside` is true, inside the box
/// [x0,x1]×[y0,y1], on an N×N grid. Simple and exact enough (< 0.1%).
fn integrate(x0: f64, x1: f64, y0: f64, y1: f64, inside: impl Fn(V2) -> bool) -> Mass {
    const N: usize = 500;
    let (dx, dy) = ((x1 - x0) / N as f64, (y1 - y0) / N as f64);
    let mut acc = Mass::default();
    for i in 0..N {
        for j in 0..N {
            let p = v2(x0 + (i as f64 + 0.5) * dx, y0 + (j as f64 + 0.5) * dy);
            if inside(p) {
                acc = acc.plus(Mass::at(dx * dy, p, 0.0));
            }
        }
    }
    acc
}

/// The wooden circular segment under the chord of a rocker with radius r.
pub fn segment_mass(r: f64) -> Mass {
    let half = r / std::f64::consts::SQRT_2;
    let acc = integrate(-half, half, -r, -half, |p| p.len2() <= r * r);
    let easy_r2 = 50.0; // (5√2)²
    acc.scaled(BASE_MASS * (r * r / easy_r2) / acc.m)
}

/// Mass (density 1, like the blocks) of the region between y = base and the
/// profile y = top(x), for x in [x0, x1].
pub fn profile_mass(x0: f64, x1: f64, base: f64, peak: f64, top: impl Fn(f64) -> f64) -> Mass {
    integrate(x0, x1, base, peak, |p| p.y <= top(p.x))
}

#[derive(Clone, Copy)]
pub struct Rocker {
    pub theta: f64,
    pub omega: f64,
    /// World x of C when θ = 0.
    pub x0: f64,
    /// Radius of the arc.
    pub r: f64,
    pub mass: Mass,
}

impl Rocker {
    pub fn new(mass: Mass, r: f64) -> Rocker {
        Rocker { theta: 0.0, omega: 0.0, x0: 0.0, r, mass }
    }

    pub fn center(&self) -> V2 {
        v2(self.x0 - self.r * self.theta, self.r)
    }
    pub fn contact(&self) -> V2 {
        v2(self.x0 - self.r * self.theta, 0.0)
    }
    pub fn to_world(&self, b: V2) -> V2 {
        self.center().add(b.rot(self.theta))
    }
    pub fn to_body(&self, w: V2) -> V2 {
        w.sub(self.center()).rot(-self.theta)
    }
    /// World velocity of a point fixed to the body.
    pub fn velocity_of(&self, b: V2) -> V2 {
        let r = b.rot(self.theta);
        // centre moves with -R·ω along x; rotation adds ω × r
        v2(-self.r * self.omega - self.omega * r.y, self.omega * r.x)
    }

    /// Moment of inertia about the contact point at tilt θ.
    pub fn i_contact(&self, mass: &Mass, theta: f64) -> f64 {
        let rc = mass.com().rot(theta);
        mass.i_com() + mass.m * (rc.x * rc.x + (self.r + rc.y) * (self.r + rc.y))
    }

    fn accel(&self, theta: f64, omega: f64) -> f64 {
        let m = &self.mass;
        let rc = m.com().rot(theta);
        let i = self.i_contact(m, theta);
        let du = m.m * G * rc.x;
        let di = 2.0 * m.m * self.r * rc.x;
        -(du + 0.5 * di * omega * omega + DAMPING * i * omega) / i
    }

    /// Advances the simulation by `dt` seconds in fixed DT substeps (RK2 midpoint).
    pub fn step(&mut self, dt: f64) {
        self.step_with(dt, DT);
    }

    /// Largest |θ| over the next `secs` if `add` lands now at `speed` and the
    /// mass then becomes `after` (a line clear). Coarse steps: for planning only.
    pub fn peak_after(&self, add: Mass, speed: f64, after: Mass, secs: f64) -> f64 {
        let mut r = *self;
        r.land(add, v2(0.0, -speed));
        r.set_mass(after);
        let mut peak = r.theta.abs();
        let h = 1.0 / 60.0;
        let mut t = 0.0;
        while t < secs {
            r.step_with(h, h);
            peak = peak.max(r.theta.abs());
            t += h;
        }
        peak
    }

    fn step_with(&mut self, dt: f64, max_h: f64) {
        let mut left = dt;
        while left > 1e-12 {
            let h = left.min(max_h);
            let a1 = self.accel(self.theta, self.omega);
            let (tm, wm) = (self.theta + 0.5 * h * self.omega, self.omega + 0.5 * h * a1);
            let a2 = self.accel(tm, wm);
            self.theta += h * wm;
            self.omega += h * a2;
            left -= h;
        }
    }

    /// Adds mass that arrives moving with velocity `v_body` (expressed in the body
    /// frame, e.g. (0, -speed) for a falling piece). Angular momentum about the
    /// contact point is conserved through the (inelastic) impact.
    pub fn land(&mut self, add: Mass, v_body: V2) {
        let i_old = self.i_contact(&self.mass, self.theta);
        let r = add.com().rot(self.theta).add(v2(0.0, self.r)); // from P to the piece
        let v = v_body.rot(self.theta);
        let l = add.m * (r.x * v.y - r.y * v.x);
        self.mass = self.mass.plus(add);
        let i_new = self.i_contact(&self.mass, self.theta);
        self.omega = (i_old * self.omega + l) / i_new;
    }

    /// Replaces the mass (line clear, debris leaving). Cells that leave take their
    /// own share of angular momentum, so ω is kept.
    pub fn set_mass(&mut self, mass: Mass) {
        self.mass = mass;
    }

    /// Resting tilt for a given mass, or None when the COM is at or above C
    /// (top-heavy: no stable resting angle).
    pub fn equilibrium(mass: &Mass) -> Option<f64> {
        let c = mass.com();
        if c.y >= 0.0 {
            return None;
        }
        // rc.x = 0  ⇒  tan θ = c.x / c.y
        Some((c.x / c.y).atan())
    }

    /// Kinetic + potential energy (for tests).
    #[cfg(test)]
    pub fn energy(&self) -> f64 {
        let i = self.i_contact(&self.mass, self.theta);
        0.5 * i * self.omega * self.omega + self.mass.m * G * self.mass.com().rot(self.theta).y
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_4, SQRT_2};

    const R: f64 = 5.0 * SQRT_2;

    fn base() -> Mass {
        segment_mass(R)
    }
    fn rocker(m: Mass) -> Rocker {
        Rocker::new(m, R)
    }

    #[test]
    fn segment_centroid_matches_formula() {
        // Centroid of a circular segment: 4 R sin³α / (3 (2α − sin 2α)) below C.
        for r in [R, 8.0] {
            let a = FRAC_PI_4;
            let d = 4.0 * r * a.sin().powi(3) / (3.0 * (2.0 * a - (2.0 * a).sin()));
            let c = segment_mass(r).com();
            assert!(c.x.abs() < 1e-9);
            assert!((c.y + d).abs() < 0.01, "centroid {} vs {}", c.y, -d);
        }
    }

    #[test]
    fn empty_base_settles_upright() {
        let mut r = rocker(base());
        r.theta = 0.3;
        r.step(20.0);
        assert!(r.theta.abs() < 0.01, "theta {}", r.theta);
    }

    #[test]
    fn energy_never_increases() {
        let mut r = rocker(base().plus(Mass::cell(v2(3.5, -4.5))));
        r.theta = 0.25;
        let mut e = r.energy();
        for _ in 0..2000 {
            r.step(0.005);
            let e2 = r.energy();
            assert!(e2 <= e + 1e-6, "energy grew {e} -> {e2}");
            e = e2;
        }
    }

    #[test]
    fn off_centre_load_settles_at_predicted_angle() {
        let mut m = base();
        for x in 0..4 {
            m = m.plus(Mass::cell(v2(1.5 + x as f64, -4.5)));
        }
        let want = Rocker::equilibrium(&m).unwrap();
        assert!(want < 0.0, "load on the right tilts clockwise");
        let mut r = rocker(m);
        r.step(30.0);
        assert!((r.theta - want).abs() < 1e-3, "{} vs {}", r.theta, want);
        // COM straight above the contact point
        let com = r.to_world(r.mass.com());
        assert!((com.x - r.contact().x).abs() < 1e-3);
    }

    #[test]
    fn one_sided_pile_tips_past_slide_angle() {
        let mut m = base();
        // 40 cells piled on the right four columns
        for row in 0..10 {
            for x in 0..4 {
                m = m.plus(Mass::cell(v2(1.5 + x as f64, -4.5 + row as f64)));
            }
        }
        let mut r = rocker(m);
        r.step(10.0);
        assert!(r.theta < -SLIDE_ANGLE, "theta {}°", r.theta.to_degrees());
    }

    #[test]
    fn tall_balanced_tower_is_unstable() {
        let mut m = base();
        for row in 0..16 {
            for x in 0..10 {
                m = m.plus(Mass::cell(v2(-4.5 + x as f64, -4.5 + row as f64)));
            }
        }
        assert!(Rocker::equilibrium(&m).is_none());
        let mut r = rocker(m);
        r.theta = 0.02;
        r.step(10.0);
        assert!(r.theta.abs() > SLIDE_ANGLE);
    }

    #[test]
    fn landing_on_right_kicks_clockwise() {
        let mut r = rocker(base());
        r.land(Mass::cell(v2(4.5, -4.5)), v2(0.0, -6.0));
        assert!(r.omega < 0.0);
    }

    #[test]
    fn body_world_round_trip() {
        let mut r = rocker(base());
        r.theta = 0.4;
        let b = v2(1.3, -2.7);
        let back = r.to_body(r.to_world(b));
        assert!(back.sub(b).len() < 1e-12);
    }
}
