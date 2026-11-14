/// A range of ray parameters or coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub min: f64,
    pub max: f64,
}

impl Interval {
    pub const EMPTY: Interval = Interval::new(f64::INFINITY, f64::NEG_INFINITY);
    pub const UNIVERSE: Interval = Interval::new(f64::NEG_INFINITY, f64::INFINITY);

    pub const fn new(min: f64, max: f64) -> Self {
        Interval { min, max }
    }

    pub fn size(&self) -> f64 {
        self.max - self.min
    }

    /// Closed-interval membership.
    pub fn contains(&self, x: f64) -> bool {
        self.min <= x && x <= self.max
    }

    /// Open-interval membership. False for NaN, so a NaN root is never a hit.
    pub fn surrounds(&self, x: f64) -> bool {
        self.min < x && x < self.max
    }

    pub fn clamp(&self, x: f64) -> f64 {
        x.clamp(self.min, self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surrounds_is_open_and_rejects_nan() {
        let i = Interval::new(0.0, 1.0);
        assert!(i.surrounds(0.5));
        assert!(!i.surrounds(0.0) && !i.surrounds(1.0));
        assert!(i.contains(0.0) && i.contains(1.0));
        assert!(!i.surrounds(f64::NAN) && !i.contains(f64::NAN));
    }

    #[test]
    fn empty_contains_nothing() {
        assert!(!Interval::EMPTY.contains(0.0));
        assert!(Interval::UNIVERSE.contains(1e300));
    }
}
