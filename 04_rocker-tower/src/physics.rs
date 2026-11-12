//! Rocker-tower physics: one rigid body (a spherical-cap base plus the tower of
//! locked bricks standing on its flat top) rolling without slipping on a flat
//! floor. Full 3D rigid body: three rotational degrees of freedom, and the
//! centre of the sphere moves over the floor as the base rolls.
//!
//! Units: 1 = one cell, seconds, mass 1 = one brick. World Y is up (as in the
//! renderer), the floor is y = 0.
//!
//! Body frame: origin at the centre of the sphere C, +Y along the tower axis.
//! The base is the cap of the ball of radius ρ below the plane y = −ρ·cos 45°,
//! so it spans ±45° from its lowest point. Its flat top is a disc of radius
//! ρ·sin 45°; ρ is chosen so that disc is exactly the tower's outer radius.
//!
//! Rolling without slipping: C stays at height ρ, directly above the contact
//! point P, and the material point of the body at P has zero velocity:
//!     v_C = ω × (ρ ŷ),   v_G = ω × d,   d = G − P
//! Newton–Euler with the contact force eliminated (moments about G):
//!     I_P α = −ω × (I_G ω) − m d × (ω × (ω × (G − C)) − g) + τ_damping
//!     I_P = I_G + m (|d|² 1 − d dᵀ)
//! This is the Chaplygin/Routh rolling-ball equation for an off-centre mass.
//! Spinning about the vertical (pivoting) is allowed by the no-slip contact.

use macroquad::math::{DMat3, DQuat, DVec3};
use std::f64::consts::FRAC_PI_4;

/// Gravity in cells/s² (9.81 m/s² with 20 cm cells), as in Rocker.
pub const G: f64 = 49.0;
/// The wooden base weighs as much as this many full rings of bricks.
pub const BASE_RINGS: f64 = 3.0;
/// Rolling resistance: damping torque = −DAMPING · I_P · ω (as in Rocker).
pub const DAMPING: f64 = 0.45;
/// Extra friction against spinning about the vertical (pivoting on the floor).
pub const PIVOT_DAMPING: f64 = 2.0;
/// Friction angle: above this tilt the stack breaks loose and slides off.
pub const SLIDE_ANGLE: f64 = 30.0 * std::f64::consts::PI / 180.0;
/// The cap spans ±45°: at this tilt its rim meets the floor.
pub const CAP_ANGLE: f64 = FRAC_PI_4;
/// Integration step.
pub const DT: f64 = 1.0 / 480.0;

const UP: DVec3 = DVec3::Y;

/// Inertia tensor of a point mass 1 at p about the origin: |p|²·1 − p pᵀ.
fn point_tensor(p: DVec3) -> DMat3 {
    DMat3::from_diagonal(DVec3::splat(p.length_squared())) - DMat3::from_cols(p * p.x, p * p.y, p * p.z)
}

/// Mass accumulator in the body frame: total mass, first moment and the
/// inertia tensor about the body origin C. Adding two combines bodies.
#[derive(Clone, Copy, Debug)]
pub struct Mass {
    pub m: f64,
    pub s: DVec3,
    pub i0: DMat3,
}

impl Default for Mass {
    fn default() -> Self {
        Mass { m: 0.0, s: DVec3::ZERO, i0: DMat3::ZERO }
    }
}

impl Mass {
    /// Mass m centred at p, with its own inertia tensor `own` about p.
    pub fn at(m: f64, p: DVec3, own: DMat3) -> Mass {
        Mass { m, s: p * m, i0: own + point_tensor(p) * m }
    }
    /// One brick (treated as a unit cube, mass 1) centred at p.
    pub fn cell(p: DVec3) -> Mass {
        Mass::at(1.0, p, DMat3::from_diagonal(DVec3::splat(1.0 / 6.0)))
    }
    pub fn plus(self, o: Mass) -> Mass {
        Mass { m: self.m + o.m, s: self.s + o.s, i0: self.i0 + o.i0 }
    }
    pub fn scaled(self, k: f64) -> Mass {
        Mass { m: self.m * k, s: self.s * k, i0: self.i0 * k }
    }
    pub fn com(&self) -> DVec3 {
        self.s / self.m
    }
    /// Inertia tensor about the centre of mass.
    pub fn i_com(&self) -> DMat3 {
        self.i0 - point_tensor(self.com()) * self.m
    }
}

