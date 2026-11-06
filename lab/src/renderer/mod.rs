//! The renderer shared by the viewer and the recorder. Each frame is drawn
//! into one RGBA8 offscreen target: the 3D scene (MSAA, HDR), the overlay
//! layer, the composite into the viewport rectangle, then egui on top. The
//! viewer blits that target to its window; the recorder reads it back.

use crate::view::ViewId;
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

mod targets;
mod shading;
mod inspect;
mod views;
mod reads;
mod encode;
pub use targets::*;
use shading::*;
use inspect::*;
use views::*;

pub struct Renderer {
    /// The last frame's `still_key` and `scene_key`, and how many frames in a row had each.
    still: (Option<u64>, u32),
    still_scene: (Option<u64>, u32),
    pub gpu: Gpu,
    pub pipelines: Pipelines,
    /// The prepared data of the scene the renderer shows.
    pub statics: Statics,
    /// The scenes not being drawn, each with its mesh buffers (so a view can show its own scene, src/view.rs).
    other_scenes: HashMap<SceneId, (Statics, SceneResources)>,
    /// The scene each view was last drawn with: its data is dropped when that changes.
    view_scenes: std::collections::BTreeMap<ViewId, SceneId>,
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
    cuboids: GrowBuffer,
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
    /// The GPU textures an inset's picture can show, by key (packages/render_lab/src/inset.rs).
    textures: Textures,
    /// The story's image instances (src/views.rs) by name, and each kind's pipeline.
    views: HashMap<String, ViewInstance>,
    view_pipelines: HashMap<(String, String), wgpu::ComputePipeline>,
    /// The group-2 layouts of the image kinds that read the pass chain.
    view_chain_layouts: HashMap<(String, String), wgpu::BindGroupLayout>,
    view_layout: wgpu::BindGroupLayout,
    /// Group 1 of a kind's entry point when the investigation has no scene shading.
    view_shading: Option<wgpu::BindGroup>,
    /// The frame's graph (src/graph/) and what it was compiled for: the
    /// shading (by address) and the resources image kinds read.
    graph: Option<(GraphKey, std::sync::Arc<crate::graph::FrameGraph>)>,
    /// The graph images shown (`ImageSource::Graph`) by name, and the
    /// inspector's pipeline and group layout for each kind of resource.
    inspections: HashMap<String, Inspection>,
    inspect_pipelines: HashMap<&'static str, (wgpu::ComputePipeline, wgpu::BindGroupLayout)>,
    last_layout: Layout,
    /// The last frame's panel had nothing to show (no record, readout, or
    /// listing), so this frame first lays out without it (`render_frame_with`).
    panel_empty: bool,
    /// The views not being drawn, each its slot (src/view.rs); `active` is the one being drawn, its slot in the fields above.
    slots: std::collections::BTreeMap<ViewId, ViewSlot>,
    active: ViewId,
    /// The free camera's view (src/free_view.rs), while it is on.
    /// Where the last frame drew the main camera's own view while the free
    /// camera was on: a corner of the viewport, in canvas points.
    pub last_main_view: Option<egui::Rect>,
    /// The last frame's views and where each is on screen (canvas points), back to front (src/view.rs).
    pub last_views: Vec<(ViewId, egui::Rect)>,
    /// The last frame's views stacked over the main one: each its id, name, and camera (src/view.rs).
    pub last_overs: Vec<(ViewId, &'static str, crate::camera::OrbitCamera)>,
    compare: Option<CompareRequest>,
    /// The difference view's gain (group 2 of `composite_diff`).
    compare_params: wgpu::Buffer,
    compare_group: wgpu::BindGroup,
    /// The last frame drew the reference (its view can be read back).
    reference_drawn: bool,
    /// The sizes the viewer's user dragged (src/config.rs); the default for a recording.
    pub user_layout: crate::config::ViewerLayout,
    /// A recording's code panel width for each listing and language (ui::code_panel_fit).
    code_fit: HashMap<(&'static str, crate::i18n::Locale, crate::pseudocode::Style), f32>,
    /// The pointer's cursor the last frame's interface asked for (egui's).
    pub cursor: egui::CursorIcon,
    /// When each recent frame started rendering, for the performance panel's frame times.
    frame_starts: std::collections::VecDeque<std::time::Instant>,
    /// The GPU reads the frames ask for, in flight (src/fetch.rs).
    fetches: crate::fetch::Queue,
    /// How deep the redraws of a frame drawn alone, waiting for its reads, are.
    settling: u8,
    /// GPU timestamps around the frame's passes (src/timing.rs), while an
    /// experiment asks for them (`set_timing`) and the device has them.
    pub timing: Option<crate::timing::Timing>,
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
    /// Whether the frame waits for its data and its read-backs, so it is the
    /// same each time (a recording's): without history it reads its own
    /// frame's, with history the frame before it's, waited for.
    pub wait: bool,
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
    /// The focused view's state when it is not the main view (src/view.rs): the dock shows and sets it.
    pub focus: Option<&'a ShotState>,
    /// Draw this frame index as a binary stamp (frame-order verification).
    pub stamp: Option<u64>,
}

pub struct FrameResult {
    pub report: UiReport,
    /// The content on screen: the free camera's while it is on, else the main view's.
    pub content: Option<FrameContent>,
    /// The main view's content, when it is not the one on screen (the free camera is on).
    pub main_content: Option<FrameContent>,
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
            cuboids: GrowBuffer::new(device, "cuboids", vertex, 1 << 16),
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
            panel_empty: false,
            slots: std::collections::BTreeMap::new(),
            active: ViewId::MAIN,
            last_main_view: None,
            last_views: Vec::new(),
            last_overs: Vec::new(),
            compare: None,
            compare_group: device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("compare"), layout: &pipelines_compare_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: compare_params.as_entire_binding() }] }),
            compare_params,
            reference_drawn: false,
            user_layout: Default::default(),
            code_fit: HashMap::new(),
            cursor: egui::CursorIcon::Default,
            frame_starts: std::collections::VecDeque::new(),
            fetches: crate::fetch::Queue::default(),
            settling: 0,
            timing: None,
            statics,
            other_scenes: HashMap::new(),
            view_scenes: std::collections::BTreeMap::new(),
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
        let (next, scene) = match self.other_scenes.remove(&id) {
            Some(kept) => kept,
            None => {
                let statics = Statics::prepare(id).unwrap_or_else(|e| panic!("preparing scene {}: {e:#}", id.key()));
                let scene = Self::scene_resources(&self.gpu.device, &statics);
                (statics, scene)
            }
        };
        let previous = std::mem::replace(&mut self.statics, next);
        let resources = SceneResources {
            mesh_vertices: std::mem::replace(&mut self.mesh_vertices, scene.mesh_vertices),
            mesh_indices: std::mem::replace(&mut self.mesh_indices, scene.mesh_indices),
            object_ids: std::mem::replace(&mut self.object_ids, scene.object_ids),
        };
        self.other_scenes.insert(previous.scene.id, (previous, resources));
    }

    /// The view being drawn shows scene `id`: its own data is dropped when its scene changed.
    fn view_scene(&mut self, id: SceneId) {
        self.switch_scene(id);
        let view = self.active;
        if self.view_scenes.insert(view, id).is_some_and(|last| last != id) {
            crate::fetch::clear_view(view);
        }
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

    /// The last frame's views and their places on screen, back to front (src/view.rs);
    /// the viewer orbits and picks from them too.
    pub fn frame_views(&self) -> &[(ViewId, egui::Rect)] {
        &self.last_views
    }

    /// `dock`: the interactive viewer's layout with the slider dock.
    /// With `values`, the panel shows (the Experiments tab's values) whatever the state's panel mode.
    /// `code_width`: the code panel's width when a listing shows (a recording's, fit to it).
    pub fn layout_for(&self, state: &ShotState, scene_type: SceneType, dock: bool, values: bool, code_width: Option<f32>) -> Layout {
        let panel = scene_type != SceneType::ThreeD || state.panel != crate::shot::PanelMode::Hidden || values;
        let mut layout = self.layout();
        if dock {
            layout = layout.with_dock();
        }
        // The pseudo-code and the knowledge graph take the wider panel.
        let code = scene_type == SceneType::ThreeD && matches!(state.panel, crate::shot::PanelMode::Code | crate::shot::PanelMode::Knowledge | crate::shot::PanelMode::Dag | crate::shot::PanelMode::Resources);
        layout = match code_width {
            Some(width) if code && state.panel == crate::shot::PanelMode::Code => layout.with_panel_width(width),
            _ => layout.with_user(&self.user_layout, dock, code),
        };
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
        self.slots.clear();
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
            self.slots.remove(&ViewId::REFERENCE);
        }
        self.compare = compare;
    }

    /// The free view's slot and the renderer's trade places.
    /// Makes `view` the one being drawn: the drawn view's slot goes back to
    /// `slots`, and `view`'s (a new one the first time) into the renderer's
    /// fields: its targets, chain, stillness, and programs.
    fn enter(&mut self, view: ViewId) {
        crate::view::set_current(view);
        if self.active == view {
            return;
        }
        let mut slot = self.slots.remove(&view).unwrap_or_default();
        self.trade(&mut slot);
        self.slots.insert(self.active, slot);
        self.active = view;
    }

    /// The renderer's per-view fields and `slot`'s trade places.
    fn trade(&mut self, slot: &mut ViewSlot) {
        std::mem::swap(&mut self.viewport, &mut slot.viewport);
        std::mem::swap(&mut self.chain, &mut slot.chain);
        std::mem::swap(&mut self.still, &mut slot.still);
        std::mem::swap(&mut self.still_scene, &mut slot.still_scene);
        let device = &self.gpu.device;
        let mut kept = Vec::new();
        for s in self.shadings.iter_mut() {
            let (Some(program), Some(pipelines)) = (s.source.program, s.pipelines.program.as_ref()) else { continue };
            let id = s.source as *const SceneShading as usize;
            // The slot's runtime for this shading, or a new one (made for this view's size when it binds).
            let incoming = match slot.programs.iter().position(|(k, _, _)| *k == id) {
                Some(i) => {
                    let (_, runtime, handles) = slot.programs.swap_remove(i);
                    (runtime, handles)
                }
                None => {
                    let runtime = std::cell::RefCell::new(crate::program::ProgramRuntime::new(device, s.source, program, pipelines));
                    let handles = std::cell::RefCell::new(runtime.borrow().handles());
                    (runtime, handles)
                }
            };
            let runtime = s.program.replace(incoming.0).expect("a shading with a program has its runtime");
            let handles = std::mem::replace(&mut s.program_handles, incoming.1);
            kept.push((id, runtime, handles));
        }
        slot.programs.extend(kept);
    }

    /// Counts how long the view being drawn has held still and writes it into
    /// `content`'s `viewParams.w` (`FrameRequest::history`; 0 without history).
    fn hold_still(&mut self, content: &mut FrameContent, viewport: [u32; 2], history: bool) {
        let count = |last: (Option<u64>, u32), key: u64| if last.0 == Some(key) { (Some(key), last.1.saturating_add(1)) } else { (Some(key), 0) };
        self.still = count(self.still, still_key(content, viewport, true));
        self.still_scene = count(self.still_scene, still_key(content, viewport, false));
        content.uniforms.view_params[3] = if history { (1 + self.still.1.min(255) + 256 * self.still_scene.1.min(32767)) as f32 } else { 0.0 };
    }

    /// The view being drawn's targets at `size` (made again, its chain with them, when the size changed).
    fn targets_at(&mut self, size: [u32; 2]) {
        if self.viewport.as_ref().is_none_or(|v| v.size != size) {
            self.chain = None;
            self.viewport = Some(ViewportTargets::new(&self.gpu.device, &self.pipelines, size));
        }
    }

    /// A view other than the main one, drawn in a submission of its own from
    /// its own slot: built (or `content` as given), planned, drawn, and, with
    /// `at`, composited there.
    fn draw_view(&mut self, view: ViewId, state: &ShotState, content: Option<FrameContent>, size: [u32; 2], frame: ViewFrame, at: Option<([u32; 4], bool)>) -> FrameContent {
        let ViewFrame { scale, time, history, wait } = frame;
        let main_scene = self.statics.scene.id;
        self.enter(view);
        // Its own scene (src/view.rs).
        self.view_scene(investigation::current().scene(state));
        self.targets_at(size);
        self.select_data(state, wait || !history);
        let mut content = content.unwrap_or_else(|| crate::shader_data::build_frame(state, &self.statics, [size[0] as f32, size[1] as f32], scale, time));
        self.hold_still(&mut content, size, history);
        self.plan_program(state, history, size);
        self.upload(&content);
        self.upload_draws(&content);
        let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("view") });
        self.encode_scene(&mut encoder, &content, false);
        // The reads it asks for, of what its own passes left (src/fetch.rs).
        if !content.fetches.is_empty() {
            let tag = still_key(&content, size, true);
            let mut queue = std::mem::take(&mut self.fetches);
            queue.issue(&self.gpu.device, &mut encoder, &content.fetches, tag, crate::fetch::BUDGET, &|source| self.fetch_target(source));
            self.fetches = queue;
        }
        if let Some((at, clear)) = at {
            self.composite(&mut encoder, at, clear);
        }
        self.gpu.queue.submit([encoder.finish()]);
        self.fetches.submitted();
        self.enter(ViewId::MAIN);
        self.switch_scene(main_scene);
        content
    }

    fn upload_draws(&mut self, content: &FrameContent) {
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        let d = &content.draw;
        // The x-ray copies draw occluded parts dimmed and dashed.
        let dim = |a: f32| a * 0.3;
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
        self.cuboids.write(device, queue, &d.cuboids);
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
        self.enter(ViewId::MAIN);
        self.view_scene(investigation::current().scene(request.state));
        // The reads earlier frames asked for that have landed (src/fetch.rs);
        // a recording's frame with history waits for the last frame's.
        self.fetches.collect(&self.gpu.device, request.wait && request.history);
        // The performance panel measures the frames it is shown with.
        if matches!(request.state.panel, crate::shot::PanelMode::Performance | crate::shot::PanelMode::Graph | crate::shot::PanelMode::Dag | crate::shot::PanelMode::Resources) && request.scene_type == SceneType::ThreeD {
            self.set_timing(true);
        }
        self.frame_starts.push_back(std::time::Instant::now());
        if self.frame_starts.len() > 121 {
            self.frame_starts.pop_front();
        }
        let dock = matches!(&request.chrome, Chrome::Viewer(v) if !(v.timeline_mode && v.playing));
        let values = dock && matches!(&request.chrome, Chrome::Viewer(v) if matches!(v.tab, ui::DockTab::Experiments | ui::DockTab::Quiz));
        // A recording's code panel is as wide as its listing needs.
        let listing = match &request.chrome {
            Chrome::Recording { .. } if request.state.panel == crate::shot::PanelMode::Code => request.state.code.as_deref().and_then(|c| investigation::current().listing(c)),
            _ => None,
        };
        let code_width = listing.map(|listing| {
            let key = (listing.name, crate::i18n::current(), crate::pseudocode::style());
            if let Some(width) = self.code_fit.get(&key) {
                return *width;
            }
            // The fonts are there from the first pass on.
            if self.egui_ctx.cumulative_pass_nr() == 0 {
                let mut warm = self.egui_ctx.run_pass(egui::RawInput::default(), |_| {});
                // Its texture changes (the font atlas) are uploaded like any pass's.
                for (id, deltas) in warm.textures_delta.set.drain() {
                    for delta in &deltas {
                        self.egui_renderer.update_texture(&self.gpu.device, &self.gpu.queue, id, delta);
                    }
                }
                for id in warm.textures_delta.free.drain() {
                    self.egui_renderer.free_texture(&id);
                }
            }
            let width = ui::code_panel_fit(&self.egui_ctx, listing);
            self.code_fit.insert(key, width);
            width
        });
        let mut layout = self.layout_for(request.state, request.scene_type, dock, values, code_width);
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
                    self.select_data(request.state, request.wait || !request.history);
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
        let vp = layout.viewport_px();
        let viewport_size = [vp[2], vp[3]];
        let frame = ViewFrame { scale: layout.scale, time: request.time, history: request.history, wait: request.wait };
        self.targets_at(viewport_size);
        let size = [vp[2] as f32, vp[3] as f32];
        self.reference_drawn = false;
        if let Some(c) = &compare {
            // The reference first, from its own slot (src/view.rs), so its history builds up as the live view's does.
            self.draw_view(ViewId::REFERENCE, &c.reference, None, viewport_size, frame, None);
            self.reference_drawn = true;
        }
        // Each other view of a split area: from its own slot, composited into its rectangle.
        let mut drawn = Vec::new();
        for (n, (pane, rect)) in panes.iter().zip(&rects).enumerate() {
            let at = Layout { viewport: *rect, ..layout }.viewport_px();
            let content = self.draw_view(ViewId::pane(n as u32), &pane.state, None, [at[2], at[3]], frame, Some((at, drawn.is_empty())));
            drawn.push((*rect, content));
        }
        for n in panes.len() as u32..8 {
            self.slots.remove(&ViewId::pane(n));
        }
        let scene_state = main.as_ref().map_or(request.state, |m| &m.state);
        // The frame graph scene (a 2D beat's `graph`) renders the frame too, unseen, for its lit boxes' pictures.
        let boxes: Vec<(String, String)> = request.diagram.filter(|_| !scene_beat).map(|d| d.lit_groups()).unwrap_or_default();
        let renders = scene_beat || !boxes.is_empty();
        // A recording waits for the data, so every frame is the same each time.
        if renders {
            self.select_data(scene_state, request.wait || !request.history);
            self.plan_program(scene_state, request.history, viewport_size);
        }
        let settle_prepared = if request.history { None } else { prepared.clone() };
        let given = prepared.is_some();
        let panel_built = built.is_some();
        let mut content = match prepared.or(built) {
            Some(c) => Some(c),
            None => renders.then(|| crate::shader_data::build_frame(scene_state, &self.statics, size, layout.scale, request.time)),
        };
        if let Some(content) = content.as_mut() {
            // The frames the view has held still: the same records, uniforms
            // (but the clock), shading values, and viewport as the last frame.
            self.hold_still(content, viewport_size, request.history);
        }
        // The free camera's view (src/free_view.rs): the main frame drawn from
        // the free camera, in its own submission and targets (so its history
        // builds up as the main view's does), before the main frame's uniforms
        // are written. The main frame is still drawn after it: the pictures of
        // its buffers and what follows its camera are read from it, and it is
        // shown in a corner.
        let free = request.state.free_camera;
        let free_content = content.as_ref().filter(|_| scene_beat && free.on && drawn.is_empty() && compare.is_none()).map(|main| {
            let content = crate::free_view::content(main, request.state, &free);
            // The program plans for the camera this view is drawn from.
            let seen = ShotState { camera: free.orbit(), ..request.state.clone() };
            self.draw_view(ViewId::FREE, &seen, Some(content), viewport_size, frame, None)
        });
        if free_content.is_none() {
            self.slots.remove(&ViewId::FREE);
        }
        // The views the investigation stacks over the main one (src/view.rs):
        // each from its own slot, in its own scene, laid over the main view below.
        let overs = if scene_beat && !given && main.is_none() && free_content.is_none() { investigation::current().views(request.state) } else { Vec::new() };
        let mut over_drawn: Vec<(ViewId, f32, FrameContent)> = Vec::new();
        for (n, over) in overs.iter().enumerate() {
            if over.alpha <= 0.0 {
                continue;
            }
            let id = ViewId::over(n as u32);
            crate::view::set_name(Some(over.name));
            let content = self.draw_view(id, &over.state, None, viewport_size, frame, None);
            crate::view::set_name(None);
            over_drawn.push((id, over.alpha.min(1.0), content));
        }
        for n in 0..8u32 {
            if !over_drawn.iter().any(|(id, _, _)| *id == ViewId::over(n)) {
                self.slots.remove(&ViewId::over(n));
            }
        }
        self.last_overs = overs.iter().enumerate().filter(|(_, o)| o.alpha > 0.0).map(|(n, o)| (ViewId::over(n as u32), o.name, o.camera)).collect();
        // The shading's data is the main view's again (an over view selected its own).
        if renders && !over_drawn.is_empty() {
            self.select_data(scene_state, request.wait || !request.history);
        }
        // The main camera's own view, in the viewport's lower right corner, at its shape.
        let main_view = free_content.as_ref().map(|_| {
            let v = layout.viewport;
            let size = v.size() * 0.26;
            egui::Rect::from_min_size(v.max - size - egui::vec2(16.0, 16.0), size)
        });
        self.last_main_view = main_view;
        // Where each view is, back to front, for the pointer (src/view.rs).
        self.last_views = match (&free_content, main_view) {
            (Some(_), Some(corner)) => vec![(ViewId::FREE, layout.viewport), (ViewId::MAIN, corner)],
            _ => {
                let mut views: Vec<(ViewId, egui::Rect)> = drawn.iter().enumerate().map(|(n, (rect, _))| (ViewId::pane(n as u32), *rect)).collect();
                if compare.as_ref().is_some_and(|c| c.mode == crate::experiment::Compare::SideBySide) {
                    views.push((ViewId::REFERENCE, reference_rect));
                }
                views.push((ViewId::MAIN, layout.viewport));
                views
            }
        };
        // The views over the main one take the pointer where they are mostly shown.
        self.last_views.extend(over_drawn.iter().filter(|(_, a, _)| *a > 0.5).map(|(id, _, _)| (*id, layout.viewport)));
        if let Some(content) = &content {
            self.upload(content);
            self.upload_draws(content);
        }

        if content.is_some() {
            self.prepare_views(request.state);
            self.prepare_inspections(request.state, &boxes);
        }
        // A frame that stands alone, of a program that reads what earlier
        // frames left: the same shot drawn first, each from the one before,
        // the first from nothing (`ShadingProgram::warmup`), untimed.
        if let (false, Some(content)) = (request.history, content.as_ref()) {
            let warmup = investigation::current().scene_shading().and_then(|s| s.program).map_or(0, |p| p.warmup);
            if warmup > 0 {
                let timing = self.timing.take();
                for k in 0..warmup {
                    self.plan_program(scene_state, k > 0, viewport_size);
                    let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("program warm-up") });
                    self.encode_scene(&mut encoder, content, true);
                    self.gpu.queue.submit([encoder.finish()]);
                }
                self.timing = timing;
                self.plan_program(scene_state, true, viewport_size);
            }
        }
        // A frame that stands alone shows its own frame's reads (src/fetch.rs):
        // when one is not of this frame's inputs, the frame is drawn (the
        // warm-up has drawn it already), read, and built again.
        if let (false, Some(c)) = (request.history, content.as_ref()) {
            let tag = still_key(c, viewport_size, true);
            if !c.fetches.is_empty() && !crate::fetch::Queue::answered(&c.fetches, tag) {
                let warmup = investigation::current().scene_shading().and_then(|s| s.program).map_or(0, |p| p.warmup);
                let timing = self.timing.take();
                let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("fetch") });
                if warmup == 0 {
                    self.encode_scene(&mut encoder, c, true);
                }
                let requests = c.fetches.clone();
                let mut queue = std::mem::take(&mut self.fetches);
                queue.collect(&self.gpu.device, true);
                queue.issue(&self.gpu.device, &mut encoder, &requests, tag, u64::MAX, &|source| self.fetch_target(source));
                self.gpu.queue.submit([encoder.finish()]);
                queue.submitted();
                queue.collect(&self.gpu.device, true);
                self.fetches = queue;
                self.timing = timing;
                // Built again with the reads in hand; drawn again from the start only
                // when that changes whether its panel shows (and so the view's size).
                let empty = |c: &FrameContent| c.panel_record.is_none() && c.readout.is_none();
                let view_params = c.uniforms.view_params;
                let mut rebuilt = crate::shader_data::build_frame(scene_state, &self.statics, size, layout.scale, request.time);
                rebuilt.uniforms.view_params = view_params;
                if panel_built && empty(&rebuilt) != empty(c) && self.settling < 2 {
                    self.settling += 1;
                    let frame = self.render_frame_with(request, readback, settle_prepared);
                    self.settling -= 1;
                    return frame;
                }
                self.upload(&rebuilt);
                self.upload_draws(&rebuilt);
                content = Some(rebuilt);
            }
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
        let over_views: Vec<ui::PaneView> = over_drawn.iter().filter(|(_, a, _)| *a > 0.5).map(|(_, _, content)| ui::PaneView { rect: layout.viewport, content, title: None }).collect();
        // egui pass content.
        let mut raw = request.raw_input;
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, layout.canvas));
        raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(layout.scale);
        let ui_input = UiInput {
            layout,
            state: request.state,
            focus: request.focus,
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
            overs: &over_views,
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
            // The reads it asks for, of what its passes left (src/fetch.rs).
            if !content.fetches.is_empty() {
                let tag = still_key(content, viewport_size, true);
                let mut queue = std::mem::take(&mut self.fetches);
                queue.issue(&self.gpu.device, &mut encoder, &content.fetches, tag, crate::fetch::BUDGET, &|source| self.fetch_target(source));
                self.fetches = queue;
            }
        }
        if let (Some(free), Some(rect)) = (self.slots.get(&ViewId::FREE).and_then(|s| s.viewport.as_ref()).filter(|_| free_content.is_some()), main_view) {
            // The free view fills the viewport; the main view sits in its corner.
            self.composite_from(&mut encoder, free, vp, true, None);
            let at = Layout { viewport: rect, ..layout }.viewport_px();
            let timed = self.timing.as_mut().and_then(|t| t.render_writes("composite"));
            self.composite_pass(&mut encoder, self.viewport.as_ref().unwrap(), at, false, None, timed.as_ref().map(|t| t.writes()));
        } else if scene_beat {
            let timed = self.timing.as_mut().and_then(|t| t.render_writes("composite"));
            self.composite_timed(&mut encoder, vp, drawn.is_empty(), timed.as_ref().map(|t| t.writes()));
        }
        // The views stacked over the main one, each at its alpha.
        for (id, alpha, _) in &over_drawn {
            if let Some(targets) = self.slots.get(id).and_then(|s| s.viewport.as_ref()) {
                self.composite_over(&mut encoder, targets, vp, *alpha);
            }
        }
        if let (Some(c), true, Some(reference)) = (&compare, self.reference_drawn, self.slots.get(&ViewId::REFERENCE).and_then(|s| s.viewport.as_ref())) {
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
            crate::readback::copy_to_readback(&mut encoder, &self.frame, buffer, [self.width, self.height]);
        }
        if let Some(t) = self.timing.as_mut() {
            t.resolve(&mut encoder);
        }
        commands.push(encoder.finish());
        self.gpu.queue.submit(commands);
        self.fetches.submitted();
        if let Some(t) = self.timing.as_mut() {
            t.submitted(&self.gpu.device);
        }
        for id in output.textures_delta.free.drain() {
            self.egui_renderer.free_texture(&id);
        }
        match free_content {
            Some(free) => FrameResult { report, content: Some(free), main_content: content },
            None => FrameResult { report, content, main_content: None },
        }
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

/// What every view of one frame shares: the layout's scale, the frame's
/// time, and whether its history carries on from the last frame.
#[derive(Clone, Copy)]
struct ViewFrame {
    scale: f32,
    time: f32,
    history: bool,
    wait: bool,
}

/// What a frame's graph was compiled for: the shading (by address) and the
/// resources image kinds read.
type GraphKey = (usize, Vec<String>);
