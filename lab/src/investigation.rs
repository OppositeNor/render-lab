//! An investigation: one engine feature the lab explains (each is the code of
//! a package, packages/<name>/src, built as the library the lab loads at run
//! time; src/investigations.rs). The lab's renderer, recorder, viewer,
//! dock, and timeline player are shared; an investigation supplies what is
//! specific to its feature: its demonstrations and their parameters, the
//! frame content each shot shows, the dock's sliders, picking, its source
//! index and pseudo-code, its timelines, its verification suite, and, if it
//! shades the scene its own way, the WGSL that does it (`SceneShading`).
//!
//! One investigation is current at a time (`set`, `current`), the way the
//! locale is (src/i18n.rs): the homepage or the command line chooses it
//! before anything is loaded or rendered. `scoped` makes another current on
//! one thread for a while (the homepage's backdrop).

use crate::camera::Projector;
use crate::controls::Dock;
use crate::demo::{Demos, Tab};
use crate::frame::{FrameContent, Statics};
use crate::pseudocode::Listing;
use crate::resources;
use crate::scene::SceneId;
use crate::shot::{DemoId, ShotState};
use crate::sources::SourceIndex;
use crate::viewer::ViewerTest;
use glam::Vec2;
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::RwLock;

/// A game engine the lab investigates; its features are investigations.
pub struct Engine {
    /// Short key, used in `engine/feature` paths.
    pub id: &'static str,
    pub name: &'static str,
    /// One or two sentences for the homepage.
    pub summary: &'static str,
}

/// A narrated timeline of an investigation, playable in the viewer: a demo
/// of its package (src/packages.rs).
pub struct TimelineInfo {
    pub id: String,
    pub title: String,
    /// The timeline JSON in its built package. Images its beats name are
    /// relative to its directory.
    pub path: String,
    /// The narration track in its built package (it may not be built).
    pub narration: String,
}

/// Words the shared chrome uses for the engine.
pub struct Wording {
    /// The engine in running text: "≈ <engine>: <structure>" in the struct panel.
    pub engine_short: &'static str,
    /// The header's tag before the engine's version and commit.
    pub engine_tag: &'static str,
    /// The checkout directory source paths are relative to, with a slash.
    pub checkout_dir: &'static str,
}

/// An investigation's own shading of the scene pass, in place of the lab's
/// (`sceneFS` in packages/render_lab/shaders/scene.wgsl). A package declares
/// it in its `shading.toml`, which src/shading_file.rs reads into this; its
/// functions are the investigation's `shading_hooks`. `module` is a shader its
/// package declares (src/shaders.rs), composed after the lab's
/// `render_lab.shader/scene` (the
/// shared prelude and scene.wgsl, so it may use `SceneVaryings`,
/// `SceneOutput`, and everything in common.wgsl), and it declares bind group 1:
///
/// - binding 0: `var<uniform> gShading`, at most `SHADING_UNIFORMS` vec4s:
///   the frame's `FrameContent::shading`, or, when it is a struct tagged
///   `{@ shader_data}`, its values packed by the lab and its derived members
///   filled by `shader_data` (src/shader_data.rs);
/// - binding 1: `var<storage, read> gShadingData: array<vec4<f32>>`, `data()`,
///   computed once per renderer;
/// - binding 2: `var<storage, read> gShadingFrame: array<vec4<f32>>`, the
///   frame's `FrameContent::shading_frame`.
///
/// Its compute entry points in `verify` take the verification buffers as
/// group 2 (`Renderer::run_shading_verify`).
///
/// The frame runs its `nodes` in order. With no `outputs`, the scene pass
/// writes the lab's HDR target. With `outputs`, it starts a chain: the
/// fragment shader writes those `targets` (in order), which stay
/// multisampled, the compute nodes after it run `passes`, and the last
/// writes `"hdr"`, which the frame shows. A pass binds group 2 as described
/// on `ShadingPass`.
///
/// With a `prepass` fragment entry, a depth pre-pass node (that entry writes
/// `prepass_outputs`, if any, and `@builtin(sample_mask)`, and reads nothing
/// of group 2) comes before the scene pass, which tests against its depth
/// (LessEqual, no depth writes); compute nodes between them read what it
/// wrote. The scene pass's fragment shader may read `scene_reads` from
/// group 2, bound as for a pass.
pub struct SceneShading {
    /// The full key of its shader module (`[[shader]]` in its package).
    pub module: &'static str,
    /// The fragment entry point of the scene pass.
    pub fragment: &'static str,
    pub data: fn() -> Vec<[f32; 4]>,
    pub verify: &'static [&'static str],
    /// The chain's colour targets, besides `"hdr"` and `"depth"`.
    pub targets: &'static [ShadingTarget],
    /// The scene pass's colour outputs, by target name; empty for `"hdr"` alone.
    pub outputs: &'static [&'static str],
    /// The chain's compute passes, in the order of their nodes
    /// (`FrameNode::Compute(i)` is `passes[i]`).
    pub passes: &'static [ShadingPass],
    pub prepass: Option<&'static str>,
    /// The targets the pre-pass's fragment entry writes besides the depth
    /// (multisampled, as the scene pass's outputs; a depth and normal pre-pass).
    /// With them and no `outputs`, the chain is the pre-pass and the compute
    /// passes after it before an ordinary scene pass, which writes the lab's HDR target.
    pub prepass_outputs: &'static [&'static str],
    /// The chain's storage buffers, besides its targets.
    pub buffers: &'static [ShadingBuffer],
    pub scene_reads: &'static [&'static str],
    /// Depth-only views of the scene's meshes (shadow maps), drawn first.
    pub depth_views: Option<&'static DepthViews>,
    /// 3D textures the chain's passes sample (group 3), written by `update`.
    pub volumes: &'static [ShadingVolume],
    /// Compute passes that run only when what they compute changes
    /// (`UpdatePlan::key`), before the frame's chain: they fill `volumes`.
    pub update: Option<&'static ShadingUpdate>,
    /// Targets whose resolution a state chooses at run time.
    pub resolution: Option<&'static ShadingResolution>,
    /// Targets and buffers whose contents a frame reads from the frames
    /// before it (a temporal filter's history): kept from frame to frame,
    /// made again (zero) when the viewport's size changes. A pass may read
    /// one before this frame's writer; the frame graph keeps its writers.
    pub history: &'static [&'static str],
    /// How the frame graph's views show a resource unless told otherwise
    /// (src/graph/): `(resource, "<show>[?<param>=<value>&…]")`, as the
    /// inspector's page takes it (`count?word=3&range=0,8`). The DAG's cards
    /// and a graph image without `imageShow` use it; a resource not listed
    /// is shown as its format suggests (`graph::default_show`).
    pub shows: &'static [(&'static str, &'static str)],
    /// The investigation's own GPU resources and passes, and the commands
    /// that run them each frame (`ShadingProgram`).
    pub program: Option<&'static ShadingProgram>,
    /// The frame's nodes in order (its file's `[[node]]` list): the frame
    /// graph declares them so, and the renderer runs them in that order where
    /// their reads and writes allow. Every declared pass is in it once.
    pub nodes: &'static [FrameNode],
}

