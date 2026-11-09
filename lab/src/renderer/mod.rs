//! The renderer shared by the viewer and the recorder. Each frame is drawn
//! into one RGBA8 offscreen target: the 3D scene (MSAA, HDR), the overlay
//! layer, the composite into the viewport rectangle, then egui on top. The
//! viewer blits that target to its window; the recorder reads it back.

use crate::frame::{self, FrameContent, Statics};
use crate::gpu::{
    self, DEPTH_FORMAT, FRAME_FORMAT, Gpu, HDR_FORMAT, MSAA, OVERLAY_FORMAT, Pipelines,
};
use crate::graph::Step;
use crate::investigation::{self, SceneShading};
use crate::layout::{FrameUniforms, ObjectData};
use crate::scene::SceneId;
use crate::shot::{SceneType, ShotState};
use crate::ui::{self, Chrome, Layout, UiInput, UiReport};
use crate::utility::inset::Textures;
use crate::view::ViewId;
use bytemuck::Pod;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

pub use crate::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows};

mod encode;
mod inspect;
mod one_frame;
mod reads;
mod shading;
mod targets;
mod views;
use inspect::*;
use shading::*;
pub use targets::*;
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
    /// The features a recording's story panels are drawn by (src/feature/), made for its first frame.
    recording_features: Option<crate::feature::Features>,
    /// The pointer's cursor the last frame's interface asked for (egui's).
    pub cursor: egui::CursorIcon,
    /// When each recent frame started rendering, for the performance panel's frame times.
    frame_starts: std::collections::VecDeque<std::time::Instant>,
    /// The GPU reads the frames ask for, in flight (src/fetch.rs).
    fetches: crate::fetch::Queue,
    /// How deep the redraws of a frame drawn alone, waiting for its reads, are.
    settling: u8,
    /// GPU timestamps around the frame's passes (src/timing.rs), while a panel
    /// or a feature asks for them (`set_timing`) and the device has them.
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
        let environment = Self::environment_texture(device, queue);
        let environment_view = environment.create_view(&Default::default());

        let records = Self::record_buffers(device, &statics);
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
        let egui_renderer = Self::egui_renderer(device);
        let textures = Textures::new();
        let frame = Self::frame_texture(device, width, height);
        let frame_view = frame.create_view(&Default::default());
        let shading_uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene shading uniforms"),
            size: gpu::SHADING_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (compare_params, compare_group) = Self::compare_resources(device, &pipelines);
        let mut renderer = Renderer {
            still: (None, 0),
            still_scene: (None, 0),
            frame_bind_group: Self::frame_bind_group(
                device,
                &pipelines,
                &records,
                &environment_view,
                &sampler,
            ),
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
            view_layout: Self::view_layout(device),
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
            compare_group,
            compare_params,
            reference_drawn: false,
            user_layout: Default::default(),
            code_fit: HashMap::new(),
            recording_features: None,
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

    /// The egui renderer of the frame's format: no MSAA, no depth, no dithering, filtering the same everywhere.
    fn egui_renderer(device: &wgpu::Device) -> egui_wgpu::Renderer {
        egui_wgpu::Renderer::new(
            device,
            FRAME_FORMAT,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: true,
            },
        )
    }

    /// An experiment's comparison composite (src/experiment.rs): its parameters' buffer, and its group.
    fn compare_resources(
        device: &wgpu::Device,
        pipelines: &Pipelines,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("compare"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compare"),
            layout: &pipelines.compare_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params.as_entire_binding(),
            }],
        });
        (params, group)
    }

    /// The bind group layout of an image kind's pass (src/views.rs): its output image and its parameters.
    fn view_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("image kind"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
    }

    /// The lab's environment (environment.rs), gEnvironment: its levels, RGB9E5.
    fn environment_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
        let env = crate::environment::Environment::get();
        device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("environment (RGB9E5)"),
                size: wgpu::Extent3d {
                    width: env.sizes[0][0] as u32,
                    height: env.sizes[0][1] as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: env.levels.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgb9e5Ufloat,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &env.levels
                .concat()
                .iter()
                .flat_map(|t| t.to_le_bytes())
                .collect::<Vec<u8>>(),
        )
    }

    /// The frame's records: its uniforms, the objects, the teapot's BVH, and the terrain's heights.
    fn record_buffers(device: &wgpu::Device, statics: &Statics) -> RecordBuffers {
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
        let mesh_nodes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("teapot BVH nodes"),
            contents: bytemuck::cast_slice(&crate::mesh::teapot().nodes),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let mesh_triangles = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("teapot triangles"),
            contents: bytemuck::cast_slice(&crate::mesh::teapot().triangles),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let terrain = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain heights"),
            contents: bytemuck::cast_slice(&crate::terrain::terrain().gpu_words()),
            usage: wgpu::BufferUsages::STORAGE,
        });
        RecordBuffers {
            uniforms: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("frame uniforms"),
                size: std::mem::size_of::<FrameUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            objects: GrowBuffer::new(
                device,
                "objects",
                storage,
                (statics.scene.objects.len() * std::mem::size_of::<ObjectData>()) as u64,
            ),
            mesh_nodes,
            mesh_triangles,
            terrain,
        }
    }

    /// The GPU data of the scene `statics` prepares: its raster meshes.
    fn scene_resources(device: &wgpu::Device, statics: &Statics) -> SceneResources {
        let meshes = &statics.meshes;
        let mesh_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh vertices"),
            contents: bytemuck::cast_slice(&meshes.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mesh_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh indices"),
            contents: bytemuck::cast_slice(&meshes.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let ids: Vec<u32> = (0..statics.scene.objects.len() as u32).collect();
        let object_ids = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("object ids"),
            contents: bytemuck::cast_slice(&ids),
            usage: wgpu::BufferUsages::VERTEX,
        });

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
                let statics = Statics::prepare(id)
                    .unwrap_or_else(|e| panic!("preparing scene {}: {e:#}", id.key()));
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
        self.other_scenes
            .insert(previous.scene.id, (previous, resources));
    }

    /// The view being drawn shows scene `id`: its own data is dropped when its scene changed.
    fn view_scene(&mut self, id: SceneId) {
        self.switch_scene(id);
        let view = self.active;
        if self
            .view_scenes
            .insert(view, id)
            .is_some_and(|last| last != id)
        {
            crate::fetch::clear_view(view);
        }
    }

    fn frame_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        target(
            device,
            "frame (RGBA8)",
            [width, height],
            FRAME_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
        )
    }

    fn frame_bind_group(
        device: &wgpu::Device,
        pipelines: &Pipelines,
        r: &RecordBuffers,
        environment: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        fn buffer(binding: u32, b: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry {
                binding,
                resource: b.as_entire_binding(),
            }
        }
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout: &pipelines.frame_layout,
            entries: &[
                buffer(0, &r.uniforms),
                buffer(1, &r.objects.buffer),
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                buffer(3, &r.mesh_nodes),
                buffer(4, &r.mesh_triangles),
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(environment),
                },
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

    /// `viewer`: the interactive viewer's layout, its windows where the user left them (`user_layout`,
    /// src/ui/windows.rs); `dock`: with its Controls and Scene windows (not while a timeline plays).
    /// With `values`, the panel shows (a feature's own, `FeatureLayout::panel`) whatever the state's panel mode.
    /// `code_width`: the code panel's width when a listing shows (a recording's, fit to it).
    pub fn layout_for(
        &self,
        state: &ShotState,
        scene_type: SceneType,
        viewer: bool,
        dock: bool,
        values: bool,
        code_width: Option<f32>,
    ) -> Layout {
        let panel = scene_type != SceneType::ThreeD
            || state.panel != crate::shot::PanelMode::Hidden
            || values;
        // The pseudo-code and the knowledge graph take the wider panel.
        let wide = scene_type == SceneType::ThreeD && state.panel.wide();
        if viewer {
            // A 2D page (a diagram, a sources card) takes the room of the panel too.
            let three_d = scene_type == SceneType::ThreeD;
            let shown = crate::ui::windows::Shown {
                scene: dock && three_d && crate::entity::any(state, &self.statics),
                controls: dock,
                panel: panel && three_d,
                wide,
            };
            return self.layout().with_windows(&self.user_layout, shown);
        }
        let video = self.layout().for_video();
        let layout = match code_width {
            Some(width) if wide && state.panel == crate::shot::PanelMode::Code => {
                video.with_panel_width(width)
            }
            _ if wide => video.with_code_panel(),
            _ => video,
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
            assert!(
                content.shading_frame.len() <= frame::SHADING_FRAME_CAPACITY,
                "shading_frame holds {} vectors (at most {})",
                content.shading_frame.len(),
                frame::SHADING_FRAME_CAPACITY
            );
            if let Some(buffer) = self.shading().map(|s| s.frame_data.clone()) {
                self.gpu.queue.write_buffer(
                    &buffer,
                    0,
                    bytemuck::cast_slice(&content.shading_frame),
                );
            }
        }
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        queue.write_buffer(
            &self.records.uniforms,
            0,
            bytemuck::bytes_of(&content.uniforms),
        );
        queue.write_buffer(
            &self.shading_uniforms,
            0,
            bytemuck::cast_slice(&content.shading),
        );
        if self.records.objects.write(device, queue, &content.objects) {
            self.frame_bind_group = Self::frame_bind_group(
                device,
                &self.pipelines,
                &self.records,
                &self.environment_view,
                &self.sampler,
            );
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
            let (Some(program), Some(pipelines)) = (s.source.program, s.pipelines.program.as_ref())
            else {
                continue;
            };
            let id = s.source as *const SceneShading as usize;
            // The slot's runtime for this shading, or a new one (made for this view's size when it binds).
            let incoming = match slot.programs.iter().position(|(k, _, _)| *k == id) {
                Some(i) => {
                    let (_, runtime, handles) = slot.programs.swap_remove(i);
                    (runtime, handles)
                }
                None => {
                    let runtime = std::cell::RefCell::new(crate::program::ProgramRuntime::new(
                        device, s.source, program, pipelines,
                    ));
                    let handles = std::cell::RefCell::new(runtime.borrow().handles());
                    (runtime, handles)
                }
            };
            let runtime = s
                .program
                .replace(incoming.0)
                .expect("a shading with a program has its runtime");
            let handles = std::mem::replace(&mut s.program_handles, incoming.1);
            kept.push((id, runtime, handles));
        }
        slot.programs.extend(kept);
    }

    /// Counts how long the view being drawn has held still and writes it into
    /// `content`'s `viewParams.w` (`FrameRequest::history`; 0 without history).
    fn hold_still(&mut self, content: &mut FrameContent, viewport: [u32; 2], history: bool) {
        let count = |last: (Option<u64>, u32), key: u64| {
            if last.0 == Some(key) {
                (Some(key), last.1.saturating_add(1))
            } else {
                (Some(key), 0)
            }
        };
        self.still = count(self.still, still_key(content, viewport, true));
        self.still_scene = count(self.still_scene, still_key(content, viewport, false));
        content.uniforms.view_params[3] = if history {
            (1 + self.still.1.min(255) + 256 * self.still_scene.1.min(32767)) as f32
        } else {
            0.0
        };
    }

    /// The view being drawn's targets at `size` (made again, its chain with them, when the size changed).
    fn targets_at(&mut self, size: [u32; 2]) {
        if self.viewport.as_ref().is_none_or(|v| v.size != size) {
            self.chain = None;
            self.viewport = Some(ViewportTargets::new(
                &self.gpu.device,
                &self.pipelines,
                size,
            ));
        }
    }

    /// A view other than the main one, drawn in a submission of its own from
    /// its own slot: built (or `content` as given), planned, drawn, and, with
    /// `at`, composited there.
    fn draw_view(
        &mut self,
        view: ViewId,
        state: &ShotState,
        content: Option<FrameContent>,
        size: [u32; 2],
        frame: ViewFrame,
        at: Option<([u32; 4], bool)>,
    ) -> FrameContent {
        let ViewFrame {
            scale,
            time,
            history,
            wait,
        } = frame;
        let main_scene = self.statics.scene.id;
        self.enter(view);
        // Its own scene (src/view.rs).
        self.view_scene(investigation::current().scene(state));
        self.targets_at(size);
        self.select_data(state, wait || !history);
        let mut content = content.unwrap_or_else(|| {
            crate::shader_data::build_frame(
                state,
                &self.statics,
                [size[0] as f32, size[1] as f32],
                scale,
                time,
            )
        });
        self.hold_still(&mut content, size, history);
        self.plan_program(state, history, size);
        self.upload(&content);
        self.upload_draws(&content);
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("view"),
            });
        self.encode_scene(&mut encoder, &content, false);
        // The reads it asks for, of what its own passes left (src/fetch.rs).
        if !content.fetches.is_empty() {
            let tag = still_key(&content, size, true);
            let mut queue = std::mem::take(&mut self.fetches);
            queue.issue(
                &self.gpu.device,
                &mut encoder,
                &content.fetches,
                tag,
                crate::fetch::BUDGET,
                &|source| self.fetch_target(source),
            );
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
        let seg_xray: Vec<_> = d
            .segments
            .iter()
            .map(|s| {
                let mut s = *s;
                s.color[3] = dim(s.color[3]);
                s.flags |= frame::FLAG_DASHED;
                s
            })
            .collect();
        let box_xray: Vec<_> = d
            .boxes
            .iter()
            .map(|b| {
                let mut b = *b;
                b.color[3] = dim(b.color[3]);
                b.rid[3] |= frame::FLAG_DASHED;
                b
            })
            .collect();
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
    pub fn render_frame(
        &mut self,
        request: FrameRequest,
        readback: Option<&wgpu::Buffer>,
    ) -> FrameResult {
        self.render_frame_with(request, readback, None)
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
