//! The renderer shared by the viewer and the recorder. Each frame is drawn
//! into one RGBA8 offscreen target: the 3D scene (MSAA, HDR), the overlay
//! layer, the composite into the viewport rectangle, then egui on top. The
//! viewer blits that target to its window; the recorder reads it back.

use crate::frame::{self, FrameContent, Statics};
use crate::investigation::{self, SceneShading};
use crate::gpu::{self, Gpu, Pipelines, DEPTH_FORMAT, FRAME_FORMAT, HDR_FORMAT, MSAA, OVERLAY_FORMAT};
use crate::graph::Step;
use crate::layout::{FrameUniforms, ObjectData};
use crate::scene::SceneId;
use crate::shot::{SceneType, ShotState};
use crate::ui::{self, Chrome, Layout, UiInput, UiReport};
use crate::utility::inset::Textures;
use bytemuck::Pod;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

pub use crate::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows};

/// A vertex/storage buffer that grows to fit its contents.
pub struct GrowBuffer {
    pub buffer: wgpu::Buffer,
    capacity: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
    pub count: u32,
}

impl GrowBuffer {
    fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages, capacity: u64) -> Self {
        let capacity = capacity.max(256);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: capacity, usage: usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        GrowBuffer { buffer, capacity, usage, label, count: 0 }
    }

    /// Returns true when the buffer was reallocated.
    fn write<T: Pod>(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[T]) -> bool {
        let bytes: &[u8] = bytemuck::cast_slice(data);
        let mut grown = false;
        if bytes.len() as u64 > self.capacity {
            self.capacity = (bytes.len() as u64).next_power_of_two();
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some(self.label), size: self.capacity, usage: self.usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            grown = true;
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytes);
        }
        self.count = data.len() as u32;
        grown
    }
}

struct ViewportTargets {
    size: [u32; 2],
    hdr_texture: wgpu::Texture,
    hdr_msaa: wgpu::TextureView,
    hdr: wgpu::TextureView,
    overlay_msaa: wgpu::TextureView,
    overlay: wgpu::TextureView,
    depth: wgpu::TextureView,
    composite: wgpu::BindGroup,
}

fn target(device: &wgpu::Device, label: &str, size: [u32; 2], format: wgpu::TextureFormat, samples: u32, usage: wgpu::TextureUsages) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: samples,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

impl ViewportTargets {
    fn new(device: &wgpu::Device, pipelines: &Pipelines, size: [u32; 2]) -> Self {
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let sampled = attach | wgpu::TextureUsages::TEXTURE_BINDING;
        let view = |t: wgpu::Texture| t.create_view(&Default::default());
        // An investigation's pass chain may write the HDR target from a compute pass.
        // An experiment reads it back (`Renderer::read_view`).
        let hdr_texture = target(device, "scene hdr", size, HDR_FORMAT, 1, sampled | wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC);
        let hdr = view(hdr_texture.clone());
        let overlay = view(target(device, "overlay", size, OVERLAY_FORMAT, 1, sampled));
        let composite = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("composite"),
            layout: &pipelines.texture_pair_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&hdr) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&overlay) }],
        });
        ViewportTargets {
            size,
            hdr_texture,
            hdr_msaa: view(target(device, "scene hdr msaa", size, HDR_FORMAT, MSAA, attach)),
            overlay_msaa: view(target(device, "overlay msaa", size, OVERLAY_FORMAT, MSAA, attach)),
            // A pass chain reads the depth.
            depth: view(target(device, "depth", size, DEPTH_FORMAT, MSAA, sampled)),
            hdr,
            overlay,
            composite,
        }
    }
}

pub struct RecordBuffers {
    pub uniforms: wgpu::Buffer,
    pub objects: GrowBuffer,
    /// The teapot's BVH and triangles (static, local space).
    pub mesh_nodes: wgpu::Buffer,
    pub mesh_triangles: wgpu::Buffer,
    /// The lab's terrain heightfield (static, local space).
    pub terrain: wgpu::Buffer,
}

/// What the renderer builds from one scene's `Statics`; switching scenes
/// rebuilds it (`Renderer::switch_scene`).
struct SceneResources {
    mesh_vertices: wgpu::Buffer,
    mesh_indices: wgpu::Buffer,
    object_ids: wgpu::Buffer,
}

pub struct Renderer {
    /// The last frame's `still_key` and `scene_key`, and how many frames in a row had each.
    still: (Option<u64>, u32),
    still_scene: (Option<u64>, u32),
    pub gpu: Gpu,
    pub pipelines: Pipelines,
    /// The prepared data of the scene the renderer shows.
    pub statics: Statics,
    /// The other scenes prepared so far, kept for switching back.
    other_scenes: HashMap<SceneId, Statics>,
    pub records: RecordBuffers,
    mesh_vertices: wgpu::Buffer,
    mesh_indices: wgpu::Buffer,
    object_ids: wgpu::Buffer,
    /// The lab's environment (environment.rs), gEnvironment.
    _environment: wgpu::Texture,
    environment_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    /// `FrameContent::shading`, group 1 of an investigation's own scene shading.
    shading_uniforms: wgpu::Buffer,
    /// The investigations' own scene shadings built so far.
    shadings: Vec<Shading>,
    /// The current shading's pass-chain targets, for the current viewport.
    chain: Option<ChainTargets>,
    frame_bind_group: wgpu::BindGroup,
    segments: GrowBuffer,
    segments_xray: GrowBuffer,
    boxes: GrowBuffer,
    boxes_xray: GrowBuffer,
    points: GrowBuffer,
    ghost_ids: GrowBuffer,
    pub width: u32,
    pub height: u32,
    /// Size of the chrome and labels relative to the recorder's (the viewer may shrink them).
    pub ui_scale: f32,
    frame: wgpu::Texture,
    pub frame_view: wgpu::TextureView,
    viewport: Option<ViewportTargets>,
    pub egui_ctx: egui::Context,
    egui_renderer: egui_wgpu::Renderer,
    /// The GPU textures an inset's picture can show, by key (packages/rlstd/src/inset.rs).
    textures: Textures,
    /// The story's image instances (src/views.rs) by name, and each kind's pipeline.
    views: HashMap<String, ViewInstance>,
    view_pipelines: HashMap<(String, String), wgpu::ComputePipeline>,
    /// The group-2 layouts of the image kinds that read the pass chain.
    view_chain_layouts: HashMap<(String, String), wgpu::BindGroupLayout>,
    view_layout: wgpu::BindGroupLayout,
    /// Group 1 of a kind's entry point when the investigation has no scene shading.
    view_shading: Option<wgpu::BindGroup>,
    /// The frame's graph (src/graph.rs) and what it was compiled for: the
    /// shading (by address) and the resources image kinds read.
    graph: Option<((usize, Vec<String>), std::sync::Arc<crate::graph::FrameGraph>)>,
    /// The graph images shown (`ImageSource::Graph`) by name, and the
    /// inspector's pipeline and group layout for each kind of resource.
    inspections: HashMap<String, Inspection>,
    inspect_pipelines: HashMap<&'static str, (wgpu::ComputePipeline, wgpu::BindGroupLayout)>,
    last_layout: Layout,
    /// The last frame's views beside the main one (`Investigation::panes`), in canvas points.
    last_panes: Vec<egui::Rect>,
    /// The last frame's panel had nothing to show (no record, readout, or
    /// listing), so this frame first lays out without it (`render_frame_with`).
    panel_empty: bool,
    /// The reference's view, while an experiment compares (src/experiment.rs).
    reference_slot: ViewSlot,
    /// The free camera's view (src/free_view.rs), while it is on.
    free_slot: ViewSlot,
    /// Where the last frame drew the main camera's own view while the free
    /// camera was on: a corner of the viewport, in canvas points.
    pub last_main_view: Option<egui::Rect>,
    compare: Option<CompareRequest>,
    /// The difference view's gain (group 2 of `composite_diff`).
    compare_params: wgpu::Buffer,
    compare_group: wgpu::BindGroup,
    /// The last frame drew the reference (its view can be read back).
    reference_drawn: bool,
    /// The sizes the viewer's user dragged (src/config.rs); the default for a recording.
    pub user_layout: crate::config::ViewerLayout,
    /// The pointer's cursor the last frame's interface asked for (egui's).
    pub cursor: egui::CursorIcon,
    /// When each recent frame started rendering, for the performance panel's frame times.
    frame_starts: std::collections::VecDeque<std::time::Instant>,
    /// GPU timestamps around the frame's passes (src/timing.rs), while an
    /// experiment asks for them (`set_timing`) and the device has them.
    pub timing: Option<crate::timing::Timing>,
}

/// A story's (or the graph panel's) image of a resource of the frame's
/// graph (src/graph.rs, `ImageSource::Graph`): its texture, written right
/// after the node it names by the inspector (packages/rlstd/shaders/inspect.wgsl).
struct Inspection {
    /// `<node>.<resource>`, and the node by its index in the graph.
    key: String,
    node: usize,
    resource: &'static str,
    entry: &'static str,
    binding: u32,
    _texture: wgpu::Texture,
    storage: wgpu::TextureView,
    size: [u32; 2],
    params: wgpu::Buffer,
    id: egui::TextureId,
}

/// The most texels along an inspector texture's longer side.
const INSPECT_SIDE: u32 = 640;

/// The DAG panel's images (`PanelMode::Dag`): one per node, named this
/// prefix and the node, smaller than the inspector's and not keeping their
/// writers alive (the view shows culled nodes as culled).
const DAG_IMAGE: &str = "@dag/";
const DAG_SIDE: u32 = 256;
/// The resources panel's images (`PanelMode::Resources`): one per resource,
/// its state at the end of the frame (`FrameGraph::last_user`).
const RESOURCE_IMAGE: &str = "@resource/";

/// The frame graph scene's pictures (a 2D beat's `graph`): one per lit box,
/// its last member's result (src/diagram.rs `BeatDiagram::lit_groups`).
const BOX_IMAGE: &str = "@box/";

/// An image of a graph panel's cards, not one a story or the inspector shows.
fn card_image(name: &str) -> bool {
    name.starts_with(DAG_IMAGE) || name.starts_with(RESOURCE_IMAGE) || name.starts_with(BOX_IMAGE)
}

/// A story's image of an image kind (src/views.rs): its texture, written
/// each frame by the kind's entry point, and shown through egui.
struct ViewInstance {
    kind: &'static crate::views::ViewKind,
    _texture: wgpu::Texture,
    /// Its texture's size in texels (`ViewKind::texels`).
    size: [u32; 2],
    params: wgpu::Buffer,
    group: wgpu::BindGroup,
    id: egui::TextureId,
}

/// The pipelines and data of one investigation's `SceneShading`.
struct Shading {
    source: &'static SceneShading,
    pipelines: gpu::ShadingPipelines,
    /// Group 1 with the data of the state being drawn (`select_data`).
    group: wgpu::BindGroup,
    /// The data built so far, most recently used first: `SceneShading::data`
    /// (key None), and `Investigation::scene_data`'s by key, each with its group.
    data: Vec<(Option<u64>, wgpu::Buffer, wgpu::BindGroup)>,
    /// A `scene_data` build on a worker thread: its key, its result, and when it started.
    pending: Option<(u64, std::sync::mpsc::Receiver<Vec<[f32; 4]>>, std::time::Instant)>,
    /// `FrameContent::shading_frame`.
    frame_data: wgpu::Buffer,
    /// The volumes (`SceneShading::volumes`), one set per update key kept.
    volume_sets: Vec<VolumeSet>,
    /// The set the state being drawn reads (`select_data`).
    volume_set: usize,
    /// Update dispatches `select_data` planned and the next `encode_scene`
    /// encodes: into which set, and what.
    update_runs: Vec<(usize, Vec<crate::investigation::UpdateDispatch>)>,
    update_buffers: Vec<wgpu::Buffer>,
    /// The dispatches' parameters, `UPDATE_PARAMS_STRIDE` apart.
    update_params: Option<wgpu::Buffer>,
    sampler: wgpu::Sampler,
    /// The divisor of the targets `SceneShading::resolution` names, for the state being drawn.
    scale: u32,
    /// A count that orders the volume sets by when they were last used.
    clock: u64,
}

/// The volumes of one update key: their textures, group 3 that samples them,
/// and each update pass's group 2 for each mip it may write.
struct VolumeSet {
    key: Option<u64>,
    used: u64,
    textures: Vec<wgpu::Texture>,
    group: wgpu::BindGroup,
    updates: Vec<Vec<wgpu::BindGroup>>,
}

/// How many update keys' volumes a shading keeps.
const VOLUME_SETS_KEPT: usize = 4;

/// The most dispatches one update plan may have.
const UPDATE_DISPATCHES_MAX: u64 = 256;

impl VolumeSet {
    fn new(device: &wgpu::Device, source: &SceneShading, pipelines: &gpu::ShadingPipelines, buffers: &[wgpu::Buffer], params: Option<&wgpu::Buffer>, sampler: &wgpu::Sampler) -> VolumeSet {
        let textures: Vec<wgpu::Texture> = source
            .volumes
            .iter()
            .map(|v| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(v.name),
                    size: wgpu::Extent3d { width: v.size[0], height: v.size[1], depth_or_array_layers: v.size[2] },
                    mip_level_count: v.mips,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D3,
                    format: v.format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
            })
            .collect();
        let views: Vec<wgpu::TextureView> = textures.iter().map(|t| t.create_view(&Default::default())).collect();
        let mut entries = vec![wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Sampler(sampler) }];
        entries.extend(views.iter().enumerate().map(|(v, view)| wgpu::BindGroupEntry { binding: 1 + v as u32, resource: wgpu::BindingResource::TextureView(view) }));
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("volumes"), layout: pipelines.volumes.as_ref().expect("a volume layout"), entries: &entries });
        let updates = source.update.map_or_else(Vec::new, |update| {
            update
                .passes
                .iter()
                .zip(&pipelines.updates)
                .map(|(pass, (_, layout))| {
                    let mips = source.volumes.iter().filter(|v| pass.writes.contains(&v.name)).map(|v| v.mips).max().unwrap_or(1);
                    (0..mips)
                        .map(|mip| {
                            let mip_views: Vec<(u32, wgpu::TextureView)> = source
                                .volumes
                                .iter()
                                .zip(&textures)
                                .enumerate()
                                .filter(|(_, (v, _))| pass.writes.contains(&v.name))
                                .map(|(k, (v, t))| (1 + update.buffers.len() as u32 + k as u32, t.create_view(&wgpu::TextureViewDescriptor { base_mip_level: mip.min(v.mips - 1), mip_level_count: Some(1), ..Default::default() })))
                                .collect();
                            let mut entries = vec![wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: params.expect("update parameters"), offset: 0, size: wgpu::BufferSize::new(64) }),
                            }];
                            entries.extend(buffers.iter().enumerate().map(|(k, b)| wgpu::BindGroupEntry { binding: 1 + k as u32, resource: b.as_entire_binding() }));
                            entries.extend(mip_views.iter().map(|(binding, view)| wgpu::BindGroupEntry { binding: *binding, resource: wgpu::BindingResource::TextureView(view) }));
                            device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(pass.entry), layout, entries: &entries })
                        })
                        .collect()
                })
                .collect()
        });
        VolumeSet { key: None, used: 0, textures, group, updates }
    }
}