/// One node of a shading's frame, in `SceneShading::nodes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FrameNode {
    /// Pass `i` of `ShadingUpdate::passes`; update passes come first (they
    /// run before the frame, when their plan's key changes).
    Update(usize),
    /// The depth views (`SceneShading::depth_views`).
    DepthViews,
    /// The depth pre-pass (`SceneShading::prepass`).
    Prepass,
    /// Compute pass `i` of `SceneShading::passes`.
    Compute(usize),
    /// Pass `i` of `ShadingProgram::passes`. A run of consecutive program
    /// nodes is recorded where the run is: the plan's commands for its passes,
    /// in the plan's order, with the uploads, clears, and copies before each.
    Program(usize),
    /// The scene pass (`SceneShading::fragment`); a plan's commands after its
    /// last pass command are recorded right before it.
    Scene,
}

impl SceneShading {
    /// The run of consecutive program nodes each program pass is in (by
    /// pass index; `u32::MAX` for a pass in no node): where its commands are recorded.
    pub fn program_runs(&self) -> Vec<u32> {
        let mut runs = vec![u32::MAX; self.program.map_or(0, |p| p.passes.len())];
        let mut run = 0u32;
        let mut previous = false;
        for node in self.nodes {
            let program = matches!(node, FrameNode::Program(_));
            if let FrameNode::Program(i) = node
                && let Some(r) = runs.get_mut(*i)
            {
                *r = run;
            }
            if previous && !program {
                run += 1;
            }
            previous = program;
        }
        runs
    }

    /// Whether its frame is a pass chain: the scene pass writes `outputs`, or
    /// a pre-pass writes targets for the passes before the scene pass.
    pub fn chained(&self) -> bool {
        !self.outputs.is_empty() || !self.prepass_outputs.is_empty()
    }

    /// Whether `target` is drawn into (multisampled) by the scene pass or the pre-pass.
    pub fn attachment(&self, target: &str) -> bool {
        self.outputs.contains(&target) || self.prepass_outputs.contains(&target)
    }
}

/// A 3D texture of a pass chain, with its mips: sampled by the chain's
/// passes, the verification kernels, and the update passes that read it, at
/// group 3 (binding 0 a filtering sampler, linear between texels and mips,
/// clamped to the edge; binding `1 + v` volume `v`, `texture_3d<f32>`);
/// written by the update passes that name it, one mip per dispatch
/// (`UpdateDispatch::mip`). It starts at zero.
pub struct ShadingVolume {
    pub name: &'static str,
    pub format: wgpu::TextureFormat,
    pub size: [u32; 3],
    pub mips: u32,
}

