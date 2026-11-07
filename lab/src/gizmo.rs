//! The gizmo the viewer draws on the selected entity (src/entity.rs) to move it in the viewport, as an
//! editor's: in Move mode three arrows (along x, y, z) and three squares (in the planes between them), in
//! Rotate mode a ring about +Y, in Scale mode a handle that grows or shrinks it. It keeps one size on
//! screen, whatever the entity's distance, and a handle lights up under the pointer.
//!
//! Hits are found on screen, against the handles as drawn. A drag along an arrow follows the pointer's ray
//! on the plane through that axis that faces the camera most, projected on the axis, so it neither jumps
//! nor stalls as the axis turns toward the camera; a square's drag follows the ray on its plane; the ring
//! turns by the angle the pointer sweeps about the entity's centre on screen; the scale handle by how much
//! farther from the centre the pointer is than where the drag started.

use crate::camera::Projector;
use glam::{Vec2, Vec3};

/// The gizmo's arms, in pixels of the viewport at 1080 lines (scaled with the viewport's canvas).
pub const ARM: f32 = 110.0;
/// How near a handle the pointer hits it, in the same pixels.
pub const REACH: f32 = 12.0;
/// Where a plane square sits along its two axes, and its size, as shares of an arm.
const SQUARE: (f32, f32) = (0.32, 0.22);

/// What the gizmo does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Move,
    Rotate,
    Scale,
}

/// One of its handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// An arrow: along x (0), y (1), or z (2).
    Axis(usize),
    /// A square: in the plane of the two axes other than this one.
    Plane(usize),
    /// The ring about +Y.
    Ring,
    /// The scale handle.
    Scale,
}

/// The world's axes, by index.
pub fn axis(i: usize) -> Vec3 {
    [Vec3::X, Vec3::Y, Vec3::Z][i]
}

/// The gizmo on screen: where its centre is, and its handles' ends, in viewport pixels.
#[derive(Clone, Debug)]
pub struct Shape {
    pub mode: Mode,
    pub center: Vec2,
    /// Each arrow's tip (None behind the camera).
    pub tips: [Option<Vec2>; 3],
    /// Each plane square's corners (None behind the camera).
    pub squares: [Option<[Vec2; 4]>; 3],
    /// The ring, as a closed line.
    pub ring: Vec<Vec2>,
    /// The scale handle's centre.
    pub scale: Vec2,
    /// The world length of an arm at the entity.
    pub arm: f32,
}

/// The length in the world that is `pixels` long on screen at `at`.
pub fn world_size(projector: &Projector, at: Vec3, pixels: f32) -> f32 {
    let depth = (at - projector.eye).length().max(1e-3);
    let (_, dir) = projector.ray(projector.viewport * 0.5);
    // The pixels a metre spans at that distance along the view's centre.
    let side = dir.any_orthonormal_vector();
    let a = projector.project(projector.eye + dir * depth);
    let b = projector.project(projector.eye + dir * depth + side);
    match (a, b) {
        (Some((a, _)), Some((b, _))) => pixels / (a - b).length().max(1e-3),
        _ => pixels * 0.01,
    }
}

/// The gizmo of `mode` at `at`, `pixels` arms on screen (`ARM` times the canvas's scale).
pub fn shape(projector: &Projector, at: Vec3, mode: Mode, pixels: f32) -> Option<Shape> {
    let (center, _) = projector.project(at)?;
    let arm = world_size(projector, at, pixels);
    let point = |p: Vec3| projector.project(p).map(|(s, _)| s);
    let tips = [0, 1, 2].map(|i| point(at + axis(i) * arm));
    let squares = [0, 1, 2].map(|n| {
        let (u, v) = (axis((n + 1) % 3), axis((n + 2) % 3));
        let (from, size) = (SQUARE.0 * arm, SQUARE.1 * arm);
        let corners = [at + u * from + v * from, at + u * (from + size) + v * from, at + u * (from + size) + v * (from + size), at + u * from + v * (from + size)];
        let projected: Vec<Vec2> = corners.iter().filter_map(|c| point(*c)).collect();
        (projected.len() == 4).then(|| [projected[0], projected[1], projected[2], projected[3]])
    });
    let ring = (0..=48).filter_map(|k| {
        let a = k as f32 / 48.0 * std::f32::consts::TAU;
        point(at + Vec3::new(a.cos(), 0.0, a.sin()) * arm * 0.9)
    }).collect();
    let scale = tips[0].unwrap_or(center + Vec2::new(pixels, 0.0));
    Some(Shape { mode, center, tips, squares, ring, scale, arm })
}

