//! The investigation's actions (lab/src/actions.rs): work its story starts
//! with `{@ run=…}` and stops with `{@ stop=…}`, setting the shot's values
//! as the seconds pass, and the click handler of its slice insets.

use super::VoxelGi;
use lab::actions::{ActionFrame, ClickFrame};

#[lab::actions]
impl VoxelGi {
    /// Steps a cone's march (`coneSteps`) from `from` (0 by default) to
    /// `to`, one sample every `seconds`.
    #[action]
    fn march_cone(frame: &mut ActionFrame, seconds: f32, from: Option<u32>, to: Option<u32>) -> Result<(), String> {
        if seconds <= 0.0 {
            return Err(format!("a step takes {seconds} s; it takes longer than 0"));
        }
        let step = (from.unwrap_or(0) + (frame.elapsed / seconds) as u32).min(to.unwrap_or(16));
        frame.state.set("coneSteps", step as f32);
        Ok(())
    }

    /// Picks the cell under a click on a slice inset (x across, z down, at
    /// the slice's height; the slice is the leaves' grid).
    #[click]
    fn pick_slice_cell(frame: &mut ClickFrame) -> bool {
        let w = super::world::world();
        let s = w.probe.octree_size;
        let y = super::demos::leaf_of(&w.probe, glam::Vec3::new(0.0, frame.shown.p("sliceHeight"), 0.0)).y.clamp(0, s.y - 1);
        let x = ((frame.uv.x * s.x as f32) as i32).clamp(0, s.x - 1);
        let z = ((frame.uv.y * s.z as f32) as i32).clamp(0, s.z - 1);
        frame.state.set("cellX", x as f32);
        frame.state.set("cellY", y as f32);
        frame.state.set("cellZ", z as f32);
        true
    }
}