/// The update passes of a pass chain: compute passes that run before a
/// frame's chain only when their `UpdatePlan`'s key changes (or every frame
/// while it says `always`), the way an engine relights a cache only when its
/// lights or settings change. The renderer keeps the volumes of the last few
/// keys, so views with different settings each draw their own.
///
/// An update pass binds groups 0 and 1 as the chain's passes do, group 2 its
/// own (binding 0 the dispatch's `UpdateDispatch::params`, a uniform of 16
/// words; binding `1 + k` buffer `k` of `buffers`, `array<u32>` read-write;
/// binding `1 + B + v` volume `v` as a write-only storage texture of the
/// dispatch's mip, for the passes that write it), and group 3 (the volumes,
/// sampled) when it reads a volume.
pub struct ShadingUpdate {
    pub passes: &'static [UpdatePass],
    pub buffers: &'static [UpdateBuffer],
    /// What to run for a state: its key, and the dispatches in order.
    pub plan: fn(&ShotState) -> UpdatePlan,
}

pub struct UpdatePass {
    pub entry: &'static str,
    /// The volumes it samples (group 3) and the update's buffers it reads
    /// (every buffer is bound whichever it uses; the lists are the frame
    /// graph's edges, checked against the WGSL: src/graph/).
    pub reads: &'static [&'static str],
    /// The volumes it writes, at the dispatch's mip, and the buffers it writes.
    pub writes: &'static [&'static str],
    /// Its workgroup's size along x; a dispatch's `groups` count these.
    pub workgroup: u32,
}

/// A storage buffer of the update passes, `words` u32 long.
pub struct UpdateBuffer {
    pub name: &'static str,
    pub words: fn() -> u64,
}

pub struct UpdatePlan {
    /// What the dispatches compute from: the update runs when it changes.
    pub key: u64,
    /// Run every frame (a measurement of the passes' cost).
    pub always: bool,
    pub dispatches: Vec<UpdateDispatch>,
}

/// One dispatch of an update pass: `groups` workgroups along x, writing its
/// volumes' mip `mip`, with its 16 words of parameters (group 2, binding 0).
/// Timed, consecutive dispatches of one pass are one timed span.
#[derive(Clone, Debug)]
pub struct UpdateDispatch {
    pub pass: &'static str,
    pub groups: u32,
    pub mip: u32,
    pub params: [u32; 16],
}

/// Chain targets drawn at a resolution a state chooses: `divisor` divides
/// the viewport's size for each target named (their `ShadingTarget::divisor`
/// is replaced), and the chain's targets are rebuilt when it changes.
pub struct ShadingResolution {
    pub targets: &'static [&'static str],
    pub divisor: fn(&ShotState) -> u32,
}

/// An investigation's own GPU work, for what the pass chain cannot express
/// (the `render-graph` agent skill): its images (2D, 2D arrays, 3D; any format,
/// integer ones too; several images bound as one binding array) and buffers
/// (indirect arguments too), its compute and raster passes, and a plan that
/// returns, each frame, the commands to record in order: uploads, clears,
/// copies, dispatches (direct or indirect, each with its own parameters), and
/// draws of the scene's meshes into a framebuffer of a size of its own.
///
/// A pass binds group 0 (the frame) and group 1 (the shading) as the chain
/// does, the command's parameters at group 2 (binding 0,
/// `var<uniform> p: array<vec4<u32>, 4>`, 16 words), and its `uses` at group 3. The scene
/// pass's fragment entry (and the pre-pass) read `scene_uses` at group 3.
/// Its resources live as long as the renderer and start at zero; the plan
/// runs in the main view only (the other views read what it left).
pub struct ShadingProgram {
    pub images: &'static [ProgramImage],
    pub buffers: &'static [ProgramBuffer],
    pub passes: &'static [ProgramPass],
    /// What the scene pass's fragment entry (and the pre-pass) read at group 3.
    pub scene_uses: &'static [ProgramUse],
    /// The plan's state, made once per renderer.
    pub init: fn() -> Box<dyn std::any::Any + Send>,
    /// This frame's commands, in order: from its state, the shot, and
    /// whether the frame builds on the ones before it (a live view) or
    /// stands alone (a recording's frame, a snapshot).
    pub plan: fn(&mut dyn std::any::Any, &ProgramFrame) -> Vec<ProgramCommand>,
    /// For a program that reads what earlier frames left (an image a
    /// pass after the scene pass copied the frame into): how many times a frame that
    /// stands alone draws the same shot first, each planned and drawn in full,
    /// the first told it has no history (so it starts from zero), the rest
    /// that they do. 0: a standalone frame is drawn once.
    pub warmup: u32,
    /// Whether a recording's frame builds on the one before it, as a live
    /// view's does (the warm-up then runs only where a beat starts, and the
    /// frame shows the read-backs of the frame before it, waited for): for a
    /// program whose result is its history, so a video shows it accumulate.
    /// false: every recorded frame stands alone.
    pub record_history: bool,
}