/// The distance from `p` to the segment from `a` to `b`.
fn to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t - p).length()
}

/// Whether `p` is inside the quad `q` (convex, in order).
fn inside(p: Vec2, q: &[Vec2; 4]) -> bool {
    let side = |a: Vec2, b: Vec2| (b - a).perp_dot(p - a);
    let s: Vec<f32> = (0..4).map(|i| side(q[i], q[(i + 1) % 4])).collect();
    s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0)
}

impl Shape {
    /// The handle under `p` (viewport pixels), `reach` pixels about it.
    pub fn hit(&self, p: Vec2, reach: f32) -> Option<Handle> {
        match self.mode {
            Mode::Move => {
                // The squares first: they sit between the arrows, which would take them otherwise.
                for n in 0..3 {
                    if self.squares[n].as_ref().is_some_and(|q| inside(p, q)) {
                        return Some(Handle::Plane(n));
                    }
                }
                (0..3).filter_map(|i| self.tips[i].map(|t| (i, to_segment(p, self.center, t)))).filter(|(_, d)| *d < reach).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| Handle::Axis(i))
            }
            Mode::Rotate => self.ring.windows(2).map(|w| to_segment(p, w[0], w[1])).fold(f32::INFINITY, f32::min).lt(&reach).then_some(Handle::Ring),
            Mode::Scale => ((p - self.scale).length() < reach * 1.6 || to_segment(p, self.center, self.scale) < reach).then_some(Handle::Scale),
        }
    }
}

/// A drag of a handle, from where it started.
#[derive(Clone, Copy, Debug)]
pub struct Drag {
    pub handle: Handle,
    /// The entity's place, turn (degrees), and size when the drag started.
    pub at: Vec3,
    pub yaw: f32,
    pub scale: f32,
    /// Where the pointer started (viewport pixels), and the gizmo's centre on screen then.
    pub start: Vec2,
    pub center: Vec2,
}

/// The point where the ray through `pixel` meets the plane through `at` with normal `n`.
fn on_plane(projector: &Projector, pixel: Vec2, at: Vec3, n: Vec3) -> Option<Vec3> {
    let (origin, dir) = projector.ray(pixel);
    let d = dir.dot(n);
    if d.abs() < 1e-4 {
        return None;
    }
    let t = (at - origin).dot(n) / d;
    (t > 0.0).then(|| origin + dir * t)
}

/// The plane a drag along axis `i` from `at` follows: through the axis, facing the camera most.
fn axis_plane(projector: &Projector, at: Vec3, i: usize) -> Vec3 {
    let a = axis(i);
    let to_eye = (projector.eye - at).normalize_or_zero();
    let n = a.cross(to_eye).cross(a);
    if n.length_squared() < 1e-8 { a.any_orthonormal_vector() } else { n.normalize() }
}