/// The solid spherical cap of radius rho spanning ±CAP_ANGLE, scaled to `total`
/// mass. Integrated as 4000 thin discs (error well under 0.1%).
pub fn cap_mass(rho: f64, total: f64) -> Mass {
    const N: usize = 4000;
    let top = -rho * CAP_ANGLE.cos();
    let dy = (top + rho) / N as f64;
    let mut acc = Mass::default();
    for k in 0..N {
        let y = -rho + (k as f64 + 0.5) * dy;
        let a2 = rho * rho - y * y;
        let dm = std::f64::consts::PI * a2 * dy;
        let own = DMat3::from_diagonal(DVec3::new(a2 / 4.0, a2 / 2.0, a2 / 4.0)) * dm;
        acc = acc.plus(Mass::at(dm, DVec3::new(0.0, y, 0.0), own));
    }
    acc.scaled(total / acc.m)
}

#[derive(Clone, Copy, Debug)]
pub struct Rocker {
    /// Orientation, body → world.
    pub q: DQuat,
    /// World position of the sphere's centre C (always at height ρ).
    pub c: DVec3,
    /// Angular velocity, world frame.
    pub w: DVec3,
    /// Radius of the spherical underside.
    pub rho: f64,
    pub mass: Mass,
}

impl Rocker {
    pub fn new(mass: Mass, rho: f64) -> Rocker {
        Rocker { q: DQuat::IDENTITY, c: DVec3::new(0.0, rho, 0.0), w: DVec3::ZERO, rho, mass }
    }

    pub fn to_world(&self, b: DVec3) -> DVec3 {
        self.c + self.q * b
    }
    pub fn to_body(&self, p: DVec3) -> DVec3 {
        self.q.inverse() * (p - self.c)
    }
    pub fn contact(&self) -> DVec3 {
        DVec3::new(self.c.x, 0.0, self.c.z)
    }
    /// The tower axis (body +Y) in the world.
    pub fn axis(&self) -> DVec3 {
        self.q * UP
    }
    /// Angle between the tower axis and the vertical.
    pub fn tilt(&self) -> f64 {
        self.axis().y.clamp(-1.0, 1.0).acos()
    }
    /// Horizontal direction the tower leans toward (world x, z), length = sin(tilt).
    pub fn lean(&self) -> (f64, f64) {
        let a = self.axis();
        (a.x, a.z)
    }
    /// World velocity of the body point at b.
    pub fn velocity_of(&self, b: DVec3) -> DVec3 {
        self.w.cross(self.q * b + UP * self.rho)
    }

    /// Inertia about G and about the contact point, world frame, plus d = G − P.
    fn tensors(&self, q: DQuat, mass: &Mass) -> (DMat3, DMat3, DVec3, DVec3) {
        let r = DMat3::from_quat(q);
        let g = r * mass.com();
        let d = g + UP * self.rho;
        let i_g = r * mass.i_com() * r.transpose();
        let i_p = i_g + point_tensor(d) * mass.m;
        (i_g, i_p, g, d)
    }

    fn accel(&self, q: DQuat, w: DVec3) -> DVec3 {
        let m = self.mass.m;
        let (i_g, i_p, g, d) = self.tensors(q, &self.mass);
        let b = w.cross(w.cross(g));
        let grav = DVec3::new(0.0, -G, 0.0);
        let mut rhs = -w.cross(i_g * w) - d.cross(b - grav) * m;
        rhs -= i_p * w * DAMPING;
        let ipy = UP.dot(i_p * UP);
        rhs -= UP * (PIVOT_DAMPING * ipy * w.y);
        i_p.inverse() * rhs
    }

    fn qdot(q: DQuat, w: DVec3) -> DQuat {
        DQuat::from_xyzw(w.x, w.y, w.z, 0.0) * q * 0.5
    }

    fn cdot(&self, w: DVec3) -> DVec3 {
        w.cross(UP * self.rho)
    }

    /// Advances the simulation by `dt` seconds in DT substeps (RK2 midpoint).
    pub fn step(&mut self, dt: f64) {
        let mut left = dt;
        while left > 1e-12 {
            let h = left.min(DT);
            let a1 = self.accel(self.q, self.w);
            let qm = (self.q + Self::qdot(self.q, self.w) * (0.5 * h)).normalize();
            let wm = self.w + a1 * (0.5 * h);
            let a2 = self.accel(qm, wm);
            self.q = (self.q + Self::qdot(qm, wm) * h).normalize();
            self.c += self.cdot(wm) * h;
            self.w += a2 * h;
            self.rim_stop();
            left -= h;
        }
    }

