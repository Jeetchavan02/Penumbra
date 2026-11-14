//! Primary ray generation with an optional thin lens.

use crate::math::{Point3, Ray, Sampler, Vec3};

/// Resolution-independent description of a camera; part of a scene.
#[derive(Debug, Clone, Copy)]
pub struct CameraSpec {
    pub look_from: Point3,
    pub look_at: Point3,
    pub up: Vec3,
    /// Vertical field of view in degrees.
    pub vfov: f64,
    /// Width divided by height.
    pub aspect_ratio: f64,
    /// Angle in degrees subtended by the lens at the focus point. Zero is a
    /// pinhole: everything in focus.
    pub defocus_angle: f64,
    /// Distance to the plane of perfect focus.
    pub focus_distance: f64,
}

impl Default for CameraSpec {
    fn default() -> Self {
        CameraSpec {
            look_from: Point3::new(0.0, 0.0, 1.0),
            look_at: Point3::ZERO,
            up: Vec3::new(0.0, 1.0, 0.0),
            vfov: 40.0,
            aspect_ratio: 16.0 / 9.0,
            defocus_angle: 0.0,
            focus_distance: 1.0,
        }
    }
}

impl CameraSpec {
    /// Image height for a given width, at least one pixel.
    pub fn height_for(&self, width: u32) -> u32 {
        ((width as f64 / self.aspect_ratio).round() as u32).max(1)
    }
}

/// A camera bound to a resolution, with everything precomputed.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    origin: Point3,
    /// World position of the centre of pixel (0, 0).
    pixel00: Point3,
    /// World-space step between horizontally and vertically adjacent pixels.
    pixel_du: Vec3,
    pixel_dv: Vec3,
    /// Lens radius vectors; zero for a pinhole.
    lens_u: Vec3,
    lens_v: Vec3,
}

impl Camera {
    /// Panics on a degenerate setup. A camera whose `up` is parallel to its
    /// view direction would otherwise render a frame of NaN without complaint.
    pub fn new(spec: &CameraSpec, width: u32, height: u32) -> Self {
        assert!(width > 0 && height > 0, "image must have at least one pixel");
        assert!(spec.vfov > 0.0 && spec.vfov < 180.0, "vfov must be in (0, 180) degrees");
        assert!(spec.focus_distance > 0.0, "focus distance must be positive");

        let w = (spec.look_from - spec.look_at)
            .try_normalized()
            .expect("camera look_from and look_at must differ");
        let u = spec.up.cross(w).try_normalized().expect("camera up must not be parallel to the view direction");
        let v = w.cross(u);

        let viewport_height = 2.0 * (spec.vfov.to_radians() / 2.0).tan() * spec.focus_distance;
        let viewport_width = viewport_height * (width as f64 / height as f64);
        let viewport_u = u * viewport_width;
        let viewport_v = -v * viewport_height;
        let pixel_du = viewport_u / width as f64;
        let pixel_dv = viewport_v / height as f64;
        let upper_left = spec.look_from - w * spec.focus_distance - viewport_u / 2.0 - viewport_v / 2.0;

        let lens_radius = spec.focus_distance * (spec.defocus_angle.max(0.0).to_radians() / 2.0).tan();
        Camera {
            origin: spec.look_from,
            pixel00: upper_left + (pixel_du + pixel_dv) * 0.5,
            pixel_du,
            pixel_dv,
            lens_u: u * lens_radius,
            lens_v: v * lens_radius,
        }
    }

    /// A ray through a random point of pixel `(x, y)`; `y` grows downward.
    pub fn ray(&self, x: u32, y: u32, sampler: &mut Sampler) -> Ray {
        let sx = x as f64 + sampler.uniform() - 0.5;
        let sy = y as f64 + sampler.uniform() - 0.5;
        let target = self.pixel00 + self.pixel_du * sx + self.pixel_dv * sy;

        let (lx, ly) = sampler.in_unit_disk();
        let origin = self.origin + self.lens_u * lx + self.lens_v * ly;
        Ray::new(origin, (target - origin).normalized())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> CameraSpec {
        CameraSpec {
            look_from: Point3::new(0.0, 0.0, 5.0),
            vfov: 90.0,
            aspect_ratio: 2.0,
            focus_distance: 5.0,
            ..CameraSpec::default()
        }
    }

    #[test]
    fn centre_pixel_looks_at_the_target() {
        let cam = Camera::new(&spec(), 201, 101);
        let mut s = Sampler::new(1);
        for _ in 0..100 {
            let r = cam.ray(100, 50, &mut s);
            assert_eq!(r.origin(), Point3::new(0.0, 0.0, 5.0));
            assert!(r.dir().dot(Vec3::new(0.0, 0.0, -1.0)) > 0.9999);
        }
    }

    #[test]
    fn image_axes_are_not_mirrored() {
        let cam = Camera::new(&spec(), 200, 100);
        let mut s = Sampler::new(1);
        let top_left = cam.ray(0, 0, &mut s).dir();
        let bottom_right = cam.ray(199, 99, &mut s).dir();
        assert!(top_left.x < 0.0 && top_left.y > 0.0);
        assert!(bottom_right.x > 0.0 && bottom_right.y < 0.0);
    }

    #[test]
    fn vertical_fov_spans_the_image_height() {
        let cam = Camera::new(&spec(), 200, 100);
        let mut s = Sampler::new(1);
        // With a 90 degree vfov the top edge is 45 degrees above the axis.
        let top = cam.ray(100, 0, &mut s).dir();
        assert!((top.y / -top.z - 1.0).abs() < 0.02);
    }

    #[test]
    fn lens_rays_converge_on_the_focus_plane() {
        let cam = Camera::new(&CameraSpec { defocus_angle: 10.0, ..spec() }, 201, 101);
        let mut s = Sampler::new(1);
        let mut origins_differ = false;
        for _ in 0..200 {
            let r = cam.ray(100, 50, &mut s);
            origins_differ |= r.origin() != Point3::new(0.0, 0.0, 5.0);
            // Intersect with the focus plane z = 0; all rays must land within the pixel.
            let p = r.at(-r.origin().z / r.dir().z);
            assert!(p.x.abs() < 0.06 && p.y.abs() < 0.06, "{p:?}");
        }
        assert!(origins_differ);
    }

    #[test]
    #[should_panic(expected = "must not be parallel")]
    fn rejects_up_parallel_to_view() {
        let bad = CameraSpec { look_from: Point3::new(0.0, 5.0, 0.0), ..CameraSpec::default() };
        let _ = Camera::new(&bad, 10, 10);
    }

    #[test]
    fn height_follows_aspect_ratio() {
        assert_eq!(spec().height_for(400), 200);
        assert_eq!(spec().height_for(1), 1);
    }
}
