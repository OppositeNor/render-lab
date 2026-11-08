//! An image drawn as a height field: one column per texel (or per block of
//! texels), as tall as its value, on a plane placed in the scene
//! (`DrawLists::cuboid`). The plane can cover the camera's view exactly, so
//! that with no height the field is the image as the screen shows it, and
//! tilt back from there, the columns rising toward the camera: a depth map
//! becoming the terrain a ray marches over.

use crate::frame::DrawLists;
use crate::layout::CameraData;
use egui::Color32;
use glam::{Mat4, Quat, Vec2, Vec3};

/// Where a field lies: the point of its window's top-left corner, the vectors
/// spanning the window across (u, 0 to 1) and down (v, 0 to 1), and one unit
/// of height.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub origin: Vec3,
    pub across: Vec3,
    pub down: Vec3,
    pub up: Vec3,
}

impl Placement {
    /// A field in front of `camera`, `distance` away (nearer than anything in
    /// the scene), heights `height` long at their full value. At `tilt` 0 the
    /// window covers the view, u across and v down as on the screen, and the
    /// columns rise toward the camera; `tilt` (degrees) lays it back about its
    /// centre's horizontal axis, `turn` (degrees) then turns it about its own
    /// up, and `scale` shrinks it about its centre.
    pub fn facing(
        camera: &CameraData,
        distance: f32,
        height: f32,
        tilt: f32,
        turn: f32,
        scale: f32,
    ) -> Placement {
        let view = Mat4::from_cols_array_2d(&camera.world_to_view);
        let to_world = view.inverse();
        let right = to_world.transform_vector3(Vec3::X).normalize();
        let up = to_world.transform_vector3(Vec3::Y).normalize();
        let back = to_world.transform_vector3(Vec3::Z).normalize();
        let eye = Vec3::from(camera.position);
        let half_h = distance * (camera.vertical_fov * 0.5).tan();
        let half_w = half_h * camera.viewport_size[0] / camera.viewport_size[1].max(1.0);
        let center = eye - back * distance;
        let tilt = Quat::from_axis_angle(right, -tilt.to_radians());
        let normal = tilt * back;
        let turn = Quat::from_axis_angle(normal, turn.to_radians());
        let rotate = turn * tilt;
        let across = rotate * (right * 2.0 * half_w) * scale;
        let down = rotate * (-up * 2.0 * half_h) * scale;
        Placement {
            origin: center - across * 0.5 - down * 0.5,
            across,
            down,
            up: normal * height * scale,
        }
    }

    /// The point at window position `uv` and height `h` (in the field's units).
    pub fn at(&self, uv: Vec2, h: f32) -> Vec3 {
        self.origin + self.across * uv.x + self.down * uv.y + self.up * h
    }

    /// The unit vector off the plane, on the camera's side of a `facing` placement.
    pub fn toward(&self) -> Vec3 {
        self.down.cross(self.across).normalize_or_zero()
    }

    /// The point at window position `uv`, `lift` off the plane toward the camera
    /// (lines and labels of a chart drawn on the plane sit a little in front of it).
    pub fn on(&self, uv: Vec2, lift: f32) -> Vec3 {
        self.origin + self.across * uv.x + self.down * uv.y + self.toward() * lift
    }

    /// A flat rectangle on the plane over window positions `a` to `b` (a chart's bar or panel), `lift` off it
    /// toward the camera: rectangles that overlap need different lifts (a panel behind its bars), or they fight.
    pub fn rect(
        &self,
        draw: &mut DrawLists,
        a: Vec2,
        b: Vec2,
        lift: f32,
        color: Color32,
        alpha: f32,
    ) {
        let (lo, hi) = (a.min(b), a.max(b));
        if hi.x - lo.x <= 0.0 || hi.y - lo.y <= 0.0 {
            return;
        }
        let center = self.on((lo + hi) * 0.5, lift);
        draw.cuboid(
            center,
            [
                self.across * (hi.x - lo.x) * 0.5,
                self.down * (hi.y - lo.y) * 0.5,
                self.toward() * 2e-4,
            ],
            color,
            alpha,
        );
    }
}

/// An image of `size` texels, its texels `window` (x0, y0, x1, y1, in texels)
/// shown on `placement`.
#[derive(Clone, Copy, Debug)]
pub struct HeightField {
    pub size: [u32; 2],
    pub window: [f32; 4],
    pub placement: Placement,
}

