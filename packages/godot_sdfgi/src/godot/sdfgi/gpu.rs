//! Godot's SDFGI as it runs on the GPU (story research.md, sections 11 to 18):
//! its resources at the engine's formats and sizes (`SDFGI::create`,
//! gi.cpp:414-1129), its passes (the voxelizer's three raster passes, the
//! pre-process's compute chain, the light, the probes), and a plan that
//! records them each frame in the engine's order (`_update_sdfgi`,
//! `update_cascades`, `pre_process_gi`, `update_light`, `update_probes`,
//! `store_probes`), as a lab program (`lab::investigation::ShadingProgram`).
//! The WGSL is in shaders/sdfgi_gpu_*.wgsl; the CPU mirrors (model.rs,
//! voxels.rs, occlusion.rs, light.rs, probes.rs) are the selftest's
//! references for these passes.
//!
//! Where the lab differs (said in the story): the voxelizer draws into a
//! colour target it never writes, since wgpu has no render pass without
//! attachments; the readers decode the RGB9E5 light and probe words and the
//! 4-bit occlusion themselves (wgpu has no reinterpreting views); wgpu orders
//! every dispatch after the one before it, so the four cascades' dispatches
//! (and the eight occlusion octants) that Godot runs without barriers run
//! one after another; the room has no static lights, so the static light
//! pass never runs; and a frame that does not follow the one before it (a
//! snapshot, the first frame, a jump of the camera) is first converged for
//! `CONVERGED_FRAMES` frames, so a still shows what a running game shows.
//!
//! Ported from Godot Engine (MIT; Copyright (c) 2014-present Godot Engine
//! contributors, Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur).

use super::model::{Cascades, Dirty, Settings, CASCADE_SIZE, PROBE_AXIS_COUNT, PROBE_CELLS};
use glam::{IVec3, Vec3};
use lab::investigation::{
    Access::{self, *},
    Element, ImageKind, PassKind, ProgramBuffer, ProgramCommand, ProgramFrame, ProgramImage, ProgramPass, ProgramStage, ProgramUse, ShadingProgram,
    Which::{self, *},
};
use wgpu::TextureFormat as F;

/// `sdfgi_cascades` (4), the cascade count the resources are made for.
pub const CASCADES: u32 = 4;
/// `history_size` at `frames_to_converge`'s default (30 frames).
pub const HISTORY: u32 = 30;
const N: u32 = CASCADE_SIZE as u32;
const P: u32 = PROBE_AXIS_COUNT as u32;
/// `solid_cell_count = cascade_size^3 * solid_cell_ratio (0.25)` (gi.cpp:429).
pub const SOLID_CELLS: u64 = (N as u64 * N as u64 * N as u64) / 4;
/// `MAX(MAX_STATIC_LIGHTS, MAX_DYNAMIC_LIGHTS)` lights of 112 bytes (gi.cpp:590).
pub const LIGHT_BYTES: u64 = 112;
/// Frames a frame that does not follow the last is converged for first.
pub const CONVERGED_FRAMES: u32 = super::sim::CONVERGED_FRAMES;

const fn image(name: &'static str, kind: ImageKind, size: [u32; 3], format: F, count: u32) -> ProgramImage {
    ProgramImage { name, kind, size, format, count }
}