/// A function of an investigation that its package's shading file names
/// (src/shading_file.rs): what the declaration cannot say, because it computes.
#[derive(Clone, Copy)]
pub enum ShadingHook {
    /// `SceneShading::data`.
    Data(fn() -> Vec<[f32; 4]>),
    /// `ShadingUpdate::plan`.
    UpdatePlan(fn(&ShotState) -> UpdatePlan),
    /// `UpdateBuffer::words`.
    Words(fn() -> u64),
    /// `ShadingResolution::divisor`.
    Divisor(fn(&ShotState) -> u32),
    /// `ShadingProgram::init` and `plan`.
    Program {
        init: fn() -> Box<dyn std::any::Any + Send>,
        plan: fn(&mut dyn std::any::Any, &ProgramFrame) -> Vec<ProgramCommand>,
    },
}

/// What a plan is told about the frame.
pub struct ProgramFrame<'a> {
    pub state: &'a ShotState,
    /// The frame follows the ones before it (`FrameRequest::history`).
    pub history: bool,
    /// The device's optional features that are on (`Gpu::features`).
    pub features: wgpu::Features,
    /// The main view's size in pixels (what `ProgramImage::viewport` divides).
    pub viewport: [u32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    D2,
    D2Array,
    D3,
}

/// A texture of a program: `count` textures of one kind, size, and format
/// (bound as a binding array when more than one), each `size` texels (a 2D
/// array's layers in `size[2]`).
pub struct ProgramImage {
    pub name: &'static str,
    pub kind: ImageKind,
    pub size: [u32; 3],
    pub format: wgpu::TextureFormat,
    pub count: u32,
    /// Its mip levels: 1, or at most this many, each half the one before
    /// (rounded down) until a side reaches one texel (`u32::MAX`: all of them).
    /// A use binds them all, or one per instance (`Which::Mip`).
    pub mips: u32,
    /// 0: `size`. Otherwise its width and height are the main view's divided
    /// by this (rounded down, at least 1), and it is made again, at zero,
    /// when the view's size changes.
    pub viewport: u32,
}

/// `count` buffers of a program, each `bytes` long; `indirect`, also a dispatch's arguments.
pub struct ProgramBuffer {
    pub name: &'static str,
    pub bytes: u64,
    pub indirect: bool,
    pub count: u32,
}

/// One element of a program's image or buffer (`ProgramImage::count`):
/// `index` u32::MAX is every element (a clear).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Element {
    pub name: &'static str,
    pub index: u32,
}

