//! The free camera's view (src/camera.rs `FreeCamera`): the frame the
//! investigation built for the main camera, drawn from the free camera.
//!
//! The investigation builds its frame once, from the shot's state, so its
//! demonstration (arrows, readouts, pictures of its buffers, and engine state
//! that follows a camera, such as shadow cascades fitted to the view) follows
//! the main camera. The free view is a copy of that frame whose camera
//! uniforms are the free camera's, so the scene renders by the same passes
//! from there; the investigation then rewrites what it derived from the
//! camera that renders (`Investigation::view_from`), and the main camera is
//! drawn into it: a body at its eye, its view cone out to what it looks at,
//! and a label.

use crate::camera::{frustum_corners, FreeCamera};
use crate::frame::{FrameContent, Label, FLAG_DASHED};
use crate::i18n::tr;
use crate::investigation;
use crate::layout::CameraData;
use crate::palette;
use crate::shot::ShotState;
use glam::Vec3;

/// The free view of `main`, the frame built for `state`'s main camera.
pub fn content(main: &FrameContent, state: &ShotState, free: &FreeCamera) -> FrameContent {
    let mut content = main.clone();
    content.uniforms.camera = free.orbit().camera_data_like(&main.uniforms.camera);
    investigation::current().view_from(&mut content);
    mark_camera(&mut content, &main.uniforms.camera, state.camera.distance, Some(tr("ui.main_camera")));
    content
}

/// A camera drawn into a frame: a small body at its eye, its view cone out to
/// `reach` metres (at most twelve), and its label (if any). The free view draws the main
/// camera with it; an investigation may draw a camera of its own (an engine's
/// camera that its demonstration moves) the same way.
pub fn mark_camera(content: &mut FrameContent, camera: &CameraData, reach: f32, label: Option<&str>) {
    let eye = Vec3::from(camera.position);
    let color = palette::VIEW;
    let draw = &mut content.draw;
    draw.point(eye, 10.0, color, 1.0);
    // The body: the cone's first fifth of a metre, closed, so the camera reads as a solid shape.
    let body = frustum_corners(camera, 0.2);
    for i in 0..4 {
        draw.segment(eye, body[i], color, 1.0, 3.0, 0);
        draw.segment(body[i], body[(i + 1) % 4], color, 1.0, 3.0, 0);
    }
    // Its up side: a small triangle on the body's top edge.
    let up = (body[0] + body[1]) * 0.5 + (body[0] - body[3]).normalize_or_zero() * 0.08;
    draw.segment(body[0], up, color, 1.0, 2.0, 0);
    draw.segment(up, body[1], color, 1.0, 2.0, 0);
    // The view cone, dashed past the body, to its far rectangle.
    let far = frustum_corners(camera, reach.clamp(1.0, 12.0));
    for i in 0..4 {
        draw.segment(body[i], far[i], color, 0.8, 1.5, FLAG_DASHED);
        draw.segment(far[i], far[(i + 1) % 4], color, 0.8, 1.5, 0);
    }
    // The line of sight, to the middle of the far rectangle.
    let centre = far.iter().copied().sum::<Vec3>() * 0.25;
    draw.segment(eye, centre, color, 0.5, 1.0, FLAG_DASHED);
    if let Some(text) = label {
        content.labels.push(Label { anchor: eye + Vec3::Y * 0.25, text: text.to_string(), color });
    }
}
