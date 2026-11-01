//! The light passes on the GPU, as `GI::VoxelGIInstance::update` runs them
//! (servers/rendering/renderer_rd/environment/gi.cpp:3096-3171): only when
//! the light settings change (the lab's one light never moves), one dispatch
//! per pass and level over that level's cells, 64 to a workgroup
//! (voxel_gi.glsl's local size). Pass 0 lights the leaves
//! (`MODE_COMPUTE_LIGHT`), builds the levels above them
//! (`MODE_UPDATE_MIPMAPS`, one dispatch per level, the leaves' parents first),
//! and writes every level into the RGBA8 3D texture (`MODE_WRITE_TEXTURE`);
//! with two bounces, pass 1 does the same from `MODE_SECOND_BOUNCE`, whose
//! cones read the texture pass 0 wrote. light.rs is their CPU mirror, which
//! the demonstrations draw and the selftest compares with.
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::world;
use lab::investigation::{ShadingUpdate, ShadingVolume, UpdateBuffer, UpdateDispatch, UpdatePass, UpdatePlan};
use lab::shot::ShotState;

/// The light texture's size: the octree's (world.rs's node, subdiv 128).
pub const LIGHT_SIZE: [u32; 3] = [128, 64, 128];
/// One mip per octree level.
pub const LIGHT_MIPS: u32 = 8;

/// `voxel_gi.glsl`'s `local_size_x = 64`.
pub const WORKGROUP: u32 = 64;

pub static VOLUMES: &[ShadingVolume] = &[ShadingVolume { name: "voxelgiLight", format: wgpu::TextureFormat::Rgba8Unorm, size: LIGHT_SIZE, mips: LIGHT_MIPS }];

pub static PASSES: &[UpdatePass] = &[
    UpdatePass { entry: "voxelgiLightCS", reads: &[], writes: &[], workgroup: WORKGROUP },
    UpdatePass { entry: "voxelgiBounceCS", reads: &["voxelgiLight"], writes: &[], workgroup: WORKGROUP },
    UpdatePass { entry: "voxelgiMipmapsCS", reads: &[], writes: &[], workgroup: WORKGROUP },
    UpdatePass { entry: "voxelgiWriteCS", reads: &[], writes: &["voxelgiLight"], workgroup: WORKGROUP },
];

/// `write_buffer` (`VoxelGIInstance::write_buffer`): one float light per cell, four words each.
pub static BUFFERS: &[UpdateBuffer] = &[UpdateBuffer { name: "voxelgiOutputs", words: outputs_words }];

fn outputs_words() -> u64 {
    world::world().probe.data.len() as u64 * 4
}

pub static UPDATE: ShadingUpdate = ShadingUpdate { passes: PASSES, buffers: BUFFERS, plan };

/// The key of the light settings a texture was lit for.
pub fn settings_key(s: &world::LightSettings) -> u64 {
    ((s.dynamic_range * 100.0).round() as u64) << 32 | ((s.propagation * 100.0).round() as u64) << 1 | s.two_bounces as u64
}

/// The dispatches of one update for the scene node's settings in `state`:
/// pass 0, and pass 1 with two bounces.
pub fn plan(state: &ShotState) -> UpdatePlan {
    let settings = world::scene_settings(state);
    let w = world::world();
    let probe = &w.probe;
    let params = world::params(probe, &settings);
    let layout = world::data_layout();
    let dispatch = |pass: &'static str, range: std::ops::Range<usize>, mip: usize| UpdateDispatch {
        pass,
        groups: (range.len() as u32).div_ceil(WORKGROUP),
        mip: mip as u32,
        params: [
            range.start as u32,
            range.len() as u32,
            mip as u32,
            layout.cells,
            layout.children,
            layout.lights,
            layout.light_count,
            0,
            params.propagation.to_bits(),
            params.dynamic_range.to_bits(),
            params.cell_size.to_bits(),
            0,
            0,
            0,
            0,
            0,
        ],
    };
    let mut dispatches = Vec::new();
    let leaves = probe.level_range(probe.leaf_level());
    for pass in 0..if settings.two_bounces { 2 } else { 1 } {
        dispatches.push(dispatch(if pass == 0 { "voxelgiLightCS" } else { "voxelgiBounceCS" }, leaves.clone(), 0));
        for mip in 1..probe.mip_count() {
            dispatches.push(dispatch("voxelgiMipmapsCS", probe.level_range(probe.mip_level(mip)), mip));
        }
        for mip in 0..probe.mip_count() {
            dispatches.push(dispatch("voxelgiWriteCS", probe.level_range(probe.mip_level(mip)), mip));
        }
    }
    UpdatePlan { key: settings_key(&settings), always: state.p("giRelightEachFrame") > 0.5, dispatches }
}
