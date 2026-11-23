//! The light passes on the GPU, as `GI::VoxelGIInstance::update` runs them
//! (servers/rendering/renderer_rd/environment/gi.cpp:2963-3397): the program's
//! plan, each frame. The lights go up, and the light passes run, only when
//! the culler's light cache or the VoxelGI's version changes
//! (renderer_scene_cull.cpp:3921-4102, gi.cpp:4302): a light moved, or its
//! colour, energy, range, attenuation, spot, area size, or shadow changed,
//! or the dynamic range, propagation, or bounce count. Then one dispatch per
//! pass and level over that level's cells, 64 to a workgroup (voxel_gi.glsl's
//! local size): pass 0 lights the leaves (`MODE_COMPUTE_LIGHT`), builds the
//! levels above them (`MODE_UPDATE_MIPMAPS`, one dispatch per level, the
//! leaves' parents first), and writes every level into the RGBA8 3D texture
//! (`MODE_WRITE_TEXTURE`); with two bounces, pass 1 does the same from
//! `MODE_SECOND_BOUNCE`, whose cones read the texture pass 0 wrote. light.rs
//! is their CPU mirror, which the demonstrations draw and the selftest
//! compares with.
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::world;
use lab::investigation::{Element, ProgramCommand, ProgramFrame};
use lab::shot::ShotState;
use std::hash::{Hash, Hasher};

/// `voxel_gi.glsl`'s `local_size_x = 64`.
pub const WORKGROUP: u32 = 64;
/// The cells `voxelgiOutputs` has room for (shading.toml: 16 bytes each).
pub const OUTPUT_CELLS: usize = 2097152 / 16;

/// What the plan keeps between frames: the bake it last uploaded, the light
/// cache it last lit for, and whether it plotted dynamic objects last frame
/// (`has_dynamic_object_data`).
#[derive(Default)]
pub struct PlanState {
    bake: Option<world::Bake>,
    lit: Option<u64>,
    plotted: bool,
}

pub fn init() -> Box<dyn std::any::Any + Send> {
    Box::new(PlanState::default())
}

/// The lights of a shot (the panel; the extra omni light when it is on) and
/// the settings the light passes read: the light cache and the version the
/// plan compares, as words.
pub fn light_cache(state: &ShotState) -> (Vec<u32>, world::LightSettings) {
    let w = world::world();
    let lights = super::demos::shot_lights(state, w);
    (world::light_words(&lights), world::scene_settings(state))
}

/// The key of a light cache and its settings.
pub fn cache_key(words: &[u32], s: &world::LightSettings) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    words.hash(&mut h);
    s.dynamic_range.to_bits().hash(&mut h);
    s.propagation.to_bits().hash(&mut h);
    s.two_bounces.hash(&mut h);
    h.finish()
}

/// What every light pass's dispatch shares: the bake, the light count, the settings.
#[derive(Clone, Copy)]
pub struct Lighting<'a> {
    pub bake: world::Bake,
    pub light_count: u32,
    pub settings: &'a world::LightSettings,
}