impl Element {
    pub const fn of(name: &'static str, index: u32) -> Element {
        Element { name, index }
    }
    pub const fn all(name: &'static str) -> Element {
        Element {
            name,
            index: u32::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassKind {
    /// A compute entry point and its workgroup's size (`@workgroup_size`).
    Compute {
        entry: &'static str,
        workgroup: [u32; 3],
    },
    /// The scene's meshes drawn by `vertex` (the lab's mesh layout, instance =
    /// object index) and `fragment` into a target the pass does not write,
    /// with no depth and no culling: for fragments that write storage images.
    Raster {
        vertex: &'static str,
        fragment: &'static str,
    },
}

pub struct ProgramPass {
    /// Its node in the frame graph and its row in the GPU timing.
    pub name: &'static str,
    pub kind: PassKind,
    pub uses: &'static [ProgramUse],
    /// Its bind groups: a command names one (`instance`), whose uses of
    /// `Which::Instance` bind that element of their resource (an engine's
    /// per-cascade uniform sets).
    pub instances: u32,
}

/// A binding of group 3: the resource (an image, a buffer, the lab's
/// samplers `"linearSampler"`, `"nearestSampler"`, `"trilinearSampler"`
/// (linear between mips too), all clamped to the edge, or, for a pass, one
/// of the frame's textures it reads with `Load`: `"depth"` (multisampled),
/// `"hdr"`, or a target of the pass chain) and how the pass uses it.
#[derive(Clone, Copy, Debug)]
pub struct ProgramUse {
    pub binding: u32,
    pub resource: &'static str,
    pub access: Access,
    pub which: Which,
}

/// Which elements of a resource a use binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    /// Element 0 (a resource of one element).
    First,
    /// Every element, as a binding array (images, `Sampled` or `Load`).
    All,
    /// The element of the command's instance.
    Instance,
    /// The element the table gives the command's instance (ping-pong: `[1, 0]`).
    Pick(&'static [u32]),
    /// Element `k`.
    Fixed(u32),
    /// Element 0's one mip level: the command's instance plus this offset,
    /// clamped to its levels (a pass per level, each reading the one before:
    /// `Mip(-1)` read, `Mip(0)` written).
    Mip(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// A texture sampled with a filter (float formats).
    Sampled,
    /// A texture read with `textureLoad` (any format).
    Load,
    /// A storage texture or buffer, read only.
    Read,
    Write,
    ReadWrite,
    /// A storage texture or buffer read and written with atomics.
    Atomic,
    Uniform,
    Sampler,
    /// The scene's acceleration structure, traced with ray queries (`resource = "tlas"`; src/ray_tracing.rs).
    Acceleration,
}

impl Access {
    /// Whether it reads a resource of the frame's graph: the scene's TLAS is no such resource (the renderer
    /// builds it from the frame's records before the passes run; src/ray_tracing.rs).
    pub fn reads(self) -> bool {
        !matches!(self, Access::Write | Access::Sampler | Access::Acceleration)
    }
    pub fn writes(self) -> bool {
        matches!(self, Access::Write | Access::ReadWrite | Access::Atomic)
    }
}

/// One command of a frame's plan.
#[derive(Clone, Debug)]
pub enum ProgramCommand {
    /// Writes `words` into a buffer at `offset` bytes, in order with the other commands.
    Upload {
        buffer: Element,
        offset: u64,
        words: Vec<u32>,
    },
    /// Zeroes an image or a buffer (an element, or all of them).
    Clear(Element),
    Copy {
        from: Element,
        to: Element,
        bytes: u64,
    },
    Dispatch {
        pass: &'static str,
        instance: u32,
        groups: [u32; 3],
        params: [u32; 16],
    },
    /// A dispatch whose group counts are three u32 at `offset` of an indirect buffer.
    DispatchIndirect {
        pass: &'static str,
        instance: u32,
        buffer: Element,
        offset: u64,
        params: [u32; 16],
    },
    /// A raster pass over a framebuffer of `size` pixels (viewport = scissor).
    Draw {
        pass: &'static str,
        instance: u32,
        size: [u32; 2],
        params: [u32; 16],
    },
}

impl ProgramCommand {
    /// The pass it runs, if it is a dispatch or a draw.
    pub fn pass(&self) -> Option<&'static str> {
        match self {
            ProgramCommand::Dispatch { pass, .. }
            | ProgramCommand::DispatchIndirect { pass, .. }
            | ProgramCommand::Draw { pass, .. } => Some(pass),
            _ => None,
        }
    }
}

/// The scene shading's data for a state whose values change it (settings
/// baked into the data, `Investigation::scene_data`): a key of those values
/// and the function that builds the data. The renderer keeps the last few by
/// key and builds a new one on a worker thread, drawing the last one ready
/// until it is done.
pub struct SceneData {
    pub key: u64,
    pub build: Box<dyn FnOnce() -> Vec<[f32; 4]> + Send>,
}

/// One of the views the scene area is split into (`Investigation::panes`):
/// the state it is built from, and the title shown in its top-left corner.
#[derive(Clone)]
pub struct Pane {
    pub state: ShotState,
    pub title: Option<String>,
}

/// Depth-only views of the scene's meshes, drawn before anything else of the
/// frame: a square depth texture array of `size` texels and one layer per
/// entry of `vertices`, each layer drawn by that vertex entry point (it
/// places the meshes in its view; a vertex outside the clip volume drops its
/// triangle). Reversed depth, as the scene pass: cleared to 0, nearer is
/// greater. The passes and the scene pass read it by `name`
/// (`texture_depth_2d_array`, binding `2 N + 2 + B` with `B` buffers).
pub struct DepthViews {
    pub name: &'static str,
    pub size: u32,
    pub vertices: &'static [&'static str],
    /// The rasteriser's depth bias for every layer, as
    /// `wgpu::DepthBiasState`'s constant and slope scale (0, 0: none). The
    /// depth is reversed, so a bias that moves the casters away from their
    /// view's origin is negative.
    pub bias: (i32, f32),
}

/// A storage buffer of an investigation's pass chain: `words_per_tile` u32
/// for each `tile` x `tile` block of the viewport.
pub struct ShadingBuffer {
    pub name: &'static str,
    pub words_per_tile: u32,
    pub tile: u32,
}

/// A colour target of an investigation's pass chain.
pub struct ShadingTarget {
    pub name: &'static str,
    pub format: wgpu::TextureFormat,
    /// The viewport's size divided by this (1 full resolution, 2 half).
    pub divisor: u32,
}

/// A compute pass of an investigation's pass chain. Group 2 binds each
/// target at a fixed place, so one module holds every pass: target `i` of
/// `SceneShading::targets` at binding `2 i` when read (`texture_2d<f32>`, or
/// `texture_multisampled_2d<f32>` for the scene pass's outputs) and
/// `2 i + 1` when written (a write-only storage texture). With `N` targets,
/// `"depth"` (the scene pass's multisampled depth,
/// `texture_depth_multisampled_2d`) is binding `2 N`, and `"hdr"` (the lab's
/// HDR target, `rgba16float`) binding `2 N + 1`; buffer `k` of
/// `SceneShading::buffers` is binding `2 N + 2 + k`
/// (`var<storage, read_write> array<u32>`, whether read or written). The
/// dispatch covers the first written target, or the viewport when that is a
/// buffer, in workgroups of `workgroup`.
///
/// `reads` lists what the entry point reads, `writes` what it writes: a
/// buffer it reads and writes is in both (it is bound `read_write` either
/// way), and a volume it samples (group 3) is in `reads`. These are the
/// frame graph's edges (src/graph/); the lab checks them against the WGSL
/// when it loads the shading and fails on a difference.
pub struct ShadingPass {
    pub entry: &'static str,
    pub reads: &'static [&'static str],
    pub writes: &'static [&'static str],
    pub workgroup: [u32; 2],
}

