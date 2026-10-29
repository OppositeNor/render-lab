//! Orbit camera, CameraData construction, and CPU projection/picking.
//! Clip space follows wgpu: right-handed view, depth in [0, 1], NDC +Y up.

use crate::layout::CameraData;
use glam::{Mat4, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

pub const NEAR: f32 = 0.03;
pub const FAR: f32 = 60.0;

/// Its values are `camera.yaw`, `camera.pitch`, `camera.distance`,
/// `camera.fov`, and `camera.target` (`camera.x`, `camera.y`, `camera.z`),
/// as timelines and stories name them (src/values.rs); the dock's camera
/// group shows those labelled `view`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, crate::Values)]
#[values(scope = "camera", package = "rlstd")]
pub struct OrbitCamera {
    /// The point it orbits.
    #[value(components(x, y, z))]
    pub target: [f32; 3],
    /// Degrees around +Y; 0 looks down -Z.
    #[value(range = -180.0..=180.0, unit = deg, wrap, label(view))]
    pub yaw: f32,
    /// Degrees above the horizon.
    #[value(range = -85.0..=85.0, unit = deg, label(view))]
    pub pitch: f32,
    /// Metres from the target.
    #[value(range = 0.4..=15.0, unit = m, label(view), text = "controls.distance")]
    pub distance: f32,
    /// Vertical field of view in degrees.
    #[value(range = 20.0..=90.0, unit = deg, label(view))]
    pub fov: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        OrbitCamera { target: [0.0, 0.9, 0.0], yaw: 18.0, pitch: 16.0, distance: 6.8, fov: 50.0 }
    }
}

impl OrbitCamera {
    pub fn eye(&self) -> Vec3 {
        let (y, p) = (self.yaw.to_radians(), self.pitch.to_radians());
        Vec3::from(self.target) + self.distance * Vec3::new(p.cos() * y.sin(), p.sin(), p.cos() * y.cos())
    }

    pub fn view(&self) -> Mat4 {
        glam::camera::rh::view::look_at_mat4(self.eye(), Vec3::from(self.target), Vec3::Y)
    }

    pub fn projection(&self, aspect: f32) -> Mat4 {
        // wgpu clip space: depth in [0, 1], NDC +Y up (glam's "directx" convention),
        // reversed: the near plane at depth 1 and the far plane at 0, as in
        // O3DE, so depth precision follows distance (near and far swapped).
        glam::camera::rh::proj::directx::perspective(self.fov.to_radians(), aspect, FAR, NEAR)
    }

    pub fn camera_data(&self, viewport: [f32; 2]) -> CameraData {
        let view = self.view();
        let proj = self.projection(viewport[0] / viewport[1].max(1.0));
        CameraData {
            world_to_view: view.to_cols_array_2d(),
            view_to_clip: proj.to_cols_array_2d(),
            world_to_clip: (proj * view).to_cols_array_2d(),
            position: self.eye().to_array(),
            vertical_fov: self.fov.to_radians(),
            viewport_size: viewport,
            near_plane: NEAR,
            far_plane: FAR,
        }
    }

    /// Orbit by a pointer drag in viewport pixels.
    pub fn orbit(&mut self, delta: Vec2) {
        self.yaw -= delta.x * 0.3;
        self.pitch = (self.pitch + delta.y * 0.3).clamp(-85.0, 85.0);
    }

    /// Pan the target in the view plane by a pointer drag in viewport pixels.
    pub fn pan(&mut self, delta: Vec2, viewport_height: f32) {
        let view = self.view();
        let right = Vec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
        let up = Vec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y);
        let scale = 2.0 * self.distance * (self.fov.to_radians() * 0.5).tan() / viewport_height.max(1.0);
        let t = Vec3::from(self.target) - right * delta.x * scale + up * delta.y * scale;
        self.target = t.to_array();
    }

    pub fn zoom(&mut self, steps: f32) {
        self.distance = (self.distance * 0.9f32.powf(steps)).clamp(0.4, 30.0);
    }
}

/// Projection helpers built from the same matrix the GPU receives.
#[derive(Clone, Copy, Debug)]
pub struct Projector {
    pub world_to_clip: Mat4,
    pub clip_to_world: Mat4,
    pub viewport: Vec2,
    pub eye: Vec3,
}

impl Projector {
    pub fn new(camera: &CameraData) -> Self {
        let m = Mat4::from_cols_array_2d(&camera.world_to_clip);
        Projector { world_to_clip: m, clip_to_world: m.inverse(), viewport: Vec2::from(camera.viewport_size), eye: Vec3::from(camera.position) }
    }

    /// Pixel position inside the viewport (origin top left, +Y down) and
    /// clip w, or None behind the camera.
    pub fn project(&self, p: Vec3) -> Option<(Vec2, f32)> {
        let c = self.world_to_clip * Vec4::new(p.x, p.y, p.z, 1.0);
        if c.w <= 1e-4 {
            return None;
        }
        let ndc = Vec2::new(c.x / c.w, c.y / c.w);
        Some((Vec2::new((ndc.x * 0.5 + 0.5) * self.viewport.x, (0.5 - ndc.y * 0.5) * self.viewport.y), c.w))
    }

    /// World-space ray through a viewport pixel.
    pub fn ray(&self, pixel: Vec2) -> (Vec3, Vec3) {
        let ndc = Vec2::new(pixel.x / self.viewport.x * 2.0 - 1.0, 1.0 - pixel.y / self.viewport.y * 2.0);
        // Reversed depth: the far plane is at depth 0.
        let far = self.clip_to_world * Vec4::new(ndc.x, ndc.y, 0.0, 1.0);
        let far = far.truncate() / far.w;
        (self.eye, (far - self.eye).normalize())
    }
}
