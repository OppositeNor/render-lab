//! The renderer shared by the viewer and the recorder. Each frame is drawn
//! into one RGBA8 offscreen target: the 3D scene (MSAA, HDR), the overlay
//! layer, the composite into the viewport rectangle, then egui on top. The
//! viewer blits that target to its window; the recorder reads it back.

use crate::frame::{self, FrameContent, Statics};
use crate::investigation::{self, SceneShading};
use crate::gpu::{self, Gpu, Pipelines, DEPTH_FORMAT, FRAME_FORMAT, HDR_FORMAT, MSAA, OVERLAY_FORMAT};
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
        let hdr = view(target(device, "scene hdr", size, HDR_FORMAT, 1, sampled | wgpu::TextureUsages::STORAGE_BINDING));
        let overlay = view(target(device, "overlay", size, OVERLAY_FORMAT, 1, sampled));
        let composite = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("composite"),
            layout: &pipelines.texture_pair_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&hdr) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&overlay) }],
        });
        ViewportTargets {
            size,
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
    last_layout: Layout,
    /// The last frame's views beside the main one (`Investigation::panes`), in canvas points.
    last_panes: Vec<egui::Rect>,
}

/// A story's image of an image kind (src/views.rs): its texture, written
/// each frame by the kind's entry point, and shown through egui.
struct ViewInstance {
    kind: &'static crate::views::ViewKind,
    _texture: wgpu::Texture,
    params: wgpu::Buffer,
    group: wgpu::BindGroup,
    id: egui::TextureId,
}

/// The pipelines and data of one investigation's `SceneShading`.
struct Shading {
    source: &'static SceneShading,
    pipelines: gpu::ShadingPipelines,
    group: wgpu::BindGroup,
    _data: wgpu::Buffer,
    /// `FrameContent::shading_frame`.
    frame_data: wgpu::Buffer,
}