/// One dispatch of light pass `pass` over cells `range`, at `mip` (the write
/// pass's instance), with voxel_gi.glsl's push constant (`VoxelgiUpdate`).
fn dispatch(
    pass: &'static str,
    range: std::ops::Range<usize>,
    mip: usize,
    l: Lighting,
) -> ProgramCommand {
    let params = world::params(world::probe_of(l.bake), l.settings);
    let layout = world::data_layout(l.bake);
    ProgramCommand::Dispatch {
        pass,
        instance: if pass == "voxelgiWriteCS" {
            mip as u32
        } else {
            0
        },
        groups: [(range.len() as u32).div_ceil(WORKGROUP), 1, 1],
        params: [
            range.start as u32,
            range.len() as u32,
            mip as u32,
            layout.cells,
            layout.children,
            0,
            l.light_count,
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
    }
}

/// The write pass over every level (`MODE_WRITE_TEXTURE`, gi.cpp:3157-3171).
fn write_texture(out: &mut Vec<ProgramCommand>, l: Lighting) {
    let probe = world::probe_of(l.bake);
    for mip in 0..probe.mip_count() {
        out.push(dispatch(
            "voxelgiWriteCS",
            probe.level_range(probe.mip_level(mip)),
            mip,
            l,
        ));
    }
}

/// The light passes for new lights or settings (gi.cpp:3098-3171): pass 0,
/// and pass 1 with two bounces, each lighting the leaves, building the mips,
/// and writing every level.
pub fn light_passes(out: &mut Vec<ProgramCommand>, l: Lighting) {
    let probe = world::probe_of(l.bake);
    let leaves = probe.level_range(probe.leaf_level());
    let passes = if l.settings.two_bounces {
        2
    } else {
        1
    };
    for pass in 0..passes {
        // Each bounce lights the leaves (the second from the first texture), builds the mips, writes every level.
        out.push(dispatch(
            if pass == 0 {
                "voxelgiLightCS"
            } else {
                "voxelgiBounceCS"
            },
            leaves.clone(),
            0,
            l,
        ));
        for mip in 1..probe.mip_count() {
            out.push(dispatch(
                "voxelgiMipmapsCS",
                probe.level_range(probe.mip_level(mip)),
                mip,
                l,
            ));
        }
        write_texture(out, l);
    }
}

/// The dispatches of one light update for the shot's lights and settings
/// (what the plan records when the light cache changes), and their workgroups.
pub fn update_dispatches(state: &ShotState) -> (usize, u32) {
    let (words, settings) = light_cache(state);
    let mut out = Vec::new();
    light_passes(
        &mut out,
        Lighting {
            bake: world::bake_of(state),
            light_count: (words.len() / 24) as u32,
            settings: &settings,
        },
    );
    let groups = out
        .iter()
        .map(|c| match c {
            ProgramCommand::Dispatch { groups, .. } => groups[0],
            _ => 0,
        })
        .sum();
    (out.len(), groups)
}

/// The shown bake's distance field into its texture, as voxel_gi_allocate_data
/// uploads it, over a light texture cleared for its cells.
fn upload_bake(out: &mut Vec<ProgramCommand>, bake: world::Bake) {
    let s = world::probe_of(bake).octree_size.as_uvec3();
    let mut params = [0u32; 16];
    params[0] = world::data_layout(bake).sdf * 4;
    out.push(ProgramCommand::Clear(Element::all("voxelgiLight")));
    out.push(ProgramCommand::Dispatch {
        pass: "voxelgiSdfUploadCS",
        instance: 0,
        groups: [s.x.div_ceil(4), s.y.div_ceil(4), s.z.div_ceil(4)],
        params,
    });
}

/// The program's plan for a frame, in `GI::VoxelGIInstance::update`'s order:
/// the shown bake uploaded when it changed or the plan starts from nothing (a
/// frame without history, whose images the lab may have made anew); the
/// light texture cleared if dynamic objects were plotted into it last frame;
/// the lights uploaded when the light cache changed or a dynamic object is
/// in the box; then the light passes when the light cache or the settings
/// changed (or every frame, with `giRelightEachFrame`), else only the write
/// pass when there are dynamic objects to plot over a clean texture; then the
/// dynamic objects (dynamic_gpu.rs).
pub fn plan(any: &mut dyn std::any::Any, frame: &ProgramFrame) -> Vec<ProgramCommand> {
    let state = any
        .downcast_mut::<PlanState>()
        .expect("VoxelGI's plan state");
    let bake = world::bake_of(frame.state);
    let probe = world::probe_of(bake);
    assert!(
        probe.data.len() <= OUTPUT_CELLS,
        "voxelgiOutputs is too small for the bake's cells"
    );
    let (words, settings) = light_cache(frame.state);
    let key = cache_key(&words, &settings);
    let mut out = Vec::new();
    if !frame.history || state.bake != Some(bake) {
        // A new bake (its data version): its field uploaded, its cells lit from a cleared texture.
        upload_bake(&mut out, bake);
        *state = PlanState {
            bake: Some(bake),
            ..PlanState::default()
        };
    }
    let dynamic = bake == world::Bake::DynamicTeapot;
    let relight = state.lit != Some(key) || frame.state.p("giRelightEachFrame") > 0.5;
    let l = Lighting {
        bake,
        light_count: (words.len() / 24) as u32,
        settings: &settings,
    };
    if state.plotted {
        // Last frame's dynamic texels are cleared before anything is written (gi.cpp:2963-2966).
        out.push(ProgramCommand::Clear(Element::all("voxelgiLight")));
    }
    if relight || dynamic {
        out.push(ProgramCommand::Upload {
            buffer: Element::of("voxelgiLights", 0),
            offset: 0,
            words,
        });
    }
    if relight {
        light_passes(&mut out, l);
    } else if dynamic || state.plotted {
        // Only re-blitting is necessary: the cells' light, from write_buffer.
        write_texture(&mut out, l);
    }
    state.lit = Some(key);
    state.plotted = dynamic;
    if dynamic {
        let teapot = world::teapot_at(frame.state.p("dynamicX"));
        let index = lab::scene::ids::TEAPOT as u32;
        super::dynamic_gpu::commands(
            &mut out,
            probe,
            (&teapot, index),
            (l.light_count, &settings),
        );
    }
    out
}