pub static IMAGES: &[ProgramImage] = &[
    // Shared by all cascades (one working set, rebuilt by each cascade in turn).
    image("renderAlbedo", ImageKind::D3, [N; 3], F::R16Uint, 1),
    image("renderEmission", ImageKind::D3, [N; 3], F::R32Uint, 1),
    image("renderEmissionAniso", ImageKind::D3, [N; 3], F::R32Uint, 1),
    image("renderGeomFacing", ImageKind::D3, [N; 3], F::R32Uint, 1),
    image("renderOcclusion", ImageKind::D3, [N; 3], F::R8Unorm, 8),
    image("renderSdf", ImageKind::D3, [N; 3], F::Rgba8Uint, 2),
    image("renderSdfHalf", ImageKind::D3, [N / 2; 3], F::Rgba8Uint, 2),
    image("occlusionData", ImageKind::D3, [N * 2, N, N * CASCADES], F::R16Uint, 1),
    image("lightprobeHistoryScroll", ImageKind::D2Array, [P * P, P * 16, HISTORY], F::Rgba16Sint, 1),
    image("lightprobeAverageScroll", ImageKind::D2, [P * P, P * 16, 1], F::Rgba32Sint, 1),
    image("lightprobeData", ImageKind::D2Array, [P * P * 8, P * 8, CASCADES * 2], F::R32Uint, 1),
    // One per cascade.
    image("sdf", ImageKind::D3, [N; 3], F::R8Unorm, CASCADES),
    image("lightData", ImageKind::D3, [N; 3], F::R32Uint, CASCADES),
    image("lightAniso0", ImageKind::D3, [N; 3], F::Rgba8Unorm, CASCADES),
    image("lightAniso1", ImageKind::D3, [N; 3], F::Rg8Unorm, CASCADES),
    image("lightprobeHistory", ImageKind::D2Array, [P * P, P * 16, HISTORY], F::Rgba16Sint, CASCADES),
    image("lightprobeAverage", ImageKind::D2, [P * P, P * 16, 1], F::Rgba32Sint, CASCADES),
];

pub static BUFFERS: &[ProgramBuffer] = &[
    // `cascades_ubo`: 8 x 48 bytes (gi.h:572-578).
    ProgramBuffer { name: "cascadesUbo", bytes: 8 * 48, indirect: false, count: 1 },
    ProgramBuffer { name: "solidCells", bytes: SOLID_CELLS * 16, indirect: false, count: CASCADES },
    ProgramBuffer { name: "dispatchStorage", bytes: 16, indirect: false, count: CASCADES },
    ProgramBuffer { name: "dispatchCall", bytes: 16, indirect: true, count: CASCADES },
    ProgramBuffer { name: "lights", bytes: 1024 * LIGHT_BYTES, indirect: false, count: CASCADES },
    // What the scene pass reads of the frame's cascades (`SDFGIData`'s cascades, gi.cpp:1883-1901):
    // two vec4 each, relative to SDFGI's camera, then that camera.
    ProgramBuffer { name: "screenData", bytes: 8 * 32 + 16, indirect: false, count: 1 },
];

const fn u(binding: u32, resource: &'static str, access: Access, which: Which) -> ProgramUse {
    ProgramUse { binding, resource, access, which }
}

const PING: &[u32] = &[0, 1];
const PONG: &[u32] = &[1, 0];
/// The parent cascade's average a scrolled cascade seeds new probes from (the
/// last binds the one below it, "won't be used", gi.cpp:1083-1099).
const PARENT: &[u32] = &[1, 2, 3, 2];

const fn compute(name: &'static str, entry: &'static str, workgroup: [u32; 3], uses: &'static [ProgramUse], stage: ProgramStage, instances: u32) -> ProgramPass {
    ProgramPass { name, kind: PassKind::Compute { entry, workgroup }, uses, stage, instances }
}

/// The eight occlusion images, as STORE reads them (bindings 3 to 10).
const STORE_USES: &[ProgramUse] = &[
    u(1, "renderSdf", Load, Fixed(1)),
    u(2, "renderAlbedo", Load, First),
    u(3, "renderOcclusion", Load, Fixed(0)),
    u(4, "renderOcclusion", Load, Fixed(1)),
    u(5, "renderOcclusion", Load, Fixed(2)),
    u(6, "renderOcclusion", Load, Fixed(3)),
    u(7, "renderOcclusion", Load, Fixed(4)),
    u(8, "renderOcclusion", Load, Fixed(5)),
    u(9, "renderOcclusion", Load, Fixed(6)),
    u(10, "renderOcclusion", Load, Fixed(7)),
    u(11, "renderEmission", Load, First),
    u(12, "renderEmissionAniso", Load, First),
    u(13, "renderGeomFacing", Load, First),
    u(14, "sdf", Write, Instance),
    u(15, "occlusionData", Write, First),
    u(16, "dispatchStorage", Atomic, Instance),
    u(17, "solidCells", Write, Instance),
];