/// How many `scene_data` builds a shading keeps.
const SCENE_DATA_KEPT: usize = 4;

/// What a frame's reference view (src/experiment.rs) draws from and into:
/// its own viewport targets, pass chain, and still counts, so its history
/// builds up as the live view's does. Swapped with the renderer's while it draws.
#[derive(Default)]
struct ViewSlot {
    viewport: Option<ViewportTargets>,
    chain: Option<ChainTargets>,
    still: (Option<u64>, u32),
    still_scene: (Option<u64>, u32),
}

/// The experiment's comparison the next frames draw (`Renderer::set_compare`).
#[derive(Clone, Debug)]
pub struct CompareRequest {
    /// The reference's state (the live camera's).
    pub reference: ShotState,
    pub mode: crate::experiment::Compare,
    pub split: f32,
    pub gain: f32,
}

/// Which view an experiment reads back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewKind {
    /// The live view (B).
    Live,
    /// The reference (A).
    Reference,
}

/// The targets of an investigation's pass chain at one viewport size, with
/// each pass's group-2 bind group.
struct ChainTargets {
    source: &'static SceneShading,
    size: [u32; 2],
    /// `Shading::scale` when it was made.
    scale: u32,
    /// Each target's divisor of the viewport's size.
    divisors: Vec<(&'static str, u32)>,
    /// The targets by name: the scene pass's outputs multisampled, the rest single-sample.
    views: Vec<(&'static str, wgpu::TextureView)>,
    buffers: Vec<(&'static str, wgpu::Buffer)>,
    /// The depth views' texture array as one view (bound by name), and each of its layers (drawn into).
    depth_views: Option<(wgpu::TextureView, Vec<wgpu::TextureView>)>,
    /// What the depth views were last drawn from (`depth_views_key`): they
    /// are drawn again only when that changes.
    depth_views_drawn: std::cell::Cell<Option<u64>>,
    /// Group 2 of each early pass, then of each later pass.
    groups: Vec<wgpu::BindGroup>,
    /// Group 2 of the scene pass.
    scene_group: wgpu::BindGroup,
}

impl ChainTargets {
    fn new(device: &wgpu::Device, shading: &Shading, viewport: &ViewportTargets) -> ChainTargets {
        let source = shading.source;
        let size = viewport.size;
        let written: Vec<&str> = source.passes.iter().flat_map(|p| p.writes.iter().copied()).collect();
        let divisors: Vec<(&'static str, u32)> = source.targets.iter().map(|t| (t.name, if source.resolution.is_some_and(|r| r.targets.contains(&t.name)) { shading.scale } else { t.divisor })).collect();
        let views: Vec<(&'static str, wgpu::TextureView)> = source
            .targets
            .iter()
            .zip(&divisors)
            .map(|(t, &(_, divisor))| {
                let output = source.outputs.contains(&t.name);
                let mut usage = wgpu::TextureUsages::TEXTURE_BINDING;
                if output {
                    usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
                }
                if written.contains(&t.name) {
                    usage |= wgpu::TextureUsages::STORAGE_BINDING;
                }
                let s = if divisor == 1 { size } else { [(size[0] / divisor).max(1), (size[1] / divisor).max(1)] };
                (t.name, target(device, t.name, s, t.format, if output { MSAA } else { 1 }, usage).create_view(&Default::default()))
            })
            .collect();
        let tiles = |tile: u32| size[0].div_ceil(tile) as u64 * size[1].div_ceil(tile) as u64;
        let buffers: Vec<(&'static str, wgpu::Buffer)> = source
            .buffers
            .iter()
            .map(|b| (b.name, device.create_buffer(&wgpu::BufferDescriptor { label: Some(b.name), size: tiles(b.tile) * b.words_per_tile as u64 * 4, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false })))
            .collect();
        let depth_views = source.depth_views.map(|d| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(d.name),
                size: wgpu::Extent3d { width: d.size, height: d.size, depth_or_array_layers: d.vertices.len() as u32 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::gpu::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let all = texture.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
            let layers = (0..d.vertices.len() as u32).map(|layer| texture.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2), base_array_layer: layer, array_layer_count: Some(1), ..Default::default() })).collect();
            (all, layers)
        });
        let resource = |name: &str| -> wgpu::BindingResource {
            if let Some((_, buffer)) = buffers.iter().find(|(n, _)| *n == name) {
                return buffer.as_entire_binding();
            }
            if let (Some(d), Some((all, _))) = (source.depth_views, &depth_views) {
                if d.name == name {
                    return wgpu::BindingResource::TextureView(all);
                }
            }
            wgpu::BindingResource::TextureView(match name {
                "hdr" => &viewport.hdr,
                "depth" => &viewport.depth,
                _ => &views.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("{}: no target {name}", source.fragment)).1,
            })
        };
        let group = |label: &str, layout: &wgpu::BindGroupLayout, reads: &[&str], writes: &[&str]| {
            let mut entries: Vec<wgpu::BindGroupEntry> = Vec::new();
            for (name, written) in reads.iter().map(|n| (*n, false)).chain(writes.iter().map(|n| (*n, true))).filter(|(n, _)| !crate::graph::is_volume(source, n)) {
                let binding = gpu::chain_binding(source, name, written);
                if !entries.iter().any(|e| e.binding == binding) {
                    entries.push(wgpu::BindGroupEntry { binding, resource: resource(name) });
                }
            }
            device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(label), layout, entries: &entries })
        };
        let groups = source.early.iter().chain(source.passes).zip(&shading.pipelines.passes).map(|(pass, (_, layout))| group(pass.entry, layout, pass.reads, pass.writes)).collect();
        let scene_group = group("scene reads", &shading.pipelines.scene_group, source.scene_reads, &[]);
        ChainTargets { source, size, scale: shading.scale, divisors, views, buffers, depth_views, depth_views_drawn: std::cell::Cell::new(None), groups, scene_group }
    }

    fn view(&self, name: &str) -> &wgpu::TextureView {
        &self.views.iter().find(|(n, _)| *n == name).expect("a scene output is a chain target").1
    }

    /// A group-2 bind group for `layout` that reads `reads` (targets, `hdr`,
    /// `depth`, buffers, or the depth views), as a chain pass binds them: an
    /// image kind that reads the chain (src/views.rs).
    fn read_group(&self, device: &wgpu::Device, viewport: &ViewportTargets, layout: &wgpu::BindGroupLayout, reads: &[&str]) -> wgpu::BindGroup {
        let mut entries: Vec<wgpu::BindGroupEntry> = Vec::new();
        for name in reads {
            let binding = gpu::chain_binding(self.source, name, false);
            if entries.iter().any(|e| e.binding == binding) {
                continue;
            }
            let resource = if let Some((_, buffer)) = self.buffers.iter().find(|(n, _)| n == name) {
                buffer.as_entire_binding()
            } else if let (Some(d), Some((all, _))) = (self.source.depth_views, &self.depth_views) {
                if d.name == *name { wgpu::BindingResource::TextureView(all) } else { wgpu::BindingResource::TextureView(self.target(viewport, name)) }
            } else {
                wgpu::BindingResource::TextureView(self.target(viewport, name))
            };
            entries.push(wgpu::BindGroupEntry { binding, resource });
        }
        device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("image kind chain reads"), layout, entries: &entries })
    }

    fn target<'a>(&'a self, viewport: &'a ViewportTargets, name: &str) -> &'a wgpu::TextureView {
        match name {
            "hdr" => &viewport.hdr,
            "depth" => &viewport.depth,
            _ => &self.views.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("{}: no target {name}", self.source.fragment)).1,
        }
    }

    /// The size in pixels a pass writing `name` covers: a target's, or the viewport's for a buffer.
    fn size_of(&self, name: &str) -> [u32; 2] {
        match self.divisors.iter().find(|(n, _)| *n == name).map_or(1, |d| d.1) {
            1 => self.size,
            divisor => [(self.size[0] / divisor).max(1), (self.size[1] / divisor).max(1)],
        }
    }

    /// Encode compute passes `steps` (indices into `early` then `passes`),
    /// in order, with their bind groups. Timed (src/timing.rs), each pass
    /// runs in a compute pass of its own that writes its timestamps.
    fn dispatch(&self, encoder: &mut wgpu::CommandEncoder, frame: &wgpu::BindGroup, shading: &Shading, steps: &[usize], timing: Option<&mut crate::timing::Timing>) {
        if steps.is_empty() {
            return;
        }
        let all: Vec<&crate::investigation::ShadingPass> = self.source.early.iter().chain(self.source.passes).collect();
        if let Some(timing) = timing {
            for &i in steps {
                let step = all[i];
                let size = self.size_of(step.writes[0]);
                let span = timing.compute_writes(step.entry);
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(step.entry), timestamp_writes: span.as_ref().map(|t| t.compute()) });
                pass.set_bind_group(0, frame, &[]);
                pass.set_bind_group(1, &shading.group, &[]);
                pass.set_pipeline(&shading.pipelines.passes[i].0);
                pass.set_bind_group(2, &self.groups[i], &[]);
                if let Some(v) = shading.volume_sets.get(shading.volume_set) {
                    pass.set_bind_group(3, &v.group, &[]);
                }
                pass.dispatch_workgroups(size[0].div_ceil(step.workgroup[0]), size[1].div_ceil(step.workgroup[1]), 1);
            }
            return;
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("shading chain"), timestamp_writes: None });
        pass.set_bind_group(0, frame, &[]);
        pass.set_bind_group(1, &shading.group, &[]);
        if let Some(v) = shading.volume_sets.get(shading.volume_set) {
            pass.set_bind_group(3, &v.group, &[]);
        }
        for &i in steps {
            let step = all[i];
            let size = self.size_of(step.writes[0]);
            pass.set_pipeline(&shading.pipelines.passes[i].0);
            pass.set_bind_group(2, &self.groups[i], &[]);
            pass.dispatch_workgroups(size[0].div_ceil(step.workgroup[0]), size[1].div_ceil(step.workgroup[1]), 1);
        }
    }
}

/// What a frame shows, but its clock: its records, uniforms (with the
/// overlay's time and the history count cleared), shading values, and size;
/// with `camera` false, all of it but the camera (and the shading's frame
/// data, which follows the camera).
fn still_key(content: &FrameContent, viewport: [u32; 2], camera: bool) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut uniforms = content.uniforms;
    uniforms.overlay_params[2] = 0.0;
    uniforms.view_params[3] = 0.0;
    if !camera {
        uniforms.camera = bytemuck::Zeroable::zeroed();
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytemuck::bytes_of(&uniforms).hash(&mut h);
    bytemuck::cast_slice::<_, u8>(&content.objects).hash(&mut h);
    bytemuck::cast_slice::<_, u8>(&content.shading).hash(&mut h);
    if camera {
        bytemuck::cast_slice::<_, u8>(&content.shading_frame).hash(&mut h);
    }
    viewport.hash(&mut h);
    h.finish()
}

/// What the depth views see: the records, the light, the fading objects, the
/// shading values, and the shading frame (not the camera itself, but what the
/// investigation derives from it there, such as a light's view matrices).
fn depth_views_key(content: &FrameContent) -> u64 {
    use std::hash::{Hash, Hasher};
    let u = &content.uniforms;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytemuck::cast_slice::<_, u8>(&content.objects).hash(&mut h);
    bytemuck::cast_slice::<_, u8>(&[u.light_center_radiance, u.light_axis_u, u.light_axis_v, [u.view_params[0], 0.0, 0.0, 0.0]]).hash(&mut h);
    u.counts.hash(&mut h);
    bytemuck::cast_slice::<_, u8>(&content.shading).hash(&mut h);
    bytemuck::cast_slice::<_, u8>(&content.shading_frame).hash(&mut h);
    h.finish()
}

/// Everything a frame needs besides the shot state.
pub struct FrameRequest<'a> {
    pub state: &'a ShotState,
    /// Whether this frame may build on the frames before it (the viewer's
    /// live frames): the renderer then counts the frames the whole view has
    /// held still (`a`, at most 255) and the frames the scene has held still
    /// while only the camera may have moved (`b`, at most 32767), and puts
    /// `1 + a + 256 b` in `viewParams.w`, which a scene shading may use to keep
    /// a history (reprojected while only the camera moves). 0 otherwise, so
    /// each frame of a recording renders on its own.
    pub history: bool,
    pub scene_type: SceneType,
    /// The diagram a 2D scene shows (src/diagram.rs).
    pub diagram: Option<crate::diagram::BeatDiagram<'a>>,
    pub beat_sources: &'a [String],
    /// A 2D scene of the legal notices (`TimelineBeat::notices`).
    pub notices: bool,
    pub chapter_number: u32,
    pub fade: f32,
    pub time: f32,
    pub raw_input: egui::RawInput,
    pub chrome: Chrome<'a>,
    /// Draw this frame index as a binary stamp (frame-order verification).
    pub stamp: Option<u64>,
}

pub struct FrameResult {
    pub report: UiReport,
    pub content: Option<FrameContent>,
}

