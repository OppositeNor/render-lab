//! What a story runs in the investigation's code, and what a click on one of
//! its images does (`#[lab::actions]`, lab/src/actions.rs).

use super::Lighting;
use lab::actions::ClickFrame;
use lab::camera::Projector;
use lab::frame::FrameContent;
use lab::scene::{self, ids};
use lab::shot::ShotState;
use glam::Vec2;

/// Picks the viewport's point at `uv` (0..1, y down): the surface point the
/// readout inspects, which every view of a screen-sized resource then rings
/// where it projects (`pickShown`). False when nothing was hit.
pub fn pick(state: &mut ShotState, content: &FrameContent, uv: Vec2) -> bool {
    let camera = &content.uniforms.camera;
    let projector = Projector::new(camera);
    let (origin, dir) = projector.ray(uv * Vec2::from(camera.viewport_size));
    let hit = scene::trace_scene(&content.objects, origin, dir, 100.0, ids::PANEL as u32);
    if !hit.is_hit() {
        return false;
    }
    let p = origin + dir * hit.t;
    let n = if hit.normal.dot(dir) > 0.0 { -hit.normal } else { hit.normal };
    for (name, value) in [("pickShown", 1.0), ("inspectX", p.x), ("inspectY", p.y), ("inspectZ", p.z), ("inspectNX", n.x), ("inspectNY", n.y), ("inspectNZ", n.z), ("inspectObject", hit.object as f32)] {
        state.set(name, value);
    }
    true
}

#[lab::actions]
impl Lighting {
    /// Picks the pixel clicked on a view of a screen-sized resource (a
    /// forward target, the depth, the stencil): ringed in every such view, and
    /// its surface point inspected in the scene.
    #[click]
    fn pick_pixel(frame: &mut ClickFrame) -> bool {
        pick(frame.state, frame.content, frame.uv)
    }
}