const SCROLL_OCCLUSION_USES: &[ProgramUse] = &[
    u(1, "renderOcclusion", Write, Fixed(0)),
    u(2, "renderOcclusion", Write, Fixed(1)),
    u(3, "renderOcclusion", Write, Fixed(2)),
    u(4, "renderOcclusion", Write, Fixed(3)),
    u(5, "renderOcclusion", Write, Fixed(4)),
    u(6, "renderOcclusion", Write, Fixed(5)),
    u(7, "renderOcclusion", Write, Fixed(6)),
    u(8, "renderOcclusion", Write, Fixed(7)),
    u(9, "occlusionData", Load, First),
];

/// What the light passes read: every cascade's field, the cascades, the cell list, the lights.
const LIGHT_USES_DYNAMIC: &[ProgramUse] = &[
    u(1, "sdf", Sampled, All),
    u(2, "linearSampler", Sampler, First),
    u(3, "dispatchStorage", Read, Instance),
    u(4, "solidCells", Read, Instance),
    u(5, "lightData", Write, Instance),
    u(6, "lightAniso0", Write, Instance),
    u(7, "lightAniso1", Write, Instance),
    u(8, "cascadesUbo", Read, First),
    u(9, "lights", Read, Instance),
    u(10, "lightprobeData", Load, First),
    u(11, "occlusionData", Load, First),
];

const LIGHT_USES_STATIC: &[ProgramUse] = &[
    u(1, "sdf", Sampled, All),
    u(2, "linearSampler", Sampler, First),
    u(3, "dispatchStorage", Read, Instance),
    u(4, "solidCells", ReadWrite, Instance),
    u(8, "cascadesUbo", Read, First),
    u(9, "lights", Read, Instance),
];

/// What the probe passes bind (`integrate_uniform_set`, gi.cpp:1000-1122).
const PROCESS_USES: &[ProgramUse] = &[
    u(1, "sdf", Sampled, All),
    u(2, "lightData", Load, All),
    u(3, "lightAniso0", Sampled, All),
    u(4, "lightAniso1", Sampled, All),
    u(6, "linearSampler", Sampler, First),
    u(7, "cascadesUbo", Read, First),
    u(9, "lightprobeHistory", ReadWrite, Instance),
    u(10, "lightprobeAverage", ReadWrite, Instance),
];

const STORE_PROBE_USES: &[ProgramUse] = &[u(8, "lightprobeData", Write, First), u(10, "lightprobeAverage", Load, Instance)];

const SCROLL_PROBE_USES: &[ProgramUse] = &[
    u(7, "cascadesUbo", Read, First),
    u(9, "lightprobeHistory", Load, Instance),
    u(10, "lightprobeAverage", Load, Instance),
    u(11, "lightprobeHistoryScroll", Write, First),
    u(12, "lightprobeAverageScroll", Write, First),
    u(13, "lightprobeAverage", Load, Pick(PARENT)),
];

const SCROLL_STORE_PROBE_USES: &[ProgramUse] = &[
    u(9, "lightprobeHistory", Write, Instance),
    u(10, "lightprobeAverage", Write, Instance),
    u(11, "lightprobeHistoryScroll", Load, First),
    u(12, "lightprobeAverageScroll", Load, First),
];

