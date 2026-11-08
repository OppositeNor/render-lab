//! What a demonstration draws into a frame's overlay, in the scene's metres:
//! points and circles sized in metres at their distance from the camera,
//! boxes' edges, labels (shown while the `labels` value is on), and a
//! readout's rows.

use crate::frame::{FrameContent, Label};
use crate::shot::ShotState;
use egui::Color32;
use glam::Vec3;

/// The size in pixels of `r` metres at `p`, for the frame's camera (points
/// are drawn as discs of a radius in pixels).
#[inline]
pub fn px(content: &FrameContent, p: Vec3, r: f32) -> f32 {
    let c = &content.uniforms.camera;
    let d = (p - Vec3::from(c.position)).length().max(1e-3);
    r / (d * (c.vertical_fov * 0.5).tan()) * c.viewport_size[1] * 0.5
}

/// A point of `r` metres (at least 1.5 pixels).
#[inline]
pub fn ball(content: &mut FrameContent, p: Vec3, r: f32, color: Color32, alpha: f32) {
    let radius = px(content, p, r).max(1.5);
    content.draw.point(p, radius, color, alpha);
}

/// A circle of `r` metres about `p`, facing the camera (a march's sphere, a
/// cone's sample).
pub fn circle(content: &mut FrameContent, p: Vec3, r: f32, color: Color32, alpha: f32) {
    let eye = Vec3::from(content.uniforms.camera.position);
    let n = (eye - p).normalize_or_zero();
    let u = n.any_orthonormal_vector();
    let v = n.cross(u);
    let pts: Vec<Vec3> = (0..=32)
        .map(|k| {
            let a = k as f32 / 32.0 * std::f32::consts::TAU;
            p + (u * a.cos() + v * a.sin()) * r
        })
        .collect();
    content.draw.polyline(&pts, color, alpha, 1.5);
}

/// The twelve edges of the axis-aligned box from `min` of `size`.
pub fn wire_box(
    content: &mut FrameContent,
    min: Vec3,
    size: Vec3,
    color: Color32,
    alpha: f32,
    width: f32,
) {
    let c = |i: u32| {
        min + size * Vec3::new((i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32)
    };
    for (a, b) in [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7),
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ] {
        content.draw.segment(c(a), c(b), color, alpha, width, 0);
    }
}

/// A label at `anchor`, while the shot's `labels` value is on.
pub fn label(
    content: &mut FrameContent,
    state: &ShotState,
    anchor: Vec3,
    text: String,
    color: Color32,
) {
    if state.p("labels") > 0.5 {
        content.labels.push(Label {
            anchor,
            text,
            color,
        });
    }
}

/// A readout's row: its name, its value, and its colour.
#[inline]
pub fn row(
    name: impl Into<String>,
    value: impl Into<String>,
    color: Color32,
) -> (String, String, Color32) {
    (name.into(), value.into(), color)
}