impl Renderer {
    /// A renderer of the scene `statics` prepares (`Statics::prepare`).
    pub fn new(gpu: Gpu, statics: Statics, width: u32, height: u32) -> anyhow::Result<Renderer> {
        let device = &gpu.device;
        let queue = &gpu.queue;
        let pipelines = Pipelines::new(device)?;
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
        let mesh_nodes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("teapot BVH nodes"), contents: bytemuck::cast_slice(&crate::mesh::teapot().nodes), usage: wgpu::BufferUsages::STORAGE });
        let mesh_triangles = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("teapot triangles"), contents: bytemuck::cast_slice(&crate::mesh::teapot().triangles), usage: wgpu::BufferUsages::STORAGE });
        // The lab's environment (environment.rs), gEnvironment.
        let env = crate::environment::Environment::get();
        let environment = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("environment (RGB9E5)"),
                size: wgpu::Extent3d { width: env.sizes[0][0] as u32, height: env.sizes[0][1] as u32, depth_or_array_layers: 1 },
                mip_level_count: env.levels.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgb9e5Ufloat,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &env.levels.concat().iter().flat_map(|t| t.to_le_bytes()).collect::<Vec<u8>>(),
        );
        let environment_view = environment.create_view(&Default::default());

        let terrain = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("terrain heights"), contents: bytemuck::cast_slice(&crate::terrain::terrain().gpu_words()), usage: wgpu::BufferUsages::STORAGE });
        let records = RecordBuffers {
            uniforms: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("frame uniforms"),
                size: std::mem::size_of::<FrameUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            objects: GrowBuffer::new(device, "objects", storage, (statics.scene.objects.len() * std::mem::size_of::<ObjectData>()) as u64),
            mesh_nodes,
            mesh_triangles,
            terrain,
        };
        let scene = Self::scene_resources(device, &statics);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let vertex = wgpu::BufferUsages::VERTEX;
        let egui_ctx = egui::Context::default();
        ui::install_fonts(&egui_ctx);
        let egui_renderer = egui_wgpu::Renderer::new(device, FRAME_FORMAT, egui_wgpu::RendererOptions { msaa_samples: 1, depth_stencil_format: None, dithering: false, predictable_texture_filtering: true });
        let textures = Textures::new();
        let frame = Self::frame_texture(device, width, height);
        let frame_view = frame.create_view(&Default::default());
        let shading_uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene shading uniforms"),
            size: gpu::SHADING_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let compare_params = device.create_buffer(&wgpu::BufferDescriptor { label: Some("compare"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let pipelines_compare_layout = pipelines.compare_layout.clone();
        let mut renderer = Renderer {
            still: (None, 0),
            still_scene: (None, 0),
            frame_bind_group: Self::frame_bind_group(device, &pipelines, &records, &environment_view, &sampler),
            shading_uniforms,
            shadings: Vec::new(),
            chain: None,
            pipelines,
            records,
            mesh_vertices: scene.mesh_vertices,
            mesh_indices: scene.mesh_indices,
            object_ids: scene.object_ids,
            _environment: environment,
            environment_view,
            sampler,
            segments: GrowBuffer::new(device, "segments", vertex, 1 << 16),
            segments_xray: GrowBuffer::new(device, "segments x-ray", vertex, 1 << 16),
            boxes: GrowBuffer::new(device, "boxes", vertex, 1 << 16),
            boxes_xray: GrowBuffer::new(device, "boxes x-ray", vertex, 1 << 16),
            points: GrowBuffer::new(device, "points", vertex, 1 << 14),
            ghost_ids: GrowBuffer::new(device, "ghost ids", vertex, 256),
            width,
            height,
            ui_scale: 1.0,
            frame,
            frame_view,
            viewport: None,
            egui_ctx,
            egui_renderer,
            textures,
            views: HashMap::new(),
            view_pipelines: HashMap::new(),
            view_chain_layouts: HashMap::new(),
            view_layout: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("image kind"),
                entries: &[
                    wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::Rgba8Unorm, view_dimension: wgpu::TextureViewDimension::D2 }, count: None },
                    wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                ],
            }),
            view_shading: None,
            graph: None,
            inspections: HashMap::new(),
            inspect_pipelines: HashMap::new(),
            last_layout: Layout::new(width, height),
            last_panes: Vec::new(),
            panel_empty: false,
            reference_slot: ViewSlot::default(),
            free_slot: ViewSlot::default(),
            last_main_view: None,
            compare: None,
            compare_group: device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("compare"), layout: &pipelines_compare_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: compare_params.as_entire_binding() }] }),
            compare_params,
            reference_drawn: false,
            user_layout: Default::default(),
            cursor: egui::CursorIcon::Default,
            frame_starts: std::collections::VecDeque::new(),
            timing: None,
            statics,
            other_scenes: HashMap::new(),
            gpu,
        };
        renderer.resize(width, height);
        Ok(renderer)
    }

    /// The GPU data of the scene `statics` prepares: its raster meshes.
    fn scene_resources(device: &wgpu::Device, statics: &Statics) -> SceneResources {
        let meshes = &statics.meshes;
        let mesh_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("mesh vertices"), contents: bytemuck::cast_slice(&meshes.vertices), usage: wgpu::BufferUsages::VERTEX });
        let mesh_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("mesh indices"), contents: bytemuck::cast_slice(&meshes.indices), usage: wgpu::BufferUsages::INDEX });
        let ids: Vec<u32> = (0..statics.scene.objects.len() as u32).collect();
        let object_ids = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("object ids"), contents: bytemuck::cast_slice(&ids), usage: wgpu::BufferUsages::VERTEX });

        SceneResources {
            mesh_vertices,
            mesh_indices,
            object_ids,
        }
    }

    /// The current investigation's starting state for `demo`, completed from
    /// the prepared data of its scene.
    pub fn default_state(&mut self, demo: impl Into<crate::shot::DemoId>) -> ShotState {
        let demo = demo.into();
        self.switch_scene(investigation::current().demo_scene(demo));
        self.statics.default_state(demo)
    }

    /// Show scene `id`: prepare it (or take it back from the scenes kept),
    /// keep the current one, and rebuild the GPU data drawn from it.
    pub fn switch_scene(&mut self, id: SceneId) {
        if self.statics.scene.id == id {
            return;
        }
        let next = match self.other_scenes.remove(&id) {
            Some(statics) => statics,
            None => Statics::prepare(id).unwrap_or_else(|e| panic!("preparing scene {}: {e:#}", id.key())),
        };
        let previous = std::mem::replace(&mut self.statics, next);
        self.other_scenes.insert(previous.scene.id, previous);
        let scene = Self::scene_resources(&self.gpu.device, &self.statics);
        self.mesh_vertices = scene.mesh_vertices;
        self.mesh_indices = scene.mesh_indices;
        self.object_ids = scene.object_ids;
    }

    fn frame_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        target(device, "frame (RGBA8)", [width, height], FRAME_FORMAT, 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING)
    }

    fn frame_bind_group(device: &wgpu::Device, pipelines: &Pipelines, r: &RecordBuffers, environment: &wgpu::TextureView, sampler: &wgpu::Sampler) -> wgpu::BindGroup {
        fn buffer(binding: u32, b: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry { binding, resource: b.as_entire_binding() }
        }
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout: &pipelines.frame_layout,
            entries: &[
                buffer(0, &r.uniforms),
                buffer(1, &r.objects.buffer),
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                buffer(3, &r.mesh_nodes),
                buffer(4, &r.mesh_triangles),
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(environment) },
                buffer(6, &r.terrain),
            ],
        })
    }

    pub fn frame_bind_group_ref(&self) -> &wgpu::BindGroup {
        &self.frame_bind_group
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.gpu.device
    }

    pub fn layout(&self) -> Layout {
        Layout::with_ui_scale(self.width, self.height, self.ui_scale)
    }

    /// The layout of the last rendered frame (the viewport widens when the
    /// struct panel is hidden). Picking uses this.
    pub fn frame_layout(&self) -> Layout {
        self.last_layout
    }

    /// The last frame's views beside the main one, in canvas points (the
    /// viewer orbits and picks from them too).
    pub fn frame_panes(&self) -> &[egui::Rect] {
        &self.last_panes
    }

    /// `dock`: the interactive viewer's layout with the slider dock.
    /// With `values`, the panel shows (the Experiments tab's values) whatever the state's panel mode.
    pub fn layout_for(&self, state: &ShotState, scene_type: SceneType, dock: bool, values: bool) -> Layout {
        let panel = scene_type != SceneType::ThreeD || state.panel != crate::shot::PanelMode::Hidden || values;
        let mut layout = self.layout();
        if dock {
            layout = layout.with_dock();
        }
        // The pseudo-code and the knowledge graph take the wider panel.
        let code = scene_type == SceneType::ThreeD && matches!(state.panel, crate::shot::PanelMode::Code | crate::shot::PanelMode::Knowledge | crate::shot::PanelMode::Dag | crate::shot::PanelMode::Resources);
        layout = layout.with_user(&self.user_layout, dock, code);
        layout.for_panel(panel)
    }

    /// Recreate the frame target for a new size (the viewport targets follow
    /// on the next frame).
    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(64);
        let height = height.max(64);
        self.width = width;
        self.height = height;
        self.frame = Self::frame_texture(&self.gpu.device, width, height);
        self.frame_view = self.frame.create_view(&Default::default());
        self.viewport = None;
        self.reference_slot = ViewSlot::default();
        self.free_slot = ViewSlot::default();
    }

    /// Upload the frame's records and uniforms.
    pub fn upload(&mut self, content: &FrameContent) {
        if !content.shading_frame.is_empty() {
            assert!(content.shading_frame.len() <= frame::SHADING_FRAME_CAPACITY, "shading_frame holds {} vectors (at most {})", content.shading_frame.len(), frame::SHADING_FRAME_CAPACITY);
            if let Some(buffer) = self.shading().map(|s| s.frame_data.clone()) {
                self.gpu.queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&content.shading_frame));
            }
        }
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        queue.write_buffer(&self.records.uniforms, 0, bytemuck::bytes_of(&content.uniforms));
        queue.write_buffer(&self.shading_uniforms, 0, bytemuck::cast_slice(&content.shading));
        if self.records.objects.write(device, queue, &content.objects) {
            self.frame_bind_group = Self::frame_bind_group(device, &self.pipelines, &self.records, &self.environment_view, &self.sampler);
        }
    }

    /// The textures of the image instances `state` shows (src/views.rs): made
    /// for a new one and registered with egui, their parameters written, and
    /// those it no longer shows let go.
    fn prepare_views(&mut self, state: &ShotState) {
        use crate::utility::inset::{view_texture, ImageSource};
        let package = investigation::current().package().unwrap_or("rlstd");
        let shown: Vec<(&str, &str, &std::collections::BTreeMap<String, Vec<f32>>)> = state.images.iter().filter_map(|i| match &i.source {
            ImageSource::View { kind, params } => Some((i.name.as_str(), kind.as_str(), params)),
            _ => None,
        }).collect();
        let gone: Vec<String> = self.views.iter().filter(|(name, v)| !shown.iter().any(|(n, k, _)| n == name && *k == v.kind.key && v.kind.package == package)).map(|(name, _)| name.clone()).collect();
        for name in gone {
            let view = self.views.remove(&name).expect("an instance");
            self.egui_renderer.free_texture(&view.id);
            self.textures.remove(&view_texture(&name));
        }
        let device = &self.gpu.device;
        let viewport = self.viewport.as_ref().map_or([0, 0], |v| v.size);
        for (name, kind, params) in shown {
            let kind = crate::views::kind(package, kind).unwrap_or_else(|e| panic!("{e}"));
            let size = kind.texels(viewport);
            // One that takes the viewport's shape is made again when that changes.
            if let Some(view) = self.views.remove(name) {
                if view.size == size {
                    self.views.insert(name.to_string(), view);
                } else {
                    self.egui_renderer.free_texture(&view.id);
                    self.textures.remove(&view_texture(name));
                }
            }
            if !self.views.contains_key(name) {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(name),
                    size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                // The entry point writes display colours, shown as they are (as a CPU picture's pixels are).
                let storage = texture.create_view(&Default::default());
                let shown = texture.create_view(&Default::default());
                let params = device.create_buffer(&wgpu::BufferDescriptor { label: Some(name), size: kind.params_size.max(16) as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(name),
                    layout: &self.view_layout,
                    entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&storage) }, wgpu::BindGroupEntry { binding: 1, resource: params.as_entire_binding() }],
                });
                let id = self.egui_renderer.register_native_texture(device, &shown, wgpu::FilterMode::Linear);
                self.textures.insert(view_texture(name), (id, size));
                self.views.insert(name.to_string(), ViewInstance { kind, _texture: texture, size, params, group, id });
            }
            let view = &self.views[name];
            self.gpu.queue.write_buffer(&view.params, 0, &view.kind.pack(params));
        }
    }

    /// Runs each image instance's entry point into its texture: before the
    /// scene those that read nothing of the pass chain, after it
    /// (`after_chain`) those that do (src/views.rs `reads`).
    fn dispatch_views(&mut self, encoder: &mut wgpu::CommandEncoder, after_chain: bool) {
        if !self.views.values().any(|v| v.kind.reads.is_empty() != after_chain) {
            return;
        }
        let shading = match self.shading().map(|s| s.group.clone()) {
            Some(group) => group,
            None => self.view_shading.get_or_insert_with(|| {
                let device = &self.gpu.device;
                let storage = |label| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: 16, usage: wgpu::BufferUsages::STORAGE, mapped_at_creation: false });
                let (data, frame) = (storage("no shading data"), storage("no shading frame"));
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("no scene shading"),
                    layout: &self.pipelines.shading_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: self.shading_uniforms.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: data.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 2, resource: frame.as_entire_binding() },
                    ],
                })
            }).clone(),
        };
        let device = &self.gpu.device;
        // The chain an after-chain kind reads: the current shading's, when it has one.
        let source = investigation::current().scene_shading();
        let chain = self.chain.as_ref().filter(|c| source.is_some_and(|s| std::ptr::eq(c.source, s)));
        for view in self.views.values() {
            let kind = view.kind;
            if kind.reads.is_empty() == after_chain {
                continue;
            }
            let reads: Vec<&str> = kind.reads.iter().map(String::as_str).collect();
            // A kind that reads the chain shows nothing without one.
            let (Some(chain), Some(viewport)) = (chain.filter(|_| after_chain), self.viewport.as_ref()) else {
                if after_chain {
                    continue;
                }
                // A kind that reads nothing: as before.
                let pipeline = self.view_pipelines.entry((kind.module.clone(), kind.entry.clone())).or_insert_with(|| {
                    view_pipeline(device, &self.pipelines.frame_layout, &self.pipelines.shading_layout, None, &self.view_layout, kind)
                });
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(&kind.key), timestamp_writes: None });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &self.frame_bind_group, &[]);
                pass.set_bind_group(1, &shading, &[]);
                pass.set_bind_group(3, &view.group, &[]);
                pass.dispatch_workgroups(view.size[0].div_ceil(kind.workgroup[0].max(1)), view.size[1].div_ceil(kind.workgroup[1].max(1)), 1);
                continue;
            };
            let layout = self.view_chain_layouts.entry((kind.module.clone(), kind.entry.clone())).or_insert_with(|| crate::pipelines::chain_layout(device, chain.source, &kind.entry, &reads, &[], wgpu::ShaderStages::COMPUTE));
            let pipeline = self.view_pipelines.entry((kind.module.clone(), kind.entry.clone())).or_insert_with(|| {
                view_pipeline(device, &self.pipelines.frame_layout, &self.pipelines.shading_layout, Some(layout), &self.view_layout, kind)
            });
            let group = chain.read_group(device, viewport, layout, &reads);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(&kind.key), timestamp_writes: None });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            pass.set_bind_group(1, &shading, &[]);
            pass.set_bind_group(2, &group, &[]);
            pass.set_bind_group(3, &view.group, &[]);
            pass.dispatch_workgroups(view.size[0].div_ceil(kind.workgroup[0].max(1)), view.size[1].div_ceil(kind.workgroup[1].max(1)), 1);
        }
    }

    /// The current investigation's own scene shading, built on first use.
    fn shading(&mut self) -> Option<&Shading> {
        let source = investigation::current().scene_shading()?;
        if !self.shadings.iter().any(|s| std::ptr::eq(s.source, source)) {
            let device = &self.gpu.device;
            let pipelines = self.pipelines.shading(device, source).unwrap_or_else(|e| panic!("{e:#}"));
            let mut data = (source.data)();
            if data.is_empty() {
                data.push([0.0; 4]);
            }
            let data = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("scene shading data"), contents: bytemuck::cast_slice(&data), usage: wgpu::BufferUsages::STORAGE });
            let frame_data = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scene shading frame data"),
                size: (frame::SHADING_FRAME_CAPACITY * 16) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("scene shading"),
                layout: &self.pipelines.shading_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.shading_uniforms.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: data.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: frame_data.as_entire_binding() },
                ],
            });
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("volumes"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            });
            let update_buffers: Vec<wgpu::Buffer> = source.update.map_or_else(Vec::new, |u| {
                u.buffers.iter().map(|b| device.create_buffer(&wgpu::BufferDescriptor { label: Some(b.name), size: ((b.words)().max(1) * 4), usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false })).collect()
            });
            let update_params = source.update.map(|_| {
                device.create_buffer(&wgpu::BufferDescriptor { label: Some("update parameters"), size: UPDATE_DISPATCHES_MAX * crate::pipelines::UPDATE_PARAMS_STRIDE, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
            });
            let volume_sets = if source.volumes.is_empty() { Vec::new() } else { vec![VolumeSet::new(device, source, &pipelines, &update_buffers, update_params.as_ref(), &sampler)] };
            self.shadings.push(Shading { source, pipelines, group: group.clone(), data: vec![(None, data, group)], pending: None, frame_data, volume_sets, volume_set: 0, update_runs: Vec::new(), update_buffers, update_params, sampler, scale: 1, clock: 0 });
        }
        self.shadings.iter().find(|s| std::ptr::eq(s.source, source))
    }

    /// Group 1 of the current shading with the data `state` needs
    /// (`Investigation::scene_data`): a kept one, or, while a new one builds
    /// on a worker thread, the last one drawn. With `wait`, the build is
    /// waited for (a research run). A finished build restarts the history.
    fn select_data(&mut self, state: &ShotState, wait: bool) {
        if self.shading().is_none() {
            return;
        }
        let wanted = investigation::current().scene_data(state);
        let source = investigation::current().scene_shading().expect("a scene shading");
        let device = self.gpu.device.clone();
        let layout = self.pipelines.shading_layout.clone();
        let uniforms = self.shading_uniforms.clone();
        let shading = self.shadings.iter_mut().find(|s| std::ptr::eq(s.source, source)).expect("built");
        let key = wanted.as_ref().map(|w| w.key);
        if let Some(w) = wanted {
            let kept = shading.data.iter().any(|d| d.0 == Some(w.key));
            let building = shading.pending.as_ref().is_some_and(|p| p.0 == w.key);
            if !kept && !building {
                let (tx, rx) = std::sync::mpsc::channel();
                let build = w.build;
                std::thread::Builder::new().name("scene data".into()).spawn(move || {
                    let _ = tx.send(build());
                }).expect("a thread for the scene data");
                shading.pending = Some((w.key, rx, std::time::Instant::now()));
            }
        }
        let mut finished = false;
        if let Some((pending_key, rx, _)) = &shading.pending {
            let result = if wait && key == Some(*pending_key) { rx.recv().ok() } else { rx.try_recv().ok() };
            if let Some(mut data) = result {
                if data.is_empty() {
                    data.push([0.0; 4]);
                }
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("scene shading data"), contents: bytemuck::cast_slice(&data), usage: wgpu::BufferUsages::STORAGE });
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("scene shading"),
                    layout: &layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: buffer.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 2, resource: shading.frame_data.as_entire_binding() },
                    ],
                });
                shading.data.insert(1.min(shading.data.len()), (Some(*pending_key), buffer, group));
                // Keep the built-once data and the latest builds.
                while shading.data.len() > SCENE_DATA_KEPT + 1 {
                    let last = shading.data.iter().rposition(|d| d.0.is_some()).expect("a build to let go");
                    shading.data.remove(last);
                }
                shading.pending = None;
                finished = true;
            }
        }
        if let Some(i) = shading.data.iter().position(|d| d.0 == key) {
            shading.group = shading.data[i].2.clone();
        }
        if let Some(r) = source.resolution {
            shading.scale = (r.divisor)(state).max(1);
        }
        if let Some(update) = source.update {
            let plan = (update.plan)(state);
            shading.clock += 1;
            let clock = shading.clock;
            let set = match shading.volume_sets.iter().position(|v| v.key == Some(plan.key)) {
                Some(i) => i,
                None => {
                    if shading.volume_sets.len() < VOLUME_SETS_KEPT {
                        let set = VolumeSet::new(&device, source, &shading.pipelines, &shading.update_buffers, shading.update_params.as_ref(), &shading.sampler);
                        shading.volume_sets.push(set);
                        shading.volume_sets.len() - 1
                    } else {
                        // The set used longest ago, unless a run still waits to fill it.
                        let waiting: Vec<usize> = shading.update_runs.iter().map(|r| r.0).collect();
                        (0..shading.volume_sets.len()).filter(|i| !waiting.contains(i)).min_by_key(|&i| shading.volume_sets[i].used).unwrap_or(0)
                    }
                }
            };
            let fresh = shading.volume_sets[set].key != Some(plan.key);
            if fresh || plan.always {
                assert!(plan.dispatches.len() as u64 <= UPDATE_DISPATCHES_MAX, "{}: {} update dispatches, at most {UPDATE_DISPATCHES_MAX}", source.fragment, plan.dispatches.len());
                shading.update_runs.retain(|r| r.0 != set);
                shading.update_runs.push((set, plan.dispatches));
                shading.volume_sets[set].key = Some(plan.key);
                finished |= fresh;
            }
            shading.volume_sets[set].used = clock;
            shading.volume_set = set;
        }
        if finished {
            self.still = (None, 0);
            self.still_scene = (None, 0);
            self.reference_slot.still = (None, 0);
            self.reference_slot.still_scene = (None, 0);
            self.free_slot.still = (None, 0);
            self.free_slot.still_scene = (None, 0);
        }
    }

    /// Seconds the current shading's scene data has been building, while it does.
    pub fn rebuilding(&self) -> Option<f32> {
        let source = investigation::current().scene_shading()?;
        let shading = self.shadings.iter().find(|s| std::ptr::eq(s.source, source))?;
        shading.pending.as_ref().map(|p| p.2.elapsed().as_secs_f32())
    }

    /// Build (and wait for) the scene data `state` needs (a research run).
    pub fn prepare_scene_data(&mut self, state: &ShotState) {
        self.switch_scene(investigation::current().scene(state));
        self.select_data(state, true);
        let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("update") });
        self.encode_updates(&mut encoder, None);
        self.gpu.queue.submit([encoder.finish()]);
    }

    /// Mip `mip` of the current shading's volume `name`, as the state last
    /// drawn (or prepared) left it: its texels, rows unpadded, and its size.
    pub fn read_volume(&self, name: &str, mip: u32) -> Option<(Vec<u8>, [u32; 3])> {
        let source = investigation::current().scene_shading()?;
        let shading = self.shadings.iter().find(|s| std::ptr::eq(s.source, source))?;
        let v = source.volumes.iter().position(|v| v.name == name)?;
        let volume = &source.volumes[v];
        let texture = &shading.volume_sets.get(shading.volume_set)?.textures[v];
        let size = [(volume.size[0] >> mip).max(1), (volume.size[1] >> mip).max(1), (volume.size[2] >> mip).max(1)];
        let texel = volume.format.block_copy_size(None)?;
        let row = size[0] * texel;
        let padded = row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let device = &self.gpu.device;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("volume readback"), size: padded as u64 * size[1] as u64 * size[2] as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture, mip_level: mip, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(size[1]) } },
            wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: size[2] },
        );
        self.gpu.queue.submit([encoder.finish()]);
        let bytes = read_buffer(device, &self.gpu.queue, &buffer, 0, buffer.size());
        let texels = bytes.chunks(padded as usize).flat_map(|r| r[..row as usize].iter().copied()).collect();
        Some((texels, size))
    }

    /// What the graph panel shows (`PanelMode::Graph`): the frame's graph,
    /// each node's last GPU time, and the inspected resource.
    fn graph_view(&mut self) -> ui::GraphView {
        let graph = self.frame_graph();
        let last: Vec<(String, f64)> = self.timing.as_ref().map(|t| t.last().to_vec()).unwrap_or_default();
        let nodes = graph
            .order
            .iter()
            .map(|&i| {
                let n = &graph.nodes[i];
                // Two nodes of one entry share its row in the timing: each shows half.
                let sharing = graph.nodes.iter().filter(|m| m.timing == n.timing).count().max(1) as f64;
                let ms = last.iter().find(|(name, _)| name == n.timing).map(|(_, ms)| ms / sharing);
                let index = crate::sources::index();
                let engine = index.pass(n.name).map(|p| {
                    let at = p.sources.first().and_then(|k| index.entries.get(k)).map_or(String::new(), |e| format!("{}:{}", e.path.rsplit('/').next().unwrap_or(&e.path), e.line));
                    (p.engine.clone(), at)
                });
                let result = graph.result_of(i).map(str::to_string);
                let thumbnail = self.inspections.get(&format!("{DAG_IMAGE}{}", n.name)).map(|x| (x.id, x.size));
                ui::GraphNodeView { name: n.name.to_string(), engine, ms, live: graph.live[i], cached: n.cached, result, thumbnail, about: crate::graph::about(n.name), reads: n.reads.iter().map(|r| r.to_string()).collect(), writes: n.writes.iter().map(|w| w.to_string()).collect() }
            })
            .collect();
        // The DAG's arrows, between rows: what flows from one node to the next
        // (its resources), or an order without data (a later write).
        let row_of = |i: usize| graph.order.iter().position(|&o| o == i).expect("every node is in the order");
        let mut edges: Vec<ui::GraphEdgeView> = Vec::new();
        for &(a, b, kind) in &graph.edges {
            let (from, to) = (row_of(a), row_of(b));
            let at = match edges.iter().position(|e| e.from == from && e.to == to) {
                Some(k) => k,
                None => {
                    edges.push(ui::GraphEdgeView { from, to, data: Vec::new() });
                    edges.len() - 1
                }
            };
            if let crate::graph::Edge::Data(r) = kind {
                edges[at].data.push(r);
            }
        }
        let users = |r: &crate::graph::Resource, writes: bool| -> Vec<String> {
            graph.order.iter().map(|&i| &graph.nodes[i]).filter(|n| if writes { n.writes.contains(&r.name) } else { n.reads.contains(&r.name) }).map(|n| n.name.to_string()).collect()
        };
        let resources = graph
            .resources
            .iter()
            .map(|r| ui::GraphResourceView {
                name: r.name.to_string(),
                lifetime: r.lifetime,
                shape: r.shape.describe(),
                writers: users(r, true),
                readers: users(r, false),
                key: graph.last_user(r.name).filter(|_| r.name != "frame").map(|n| format!("{}.{}", graph.nodes[n].name, r.name)),
                thumbnail: self.inspections.get(&format!("{RESOURCE_IMAGE}{}", r.name)).map(|x| (x.id, x.size)),
            })
            .collect();
        let mut uses = Vec::new();
        for (row, &i) in graph.order.iter().enumerate() {
            let n = &graph.nodes[i];
            for (c, r) in graph.resources.iter().enumerate() {
                let (reads, writes) = (n.reads.contains(&r.name), n.writes.contains(&r.name));
                if reads || writes {
                    uses.push((row, c, reads, writes));
                }
            }
        }
        let inspected = self.inspections.get(crate::viewer::INSPECTOR).map(|x| x.key.clone());
        ui::GraphView { nodes, resources, uses, edges, inspected, timestamps: self.timing.is_some() }
    }

    /// What the performance panel shows (`PanelMode::Performance`): the GPU,
    /// each pass's times, and the recent frames' GPU and frame times.
    fn performance_view(&self) -> ui::PerformanceView {
        let starts: Vec<&std::time::Instant> = self.frame_starts.iter().collect();
        let frames = starts.windows(2).map(|w| (*w[1] - *w[0]).as_secs_f64() * 1000.0).collect();
        let (passes, totals) = match &self.timing {
            Some(t) => {
                let last = t.last();
                // The passes the last measured frame ran: a pass that runs only
                // on a change (an update, the depth views) shows while it runs.
                let passes = t.summary_last(30).into_iter().filter_map(|(name, mean, p95)| {
                    let now = last.iter().find(|(n, _)| *n == name)?.1;
                    Some((name, mean, p95, now))
                }).collect();
                (passes, t.totals())
            }
            None => (Vec::new(), Vec::new()),
        };
        ui::PerformanceView { gpu: self.gpu.info.name.clone(), timestamps: self.timing.is_some(), passes, totals, frames }
    }

    /// Time the live frame's passes (src/timing.rs), or stop; false when the device cannot.
    pub fn set_timing(&mut self, on: bool) -> bool {
        match (on, self.timing.is_some()) {
            (true, false) => self.timing = crate::timing::Timing::new(&self.gpu),
            (false, true) => self.timing = None,
            _ => {}
        }
        self.timing.is_some()
    }

    /// Compare the next frames' live view with a reference (src/experiment.rs), or stop.
    pub fn set_compare(&mut self, compare: Option<CompareRequest>) {
        if compare.is_none() {
            self.reference_slot = ViewSlot::default();
        }
        self.compare = compare;
    }

    /// The free view's slot and the renderer's trade places.
    fn swap_free_slot(&mut self) {
        std::mem::swap(&mut self.viewport, &mut self.free_slot.viewport);
        std::mem::swap(&mut self.chain, &mut self.free_slot.chain);
        std::mem::swap(&mut self.still, &mut self.free_slot.still);
        std::mem::swap(&mut self.still_scene, &mut self.free_slot.still_scene);
    }

    /// The reference's slot and the renderer's trade places.
    fn swap_slot(&mut self) {
        std::mem::swap(&mut self.viewport, &mut self.reference_slot.viewport);
        std::mem::swap(&mut self.chain, &mut self.reference_slot.chain);
        std::mem::swap(&mut self.still, &mut self.reference_slot.still);
        std::mem::swap(&mut self.still_scene, &mut self.reference_slot.still_scene);
    }

    /// The last frame's image of a view, without the overlay: displayed
    /// (tone mapped, sRGB-encoded) and linear (its HDR target). None when it
    /// was not drawn (the reference without a comparison).
    pub fn read_view(&mut self, which: ViewKind) -> Option<(crate::metrics::Image, crate::metrics::Image)> {
        if which == ViewKind::Reference && !self.reference_drawn {
            return None;
        }
        let targets = match which {
            ViewKind::Live => self.viewport.as_ref()?,
            ViewKind::Reference => self.reference_slot.viewport.as_ref()?,
        };
        let device = &self.gpu.device;
        let [w, h] = targets.size;
        // Displayed: the scene composite into a frame-sized target of its own.
        let display = target(device, "experiment display", [w, h], FRAME_FORMAT, 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let display_view = display.create_view(&Default::default());
        let rgba_row = padded_row_bytes(w);
        let rgba_buffer = readback_buffer(device, w, h);
        let half_row = (w * 8).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let half_buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("experiment hdr"), size: half_row as u64 * h as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("experiment readback") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("experiment display"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &display_view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines.composite_scene);
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            pass.set_bind_group(1, &targets.composite, &[]);
            pass.draw(0..3, 0..1);
        }
        let extent = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &display, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &rgba_buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(rgba_row), rows_per_image: Some(h) } },
            extent,
        );
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &targets.hdr_texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &half_buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(half_row), rows_per_image: Some(h) } },
            extent,
        );
        self.gpu.queue.submit([encoder.finish()]);
        let rgba = unpad_rows(&read_buffer(device, &self.gpu.queue, &rgba_buffer, 0, rgba_row as u64 * h as u64), w, h);
        let halves_padded = read_buffer(device, &self.gpu.queue, &half_buffer, 0, half_row as u64 * h as u64);
        let mut words = Vec::with_capacity((w * h * 4) as usize);
        for row in halves_padded.chunks_exact(half_row as usize) {
            words.extend(row[..(w * 8) as usize].chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])));
        }
        Some((crate::metrics::Image::from_rgba8(w as usize, h as usize, &rgba), crate::metrics::Image::from_rgba16f(w as usize, h as usize, &words)))
    }

    fn upload_draws(&mut self, content: &FrameContent) {
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        let d = &content.draw;
        // The x-ray copies draw occluded parts dimmed and dashed.
        let dim = |a: f32| if a < 0.0 { a * 0.3 } else { a * 0.3 };
        let seg_xray: Vec<_> = d.segments.iter().map(|s| {
            let mut s = *s;
            s.color[3] = dim(s.color[3]);
            s.flags |= frame::FLAG_DASHED;
            s
        }).collect();
        let box_xray: Vec<_> = d.boxes.iter().map(|b| {
            let mut b = *b;
            b.color[3] = dim(b.color[3]);
            b.rid[3] |= frame::FLAG_DASHED;
            b
        }).collect();
        self.segments.write(device, queue, &d.segments);
        self.segments_xray.write(device, queue, &seg_xray);
        self.boxes.write(device, queue, &d.boxes);
        self.boxes_xray.write(device, queue, &box_xray);
        self.points.write(device, queue, &d.points);
        self.ghost_ids.write(device, queue, &d.ghosts);
    }

    /// Render one frame into the RGBA target. When `readback` is given, the
    /// frame is copied into it (rows padded to 256 bytes) in the same submission.
    pub fn render_frame(&mut self, request: FrameRequest, readback: Option<&wgpu::Buffer>) -> FrameResult {
        self.render_frame_with(request, readback, None)
    }

    /// Like `render_frame`, but with prepared frame content (used by the
    /// selftest to draw known markers).
    pub fn render_frame_with(&mut self, request: FrameRequest, readback: Option<&wgpu::Buffer>, prepared: Option<FrameContent>) -> FrameResult {
        self.switch_scene(investigation::current().scene(request.state));
        // The performance panel measures the frames it is shown with.
        if matches!(request.state.panel, crate::shot::PanelMode::Performance | crate::shot::PanelMode::Graph | crate::shot::PanelMode::Dag | crate::shot::PanelMode::Resources) && request.scene_type == SceneType::ThreeD {
            self.set_timing(true);
        }
        self.frame_starts.push_back(std::time::Instant::now());
        if self.frame_starts.len() > 121 {
            self.frame_starts.pop_front();
        }
        let dock = matches!(&request.chrome, Chrome::Viewer(v) if !(v.timeline_mode && v.playing));
        let values = dock && matches!(&request.chrome, Chrome::Viewer(v) if v.tab == ui::DockTab::Experiments);
        let mut layout = self.layout_for(request.state, request.scene_type, dock, values);
        let scene_beat = request.scene_type == SceneType::ThreeD;
        // A split scene area: the views beside the main one, each in its own
        // rectangle; the main view takes the last (Investigation::panes).
        let mut panes = if scene_beat && prepared.is_none() { investigation::current().panes(request.state) } else { Vec::new() };
        let main = panes.pop();
        // A panel with nothing to show gives its room to the viewport: a
        // listing the investigation does not have, or a struct panel whose
        // frame has neither a record nor a readout. That is known once the
        // frame is built, at the viewport's size: it is built at the last
        // frame's layout, and again at the other if that was wrong.
        let mut built = None;
        if layout.panel_shown && scene_beat && prepared.is_none() && main.is_none() && !values && self.compare.is_none() {
            use crate::shot::PanelMode;
            match request.state.panel {
                PanelMode::Code if request.state.code.as_deref().and_then(|c| investigation::current().listing(c)).is_none() => layout = layout.for_panel(false),
                PanelMode::Declaration | PanelMode::Values | PanelMode::Memory => {
                    self.select_data(request.state, !request.history);
                    let build = |layout: &Layout| {
                        let vp = layout.viewport_px();
                        crate::shader_data::build_frame(request.state, &self.statics, [vp[2] as f32, vp[3] as f32], layout.scale, request.time)
                    };
                    let shown = layout;
                    let hidden = layout.for_panel(false);
                    let mut content = build(if self.panel_empty { &hidden } else { &shown });
                    let empty = content.panel_record.is_none() && content.readout.is_none();
                    if empty != self.panel_empty {
                        content = build(if empty { &hidden } else { &shown });
                    }
                    self.panel_empty = empty;
                    layout = if empty { hidden } else { shown };
                    built = Some(content);
                }
                _ => {}
            }
        }
        let rects = if main.is_some() { layout.split(panes.len() + 1) } else { Vec::new() };
        if let Some(rect) = rects.last() {
            layout.viewport = *rect;
        }
        // An experiment's reference (src/experiment.rs): in a view of its own,
        // beside the live one, or behind it (a wipe, a difference, alone).
        let compare = self.compare.clone().filter(|_| scene_beat && prepared.is_none() && main.is_none());
        let mut reference_rect = layout.viewport;
        if compare.as_ref().is_some_and(|c| c.mode == crate::experiment::Compare::SideBySide) {
            let halves = layout.split(2);
            reference_rect = halves[0];
            layout.viewport = halves[1];
        }
        self.last_layout = layout;
        self.last_panes = rects.iter().take(panes.len()).copied().collect();
        let vp = layout.viewport_px();
        let viewport_size = [vp[2], vp[3]];
        if self.viewport.as_ref().is_none_or(|v| v.size != viewport_size) {
            self.chain = None;
            self.viewport = Some(ViewportTargets::new(&self.gpu.device, &self.pipelines, viewport_size));
        }
        let size = [vp[2] as f32, vp[3] as f32];
        self.reference_drawn = false;
        if let Some(c) = &compare {
            // The reference first, in its own submission and its own targets,
            // so its history builds up as the live view's does.
            self.swap_slot();
            if self.viewport.as_ref().is_none_or(|v| v.size != viewport_size) {
                self.chain = None;
                self.viewport = Some(ViewportTargets::new(&self.gpu.device, &self.pipelines, viewport_size));
            }
            self.select_data(&c.reference, !request.history);
            let mut content = crate::shader_data::build_frame(&c.reference, &self.statics, size, layout.scale, request.time);
            let count = |last: (Option<u64>, u32), key: u64| if last.0 == Some(key) { (Some(key), last.1.saturating_add(1)) } else { (Some(key), 0) };
            self.still = count(self.still, still_key(&content, viewport_size, true));
            self.still_scene = count(self.still_scene, still_key(&content, viewport_size, false));
            content.uniforms.view_params[3] = if request.history { (1 + self.still.1.min(255) + 256 * self.still_scene.1.min(32767)) as f32 } else { 0.0 };
            self.upload(&content);
            self.upload_draws(&content);
            let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("reference") });
            self.encode_scene(&mut encoder, &content, false);
            self.gpu.queue.submit([encoder.finish()]);
            self.swap_slot();
            self.reference_drawn = true;
        }
        // Each other view: built, drawn, and composited into its rectangle in
        // a submission of its own, so the shared uniforms and targets hold its
        // camera and values while it draws.
        let mut drawn = Vec::new();
        for (pane, rect) in panes.iter().zip(&rects) {
            let mut content = crate::shader_data::build_frame(&pane.state, &self.statics, size, layout.scale, request.time);
            content.uniforms.view_params[3] = 0.0;
            self.select_data(&pane.state, !request.history);
            self.upload(&content);
            self.upload_draws(&content);
            let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pane") });
            self.encode_scene(&mut encoder, &content, false);
            let at = Layout { viewport: *rect, ..layout }.viewport_px();
            self.composite(&mut encoder, at, drawn.is_empty());
            self.gpu.queue.submit([encoder.finish()]);
            drawn.push((*rect, content));
        }
        let scene_state = main.as_ref().map_or(request.state, |m| &m.state);
        // The frame graph scene (a 2D beat's `graph`) renders the frame too, unseen, for its lit boxes' pictures.
        let boxes: Vec<(String, String)> = request.diagram.filter(|_| !scene_beat).map(|d| d.lit_groups()).unwrap_or_default();
        let renders = scene_beat || !boxes.is_empty();
        // A recording (no history) waits for the data, so every frame is the same each time.
        if renders {
            self.select_data(scene_state, !request.history);
        }
        let mut content = match prepared.or(built) {
            Some(c) => Some(c),
            None => renders.then(|| crate::shader_data::build_frame(scene_state, &self.statics, size, layout.scale, request.time)),
        };
        if let Some(content) = content.as_mut() {
            // The frames the view has held still: the same records, uniforms
            // (but the clock), shading values, and viewport as the last frame.
            // None while the area is split: its views share the targets that
            // hold what earlier frames built up.
            let count = |last: (Option<u64>, u32), key: u64| if last.0 == Some(key) { (Some(key), last.1.saturating_add(1)) } else { (Some(key), 0) };
            self.still = count(self.still, still_key(content, viewport_size, true));
            self.still_scene = count(self.still_scene, still_key(content, viewport_size, false));
            content.uniforms.view_params[3] = if request.history && drawn.is_empty() { (1 + self.still.1.min(255) + 256 * self.still_scene.1.min(32767)) as f32 } else { 0.0 };
        }
        // The free camera's view (src/free_view.rs): the main frame drawn from
        // the free camera, in its own submission and targets (so its history
        // builds up as the main view's does), before the main frame's uniforms
        // are written. The main frame is still drawn after it: the pictures of
        // its buffers and what follows its camera are read from it, and it is
        // shown in a corner.
        let free = request.state.free_camera;
        let free_content = content.as_ref().filter(|_| scene_beat && free.on && drawn.is_empty() && compare.is_none()).map(|main| {
            let mut content = crate::free_view::content(main, request.state, &free);
            self.swap_free_slot();
            if self.viewport.as_ref().is_none_or(|v| v.size != viewport_size) {
                self.chain = None;
                self.viewport = Some(ViewportTargets::new(&self.gpu.device, &self.pipelines, viewport_size));
            }
            let count = |last: (Option<u64>, u32), key: u64| if last.0 == Some(key) { (Some(key), last.1.saturating_add(1)) } else { (Some(key), 0) };
            self.still = count(self.still, still_key(&content, viewport_size, true));
            self.still_scene = count(self.still_scene, still_key(&content, viewport_size, false));
            content.uniforms.view_params[3] = if request.history { (1 + self.still.1.min(255) + 256 * self.still_scene.1.min(32767)) as f32 } else { 0.0 };
            self.upload(&content);
            self.upload_draws(&content);
            let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("free camera") });
            self.encode_scene(&mut encoder, &content, false);
            self.gpu.queue.submit([encoder.finish()]);
            self.swap_free_slot();
            content
        });
        if free_content.is_none() {
            self.free_slot = ViewSlot::default();
        }
        // The main camera's own view, in the viewport's lower right corner, at its shape.
        let main_view = free_content.as_ref().map(|_| {
            let v = layout.viewport;
            let size = v.size() * 0.26;
            egui::Rect::from_min_size(v.max - size - egui::vec2(16.0, 16.0), size)
        });
        self.last_main_view = main_view;
        if let Some(content) = &content {
            self.upload(content);
            self.upload_draws(content);
        }

        if content.is_some() {
            self.prepare_views(request.state);
            self.prepare_inspections(request.state, &boxes);
        }
        let diagram_pictures: Vec<crate::diagram::Picture> = boxes.iter().filter_map(|(group, _)| self.inspections.get(&format!("{BOX_IMAGE}{group}")).map(|x| (group.clone(), x.id, x.size))).collect();
        let shows_performance = matches!(request.state.panel, crate::shot::PanelMode::Performance | crate::shot::PanelMode::Graph | crate::shot::PanelMode::Dag | crate::shot::PanelMode::Resources);
        // A recording (no live frames before it, or a single snapshot) times a
        // few renders of the same shot first, so its panel shows measurements.
        const PERFORMANCE_WARMUP: usize = 12;
        if let (true, false, Some(content)) = (shows_performance, request.history, content.as_ref()) {
            let measured = self.timing.as_ref().map_or(usize::MAX, |t| t.totals().len());
            for _ in measured..PERFORMANCE_WARMUP {
                self.frame_starts.push_back(std::time::Instant::now());
                let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("performance warm-up") });
                self.encode_scene(&mut encoder, content, true);
                if let Some(t) = self.timing.as_mut() {
                    t.resolve(&mut encoder);
                }
                self.gpu.queue.submit([encoder.finish()]);
                if let Some(t) = self.timing.as_mut() {
                    t.submitted(&self.gpu.device);
                    t.finish(&self.gpu.device);
                }
            }
            self.frame_starts.push_back(std::time::Instant::now());
            while self.frame_starts.len() > 121 {
                self.frame_starts.pop_front();
            }
        }
        let performance = shows_performance.then(|| self.performance_view());
        let graph_view = matches!(request.state.panel, crate::shot::PanelMode::Graph | crate::shot::PanelMode::Dag | crate::shot::PanelMode::Resources).then(|| self.graph_view());
        let pane_views: Vec<ui::PaneView> = drawn.iter().zip(&panes).map(|((rect, content), pane)| ui::PaneView { rect: *rect, content, title: pane.title.as_deref() }).collect();
        // egui pass content.
        let mut raw = request.raw_input;
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, layout.canvas));
        raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(layout.scale);
        let ui_input = UiInput {
            layout,
            state: request.state,
            content: free_content.as_ref().or(content.as_ref()),
            main_view,
            statics: &self.statics,
            textures: &self.textures,
            scene_type: request.scene_type,
            diagram: request.diagram,
            diagram_pictures: &diagram_pictures,
            beat_sources: request.beat_sources,
            notices: request.notices,
            chapter_number: request.chapter_number,
            fade: request.fade,
            stamp: request.stamp,
            panes: &pane_views,
            title: main.as_ref().and_then(|m| m.title.as_deref()),
            compare: compare.as_ref().filter(|_| self.reference_drawn).map(|c| ui::CompareOverlay { mode: c.mode, reference: reference_rect, split: c.split, gain: c.gain }),
            performance: performance.as_ref(),
            graph: graph_view.as_ref(),
        };
        let mut chrome = request.chrome;
        let mut report = UiReport::default();
        // Start every frame with an empty glyph atlas. Otherwise the atlas
        // grows and repacks as new characters appear (hundreds in Chinese),
        // which moves glyph texels and changes text pixels by a level, so a
        // frame would depend on the frames rendered before it.
        self.egui_ctx.set_glyph_rasterizers(Vec::new());
        ui::restyle(&self.egui_ctx);
        let mut output = self.egui_ctx.run_pass(raw, |ctx| {
            let chrome = match &mut chrome {
                Chrome::Recording { timeline, frame } => Chrome::Recording { timeline, frame: *frame },
                Chrome::Viewer(v) => Chrome::Viewer(v),
            };
            report = ui::draw(ctx, &ui_input, chrome);
        });
        self.cursor = output.platform_output.cursor_icon;
        let paint_jobs = self.egui_ctx.tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: [self.width, self.height], pixels_per_point: output.pixels_per_point };
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        // Drain the deltas: epaint asserts (in debug builds) that a
        // TexturesDelta is empty when it is dropped.
        for (id, deltas) in output.textures_delta.set.drain() {
            for delta in &deltas {
                self.egui_renderer.update_texture(device, queue, id, delta);
            }
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        let mut commands = self.egui_renderer.update_buffers(device, queue, &mut encoder, &paint_jobs, &screen);

        if let Some(content) = &content {
            self.dispatch_views(&mut encoder, false);
            self.encode_scene(&mut encoder, content, true);
            self.dispatch_views(&mut encoder, true);
        }
        if let (Some(free), Some(rect)) = (self.free_slot.viewport.as_ref().filter(|_| free_content.is_some()), main_view) {
            // The free view fills the viewport; the main view sits in its corner.
            self.composite_from(&mut encoder, free, vp, true, None);
            let at = Layout { viewport: rect, ..layout }.viewport_px();
            let timed = self.timing.as_mut().and_then(|t| t.render_writes("composite"));
            self.composite_pass(&mut encoder, self.viewport.as_ref().unwrap(), at, false, None, timed.as_ref().map(|t| t.writes()));
        } else if scene_beat {
            let timed = self.timing.as_mut().and_then(|t| t.render_writes("composite"));
            self.composite_timed(&mut encoder, vp, drawn.is_empty(), timed.as_ref().map(|t| t.writes()));
        }
        if let (Some(c), true, Some(reference)) = (&compare, self.reference_drawn, self.reference_slot.viewport.as_ref()) {
            use crate::experiment::Compare;
            match c.mode {
                Compare::Off => {}
                Compare::Wipe => {
                    let width = (vp[2] as f32 * c.split.clamp(0.0, 1.0)).round() as u32;
                    if width > 0 {
                        self.composite_from(&mut encoder, reference, vp, false, Some([vp[0], vp[1], width, vp[3]]));
                    }
                }
                Compare::SideBySide => self.composite_from(&mut encoder, reference, Layout { viewport: reference_rect, ..layout }.viewport_px(), false, None),
                Compare::Reference => self.composite_from(&mut encoder, reference, vp, false, None),
                Compare::Difference => self.composite_difference(&mut encoder, vp, c.gain),
            }
        }
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.frame_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: if scene_beat { wgpu::LoadOp::Load } else { self.background_clear() }, store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            self.egui_renderer.render(&mut pass, &paint_jobs, &screen);
        }
        if let Some(buffer) = readback {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo { texture: &self.frame, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyBufferInfo { buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded_row_bytes(self.width)), rows_per_image: Some(self.height) } },
                wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
            );
        }
        if let Some(t) = self.timing.as_mut() {
            t.resolve(&mut encoder);
        }
        commands.push(encoder.finish());
        self.gpu.queue.submit(commands);
        if let Some(t) = self.timing.as_mut() {
            t.submitted(&self.gpu.device);
        }
        for id in output.textures_delta.free.drain() {
            self.egui_renderer.free_texture(&id);
        }
        FrameResult { report, content: free_content.or(content) }
    }

    /// The frame cleared to the background colour.
    fn background_clear(&self) -> wgpu::LoadOp<wgpu::Color> {
        // The frame is UNORM and holds sRGB-encoded bytes, so the clear
        // colour is the encoded background colour itself.
        let bg = crate::palette::background();
        wgpu::LoadOp::Clear(wgpu::Color { r: bg.r() as f64 / 255.0, g: bg.g() as f64 / 255.0, b: bg.b() as f64 / 255.0, a: 1.0 })
    }

    /// The viewport targets tone mapped into the frame at `at` (x, y, width,
    /// height in pixels), the frame cleared first when `clear`.
    fn composite(&self, encoder: &mut wgpu::CommandEncoder, at: [u32; 4], clear: bool) {
        self.composite_timed(encoder, at, clear, None);
    }

    /// `composite`, its pass timed (src/timing.rs).
    fn composite_timed(&self, encoder: &mut wgpu::CommandEncoder, at: [u32; 4], clear: bool, timestamps: Option<wgpu::RenderPassTimestampWrites>) {
        self.composite_pass(encoder, self.viewport.as_ref().unwrap(), at, clear, None, timestamps);
    }

    /// `composite` of `targets`, drawn only inside `scissor` (pixels) when given.
    fn composite_from(&self, encoder: &mut wgpu::CommandEncoder, targets: &ViewportTargets, at: [u32; 4], clear: bool, scissor: Option<[u32; 4]>) {
        self.composite_pass(encoder, targets, at, clear, scissor, None);
    }

    fn composite_pass(&self, encoder: &mut wgpu::CommandEncoder, targets: &ViewportTargets, at: [u32; 4], clear: bool, scissor: Option<[u32; 4]>, timestamps: Option<wgpu::RenderPassTimestampWrites>) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.frame_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: if clear { self.background_clear() } else { wgpu::LoadOp::Load }, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timestamps,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(at[0] as f32, at[1] as f32, at[2] as f32, at[3] as f32, 0.0, 1.0);
        if let Some(r) = scissor {
            pass.set_scissor_rect(r[0], r[1], r[2].max(1), r[3].max(1));
        }
        pass.set_pipeline(&self.pipelines.composite);
        pass.set_bind_group(0, &self.frame_bind_group, &[]);
        pass.set_bind_group(1, &targets.composite, &[]);
        pass.draw(0..3, 0..1);
    }

    /// The experiment's difference view of the live and reference HDR targets into `at`.
    fn composite_difference(&self, encoder: &mut wgpu::CommandEncoder, at: [u32; 4], gain: f32) {
        let (Some(live), Some(reference)) = (self.viewport.as_ref(), self.reference_slot.viewport.as_ref()) else { return };
        self.gpu.queue.write_buffer(&self.compare_params, 0, bytemuck::cast_slice(&[gain, 0.0, 0.0, 0.0]));
        let pair = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compare pair"),
            layout: &self.pipelines.texture_pair_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&live.hdr) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&reference.hdr) }],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite difference"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &self.frame_view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(at[0] as f32, at[1] as f32, at[2] as f32, at[3] as f32, 0.0, 1.0);
        pass.set_pipeline(&self.pipelines.composite_diff);
        pass.set_bind_group(0, &self.frame_bind_group, &[]);
        pass.set_bind_group(1, &pair, &[]);
        pass.set_bind_group(2, &self.compare_group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// The scene and overlay passes into the viewport targets (the composite
    /// reads them).
    /// With `timed`, its passes write GPU timestamps (src/timing.rs).
    /// The update dispatches `select_data` planned (`SceneShading::update`):
    /// their parameters written, each run of one pass in a compute pass of
    /// its own (timed while `timing` is given, under the pass's entry).
    fn encode_updates(&mut self, encoder: &mut wgpu::CommandEncoder, mut timing: Option<&mut crate::timing::Timing>) {
        let Some(source) = investigation::current().scene_shading() else { return };
        let Some(update) = source.update else { return };
        let Some(shading) = self.shadings.iter_mut().find(|s| std::ptr::eq(s.source, source)) else { return };
        let runs = std::mem::take(&mut shading.update_runs);
        let shading = &*shading;
        let stride = crate::pipelines::UPDATE_PARAMS_STRIDE;
        for (set, dispatches) in runs {
            let params = shading.update_params.as_ref().expect("update parameters");
            let mut bytes = vec![0u8; dispatches.len() * stride as usize];
            for (i, d) in dispatches.iter().enumerate() {
                bytes[i * stride as usize..i * stride as usize + 64].copy_from_slice(bytemuck::cast_slice(&d.params));
            }
            // One run per submission: a second run writes the parameters after the first is submitted.
            self.gpu.queue.write_buffer(params, 0, &bytes);
            let volumes = &shading.volume_sets[set];
            let mut i = 0;
            while i < dispatches.len() {
                let p = update.passes.iter().position(|pass| pass.entry == dispatches[i].pass).unwrap_or_else(|| panic!("{}: no update pass {}", source.fragment, dispatches[i].pass));
                let mut end = i + 1;
                while end < dispatches.len() && dispatches[end].pass == dispatches[i].pass {
                    end += 1;
                }
                let span = timing.as_mut().and_then(|t| t.compute_writes(update.passes[p].entry));
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(update.passes[p].entry), timestamp_writes: span.as_ref().map(|t| t.compute()) });
                pass.set_pipeline(&shading.pipelines.updates[p].0);
                pass.set_bind_group(0, &self.frame_bind_group, &[]);
                pass.set_bind_group(1, &shading.group, &[]);
                if update.passes[p].reads.iter().any(|r| crate::graph::is_volume(source, r)) {
                    pass.set_bind_group(3, &volumes.group, &[]);
                }
                for (k, d) in dispatches[i..end].iter().enumerate() {
                    let groups = &volumes.updates[p];
                    pass.set_bind_group(2, &groups[(d.mip as usize).min(groups.len() - 1)], &[((i + k) as u64 * stride) as u32]);
                    if d.groups > 0 {
                        pass.dispatch_workgroups(d.groups, 1, 1);
                    }
                }
                drop(pass);
                i = end;
            }
        }
    }

    /// Makes (or keeps) the texture of each graph image the state shows,
    /// and writes its parameters (src/graph.rs `Show`, `INSPECT_PARAMS`).
    fn prepare_inspections(&mut self, state: &ShotState, boxes: &[(String, String)]) {
        use crate::graph::{inspect_entry, FrameGraph, Shape, Show};
        use crate::utility::inset::{view_texture, ImageSource};
        let shown: Vec<(String, String, Option<String>, std::collections::BTreeMap<String, Vec<f32>>)> = state
            .images
            .iter()
            .filter_map(|i| match &i.source {
                ImageSource::Graph { key, show, params } => Some((i.name.clone(), key.clone(), show.clone(), params.clone())),
                _ => None,
            })
            .collect();
        let mut shown = shown;
        if state.panel == crate::shot::PanelMode::Dag {
            let graph = FrameGraph::new(investigation::current().scene_shading(), &[]);
            for (i, n) in graph.nodes.iter().enumerate() {
                if let Some(r) = graph.result_of(i) {
                    shown.push((format!("{DAG_IMAGE}{}", n.name), format!("{}.{r}", n.name), None, Default::default()));
                }
            }
        }
        // The frame graph scene's lit boxes: a pass's result, or a resource as the frame leaves it.
        if !boxes.is_empty() {
            let graph = FrameGraph::new(investigation::current().scene_shading(), &[]);
            for (group, member) in boxes {
                let key = match graph.node(member) {
                    _ if member.contains('.') => Some(member.clone()),
                    Some(n) => graph.result_of(n).map(|r| format!("{member}.{r}")),
                    None => graph.last_user(member).map(|n| format!("{}.{member}", graph.nodes[n].name)),
                };
                if let Some(key) = key {
                    shown.push((format!("{BOX_IMAGE}{group}"), key, None, Default::default()));
                }
            }
        }
        if state.panel == crate::shot::PanelMode::Resources {
            let graph = FrameGraph::new(investigation::current().scene_shading(), &[]);
            for r in graph.resources.iter().filter(|r| r.name != "frame") {
                if let Some(node) = graph.last_user(r.name) {
                    shown.push((format!("{RESOURCE_IMAGE}{}", r.name), format!("{}.{}", graph.nodes[node].name, r.name), None, Default::default()));
                }
            }
        }
        let gone: Vec<String> = self.inspections.iter().filter(|(name, x)| !shown.iter().any(|(n, k, _, _)| n == *name && *k == x.key)).map(|(n, _)| n.clone()).collect();
        for name in gone {
            let x = self.inspections.remove(&name).expect("an inspection");
            self.egui_renderer.free_texture(&x.id);
            self.textures.remove(&view_texture(&name));
        }
        if shown.is_empty() {
            return;
        }
        let device = &self.gpu.device;
        if self.inspect_pipelines.is_empty() {
            let source = crate::shaders::module(crate::shaders::INSPECT).unwrap_or_else(|e| panic!("{e:#}"));
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("inspect"), source: wgpu::ShaderSource::Wgsl(source.into()) });
            let sources: [(&str, wgpu::BindingType); 6] = [
                ("inspectTextureCS", wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }),
                ("inspectTextureMsCS", wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: true }),
                ("inspectDepthMsCS", wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: true }),
                ("inspectDepthArrayCS", wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false }),
                ("inspectBufferCS", wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }),
                ("inspectVolumeCS", wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D3, multisampled: false }),
            ];
            for (entry, ty) in sources {
                let (_, binding) = [Shape::Texture { format: wgpu::TextureFormat::Rgba8Unorm, divisor: 1, samples: 1 }, Shape::Texture { format: wgpu::TextureFormat::Rgba8Unorm, divisor: 1, samples: 4 }, Shape::Texture { format: wgpu::TextureFormat::Depth32Float, divisor: 1, samples: 4 }, Shape::DepthArray { size: 1, layers: 1 }, Shape::UpdateBuffer, Shape::Volume { format: wgpu::TextureFormat::Rgba8Unorm, size: [1; 3], mips: 1 }]
                    .iter()
                    .map(inspect_entry)
                    .find(|(e, _)| *e == entry)
                    .expect("an entry per kind");
                let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some(entry),
                    entries: &[
                        wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                        wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::Rgba8Unorm, view_dimension: wgpu::TextureViewDimension::D2 }, count: None },
                        wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty, count: None },
                    ],
                });
                let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(entry), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
                let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(entry), layout: Some(&pipeline_layout), module: &module, entry_point: Some(entry), compilation_options: Default::default(), cache: None });
                self.inspect_pipelines.insert(entry, (pipeline, layout));
            }
        }
        let source = investigation::current().scene_shading();
        let graph = FrameGraph::new(source, &[]);
        let viewport = self.viewport.as_ref().map_or([self.width, self.height], |v| v.size);
        let scale = source.and_then(|s| self.shadings.iter().find(|x| std::ptr::eq(x.source, s))).map_or(1, |s| s.scale.max(1));
        for (name, key, show, params) in shown {
            let Ok((node, r)) = graph.inspected(&key) else { continue };
            let resource = graph.resources[r].clone();
            // The investigation's view of it (its parameters under the image's), unless the image names another visualizer.
            let (declared, mut defaults) = graph.view_of(r);
            let show = show.as_deref().map_or(declared, |s| Show::from_key(s).unwrap_or_else(|e| panic!("{e}")));
            if show != declared {
                defaults.clear();
            }
            defaults.extend(params);
            let params = defaults;
            let param = |k: &str, i: usize| params.get(k).and_then(|v| v.get(i)).copied();
            let default = |k: &str| crate::graph::INSPECT_PARAMS.iter().find(|p| p.0 == k).map_or(0.0, |p| p.2);
            let level = param("level", 0).unwrap_or(0.0).max(0.0) as u32;
            let axis = param("axis", 0).unwrap_or(default("axis")).round().clamp(0.0, 2.0) as u32;
            let update_words = |name: &str| {
                source.and_then(|s| s.update).and_then(|u| u.buffers.iter().find(|b| b.name == name)).map_or(1, |b| (b.words)().max(1))
            };
            // The source's size in texels (a buffer: the viewport's pixels, or its elements row by row).
            let src: [u32; 2] = match &resource.shape {
                Shape::Texture { divisor, .. } => {
                    let d = if *divisor == 0 { scale } else { *divisor };
                    [(viewport[0] / d).max(1), (viewport[1] / d).max(1)]
                }
                Shape::Buffer { .. } => viewport,
                Shape::UpdateBuffer => [256, (update_words(resource.name) as u32).div_ceil(256).max(1)],
                Shape::DepthArray { size, .. } => [*size, *size],
                Shape::Volume { size, mips, .. } => {
                    let m = level.min(mips.saturating_sub(1));
                    let s = size.map(|v| (v >> m).max(1));
                    match axis {
                        0 => [s[2], s[1]],
                        2 => [s[0], s[1]],
                        _ => [s[0], s[2]],
                    }
                }
            };
            let long = src[0].max(src[1]).max(1);
            let side = if card_image(&name) { DAG_SIDE } else { INSPECT_SIDE.min(long.max(256)) };
            let size = [(src[0] * side / long).max(1), (src[1] * side / long).max(1)];
            if self.inspections.get(&name).is_some_and(|x| x.size != size) {
                let x = self.inspections.remove(&name).expect("an inspection");
                self.egui_renderer.free_texture(&x.id);
                self.textures.remove(&view_texture(&name));
            }
            if !self.inspections.contains_key(&name) {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(&name),
                    size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let storage = texture.create_view(&Default::default());
                let id = self.egui_renderer.register_native_texture(device, &texture.create_view(&Default::default()), wgpu::FilterMode::Linear);
                let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some(&name), size: 80, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
                let (entry, binding) = inspect_entry(&resource.shape);
                self.textures.insert(view_texture(&name), (id, size));
                self.inspections.insert(name.clone(), Inspection { key: key.clone(), node, resource: resource.name, entry, binding, _texture: texture, storage, size, params: buffer, id });
            }
            let range = [param("range", 0).unwrap_or(show.range()[0]), param("range", 1).unwrap_or(show.range()[1])];
            let (tile, words, tiles_x, samples) = match &resource.shape {
                Shape::Buffer { words_per_tile, tile } => (*tile, *words_per_tile, viewport[0].div_ceil((*tile).max(1)), 1),
                Shape::UpdateBuffer => (0, 1, 0, 1),
                Shape::Texture { samples, .. } => (1, 0, 0, *samples),
                _ => (1, 0, 0, 1),
            };
            let u = |v: f32| v.max(0.0).round() as u32;
            let words_out: [u32; 20] = [
                show.mode(),
                u(param("channel", 0).unwrap_or(0.0)),
                u(param("encoding", 0).unwrap_or(0.0)),
                u(param("average", 0).unwrap_or(0.0)),
                range[0].to_bits(),
                range[1].to_bits(),
                param("exposure", 0).unwrap_or(0.0).to_bits(),
                param("slice", 0).unwrap_or(default("slice")).to_bits(),
                size[0],
                size[1],
                src[0],
                src[1],
                tile,
                words,
                u(param("word", 0).unwrap_or(0.0)),
                tiles_x,
                level,
                axis,
                samples,
                0,
            ];
            let x = &self.inspections[&name];
            self.gpu.queue.write_buffer(&x.params, 0, bytemuck::cast_slice(&words_out));
        }
    }

    /// Draws each graph image that names node `node` from its resource as it
    /// stands now (right after the node): packages/rlstd/shaders/inspect.wgsl.
    fn capture(&self, encoder: &mut wgpu::CommandEncoder, node: usize, chain: Option<&ChainTargets>, shading: Option<&Shading>, targets: &ViewportTargets) {
        for x in self.inspections.values().filter(|x| x.node == node) {
            let Some((pipeline, layout)) = self.inspect_pipelines.get(x.entry) else { continue };
            let volume_view;
            let resource: wgpu::BindingResource = match x.resource {
                "hdr" => wgpu::BindingResource::TextureView(&targets.hdr),
                "depth" => wgpu::BindingResource::TextureView(&targets.depth),
                "overlay" => wgpu::BindingResource::TextureView(&targets.overlay),
                name => {
                    let chained = chain.and_then(|c| {
                        if let Some((_, b)) = c.buffers.iter().find(|(n, _)| *n == name) {
                            return Some(b.as_entire_binding());
                        }
                        if let (Some(d), Some((all, _))) = (c.source.depth_views, &c.depth_views) {
                            if d.name == name {
                                return Some(wgpu::BindingResource::TextureView(all));
                            }
                        }
                        c.views.iter().find(|(n, _)| *n == name).map(|(_, v)| wgpu::BindingResource::TextureView(v))
                    });
                    match chained {
                        Some(r) => r,
                        None => {
                            let Some(s) = shading else { continue };
                            if let Some(k) = s.source.update.and_then(|u| u.buffers.iter().position(|b| b.name == name)) {
                                s.update_buffers[k].as_entire_binding()
                            } else if let (Some(v), Some(set)) = (s.source.volumes.iter().position(|v| v.name == name), s.volume_sets.get(s.volume_set)) {
                                volume_view = set.textures[v].create_view(&Default::default());
                                wgpu::BindingResource::TextureView(&volume_view)
                            } else {
                                continue;
                            }
                        }
                    }
                }
            };
            let group = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("inspect"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: x.params.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&x.storage) },
                    wgpu::BindGroupEntry { binding: x.binding, resource },
                ],
            });
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("inspect"), timestamp_writes: None });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(x.size[0].div_ceil(8), x.size[1].div_ceil(8), 1);
        }
    }

    /// The graph images shown: their names and `<node>.<resource>` keys.
    pub fn inspected(&self) -> Vec<(String, String)> {
        self.inspections.iter().filter(|(n, _)| !card_image(n)).map(|(n, x)| (n.clone(), x.key.clone())).collect()
    }

    /// A graph image's texture read back (RGBA8, its size).
    pub fn read_inspection(&self, name: &str) -> Option<(Vec<u8>, [u32; 2])> {
        let x = self.inspections.get(name)?;
        let device = &self.gpu.device;
        let buffer = crate::readback::readback_buffer(device, x.size[0], x.size[1]);
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &x._texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(crate::readback::padded_row_bytes(x.size[0])), rows_per_image: Some(x.size[1]) } },
            wgpu::Extent3d { width: x.size[0], height: x.size[1], depth_or_array_layers: 1 },
        );
        self.gpu.queue.submit([encoder.finish()]);
        let size = buffer.size();
        let data = crate::readback::read_buffer(device, &self.gpu.queue, &buffer, 0, size);
        Some((crate::readback::unpad_rows(&data, x.size[0], x.size[1]), x.size))
    }

    /// The current frame's graph (src/graph.rs), kept alive for what the
    /// image kinds read: compiled again when the shading or those change.
    fn frame_graph(&mut self) -> std::sync::Arc<crate::graph::FrameGraph> {
        let source = investigation::current().scene_shading();
        let mut sinks: Vec<String> = self.views.values().flat_map(|v| v.kind.reads.iter().cloned()).collect();
        sinks.extend(self.inspections.iter().filter(|(n, _)| !card_image(n)).map(|(_, x)| x.resource.to_string()));
        sinks.sort();
        sinks.dedup();
        let key = (source.map_or(0, |s| s as *const SceneShading as usize), sinks);
        if let Some((k, graph)) = &self.graph {
            if *k == key {
                return graph.clone();
            }
        }
        let names: Vec<&str> = key.1.iter().map(String::as_str).collect();
        let graph = std::sync::Arc::new(crate::graph::FrameGraph::new(source, &names));
        if !graph.errors.is_empty() {
            panic!("the frame's graph does not compile (render-lab graph):\n  {}", graph.errors.join("\n  "));
        }
        self.graph = Some((key, graph.clone()));
        graph
    }

    /// Encode a frame's scene in its graph's order (src/graph.rs): the
    /// update passes (their plan's dispatches), then each live node: the
    /// depth views, the pre-pass, the chain's compute passes, the scene pass,
    /// and the overlay. The composite is the caller's.
    fn encode_scene(&mut self, encoder: &mut wgpu::CommandEncoder, content: &FrameContent, timed: bool) {
        self.shading();
        let graph = self.frame_graph();
        let mut timing = if timed { self.timing.take() } else { None };
        // The update passes run first, in their plan's order (they read nothing this frame writes).
        self.encode_updates(encoder, timing.as_mut());
        let shading = investigation::current().scene_shading().and_then(|source| self.shadings.iter().find(|s| std::ptr::eq(s.source, source)));
        let targets = self.viewport.as_ref().unwrap();
        // An investigation's pass chain: its targets follow the viewport.
        let chained = shading.filter(|s| !s.source.outputs.is_empty());
        if let Some(s) = chained {
            if self.chain.as_ref().is_none_or(|c| c.size != targets.size || c.scale != s.scale || !std::ptr::eq(c.source, s.source)) {
                self.chain = Some(ChainTargets::new(&self.gpu.device, s, targets));
            }
        }
        let chain = chained.and(self.chain.as_ref());
        let color_attachments: Vec<Option<wgpu::RenderPassColorAttachment>> = match chain {
            // The chain's targets start at zero: its passes decide what the background is.
            Some(c) => c
                .source
                .outputs
                .iter()
                .map(|name| Some(wgpu::RenderPassColorAttachment { view: c.view(name), depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } }))
                .collect(),
            None => vec![Some(wgpu::RenderPassColorAttachment { view: &targets.hdr_msaa, depth_slice: None, resolve_target: Some(&targets.hdr), ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Discard } })],
        };
        let prepassed = chain.is_some() && chained.is_some_and(|s| s.pipelines.prepass.is_some());
        // The chain's compute passes waiting to be encoded: consecutive ones share a compute pass.
        let mut pending: Vec<usize> = Vec::new();
        for &n in &graph.order {
            let node = &graph.nodes[n];
            if !graph.live[n] {
                continue;
            }
            // The inspector draws what a node wrote right after it, in the main view only.
            let inspected = timed && self.inspections.values().any(|x| x.node == n);
            if let Step::Compute(i) = node.step {
                pending.push(i);
                if inspected {
                    if let (Some(s), Some(c)) = (chained, chain) {
                        c.dispatch(encoder, &self.frame_bind_group, s, &std::mem::take(&mut pending), timing.as_mut());
                    }
                    self.capture(encoder, n, chain, shading, targets);
                }
                continue;
            }
            if let (Some(s), Some(c)) = (chained, chain) {
                c.dispatch(encoder, &self.frame_bind_group, s, &std::mem::take(&mut pending), timing.as_mut());
            }
            match node.step {
                Step::Update(_) | Step::Compute(_) | Step::Composite => {}
                Step::DepthViews => {
                    let (Some(s), Some(c)) = (chained, chain) else { continue };
                    // The depth views (shadow maps), one layer at a time, when what
                    // they see (the objects, the light, the shading values) changed.
                    let key = depth_views_key(content);
                    if let Some((_, layers)) = c.depth_views.as_ref().filter(|_| c.depth_views_drawn.replace(Some(key)) != Some(key)) {
                        let span = timing.as_mut().and_then(|t| t.render_writes("depth views"));
                        let last = layers.len().saturating_sub(1);
                        for (i, (pipeline, layer)) in s.pipelines.depth_views.iter().zip(layers).enumerate() {
                            let writes = span.as_ref().filter(|_| i == 0 || i == last).map(|t| t.span(i == 0, i == last));
                            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("depth view"),
                                color_attachments: &[],
                                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: layer, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                                timestamp_writes: writes,
                                occlusion_query_set: None,
                                multiview_mask: None,
                            });
                            pass.set_pipeline(pipeline);
                            pass.set_bind_group(0, &self.frame_bind_group, &[]);
                            pass.set_bind_group(1, &s.group, &[]);
                            pass.set_vertex_buffer(0, self.mesh_vertices.slice(..));
                            pass.set_vertex_buffer(1, self.object_ids.slice(..));
                            pass.set_index_buffer(self.mesh_indices.slice(..), wgpu::IndexFormat::Uint32);
                            for (i, range) in self.statics.meshes.ranges.iter().enumerate() {
                                pass.draw_indexed(range.clone(), 0, i as u32..i as u32 + 1);
                            }
                        }
                    }
                }
                Step::Prepass => {
                    let (Some(s), Some(c)) = (chained, chain) else { continue };
                    if let Some(prepass) = &s.pipelines.prepass {
                        let span = timing.as_mut().and_then(|t| t.render_writes("depth pre-pass"));
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("depth pre-pass"),
                            color_attachments: &[],
                            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                                view: &targets.depth,
                                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                                stencil_ops: None,
                            }),
                            timestamp_writes: span.as_ref().map(|t| t.writes()),
                            occlusion_query_set: None,
                            multiview_mask: None,
                        });
                        pass.set_pipeline(prepass);
                        pass.set_bind_group(0, &self.frame_bind_group, &[]);
                        pass.set_bind_group(1, &s.group, &[]);
                        pass.set_bind_group(2, &c.scene_group, &[]);
                        pass.set_vertex_buffer(0, self.mesh_vertices.slice(..));
                        pass.set_vertex_buffer(1, self.object_ids.slice(..));
                        pass.set_index_buffer(self.mesh_indices.slice(..), wgpu::IndexFormat::Uint32);
                        for (i, range) in self.statics.meshes.ranges.iter().enumerate() {
                            pass.draw_indexed(range.clone(), 0, i as u32..i as u32 + 1);
                        }
                    }
                }
                Step::Scene => {
                    let span = timing.as_mut().and_then(|t| t.render_writes("scene pass"));
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("scene"),
                        color_attachments: &color_attachments,
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &targets.depth,
                            depth_ops: Some(wgpu::Operations { load: if prepassed { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(0.0) }, store: wgpu::StoreOp::Store }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: span.as_ref().map(|t| t.writes()),
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_bind_group(0, &self.frame_bind_group, &[]);
                    // Without a chain, the sky is the background; a chain's passes draw their own.
                    if chain.is_none() {
                        pass.set_pipeline(&self.pipelines.sky);
                        pass.draw(0..3, 0..1);
                    }
                    pass.set_pipeline(shading.map_or(&self.pipelines.scene, |s| &s.pipelines.scene));
                    if let Some(s) = shading {
                        pass.set_bind_group(1, &s.group, &[]);
                        if let Some(c) = chain {
                            pass.set_bind_group(2, &c.scene_group, &[]);
                        }
                    }
                    pass.set_vertex_buffer(0, self.mesh_vertices.slice(..));
                    pass.set_vertex_buffer(1, self.object_ids.slice(..));
                    pass.set_index_buffer(self.mesh_indices.slice(..), wgpu::IndexFormat::Uint32);
                    for (i, range) in self.statics.meshes.ranges.iter().enumerate() {
                        pass.draw_indexed(range.clone(), 0, i as u32..i as u32 + 1);
                    }
                    drop(pass);
                }
                Step::Overlay => {
                    let span = timing.as_mut().and_then(|t| t.render_writes("overlay"));

                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("overlay"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &targets.overlay_msaa,
                            depth_slice: None,
                            resolve_target: Some(&targets.overlay),
                            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Discard },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &targets.depth,
                            // Kept when an image kind reads the chain's depth after this pass (src/views.rs).
                            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: if self.views.values().any(|v| !v.kind.reads.is_empty()) { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard } }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: span.as_ref().map(|t| t.writes()),
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_bind_group(0, &self.frame_bind_group, &[]);
                    let draw = |pass: &mut wgpu::RenderPass, pipeline: &wgpu::RenderPipeline, buffer: &GrowBuffer, vertices: u32| {
                        if buffer.count > 0 {
                            pass.set_pipeline(pipeline);
                            pass.set_vertex_buffer(0, buffer.buffer.slice(..));
                            pass.draw(0..vertices, 0..buffer.count);
                        }
                    };
                    if self.ghost_ids.count > 0 {
                        pass.set_pipeline(&self.pipelines.ghost);
                        pass.set_vertex_buffer(0, self.mesh_vertices.slice(..));
                        pass.set_vertex_buffer(1, self.object_ids.slice(..));
                        pass.set_index_buffer(self.mesh_indices.slice(..), wgpu::IndexFormat::Uint32);
                        for &object in &content.draw.ghosts {
                            let range = self.statics.meshes.ranges[object as usize].clone();
                            pass.draw_indexed(range, 0, object..object + 1);
                        }
                    }
                    draw(&mut pass, &self.pipelines.boxes_xray, &self.boxes_xray, 108);
                    draw(&mut pass, &self.pipelines.segment_xray, &self.segments_xray, 9);
                    draw(&mut pass, &self.pipelines.boxes, &self.boxes, 108);
                    draw(&mut pass, &self.pipelines.segment, &self.segments, 9);
                    draw(&mut pass, &self.pipelines.point, &self.points, 6);
                }
            }
            if inspected {
                self.capture(encoder, n, chain, shading, targets);
            }
        }
        if let (Some(s), Some(c)) = (chained, chain) {
            c.dispatch(encoder, &self.frame_bind_group, s, &pending, timing.as_mut());
        }
        self.timing = timing.or(self.timing.take());
    }

    /// A research run's frame (src/research.rs): `state` filling the frame
    /// target with no chrome, building on the frames before it as the viewer's
    /// still frames do (`FrameRequest::history`), its scene data built first,
    /// its passes timed while timing is on (`set_timing`).
    pub fn render_experiment(&mut self, state: &ShotState) {
        let mut layout = Layout::new(self.width, self.height);
        layout.viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, layout.canvas);
        self.last_layout = layout;
        let vp = layout.viewport_px();
        let size = [vp[2], vp[3]];
        if self.viewport.as_ref().is_none_or(|v| v.size != size) {
            self.chain = None;
            self.viewport = Some(ViewportTargets::new(&self.gpu.device, &self.pipelines, size));
        }
        self.switch_scene(investigation::current().scene(state));
        self.select_data(state, true);
        let mut content = crate::shader_data::build_frame(state, &self.statics, [vp[2] as f32, vp[3] as f32], layout.scale, 0.0);
        let count = |last: (Option<u64>, u32), key: u64| if last.0 == Some(key) { (Some(key), last.1.saturating_add(1)) } else { (Some(key), 0) };
        self.still = count(self.still, still_key(&content, size, true));
        self.still_scene = count(self.still_scene, still_key(&content, size, false));
        content.uniforms.view_params[3] = (1 + self.still.1.min(255) + 256 * self.still_scene.1.min(32767)) as f32;
        self.upload(&content);
        self.upload_draws(&content);
        // An experiment that measures a resource of the frame's graph (`Experiment::inspect`).
        self.prepare_inspections(state, &[]);
        let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("experiment") });
        self.encode_scene(&mut encoder, &content, true);
        let timed = self.timing.as_mut().and_then(|t| t.render_writes("composite"));
        self.composite_timed(&mut encoder, vp, true, timed.as_ref().map(|t| t.writes()));
        if let Some(t) = self.timing.as_mut() {
            t.resolve(&mut encoder);
        }
        self.gpu.queue.submit([encoder.finish()]);
        if let Some(t) = self.timing.as_mut() {
            t.submitted(&self.gpu.device);
        }
    }

    /// The next frame starts a new history (a research run's next variant).
    pub fn reset_history(&mut self) {
        self.still = (None, 0);
        self.still_scene = (None, 0);
    }

    /// The scene of `state` filling the whole frame target, without any chrome
    /// (the homepage's backdrop); with no state, only the background colour.
    pub fn render_backdrop(&mut self, state: Option<&ShotState>, time: f32) {
        let mut layout = Layout::new(self.width, self.height);
        layout.viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, layout.canvas);
        self.last_layout = layout;
        self.last_panes.clear();
        let vp = layout.viewport_px();
        if self.viewport.as_ref().is_none_or(|v| v.size != [vp[2], vp[3]]) {
            self.chain = None;
            self.viewport = Some(ViewportTargets::new(&self.gpu.device, &self.pipelines, [vp[2], vp[3]]));
        }
        if let Some(s) = state {
            self.switch_scene(investigation::current().scene(s));
        }
        let content = state.map(|s| crate::shader_data::build_frame(s, &self.statics, [vp[2] as f32, vp[3] as f32], layout.scale, time));
        if let Some(content) = &content {
            self.upload(content);
            self.upload_draws(content);
        }
        let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("backdrop") });
        if let Some(content) = &content {
            self.encode_scene(&mut encoder, content, false);
        }
        {
            let bg = crate::palette::background();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("backdrop composite"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.frame_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: bg.r() as f64 / 255.0, g: bg.g() as f64 / 255.0, b: bg.b() as f64 / 255.0, a: 1.0 }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if content.is_some() {
                let targets = self.viewport.as_ref().unwrap();
                pass.set_pipeline(&self.pipelines.composite);
                pass.set_bind_group(0, &self.frame_bind_group, &[]);
                pass.set_bind_group(1, &targets.composite, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        self.gpu.queue.submit([encoder.finish()]);
    }

    /// Run a verification kernel of the current investigation's scene shading
    /// (`SceneShading::verify`) with `count` workgroups and read `words` back;
    /// the shading's uniforms are those of the last uploaded frame.
    pub fn run_shading_verify(&mut self, entry: &str, count: u32, tests: &[[f32; 4]], words: usize) -> Vec<u32> {
        self.shading().expect("the current investigation has no scene shading");
        let source = investigation::current().scene_shading().unwrap();
        let shading = self.shadings.iter().find(|s| std::ptr::eq(s.source, source)).unwrap();
        let device = &self.gpu.device;
        let out = device.create_buffer(&wgpu::BufferDescriptor { label: Some("verify out"), size: (words.max(1) * 4) as u64, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
        let test_data: Vec<[f32; 4]> = if tests.is_empty() { vec![[0.0; 4]] } else { tests.to_vec() };
        let input = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("verify tests"), contents: bytemuck::cast_slice(&test_data), usage: wgpu::BufferUsages::STORAGE });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verify"),
            layout: &self.pipelines.verify_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: out.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: input.as_entire_binding() }],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&shading.pipelines.verify[entry]);
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            pass.set_bind_group(1, &shading.group, &[]);
            pass.set_bind_group(2, &bind, &[]);
            if let Some(v) = shading.volume_sets.get(shading.volume_set) {
                pass.set_bind_group(3, &v.group, &[]);
            }
            pass.dispatch_workgroups(count, 1, 1);
        }
        self.gpu.queue.submit([encoder.finish()]);
        read_buffer(device, &self.gpu.queue, &out, 0, (words * 4) as u64).chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
    }

    /// A buffer of the current shading's pass chain as the last frame left it
    /// (selftests), with the viewport size it was made for.
    pub fn read_chain_buffer(&self, name: &str) -> Option<(Vec<u32>, [u32; 2])> {
        let chain = self.chain.as_ref()?;
        let (_, buffer) = chain.buffers.iter().find(|(n, _)| *n == name)?;
        let bytes = read_buffer(&self.gpu.device, &self.gpu.queue, buffer, 0, buffer.size());
        Some((bytes.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect(), chain.size))
    }

    /// Blocking read of the current frame (unpadded RGBA rows).
    pub fn read_frame(&self) -> Vec<u8> {
        let device = &self.gpu.device;
        let padded = padded_row_bytes(self.width);
        let buffer = readback_buffer(device, self.width, self.height);
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &self.frame, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(self.height) } },
            wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
        );
        self.gpu.queue.submit([encoder.finish()]);
        let data = read_buffer(device, &self.gpu.queue, &buffer, 0, padded as u64 * self.height as u64);
        unpad_rows(&data, self.width, self.height)
    }

}