pub static PASSES: &[ProgramPass] = &[
    // _update_sdfgi: each region voxelized, then each dirty cascade pre-processed (gi.cpp:2061-2418).
    ProgramPass {
        name: "sdfgiVoxelize",
        kind: PassKind::Raster { vertex: "sdfgiVoxelizeVS", fragment: "sdfgiVoxelizeFS" },
        uses: &[u(1, "renderAlbedo", Write, First), u(2, "renderEmission", Write, First), u(3, "renderEmissionAniso", Write, First), u(4, "renderGeomFacing", Atomic, First)],
        stage: ProgramStage::Start,
        instances: 1,
    },
    compute(
        "sdfgiScroll",
        "sdfgiScrollCS",
        [64, 1, 1],
        &[u(1, "renderAlbedo", Write, First), u(2, "renderGeomFacing", Write, First), u(3, "renderEmission", Write, First), u(4, "renderEmissionAniso", Write, First), u(5, "dispatchStorage", Read, Instance), u(6, "solidCells", Read, Instance)],
        ProgramStage::Start,
        CASCADES,
    ),
    compute("sdfgiScrollOcclusion", "sdfgiScrollOcclusionCS", [4, 4, 4], SCROLL_OCCLUSION_USES, ProgramStage::Start, 1),
    compute("sdfgiProbeScroll", "sdfgiProbeScrollCS", [8, 8, 1], SCROLL_PROBE_USES, ProgramStage::Start, CASCADES),
    compute("sdfgiProbeScrollStore", "sdfgiProbeScrollStoreCS", [8, 8, 1], SCROLL_STORE_PROBE_USES, ProgramStage::Start, CASCADES),
    compute("sdfgiProbeStoreScroll", "sdfgiProbeStoreCS", [8, 8, 1], STORE_PROBE_USES, ProgramStage::Start, CASCADES),
    compute("sdfgiJumpFloodInit", "sdfgiJumpFloodInitCS", [4, 4, 4], &[u(1, "renderAlbedo", Load, First), u(2, "renderSdfHalf", Write, Fixed(0))], ProgramStage::Start, 1),
    compute("sdfgiJumpFlood", "sdfgiJumpFloodCS", [4, 4, 4], &[u(1, "renderSdfHalf", Load, Pick(PING)), u(2, "renderSdfHalf", Write, Pick(PONG))], ProgramStage::Start, 2),
    compute("sdfgiJumpFloodShared", "sdfgiJumpFloodSharedCS", [8, 8, 8], &[u(1, "renderSdfHalf", Load, Pick(PING)), u(2, "renderSdfHalf", Write, Pick(PONG))], ProgramStage::Start, 2),
    compute(
        "sdfgiUpscale",
        "sdfgiUpscaleCS",
        [4, 4, 4],
        &[u(1, "renderAlbedo", Load, First), u(2, "renderSdfHalf", Load, Fixed(0)), u(3, "renderSdf", Write, Fixed(0))],
        ProgramStage::Start,
        1,
    ),
    compute("sdfgiJumpFloodFull", "sdfgiJumpFloodSharedCS", [8, 8, 8], &[u(1, "renderSdf", Load, Fixed(0)), u(2, "renderSdf", Write, Fixed(1))], ProgramStage::Start, 1),
    compute("sdfgiOcclusion", "sdfgiOcclusionCS", [64, 1, 1], &[u(1, "renderOcclusion", ReadWrite, Instance), u(2, "renderGeomFacing", Load, First)], ProgramStage::Start, 8),
    compute("sdfgiStore", "sdfgiStoreCS", [4, 4, 4], STORE_USES, ProgramStage::Start, CASCADES),
    // render_static_lights, update_light (gi.cpp:2422-2610, 1249-1309).
    compute("sdfgiStaticLight", "sdfgiStaticLightCS", [64, 1, 1], LIGHT_USES_STATIC, ProgramStage::Start, CASCADES),
    compute("sdfgiDynamicLight", "sdfgiDynamicLightCS", [64, 1, 1], LIGHT_USES_DYNAMIC, ProgramStage::Start, CASCADES),
    // update_probes, inside the depth pre-pass (rfc.cpp:2120-2133); store_probes in _pre_opaque_render (rfc.cpp:1534).
    compute("sdfgiProbeProcess", "sdfgiProbeProcessCS", [8, 8, 1], PROCESS_USES, ProgramStage::AfterPrepass, CASCADES),
    // Both after the pre-pass, before the screen gather that reads the maps (_pre_opaque_render, rfc.cpp:1517-1607).
    compute("sdfgiProbeStore", "sdfgiProbeStoreCS", [8, 8, 1], STORE_PROBE_USES, ProgramStage::AfterPrepass, CASCADES),
];