    /// Past CAP_ANGLE the rim of the cap meets the floor: the tilt is held at
    /// the limit and any motion tipping it further is stopped (inelastic).
    /// Simplification: the body is not allowed to pivot onto the rim.
    fn rim_stop(&mut self) {
        let a = self.axis();
        let tilt = a.y.clamp(-1.0, 1.0).acos();
        if tilt <= CAP_ANGLE {
            return;
        }
        let u = a.cross(UP);
        if u.length_squared() < 1e-18 {
            return;
        }
        let u = u.normalize();
        self.q = (DQuat::from_axis_angle(u, tilt - CAP_ANGLE) * self.q).normalize();
        let back = self.w.dot(u);
        if back < 0.0 {
            self.w -= u * back;
        }
    }

    /// Turns the whole body by `delta` radians about the vertical through C
    /// (the player twisting the tower; pivoting is allowed by the contact).
    /// Energy-neutral: C, the contact point and the COM height don't change.
    pub fn twist(&mut self, delta: f64) {
        let r = DQuat::from_rotation_y(delta);
        self.q = (r * self.q).normalize();
        self.w = r * self.w;
    }

    /// Adds mass that arrives falling along the tower axis at `speed` (cells/s).
    /// Angular momentum about the contact point is conserved through the
    /// (inelastic) impact; for a rolling body that momentum is I_P·ω.
    pub fn land(&mut self, add: Mass, speed: f64) {
        let (_, i_old, _, _) = self.tensors(self.q, &self.mass);
        let r = self.to_world(add.com()) - self.contact();
        let v = -self.axis() * speed;
        let l = i_old * self.w + r.cross(v) * add.m;
        self.mass = self.mass.plus(add);
        let (_, i_new, _, _) = self.tensors(self.q, &self.mass);
        self.w = i_new.inverse() * l;
    }

    /// Replaces the mass (rings cleared, stack gone). Leaving cells take their
    /// own share of angular momentum, so ω is kept.
    pub fn set_mass(&mut self, mass: Mass) {
        self.mass = mass;
    }

    /// Resting tilt for a mass, or None when the COM is at or above C
    /// (top-heavy: upright is unstable and there is no resting angle).
    pub fn equilibrium(mass: &Mass) -> Option<f64> {
        let c = mass.com();
        if c.y >= 0.0 {
            return None;
        }
        Some(c.x.hypot(c.z).atan2(-c.y))
    }