pub fn gpu_summary(gpu: &gpu::Gpu) -> serde_json::Value {
    serde_json::json!({
        "name": gpu.info.name,
        "backend": format!("{:?}", gpu.info.backend),
        "driver": gpu.info.driver,
        "driverInfo": gpu.info.driver_info,
        "deviceType": format!("{:?}", gpu.info.device_type),
    })
}

/// An image kind's compute pipeline (src/views.rs): the frame and the scene
/// shading at groups 0 and 1, the chain reads at group 2 when it reads any,
/// and its texture and parameters at group 3.
fn view_pipeline(device: &wgpu::Device, frame: &wgpu::BindGroupLayout, shading: &wgpu::BindGroupLayout, chain: Option<&wgpu::BindGroupLayout>, view: &wgpu::BindGroupLayout, kind: &crate::views::ViewKind) -> wgpu::ComputePipeline {
    let source = crate::shaders::module(&kind.module).unwrap_or_else(|e| panic!("{e:#}"));
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(&kind.module), source: wgpu::ShaderSource::Wgsl(source.into()) });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(&kind.entry), bind_group_layouts: &[Some(frame), Some(shading), chain, Some(view)], immediate_size: 0 });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(&kind.entry), layout: Some(&layout), module: &module, entry_point: Some(&kind.entry), compilation_options: Default::default(), cache: None })
}
