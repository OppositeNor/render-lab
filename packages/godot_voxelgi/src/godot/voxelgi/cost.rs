//! What the lab's VoxelGI costs, counted from the code that runs it: the
//! light passes' work per update (update.rs's plan) and the screen pass's
//! samples per pixel (the cones' step counts, gi.glsl's loops without their
//! early exits). The cost demonstration shows them and the selftest checks
//! the ones the story says.

use super::update;
use super::world;
use lab::shot::ShotState;

/// `voxel_cone_trace_45_degrees`'s steps from `bias` to `max_distance`: the
/// distance doubles each step (at least half a cell).
pub fn steps_45(bias: f32, max_distance: f32) -> usize {
    let mut dist = bias;
    let mut radius = dist.max(0.5);
    let mut n = 0;
    while dist < max_distance {
        n += 1;
        dist += radius;
        radius = dist.max(0.5);
    }
    n
}

/// `voxel_cone_trace`'s steps for a cone of `tan_half_angle`: each step half
/// the cone's width there, at least one cell wide.
pub fn steps_cone(tan_half_angle: f32, bias: f32, max_distance: f32) -> usize {
    let mut dist = bias;
    let mut n = 0;
    while dist < max_distance {
        n += 1;
        dist += (2.0 * tan_half_angle * dist).max(1.0) * 0.5;
    }
    n
}

/// The reflection cone's half-angle tangent at a material's roughness
/// (process_gi squares it; voxel_gi_compute's `tan(roughness * 0.5 * M_PI * 0.99)`).
pub fn spec_tan(roughness: f32) -> f32 {
    (roughness * roughness * 0.5 * 3.141592 * 0.99).tan()
}

pub struct Costs {
    pub leaves: usize,
    pub cells: usize,
    /// Texels of the light texture's mip 0.
    pub texels: usize,
    pub passes: usize,
    pub dispatches: usize,
    pub workgroups: u32,
    /// Steps of one diffuse cone, Low (4 cones) and High (6).
    pub low_steps: usize,
    pub high_steps: usize,
    /// The reflection cone's steps at roughness 1, 0.5, and 0.
    pub spec_steps: [usize; 3],
}

pub fn costs(state: &ShotState) -> Costs {
    let w = world::world();
    let probe = &w.probe;
    let plan = update::plan(state);
    let max = probe.octree_size.as_vec3().length();
    let bias = state.p("giBias");
    let s = probe.mip_size(0);
    Costs {
        leaves: probe.level_range(probe.leaf_level()).len(),
        cells: probe.data.len(),
        texels: (s.x * s.y * s.z) as usize,
        passes: if world::scene_settings(state).two_bounces { 2 } else { 1 },
        dispatches: plan.dispatches.len(),
        workgroups: plan.dispatches.iter().map(|d| d.groups).sum(),
        low_steps: steps_45(bias, max),
        high_steps: steps_cone(0.577, bias, max),
        spec_steps: [1.0, 0.5, 0.0].map(|r| steps_cone(spec_tan(r), bias, max)),
    }
}