pub static PROGRAM: ShadingProgram = ShadingProgram {
    images: IMAGES,
    buffers: BUFFERS,
    passes: PASSES,
    scene_uses: &[u(1, "lightprobeData", Load, First), u(2, "screenData", Read, First)],
    init: || Box::new(Plan::default()),
    plan: |state, frame| state.downcast_mut::<Plan>().expect("the SDFGI plan").frame(frame),
};

/// The CPU side of SDFGI for the GPU program: the cascades and the counters
/// gi.cpp keeps between frames.
#[derive(Default)]
pub struct Plan {
    cascades: Option<Cascades>,
    /// `RSG::rasterizer->get_frame_number()`.
    frame: u32,
    /// `render_pass`: probe updates so far.
    render_pass: u32,
    /// `all_dynamic_lights_dirty`, per cascade.
    lights_dirty: Vec<bool>,
    /// The camera of the last frame planned, and the shading values then (a jump converges again).
    last: Option<(Vec3, u64)>,
}

fn params(words: &[u32]) -> [u32; 16] {
    let mut p = [0u32; 16];
    p[..words.len()].copy_from_slice(words);
    p
}

fn f(v: f32) -> u32 {
    v.to_bits()
}

fn i(v: i32) -> u32 {
    v as u32
}

fn dispatch(pass: &'static str, instance: u32, groups: [u32; 3], words: &[u32]) -> ProgramCommand {
    ProgramCommand::Dispatch { pass, instance, groups, params: params(words) }
}

fn threads(n: [u32; 3], local: [u32; 3]) -> [u32; 3] {
    [n[0].div_ceil(local[0]), n[1].div_ceil(local[1]), n[2].div_ceil(local[2])]
}

/// The scene's SDFGI settings, from the shot (`SceneSettings`).
pub fn settings_of(state: &lab::shot::ShotState) -> super::sim::SceneSettings {
    super::sim::SceneSettings::of(state)
}