/// The targets of an investigation's pass chain at one viewport size, with
/// each pass's group-2 bind group.
struct ChainTargets {
    source: &'static SceneShading,
    size: [u32; 2],
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
        let views: Vec<(&'static str, wgpu::TextureView)> = source
            .targets
            .iter()
            .map(|t| {
                let output = source.outputs.contains(&t.name);
                let mut usage = wgpu::TextureUsages::TEXTURE_BINDING;
                if output {
                    usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
                }
                if written.contains(&t.name) {
                    usage |= wgpu::TextureUsages::STORAGE_BINDING;
                }
                let s = [size[0].div_ceil(t.divisor), size[1].div_ceil(t.divisor)];
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
            for (name, written) in reads.iter().map(|n| (*n, false)).chain(writes.iter().map(|n| (*n, true))) {
                let binding = gpu::chain_binding(source, name, written);
                if !entries.iter().any(|e| e.binding == binding) {
                    entries.push(wgpu::BindGroupEntry { binding, resource: resource(name) });
                }
            }
            device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(label), layout, entries: &entries })
        };
        let groups = source.early.iter().chain(source.passes).zip(&shading.pipelines.passes).map(|(pass, (_, layout))| group(pass.entry, layout, pass.reads, pass.writes)).collect();
        let scene_group = group("scene reads", &shading.pipelines.scene_group, source.scene_reads, &[]);
        ChainTargets { source, size, views, buffers, depth_views, depth_views_drawn: std::cell::Cell::new(None), groups, scene_group }
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
        let divisor = self.source.targets.iter().find(|t| t.name == name).map_or(1, |t| t.divisor);
        [self.size[0].div_ceil(divisor), self.size[1].div_ceil(divisor)]
    }

    /// Encode a run of compute passes with their bind groups.
    fn dispatch(&self, encoder: &mut wgpu::CommandEncoder, frame: &wgpu::BindGroup, shading: &Shading, range: std::ops::Range<usize>) {
        if range.is_empty() {
            return;
        }
        let steps: Vec<&crate::investigation::ShadingPass> = self.source.early.iter().chain(self.source.passes).collect();
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("shading chain"), timestamp_writes: None });
        pass.set_bind_group(0, frame, &[]);
        pass.set_bind_group(1, &shading.group, &[]);
        for i in range {
            let step = steps[i];
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
    pub image: Option<(egui::TextureId, egui::Vec2)>,
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
            last_layout: Layout::new(width, height),
            last_panes: Vec::new(),
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
    pub fn layout_for(&self, state: &ShotState, scene_type: SceneType, dock: bool) -> Layout {
        let panel = scene_type != SceneType::ThreeD || state.panel != crate::shot::PanelMode::Hidden;
        let mut layout = self.layout();
        if dock {
            layout = layout.with_dock();
        }
        if scene_type == SceneType::ThreeD && state.panel == crate::shot::PanelMode::Code {
            layout = layout.with_code_panel();
        }
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
        for (name, kind, params) in shown {
            if !self.views.contains_key(name) {
                let kind = crate::views::kind(package, kind).unwrap_or_else(|e| panic!("{e}"));
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(name),
                    size: wgpu::Extent3d { width: kind.size[0], height: kind.size[1], depth_or_array_layers: 1 },
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
                self.textures.insert(view_texture(name), id);
                self.views.insert(name.to_string(), ViewInstance { kind, _texture: texture, params, group, id });
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
                pass.dispatch_workgroups(kind.size[0].div_ceil(kind.workgroup[0].max(1)), kind.size[1].div_ceil(kind.workgroup[1].max(1)), 1);
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
            pass.dispatch_workgroups(kind.size[0].div_ceil(kind.workgroup[0].max(1)), kind.size[1].div_ceil(kind.workgroup[1].max(1)), 1);
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
            self.shadings.push(Shading { source, pipelines, group, _data: data, frame_data });
        }
        self.shadings.iter().find(|s| std::ptr::eq(s.source, source))
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
        let dock = matches!(&request.chrome, Chrome::Viewer(v) if !(v.timeline_mode && v.playing));
        let mut layout = self.layout_for(request.state, request.scene_type, dock);
        let scene_beat = request.scene_type == SceneType::ThreeD;
        // A split scene area: the views beside the main one, each in its own
        // rectangle; the main view takes the last (Investigation::panes).
        let mut panes = if scene_beat && prepared.is_none() { investigation::current().panes(request.state) } else { Vec::new() };
        let main = panes.pop();
        let rects = if main.is_some() { layout.split(panes.len() + 1) } else { Vec::new() };
        if let Some(rect) = rects.last() {
            layout.viewport = *rect;
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
        // Each other view: built, drawn, and composited into its rectangle in
        // a submission of its own, so the shared uniforms and targets hold its
        // camera and values while it draws.
        let mut drawn = Vec::new();
        for (pane, rect) in panes.iter().zip(&rects) {
            let mut content = crate::shader_data::build_frame(&pane.state, &self.statics, size, layout.scale, request.time);
            content.uniforms.view_params[3] = 0.0;
            self.upload(&content);
            self.upload_draws(&content);
            let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pane") });
            self.encode_scene(&mut encoder, &content);
            let at = Layout { viewport: *rect, ..layout }.viewport_px();
            self.composite(&mut encoder, at, drawn.is_empty());
            self.gpu.queue.submit([encoder.finish()]);
            drawn.push((*rect, content));
        }
        let scene_state = main.as_ref().map_or(request.state, |m| &m.state);
        let mut content = match prepared {
            Some(c) => Some(c),
            None => scene_beat.then(|| crate::shader_data::build_frame(scene_state, &self.statics, size, layout.scale, request.time)),
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
        if let Some(content) = &content {
            self.upload(content);
            self.upload_draws(content);
        }

        if content.is_some() {
            self.prepare_views(request.state);
        }
        let pane_views: Vec<ui::PaneView> = drawn.iter().zip(&panes).map(|((rect, content), pane)| ui::PaneView { rect: *rect, content, title: pane.title.as_deref() }).collect();
        // egui pass content.
        let mut raw = request.raw_input;
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, layout.canvas));
        raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(layout.scale);
        let ui_input = UiInput {
            layout,
            state: request.state,
            content: content.as_ref(),
            statics: &self.statics,
            textures: &self.textures,
            scene_type: request.scene_type,
            image: request.image,
            beat_sources: request.beat_sources,
            notices: request.notices,
            chapter_number: request.chapter_number,
            fade: request.fade,
            stamp: request.stamp,
            panes: &pane_views,
            title: main.as_ref().and_then(|m| m.title.as_deref()),
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
            self.encode_scene(&mut encoder, content);
            self.dispatch_views(&mut encoder, true);
        }
        if scene_beat {
            self.composite(&mut encoder, vp, drawn.is_empty());
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
        commands.push(encoder.finish());
        self.gpu.queue.submit(commands);
        for id in output.textures_delta.free.drain() {
            self.egui_renderer.free_texture(&id);
        }
        FrameResult { report, content }
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
        let targets = self.viewport.as_ref().unwrap();
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.frame_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: if clear { self.background_clear() } else { wgpu::LoadOp::Load }, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(at[0] as f32, at[1] as f32, at[2] as f32, at[3] as f32, 0.0, 1.0);
        pass.set_pipeline(&self.pipelines.composite);
        pass.set_bind_group(0, &self.frame_bind_group, &[]);
        pass.set_bind_group(1, &targets.composite, &[]);
        pass.draw(0..3, 0..1);
    }

    /// The scene and overlay passes into the viewport targets (the composite
    /// reads them).
    fn encode_scene(&mut self, encoder: &mut wgpu::CommandEncoder, content: &FrameContent) {
        self.shading();
        let shading = investigation::current().scene_shading().and_then(|source| self.shadings.iter().find(|s| std::ptr::eq(s.source, source)));
        let targets = self.viewport.as_ref().unwrap();
        // An investigation's pass chain: its targets follow the viewport.
        let chained = shading.filter(|s| !s.source.outputs.is_empty());
        if let Some(s) = chained {
            if self.chain.as_ref().is_none_or(|c| c.size != targets.size || !std::ptr::eq(c.source, s.source)) {
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
        // The chain's depth pre-pass and early passes, before the scene pass.
        let prepassed = chain.is_some() && chained.is_some_and(|s| s.pipelines.prepass.is_some());
        if let (Some(s), Some(c)) = (chained, chain) {
            // The depth views (shadow maps), one layer at a time, when what
            // they see (the objects, the light, the shading values) changed.
            let key = depth_views_key(content);
            if let Some((_, layers)) = c.depth_views.as_ref().filter(|_| c.depth_views_drawn.replace(Some(key)) != Some(key)) {
                for (pipeline, layer) in s.pipelines.depth_views.iter().zip(layers) {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("depth view"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: layer, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                        timestamp_writes: None,
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
            if let Some(prepass) = &s.pipelines.prepass {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("depth pre-pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &targets.depth,
                        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
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
            c.dispatch(encoder, &self.frame_bind_group, s, 0..s.source.early.len());
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
                depth_ops: Some(wgpu::Operations { load: if prepassed { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(0.0) }, store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
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
        // The rest of the chain, in order; the last pass writes the HDR target.
        if let (Some(s), Some(c)) = (chained, chain) {
            let early = s.source.early.len();
            c.dispatch(encoder, &self.frame_bind_group, s, early..early + s.source.passes.len());
        }

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
            timestamp_writes: None,
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
            self.encode_scene(&mut encoder, content);
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

    /// Run a verification kernel with `count` workgroups and read `words` back.
    pub fn run_verify(&self, entry: &str, count: u32, tests: &[[f32; 4]], words: usize) -> Vec<u32> {
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
            pass.set_pipeline(&self.pipelines.verify[entry]);
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            pass.set_bind_group(1, &bind, &[]);
            pass.dispatch_workgroups(count, 1, 1);
        }
        self.gpu.queue.submit([encoder.finish()]);
        read_buffer(device, &self.gpu.queue, &out, 0, (words * 4) as u64).chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
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

    /// Read back a byte range of one of the record buffers.
    pub fn read_record_bytes(&self, buffer: &str, offset: u64, size: u64) -> Vec<u8> {
        let b = match buffer {
            "objects" => &self.records.objects.buffer,
            _ => return Vec::new(),
        };
        let device = &self.gpu.device;
        let staging = device.create_buffer(&wgpu::BufferDescriptor { label: Some("record readback"), size, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(b, offset, &staging, 0, size);
        self.gpu.queue.submit([encoder.finish()]);
        read_buffer(device, &self.gpu.queue, &staging, 0, size)
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