/// What the drag `drag` with the pointer at `pixel` gives: the entity's new place, turn, and size.
pub fn dragged(projector: &Projector, drag: &Drag, pixel: Vec2) -> (Vec3, f32, f32) {
    let (at, yaw, scale) = (drag.at, drag.yaw, drag.scale);
    match drag.handle {
        Handle::Axis(i) => {
            let n = axis_plane(projector, at, i);
            match (on_plane(projector, drag.start, at, n), on_plane(projector, pixel, at, n)) {
                (Some(a), Some(b)) => (at + axis(i) * axis(i).dot(b - a), yaw, scale),
                _ => (at, yaw, scale),
            }
        }
        Handle::Plane(n) => match (on_plane(projector, drag.start, at, axis(n)), on_plane(projector, pixel, at, axis(n))) {
            (Some(a), Some(b)) => {
                let mut moved = b - a;
                moved[n] = 0.0;
                (at + moved, yaw, scale)
            }
            _ => (at, yaw, scale),
        },
        Handle::Ring => {
            // Seen from above, a turn toward +Z is clockwise on screen: the angle's sign follows the camera.
            let (a, b) = (drag.start - drag.center, pixel - drag.center);
            let swept = a.angle_to(b).to_degrees();
            let below = projector.eye.y < at.y;
            (at, yaw + if below { swept } else { -swept }, scale)
        }
        Handle::Scale => {
            let start = (drag.start - drag.center).length().max(8.0);
            let now = (pixel - drag.center).length();
            (at, yaw, scale * (now / start).clamp(0.05, 20.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::OrbitCamera;

    fn projector() -> Projector {
        let camera = OrbitCamera { target: [0.0, 0.0, 0.0], yaw: 30.0, pitch: 25.0, distance: 6.0, fov: 50.0 };
        Projector::new(&camera.camera_data([1600.0, 900.0]))
    }

    #[test]
    fn the_gizmo_keeps_its_size_on_screen() {
        let p = projector();
        for at in [Vec3::ZERO, Vec3::new(1.0, 0.5, -2.0)] {
            let s = shape(&p, at, Mode::Move, 100.0).unwrap();
            let longest = s.tips.iter().flatten().map(|t| (*t - s.center).length()).fold(0.0, f32::max);
            assert!((longest - 100.0).abs() < 15.0, "an arm is {longest} px");
        }
    }

    #[test]
    fn a_drag_along_an_arrow_moves_along_its_axis() {
        let p = projector();
        let s = shape(&p, Vec3::ZERO, Mode::Move, 100.0).unwrap();
        let tip = s.tips[0].unwrap();
        // The arrow's middle hits it; a drag to its tip moves about one arm along x only.
        let start = s.center.lerp(tip, 0.5);
        assert_eq!(s.hit(start, REACH), Some(Handle::Axis(0)));
        let drag = Drag { handle: Handle::Axis(0), at: Vec3::ZERO, yaw: 0.0, scale: 1.0, start, center: s.center };
        let (to, _, _) = dragged(&p, &drag, start + (tip - s.center) * 0.5);
        assert!((to.x - s.arm * 0.5).abs() < s.arm * 0.05 && to.y == 0.0 && to.z == 0.0, "{to}");
    }

    #[test]
    fn a_drag_on_a_square_stays_in_its_plane() {
        let p = projector();
        let s = shape(&p, Vec3::ZERO, Mode::Move, 100.0).unwrap();
        let q = s.squares[1].unwrap();
        let start = (q[0] + q[2]) * 0.5;
        assert_eq!(s.hit(start, REACH), Some(Handle::Plane(1)));
        let drag = Drag { handle: Handle::Plane(1), at: Vec3::ZERO, yaw: 0.0, scale: 1.0, start, center: s.center };
        let (to, _, _) = dragged(&p, &drag, start + Vec2::new(40.0, 25.0));
        assert!(to.y == 0.0 && to.length() > 0.05, "{to}");
    }

    #[test]
    fn the_ring_turns_and_the_handle_scales() {
        let p = projector();
        let s = shape(&p, Vec3::ZERO, Mode::Rotate, 100.0).unwrap();
        assert_eq!(s.hit(s.ring[0], REACH), Some(Handle::Ring));
        let drag = Drag { handle: Handle::Ring, at: Vec3::ZERO, yaw: 10.0, scale: 1.0, start: s.center + Vec2::new(100.0, 0.0), center: s.center };
        let (_, yaw, _) = dragged(&p, &drag, s.center + Vec2::new(0.0, 100.0));
        assert!((yaw - 10.0).abs() > 80.0, "{yaw}");
        let drag = Drag { handle: Handle::Scale, ..drag };
        let (_, _, scale) = dragged(&p, &drag, s.center + Vec2::new(200.0, 0.0));
        assert!((scale - 2.0).abs() < 1e-4);
    }
}