impl Plan {
    fn frame(&mut self, frame: &ProgramFrame) -> Vec<ProgramCommand> {
        let state = frame.state;
        let camera = state.camera.eye();
        let s = settings_of(state);
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            for v in [s.bounce_feedback, s.probe_bias] {
                v.to_bits().hash(&mut h);
            }
            (s.ray_count, s.history_size, s.frames_to_update_light, s.read_sky).hash(&mut h);
            h.finish()
        };
        let mut out = Vec::new();
        // A frame that does not follow the last starts again and converges first.
        let jumped = self.last.is_none_or(|(eye, k)| k != key || (!frame.history && eye.distance(camera) > 0.5));
        if jumped {
            self.cascades = None;
        }
        self.last = Some((camera, key));
        // RenderForwardClustered::sdfgi_update: create, or follow the camera.
        let created = self.cascades.is_none();
        let cascades = self.cascades.get_or_insert_with(|| Cascades::create(Settings::default(), camera));
        if created {
            self.lights_dirty = vec![true; cascades.cascades.len()];
            self.render_pass = 0;
            out.push(ProgramCommand::Clear(Element::all("lightprobeHistory")));
            out.push(ProgramCommand::Clear(Element::all("lightprobeAverage")));
            out.push(ProgramCommand::Clear(Element::all("lightprobeData")));
        } else {
            cascades.update(camera);
        }
        let cascades = cascades.clone();
        regions(&cascades, &s, &mut out, &mut self.lights_dirty);
        let objects = lab::scene::Scene::study_room();
        let rounds = if jumped { CONVERGED_FRAMES } else { 1 };
        for _ in 0..rounds {
            self.light_and_probes(&cascades, &s, &objects, camera, &mut out);
        }
        for c in &mut self.cascades.iter_mut().flat_map(|c| c.cascades.iter_mut()) {
            c.dirty = Dirty::Regions(IVec3::ZERO);
        }
        out
    }

    /// `update_cascades`, `pre_process_gi`, `update_light`, `update_probes`, `store_probes`: one frame's work after the regions.
    fn light_and_probes(&mut self, cascades: &Cascades, s: &super::sim::SceneSettings, scene: &lab::scene::Scene, camera: Vec3, out: &mut Vec<ProgramCommand>) {
        let n = cascades.cascades.len();
        let y_mult = cascades.settings.y_mult();
        // update_cascades (gi.cpp:1514-1533).
        let mut ubo: Vec<u32> = Vec::with_capacity(8 * 12);
        for k in 0..8 {
            let c = cascades.cascades.get(k).copied().unwrap_or(cascades.cascades[n - 1]);
            let offset = (IVec3::splat(-(CASCADE_SIZE >> 1)) + c.position).as_vec3() * c.cell_size;
            let probe = c.position / PROBE_CELLS;
            ubo.extend([f(offset.x), f(offset.y), f(offset.z), f(1.0 / c.cell_size), i(probe.x), i(probe.y), i(probe.z), 0, 0, 0, 0, 0]);
        }
        out.push(ProgramCommand::Upload { buffer: Element::of("cascadesUbo", 0), offset: 0, words: ubo });
        // pre_process_gi: the dynamic lights of each cascade (the room's panel), and the scene pass's cascades.
        let panel = super::sim::panel_light(scene, y_mult);
        for c in 0..n {
            out.push(ProgramCommand::Upload { buffer: Element::of("lights", c as u32), offset: 0, words: light_words(&panel) });
        }
        let mut screen = vec![0u32; 8 * 8];
        for (k, v) in super::sim::frame_cascades(cascades, camera).into_iter().enumerate() {
            screen[k * 4..k * 4 + 4].copy_from_slice(&v.map(f));
        }
        screen.extend([f(camera.x), f(camera.y), f(camera.z), 0]);
        out.push(ProgramCommand::Upload { buffer: Element::of("screenData", 0), offset: 0, words: screen });
        // update_light (gi.cpp:1249-1309): every cell of a rebuilt cascade, else every Nth.
        let every = s.frames_to_update_light as u32;
        for c in 0..n {
            out.push(ProgramCommand::Copy { from: Element::of("dispatchStorage", c as u32), to: Element::of("dispatchCall", c as u32), bytes: 16 });
        }
        for c in 0..n {
            let (offset, increment) = if self.lights_dirty[c] || every <= 1 { (0, 1) } else { (self.frame % every, every) };
            out.push(ProgramCommand::DispatchIndirect {
                pass: "sdfgiDynamicLight",
                instance: c as u32,
                buffer: Element::of("dispatchCall", c as u32),
                offset: 0,
                params: params(&[n as u32, c as u32, 1, offset, increment, f(s.bounce_feedback), f(y_mult), 0]),
            });
            self.lights_dirty[c] = false;
        }
        // update_probes (gi.cpp:1311-1411).
        let history_index = self.render_pass % s.history_size.min(HISTORY);
        let ray_count = s.ray_count;
        let sky = u32::from(s.read_sky);
        for c in 0..n {
            let probe = cascades.cascades[c].position / PROBE_CELLS;
            out.push(dispatch("sdfgiProbeProcess", c as u32, threads([P * P, P, 1], [8, 8, 1]), &[n as u32, c as u32, history_index, s.history_size.min(HISTORY), ray_count, f(s.probe_bias), P * P, P, i(probe.x), i(probe.y), i(probe.z), sky, 0, 0, 0, f(y_mult)]));
        }
        self.render_pass += 1;
        // store_probes (gi.cpp:1414-1456).
        for c in 0..n {
            out.push(dispatch("sdfgiProbeStore", c as u32, threads([P * P * 6, P * 6, 1], [8, 8, 1]), &[n as u32, c as u32, 0, s.history_size.min(HISTORY), 0, 0, P * P * 6, P * 6]));
        }
        self.frame += 1;
    }
}