pub struct SelftestOptions {
    pub report: PathBuf,
    /// Also check labels and text for every scene beat of this timeline.
    pub timeline: Option<PathBuf>,
    pub ffmpeg: PathBuf,
    pub work_dir: PathBuf,
}

/// A view an investigation stacks over the main one (`Investigation::views`,
/// src/view.rs): a picture of its own, with its own state (its scene, its
/// camera, its values), drawn from its own slot and laid over the main view
/// at `alpha`. The pointer and, focused, the dock act on it.
#[derive(Clone, Debug)]
pub struct OverView {
    /// Its name: its values are `view.<name>.<key>` (`ShotState::view_state`), and `lab::view::name()` while it is built.
    pub name: &'static str,
    /// What it is built from: the main view's state (its values and camera), whose `view.<name>.…` values are
    /// this view's own (`ShotState::view_state`).
    pub state: ShotState,
    /// The camera it is drawn from: its own (`ShotState::own_camera`), which the pointer moves.
    pub camera: crate::camera::OrbitCamera,
    pub alpha: f32,
}

pub trait Investigation: Sync {
    fn engine(&self) -> &'static Engine;
    /// Short key, used in `engine/feature` paths.
    fn id(&self) -> &'static str;
    fn title(&self) -> &'static str;
    /// A paragraph for the homepage.
    fn summary(&self) -> &'static str;
    fn wording(&self) -> &'static Wording;
    /// The footer line, in the current locale.
    fn footer(&self) -> &'static str;
    /// Its narrated timelines: its package's demos.
    fn timelines(&self) -> &'static [TimelineInfo] {
        self.package().map(resources::timelines).unwrap_or(&[])
    }
    fn sources(&self) -> &'static SourceIndex;
    fn listings(&self) -> &'static [Listing];
    /// The package of its content and code (packages/<name>, src/packages.rs), if it
    /// has one.
    fn package(&self) -> Option<&'static str> {
        None
    }
    /// The choices its values use (`#[derive(Choice)]` enums, src/values.rs):
    /// a shader's tag names one (`choice=ViewMode`), and the lab writes the
    /// WGSL constants of those with a prefix into the package's `build/`
    /// when it loads the investigations (`shader_data::write_choices`).
    fn choices(&self) -> &'static [&'static crate::values::Choices] {
        &[]
    }

    fn listing(&self, name: &str) -> Option<&'static Listing> {
        self.listings().iter().find(|l| l.name == name)
    }

    /// The listing the viewer's Code tab opens for a demonstration.
    fn listing_for_demo(&self, demo: DemoId) -> Option<&'static Listing> {
        self.listings()
            .iter()
            .find(|l| l.demo.as_deref() == Some(demo.key()))
    }

    /// Its demonstrations as data (src/demo.rs), when it describes them
    /// in files; the methods below then read them. An investigation without
    /// them implements `demos`, `demo_title`, `demo_tab`, `param_default`,
    /// and `shot_state` itself.
    fn demo_specs(&self) -> Option<&'static Demos> {
        None
    }
    /// The text of a key its data names, in the current locale (its own table).
    fn text(&self, key: &'static str) -> &'static str {
        crate::i18n::tr_key(key)
    }

    /// Demonstrations in the order of the viewer's keys (1–9, 0, −, =).
    fn demos(&self) -> &'static [DemoId] {
        specs(self).ids()
    }
    fn demo_title(&self, demo: DemoId) -> &'static str {
        self.text(specs(self).spec(demo).title)
    }
    /// The demonstration's short name on the viewer's tab row.
    fn demo_tab(&self, demo: DemoId) -> &'static str {
        match specs(self).spec(demo).tab {
            Tab::Text(key) => self.text(key),
            Tab::Plain(name) => name,
        }
    }
    /// The scene a shot shows: its demonstration's, unless the shot's state
    /// picks one of several prepared variants of it (each prepared once).
    fn scene(&self, state: &ShotState) -> SceneId {
        self.demo_scene(state.demo)
    }
    /// The lab scene a demonstration shows (src/scene.rs); the renderer
    /// switches to it for the demonstration's frames.
    fn demo_scene(&self, demo: DemoId) -> SceneId {
        self.demo_specs()
            .map_or(SceneId::STUDY_ROOM, |d| d.spec(demo).scene)
    }
    /// The viewer's status-line hint, if the demonstration has its own.
    fn demo_hint(&self, demo: DemoId) -> Option<&'static str> {
        self.demo_specs()
            .and_then(|d| d.spec(demo).hint)
            .map(|key| self.text(key))
    }
    /// Whether "Reset parameters" also resets the camera (its dock shows the camera).
    fn resets_camera(&self, demo: DemoId) -> bool {
        self.demo_specs()
            .is_some_and(|d| d.spec(demo).resets_camera)
    }

    /// The default of every shot parameter; None for an unknown name. The lab
    /// reads `cameraDrift` (the camera sway) when an investigation has it.
    fn param_default(&self, name: &str) -> Option<f32> {
        specs(self).param_default(name)
    }
    /// The demonstration's starting state.
    fn shot_state(&self, demo: DemoId) -> ShotState {
        specs(self).shot_state(demo)
    }
    /// The starting state, completed from the prepared data.
    fn default_state(&self, demo: DemoId, _statics: &Statics) -> ShotState {
        self.shot_state(demo)
    }
    /// The scene behind the homepage `t` seconds after it opened, drawn
    /// full-screen under frosted glass; None for a plain background.
    fn backdrop(&self, _t: f32) -> Option<ShotState> {
        None
    }
    /// The viewer's Play animation: parameter values at `t` seconds.
    fn animation(&self, _demo: DemoId, _t: f32) -> Vec<(&'static str, f32)> {
        Vec::new()
    }
    /// Parameters the viewer sets together with `name`.
    fn linked_params(&self, _name: &str, _value: f32) -> Vec<(&'static str, f32)> {
        Vec::new()
    }
    /// Its frames follow the scene's movable objects where the viewer moves them (src/entity.rs): the lab
    /// poses the records `build_frame` returns, and the scene's light with its object. False (the default)
    /// for a frame drawn from data of its own baked from the scene, which would not follow them.
    fn movable_objects(&self) -> bool {
        false
    }
    /// What the viewer's Scene window lists besides the scene's objects in `demo`: the lights and points
    /// its values place (src/entity.rs), which the gizmo then moves.
    fn entities(&self, _demo: DemoId) -> Vec<crate::entity::EntitySpec> {
        Vec::new()
    }

    /// The scene shading's data for `state` when its values differ from the
    /// ones `SceneShading::data` was built with (an engine setting baked into
    /// the data, such as a bake's resolution); None (the default) for those,
    /// which draw `SceneShading::data`. See `SceneData`.
    fn scene_data(&self, _state: &ShotState) -> Option<SceneData> {
        None
    }
    /// Everything one shot shows. A pure function of its arguments, so the
    /// recorder stays deterministic.
    fn build_frame(
        &self,
        state: &ShotState,
        statics: &Statics,
        viewport: [f32; 2],
        pixel_scale: f32,
        time: f32,
    ) -> FrameContent;
    /// The frame built for the main camera, drawn from another (the free
    /// camera, src/free_view.rs): `content.uniforms.camera` is already that
    /// camera's. Rewrite here only what the scene's own passes derive from the
    /// camera that renders (an inverse view-projection the shading rebuilds
    /// positions with); what the investigation demonstrates, and the engine
    /// state that follows the main camera, stay as built. The default: nothing.
    fn view_from(&self, _content: &mut FrameContent) {}
    /// The views the scene area is split into, left to right and all the same
    /// size, each built from its own state (its camera, its values). The last
    /// is the main one: the struct panel, the insets, and clicks are its.
    /// Empty (the default): one view of `state`. A pure function of the
    /// state, as `build_frame` is.
    fn panes(&self, _state: &ShotState) -> Vec<Pane> {
        Vec::new()
    }
    /// The points and arrow sets it publishes for a story's arrows and labels
    /// (packages/render_lab/src/annotations.rs), in `FrameContent::points` and
    /// `arrow_sets`.
    fn annotations(&self) -> &'static [crate::utility::annotations::AnnotationSpec] {
        &[]
    }
    /// Functions of its code a storyboard may call, so its narration says the numbers the code computes
    /// (`${spoken {leaves}}`): `Pure` natives registered with ids `<package>/<name>` (lab/src/script), which a
    /// storyboard imports as the package `<package>_natives` (`render-lab --investigation <id> lamlet …`).
    fn script_natives(
        &self,
        _natives: &mut crate::script::lamlet::embed::NativeRegistry,
    ) -> Result<(), crate::script::lamlet::embed::Error> {
        Ok(())
    }
    /// The images it publishes for a story to show (`{inset "key"}`,
    /// packages/render_lab/src/inset.rs), in `FrameContent::images` of the frames
    /// that show them.
    fn images(&self) -> &'static [crate::utility::inset::ImageSpec] {
        &[]
    }
    /// The actions a story can run and the click handlers it can give an
    /// image (src/actions.rs): `&Self::ACTIONS` of a `#[lab::actions]` block.
    fn actions(&self) -> &'static crate::actions::Actions {
        &crate::actions::NONE
    }
    /// What a node of the frame graph does (its entry point; src/graph/), in
    /// the interface's language, for the graph's panels and its tab: a key
    /// of the package's text table, one per pass (`pass.<entry>`).
    fn pass_about(&self, _node: &str) -> Option<&'static str> {
        None
    }
    /// The scene pass's shading, when the investigation brings its own: by
    /// default its package's `shading` resource (src/shading_file.rs), read
    /// once, its functions looked up in `shading_hooks`.
    fn scene_shading(&self) -> Option<&'static SceneShading> {
        crate::shading_file::of(self.package()?, self.shading_hooks())
    }
    /// The functions its package's shading file names, by name.
    fn shading_hooks(&self) -> &'static [(&'static str, ShadingHook)] {
        &[]
    }
    /// The derived members of the scene shading's `@shader_data` uniform
    /// (src/shader_data.rs), from the state; the lab has packed its values.
    fn shader_data(&self, _block: &mut crate::shader_data::Block, _state: &ShotState) {}
    /// The dock's Sliders tab for the current demonstration.
    fn dock(&self, dock: &mut Dock, ui: &mut egui::Ui);
    /// A click in the viewport at `pixel` on the frame of `shown` (the state on
    /// screen); changes `state` and returns true if it selected something.
    fn click(
        &self,
        shown: &ShotState,
        state: &mut ShotState,
        content: &FrameContent,
        statics: &Statics,
        projector: &Projector,
        pixel: Vec2,
    ) -> bool;

    /// The views stacked over the main one this frame (src/view.rs), back to
    /// front: each is built with `build_frame` (`lab::view::name()` names it)
    /// in its own scene (`scene` of its state) and its own slot. None by default.
    fn views(&self, state: &ShotState) -> Vec<OverView> {
        let _ = state;
        Vec::new()
    }

    /// The scripted test of the viewer's controls (`viewer-autotest`).
    fn viewer_test(&self, report: PathBuf) -> Option<Box<dyn ViewerTest>> {
        let _ = report;
        None
    }
    /// The verification suite (`selftest`); Ok(false) when a check failed.
    fn selftest(&self, options: &SelftestOptions) -> anyhow::Result<bool>;
}