    /// Kinetic + potential energy (for tests).
    #[cfg(test)]
    pub fn energy(&self) -> f64 {
        let (_, i_p, g, _) = self.tensors(self.q, &self.mass);
        0.5 * self.w.dot(i_p * self.w) + self.mass.m * G * (self.c.y + g.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::SQRT_2;

    const RHO: f64 = 5.59 * SQRT_2;

    fn base() -> Mass {
        cap_mass(RHO, 96.0)
    }

    #[test]
    fn cap_centroid_and_volume_match_formulas() {
        // Solid cap of height h: centroid 3(2ρ−h)²/(4(3ρ−h)) below the centre.
        let h = RHO * (1.0 - CAP_ANGLE.cos());
        let want = 3.0 * (2.0 * RHO - h).powi(2) / (4.0 * (3.0 * RHO - h));
        let m = base();
        let c = m.com();
        assert!((m.m - 96.0).abs() < 1e-9);
        assert!(c.x.abs() < 1e-12 && c.z.abs() < 1e-12);
        assert!((c.y + want).abs() < 1e-3, "centroid {} vs {}", c.y, -want);
        // symmetric about the axis: Ixx = Izz, no products of inertia
        let i = m.i_com();
        assert!((i.x_axis.x - i.z_axis.z).abs() < 1e-6);
        assert!(i.x_axis.y.abs() < 1e-9 && i.x_axis.z.abs() < 1e-9);
    }

    #[test]
    fn empty_base_settles_upright_from_any_direction() {
        for axis in [DVec3::X, DVec3::Z, DVec3::new(1.0, 0.0, -1.0).normalize()] {
            let mut r = Rocker::new(base(), RHO);
            r.q = DQuat::from_axis_angle(axis, 0.35);
            r.step(20.0);
            assert!(r.tilt() < 0.01, "tilt {}", r.tilt());
        }
    }

    #[test]
    fn energy_never_increases() {
        let mut r = Rocker::new(base().plus(Mass::cell(DVec3::new(3.0, -5.0, 2.0))), RHO);
        r.q = DQuat::from_axis_angle(DVec3::new(1.0, 0.0, 0.4).normalize(), 0.3);
        r.w = DVec3::new(0.0, 1.5, 0.0);
        let mut e = r.energy();
        for _ in 0..2000 {
            r.step(0.005);
            let e2 = r.energy();
            assert!(e2 <= e + 1e-6, "energy grew {e} -> {e2}");
            e = e2;
        }
    }

    /// Without damping, rolling is conservative: the energy must stay put.
    #[test]
    fn undamped_rolling_conserves_energy() {
        let m = base().plus(Mass::cell(DVec3::new(3.0, -5.0, 2.0))).plus(Mass::cell(DVec3::new(-1.0, 2.0, -4.0)));
        let mut r = Rocker::new(m, RHO);
        r.q = DQuat::from_axis_angle(DVec3::new(0.3, 0.0, 1.0).normalize(), 0.4);
        r.w = DVec3::new(0.2, 2.0, -0.3);
        let e0 = r.energy();
        let mut t = 0.0;
        while t < 5.0 {
            // the accel() damping terms are bypassed by integrating by hand
            let h = DT;
            let a1 = r.accel_free(r.q, r.w);
            let qm = (r.q + Rocker::qdot(r.q, r.w) * (0.5 * h)).normalize();
            let wm = r.w + a1 * (0.5 * h);
            let a2 = r.accel_free(qm, wm);
            r.q = (r.q + Rocker::qdot(qm, wm) * h).normalize();
            r.c += r.cdot(wm) * h;
            r.w += a2 * h;
            t += h;
        }
        let e1 = r.energy();
        assert!(((e1 - e0) / e0.abs()).abs() < 1e-4, "energy drift {e0} -> {e1}");
    }

    impl Rocker {
        fn accel_free(&self, q: DQuat, w: DVec3) -> DVec3 {
            let m = self.mass.m;
            let (i_g, i_p, g, d) = self.tensors(q, &self.mass);
            let b = w.cross(w.cross(g));
            let rhs = -w.cross(i_g * w) - d.cross(b - DVec3::new(0.0, -G, 0.0)) * m;
            i_p.inverse() * rhs
        }
    }

    /// A load in the body's x–y plane rocks in that plane only, exactly like
    /// Rocker's 1-DOF model (02_rocker/src/physics.rs), integrated here side by side.
    #[test]
    fn planar_case_matches_the_2d_rocker() {
        let mut m = base();
        for k in 0..4 {
            m = m.plus(Mass::cell(DVec3::new(1.5 + k as f64, -5.0, 0.0)));
        }
        let mut r = Rocker::new(m, RHO);
        let th0 = 0.2;
        r.q = DQuat::from_rotation_z(th0);
        // 2D model: θ about +z (counter-clockwise, x → y)
        let com = m.com();
        let izz = m.i_com().z_axis.z;
        let acc2d = |th: f64, om: f64| {
            let (s, c) = th.sin_cos();
            let rc = (com.x * c - com.y * s, com.x * s + com.y * c);
            let i = izz + m.m * (rc.0 * rc.0 + (RHO + rc.1) * (RHO + rc.1));
            let du = m.m * G * rc.0;
            let di = 2.0 * m.m * RHO * rc.0;
            -(du + 0.5 * di * om * om + DAMPING * i * om) / i
        };
        let (mut th, mut om) = (th0, 0.0);
        let mut t = 0.0;
        while t < 3.0 {
            let h = DT;
            let a1 = acc2d(th, om);
            let (tm, wm) = (th + 0.5 * h * om, om + 0.5 * h * a1);
            let a2 = acc2d(tm, wm);
            th += h * wm;
            om += h * a2;
            r.step(h);
            t += h;
        }
        let (axis, angle) = r.q.to_axis_angle();
        let th3 = angle * axis.z.signum();
        assert!((th3 - th).abs() < 1e-3, "3D {th3} vs 2D {th}");
        assert!(r.w.x.abs() < 1e-9 && r.w.y.abs() < 1e-9, "stays planar: {:?}", r.w);
        // rolled without slipping: C moved −ρ·Δθ along x
        assert!((r.c.x + RHO * (th3 - th0)).abs() < 1e-6, "c.x {}", r.c.x);
        assert!((r.c.y - RHO).abs() < 1e-12);
    }

    #[test]
    fn off_centre_load_settles_at_predicted_angle_over_the_contact() {
        let mut m = base();
        for k in 0..6 {
            m = m.plus(Mass::cell(DVec3::new(3.0, -5.0 + k as f64, 3.0)));
        }
        let want = Rocker::equilibrium(&m).unwrap();
        let mut r = Rocker::new(m, RHO);
        r.step(40.0);
        assert!((r.tilt() - want).abs() < 1e-3, "{} vs {}", r.tilt(), want);
        let g = r.to_world(m.com());
        let p = r.contact();
        assert!((g.x - p.x).hypot(g.z - p.z) < 1e-3, "COM over contact");
        // and it leans toward the load (+x, +z)
        let (lx, lz) = r.lean();
        assert!(lx > 0.0 && lz > 0.0, "lean {lx} {lz}");
    }

    #[test]
    fn tall_balanced_tower_is_unstable() {
        let mut m = base();
        let r_mid = 32.0 / std::f64::consts::TAU;
        for row in 0..18 {
            for col in 0..32 {
                let a = col as f64 * std::f64::consts::TAU / 32.0;
                m = m.plus(Mass::cell(DVec3::new(r_mid * a.sin(), -5.09 + row as f64, r_mid * a.cos())));
            }
        }
        assert!(Rocker::equilibrium(&m).is_none());
        let mut r = Rocker::new(m, RHO);
        r.q = DQuat::from_rotation_x(0.01);
        r.step(10.0);
        assert!(r.tilt() > SLIDE_ANGLE);
    }

    #[test]
    fn tilt_never_passes_the_rim() {
        let mut m = base();
        for k in 0..40 {
            m = m.plus(Mass::cell(DVec3::new(4.5, -5.0 + (k / 2) as f64, (k % 2) as f64)));
        }
        let mut r = Rocker::new(m, RHO);
        let mut peak: f64 = 0.0;
        for _ in 0..1000 {
            r.step(0.01);
            peak = peak.max(r.tilt());
        }
        assert!(peak > SLIDE_ANGLE);
        assert!(peak <= CAP_ANGLE + 1e-6, "peak {}", peak.to_degrees());
    }

    #[test]
    fn landing_on_the_right_tips_it_right() {
        let mut r = Rocker::new(base(), RHO);
        r.land(Mass::cell(DVec3::new(5.0, -5.0, 0.0)), 6.0);
        // tipping toward +x is a rotation about −z
        assert!(r.w.z < 0.0 && r.w.x.abs() < 1e-12);
        r.step(0.2);
        assert!(r.lean().0 > 0.0);
    }

    #[test]
    fn twist_keeps_energy_contact_and_tilt() {
        let mut r = Rocker::new(base().plus(Mass::cell(DVec3::new(4.0, -5.0, 0.0))), RHO);
        r.q = DQuat::from_rotation_z(-0.2);
        r.w = DVec3::new(0.0, 0.0, -0.5);
        let (e, c, t) = (r.energy(), r.c, r.tilt());
        r.twist(1.0);
        assert!((r.energy() - e).abs() < 1e-9);
        assert!((r.c - c).length() < 1e-12);
        assert!((r.tilt() - t).abs() < 1e-12);
        // the lean turned with the tower: +x lean rotated by 1 rad about y
        let (lx, lz) = r.lean();
        assert!((lz.atan2(lx) + 1.0).abs() < 1e-9, "lean angle {}", lz.atan2(lx));
    }

    #[test]
    fn body_world_round_trip() {
        let mut r = Rocker::new(base(), RHO);
        r.q = DQuat::from_euler(macroquad::math::EulerRot::XYZ, 0.3, 1.2, -0.4);
        r.c = DVec3::new(2.0, RHO, -1.0);
        let b = DVec3::new(1.3, -2.7, 0.4);
        assert!((r.to_body(r.to_world(b)) - b).length() < 1e-12);
    }
}