/// `_update_sdfgi`: each pending region voxelized (`render_region`), and each
/// cascade's pre-process after its last region (gi.cpp:2061-2418).
fn regions(cascades: &Cascades, s: &super::sim::SceneSettings, out: &mut Vec<ProgramCommand>, lights_dirty: &mut [bool]) {
    let regions = cascades.pending_regions();
    for (k, r) in regions.iter().enumerate() {
        let c = r.cascade;
        let first = k == 0 || regions[k - 1].cascade != c;
        let last = k + 1 == regions.len() || regions[k + 1].cascade != c;
        if first {
            for name in ["renderAlbedo", "renderEmission", "renderEmissionAniso", "renderGeomFacing"] {
                out.push(ProgramCommand::Clear(Element::all(name)));
            }
        }
        // _render_sdfgi: one draw per axis (rfc.cpp:3131-3176); the framebuffer is the face (right, up).
        for axis in 0..3usize {
            let (right, up) = ((axis + 1) % 3, (axis + 2) % 3);
            let size = [r.local_size[right] as u32, r.local_size[up] as u32];
            let o = r.local_offset;
            let z = r.local_size;
            out.push(ProgramCommand::Draw {
                pass: "sdfgiVoxelize",
                instance: 0,
                size,
                params: params(&[axis as u32, i(o.x), i(o.y), i(o.z), i(z.x), i(z.y), i(z.z), f(r.bounds_min.x), f(r.bounds_min.y), f(r.bounds_min.z), f(r.bounds_size.x), f(r.bounds_size.y), f(r.bounds_size.z)]),
            });
        }
        if last {
            preprocess(cascades, c, s, out);
            lights_dirty[c] = true;
        }
    }
}

