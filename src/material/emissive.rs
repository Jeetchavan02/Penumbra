use crate::math::Color;

/// A light source. Emits uniformly from both sides and scatters nothing.
#[derive(Debug, Clone, Copy)]
pub struct Emissive {
    pub radiance: Color,
}
