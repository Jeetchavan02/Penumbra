//! Three-component `f64` vector used for points, directions and linear RGB.

use std::iter::Sum;
use std::ops::{Add, AddAssign, Div, DivAssign, Index, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A position in world space.
pub type Point3 = Vec3;
/// Linear RGB radiance or reflectance.
pub type Color = Vec3;

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::splat(0.0);
    pub const ONE: Vec3 = Vec3::splat(1.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    pub const fn splat(v: f64) -> Self {
        Vec3 { x: v, y: v, z: v }
    }

    pub fn dot(self, o: Vec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn length_squared(self) -> f64 {
        self.dot(self)
    }

    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }

    /// Unit vector in the same direction. The caller guarantees a non-zero length;
    /// use [`Vec3::try_normalized`] when that is not known.
    pub fn normalized(self) -> Vec3 {
        self / self.length()
    }

    /// `None` when the length is zero, vanishingly small or not finite.
    pub fn try_normalized(self) -> Option<Vec3> {
        let len = self.length();
        (len > 1e-12 && len.is_finite()).then(|| self / len)
    }

    pub fn near_zero(self) -> bool {
        const EPS: f64 = 1e-8;
        self.x.abs() < EPS && self.y.abs() < EPS && self.z.abs() < EPS
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    /// Component-wise minimum.
    pub fn min(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z))
    }

    /// Component-wise maximum.
    pub fn max(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z))
    }

    pub fn max_component(self) -> f64 {
        self.x.max(self.y).max(self.z)
    }

    pub fn min_component(self) -> f64 {
        self.x.min(self.y).min(self.z)
    }

    pub fn lerp(a: Vec3, b: Vec3, t: f64) -> Vec3 {
        a * (1.0 - t) + b * t
    }

    /// Mirror `self` about the unit normal `n`.
    pub fn reflect(self, n: Vec3) -> Vec3 {
        self - n * (2.0 * self.dot(n))
    }

    /// Refract the unit vector `self` through a surface with unit normal `n`
    /// (opposing `self`) and index ratio `eta_in / eta_out`.
    ///
    /// The caller must have ruled out total internal reflection.
    pub fn refract(self, n: Vec3, eta_ratio: f64) -> Vec3 {
        let cos_theta = (-self).dot(n).min(1.0);
        let perpendicular = (self + n * cos_theta) * eta_ratio;
        let parallel = n * -(1.0 - perpendicular.length_squared()).max(0.0).sqrt();
        perpendicular + parallel
    }

    /// Two unit vectors that, with the unit vector `self`, form an orthonormal
    /// basis. Branch-free construction from Duff et al. (2017).
    pub fn orthonormal_basis(self) -> (Vec3, Vec3) {
        let sign = 1.0_f64.copysign(self.z);
        let a = -1.0 / (sign + self.z);
        let b = self.x * self.y * a;
        (
            Vec3::new(1.0 + sign * self.x * self.x * a, sign * b, -sign * self.x),
            Vec3::new(b, sign + self.y * self.y * a, -self.y),
        )
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f64) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Mul<Vec3> for f64 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        v * self
    }
}

/// Component-wise product, used to tint radiance by reflectance.
impl Mul<Vec3> for Vec3 {
    type Output = Vec3;
    fn mul(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
}

impl Div<f64> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f64) -> Vec3 {
        self * (1.0 / s)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}

impl SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Vec3) {
        *self = *self - o;
    }
}

impl MulAssign<f64> for Vec3 {
    fn mul_assign(&mut self, s: f64) {
        *self = *self * s;
    }
}

impl MulAssign<Vec3> for Vec3 {
    fn mul_assign(&mut self, o: Vec3) {
        *self = *self * o;
    }
}

impl DivAssign<f64> for Vec3 {
    fn div_assign(&mut self, s: f64) {
        *self = *self / s;
    }
}

impl Index<usize> for Vec3 {
    type Output = f64;
    fn index(&self, axis: usize) -> &f64 {
        match axis {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            _ => panic!("Vec3 axis out of range: {axis}"),
        }
    }
}

impl Sum for Vec3 {
    fn sum<I: Iterator<Item = Vec3>>(iter: I) -> Vec3 {
        iter.fold(Vec3::ZERO, |a, b| a + b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-12
    }

    #[test]
    fn cross_is_right_handed() {
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        assert_eq!(x.cross(y), Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn try_normalized_rejects_zero_and_nan() {
        assert!(Vec3::ZERO.try_normalized().is_none());
        assert!(Vec3::new(f64::NAN, 0.0, 0.0).try_normalized().is_none());
        let n = Vec3::new(3.0, 0.0, 4.0).try_normalized().unwrap();
        assert!((n.length() - 1.0).abs() < 1e-15);
    }

    #[test]
    fn reflect_flips_the_normal_component() {
        let v = Vec3::new(1.0, -1.0, 0.0);
        assert_eq!(v.reflect(Vec3::new(0.0, 1.0, 0.0)), Vec3::new(1.0, 1.0, 0.0));
    }

    #[test]
    fn refract_with_matched_indices_is_identity() {
        let v = Vec3::new(1.0, -1.0, 0.5).normalized();
        assert!(close(v.refract(Vec3::new(0.0, 1.0, 0.0), 1.0), v));
    }

    #[test]
    fn refract_obeys_snell() {
        let n = Vec3::new(0.0, 1.0, 0.0);
        let v = Vec3::new(1.0, -1.0, 0.0).normalized();
        let r = v.refract(n, 1.0 / 1.5);
        let sin_in = v.cross(n).length();
        let sin_out = r.cross(n).length();
        assert!((sin_in - 1.5 * sin_out).abs() < 1e-12);
        assert!((r.length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn orthonormal_basis_is_orthonormal() {
        for w in [
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(1.0, 2.0, 3.0).normalized(),
            Vec3::new(-0.3, 0.9, -0.1).normalized(),
        ] {
            let (u, v) = w.orthonormal_basis();
            assert!(u.dot(v).abs() < 1e-12 && u.dot(w).abs() < 1e-12 && v.dot(w).abs() < 1e-12);
            assert!((u.length() - 1.0).abs() < 1e-12 && (v.length() - 1.0).abs() < 1e-12);
            assert!(close(u.cross(v), w));
        }
    }
}