fn preprocess(cascades: &Cascades, c: usize, s: &super::sim::SceneSettings, out: &mut Vec<ProgramCommand>) {
    let cascade = &cascades.cascades[c];
    let ci = c as u32;
    let scroll = match cascade.dirty {
        Dirty::All => None,
        Dirty::Regions(d) => Some(d),
    };
    let sc = scroll.unwrap_or(IVec3::ZERO);
    let base = [i(sc.x), i(sc.y), i(sc.z), N, 0, 0, 0, 0, 0, 0, ci];
    if let Some(d) = scroll {
        // The scroll (gi.cpp:2113-2211): kept cells back into the working set, the occlusion, the probes.
        out.push(ProgramCommand::Copy { from: Element::of("dispatchStorage", ci), to: Element::of("dispatchCall", ci), bytes: 16 });
        out.push(ProgramCommand::DispatchIndirect { pass: "sdfgiScroll", instance: ci, buffer: Element::of("dispatchCall", ci), offset: 0, params: params(&base) });
        let keep = [N - d.x.unsigned_abs(), N - d.y.unsigned_abs(), N - d.z.unsigned_abs()];
        out.push(dispatch("sdfgiScrollOcclusion", 0, threads(keep, [4, 4, 4]), &base));
        let probe_scroll = d / PROBE_CELLS;
        let n = cascades.cascades.len() as u32;
        let ip = [n, ci, 0, s.history_size.min(HISTORY), 0, 0, P * P, P, 0, 0, 0, 0, i(probe_scroll.x), i(probe_scroll.y), i(probe_scroll.z), f(cascades.settings.y_mult())];
        out.push(dispatch("sdfgiProbeScroll", ci, threads([P * P, P, 1], [8, 8, 1]), &ip));
        out.push(dispatch("sdfgiProbeScrollStore", ci, threads([P * P, P, 1], [8, 8, 1]), &ip));
        if s.bounce_feedback > 0.0 {
            out.push(dispatch("sdfgiProbeStoreScroll", ci, threads([P * P * 6, P * 6, 1], [8, 8, 1]), &[n, ci, 0, s.history_size.min(HISTORY), 0, 0, P * P * 6, P * 6]));
        }
    }
    // The counters zeroed before STORE's atomics (gi.cpp:2214-2215).
    out.push(ProgramCommand::Clear(Element::of("dispatchStorage", ci)));
    // The half-size jump flood (gi.cpp:2217-2290): init, steps 32 and 16, the shared-memory steps 8 to 1, upscale, one full-size pass.
    let half = N / 2;
    let mut p = base;
    p[3] = half;
    out.push(dispatch("sdfgiJumpFloodInit", 0, threads([half; 3], [4, 4, 4]), &p));
    let mut ping = 0u32;
    p[8] = 1;
    for step in super::voxels::half_schedule() {
        p[7] = step.0 as u32;
        let (pass, local) = if step.1 { ("sdfgiJumpFloodShared", [8, 8, 8]) } else { ("sdfgiJumpFlood", [4, 4, 4]) };
        out.push(dispatch(pass, ping, threads([half; 3], local), &p));
        ping ^= 1;
    }
    let mut p = base;
    p[7] = 1;
    out.push(dispatch("sdfgiUpscale", 0, threads([N; 3], [4, 4, 4]), &p));
    out.push(dispatch("sdfgiJumpFloodFull", 0, threads([N; 3], [8, 8, 8]), &p));
    // Occlusion: eight parity classes of probes, one workgroup per probe (gi.cpp:2342-2373).
    let probe_global = cascade.position / PROBE_CELLS;
    for k in 0..8u32 {
        let mut o = IVec3::new((k & 1) as i32, ((k >> 1) & 1) as i32, ((k >> 2) & 1) as i32);
        for a in 0..3 {
            if probe_global[a] & 1 != 0 {
                o[a] = (o[a] + 1) & 1;
            }
        }
        let mut q = base;
        q[4] = i(o.x);
        q[5] = i(o.y);
        q[6] = i(o.z);
        q[7] = 1;
        q[9] = k;
        let g = PROBE_CELLS as u32 + 1;
        out.push(dispatch("sdfgiOcclusion", k, [g - o.x as u32, g - o.y as u32, g - o.z as u32], &q));
    }
    out.push(dispatch("sdfgiStore", ci, threads([N; 3], [4, 4, 4]), &base));
    for name in ["lightData", "lightAniso0", "lightAniso1"] {
        out.push(ProgramCommand::Clear(Element::of(name, ci)));
    }
}

/// A light as `SDFGIShader::Light` (std140, 112 bytes, gi.h:351-367).
pub fn light_words(l: &super::light::Light) -> Vec<u32> {
    let mut w = vec![0u32; 28];
    let put3 = |w: &mut Vec<u32>, at: usize, v: Vec3| {
        w[at] = f(v.x);
        w[at + 1] = f(v.y);
        w[at + 2] = f(v.z);
    };
    put3(&mut w, 0, l.color);
    w[3] = f(l.energy);
    put3(&mut w, 4, l.direction);
    w[7] = 1;
    put3(&mut w, 8, l.position);
    w[11] = f(l.attenuation);
    w[12] = l.kind;
    w[13] = f(l.cos_spot_angle);
    w[14] = f(l.inv_spot_attenuation);
    w[15] = f(l.radius);
    put3(&mut w, 16, l.area_width);
    put3(&mut w, 20, l.area_height);
    w
}