/// An investigation's demonstration files, for the methods that read them.
fn specs<I: Investigation + ?Sized>(investigation: &I) -> &'static Demos {
    investigation.demo_specs().unwrap_or_else(|| panic!("{} has no demonstration files: it implements demos, demo_title, demo_tab, param_default, and shot_state", investigation.title()))
}

impl dyn Investigation {
    /// `engine/feature`, as the command line names it.
    pub fn path(&self) -> String {
        format!("{}/{}", self.engine().id, self.id())
    }

    /// The scenes its demonstrations show, in the order they first appear:
    /// the viewer's scene tabs.
    pub fn scenes(&self) -> Vec<SceneId> {
        let mut scenes = Vec::new();
        for demo in self.demos() {
            let scene = self.demo_scene(*demo);
            if !scenes.contains(&scene) {
                scenes.push(scene);
            }
        }
        scenes
    }

    /// Its demonstrations of `scene`, in order.
    pub fn scene_demos(&self, scene: SceneId) -> Vec<DemoId> {
        self.demos()
            .iter()
            .copied()
            .filter(|d| self.demo_scene(*d) == scene)
            .collect()
    }

    /// The scene of its first demonstration, which the renderer prepares first.
    pub fn first_scene(&self) -> SceneId {
        self.demo_scene(self.demos()[0])
    }
}

static CURRENT: RwLock<Option<&'static dyn Investigation>> = RwLock::new(None);

thread_local! {
    /// The investigation `scoped` makes current on this thread.
    static SCOPED: Cell<Option<&'static dyn Investigation>> = const { Cell::new(None) };
}

pub fn set(investigation: &'static dyn Investigation) {
    *CURRENT.write().unwrap() = Some(investigation);
}

/// The investigation chosen on the homepage or the command line, or the one
/// `scoped` made current on this thread.
pub fn current() -> &'static dyn Investigation {
    SCOPED
        .get()
        .unwrap_or_else(|| CURRENT.read().unwrap().expect("no investigation selected"))
}

/// Runs `f` with `investigation` current on this thread alone: a scene of
/// one investigation (the homepage's backdrop) prepared or drawn while
/// another is chosen.
pub fn scoped<R>(investigation: &'static dyn Investigation, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<&'static dyn Investigation>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED.set(self.0);
        }
    }
    let _restore = Restore(SCOPED.replace(Some(investigation)));
    f()
}