impl HeightField {
    /// The whole of an image of `size`.
    pub fn whole(size: [u32; 2], placement: Placement) -> HeightField {
        HeightField {
            size,
            window: [0.0, 0.0, size[0] as f32, size[1] as f32],
            placement,
        }
    }

    /// The window position of texel coordinates `p` (texels of the full image).
    pub fn uv(&self, p: Vec2) -> Vec2 {
        let [x0, y0, x1, y1] = self.window;
        Vec2::new(
            (p.x - x0) / (x1 - x0).max(1e-6),
            (p.y - y0) / (y1 - y0).max(1e-6),
        )
    }

    /// The world point over texel coordinates `p` (texels of the full image) at height `h`.
    pub fn point(&self, p: Vec2, h: f32) -> Vec3 {
        self.placement.at(self.uv(p), h)
    }

    /// The world point of an image's uv (0 to 1 over the whole image) at height `h`.
    pub fn point_uv(&self, uv: Vec2, h: f32) -> Vec3 {
        self.point(uv * Vec2::new(self.size[0] as f32, self.size[1] as f32), h)
    }

    /// Whether a texel box (`[x0, y0, x1, y1]`, as `window`) overlaps the window.
    pub fn shows(&self, texels: [f32; 4]) -> bool {
        let [x0, y0, x1, y1] = texels;
        let [wx0, wy0, wx1, wy1] = self.window;
        x1 > wx0 && x0 < wx1 && y1 > wy0 && y0 < wy1
    }

    /// A column over a texel box (`[x0, y0, x1, y1]`, clipped to the window),
    /// `h` high (a flat tile when `h` is 0).
    pub fn column(
        &self,
        draw: &mut DrawLists,
        texels: [f32; 4],
        h: f32,
        color: Color32,
        alpha: f32,
    ) {
        let [x0, y0, x1, y1] = texels;
        let [wx0, wy0, wx1, wy1] = self.window;
        let (x0, y0, x1, y1) = (x0.max(wx0), y0.max(wy0), x1.min(wx1), y1.min(wy1));
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        // A tile is a sliver of height, so a flat field still reads as the image.
        let h = h.max(1e-3);
        let (a, b) = (self.uv(Vec2::new(x0, y0)), self.uv(Vec2::new(x1, y1)));
        let p = &self.placement;
        let center = p.at((a + b) * 0.5, h * 0.5);
        draw.cuboid(
            center,
            [
                p.across * (b.x - a.x) * 0.5,
                p.down * (b.y - a.y) * 0.5,
                p.up * h * 0.5,
            ],
            color,
            alpha,
        );
    }

    /// The slab under the window, a little below its zero height.
    pub fn base(&self, draw: &mut DrawLists, color: Color32, alpha: f32) {
        let p = &self.placement;
        draw.cuboid(
            p.at(Vec2::splat(0.5), -0.006),
            [p.across * 0.5, p.down * 0.5, p.up * 0.004],
            color,
            alpha,
        );
    }
}

/// A grey for value `v` of `max`: the depth map's ramp (the square root, so far detail reads).
pub fn grey(v: f32, max: f32) -> Color32 {
    let g = ((v / max.max(1e-6)).clamp(0.0, 1.0).sqrt() * 235.0 + 20.0) as u8;
    Color32::from_rgb(g, g, g)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::OrbitCamera;

    #[test]
    fn a_flat_facing_field_covers_the_view() {
        let camera = OrbitCamera::default().camera_data([1600.0, 900.0]);
        let p = Placement::facing(&camera, 0.25, 0.05, 0.0, 0.0, 1.0);
        let clip = Mat4::from_cols_array_2d(&camera.world_to_clip);
        let ndc = |w: Vec3| {
            let c = clip * w.extend(1.0);
            Vec2::new(c.x / c.w, c.y / c.w)
        };
        // The window's corners are the view's corners; uv's v runs down the screen.
        assert!((ndc(p.at(Vec2::new(0.0, 0.0), 0.0)) - Vec2::new(-1.0, 1.0)).length() < 1e-3);
        assert!((ndc(p.at(Vec2::new(1.0, 1.0), 0.0)) - Vec2::new(1.0, -1.0)).length() < 1e-3);
        // Heights rise toward the camera.
        let eye = Vec3::from(camera.position);
        assert!(
            p.at(Vec2::splat(0.5), 1.0).distance(eye) < p.at(Vec2::splat(0.5), 0.0).distance(eye)
        );
    }
}
