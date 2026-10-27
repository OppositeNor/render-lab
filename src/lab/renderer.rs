//! The renderer shared by the viewer and the recorder. Each frame is drawn
//! into one RGBA8 offscreen target: the 3D scene (MSAA, HDR), the overlay
//! layer, the composite into the viewport rectangle, then egui on top. The
//! viewer blits that target to its window; the recorder reads it back.

use crate::lab::frame::{self, FrameContent, Statics};
use crate::lab::investigation::{self, SceneShading};
use crate::lab::gpu::{self, Gpu, Pipelines, DEPTH_FORMAT, FRAME_FORMAT, HDR_FORMAT, MSAA, OVERLAY_FORMAT};
#[cfg(commercial)]
use crate::lab::layout::{BrickEntry, CardData, PageEntry, ProbeRecord, RadianceSample, VoxelSample};
use crate::lab::layout::{FrameUniforms, ObjectData, RayDebugData};
use crate::lab::shot::{BeatKind, ShotState};
#[cfg(commercial)]
use crate::lab::surface_cache_gpu::{self, CacheInputs, SurfaceCacheGpu};
use crate::lab::ui::{self, Chrome, Layout, UiInput, UiReport};
use bytemuck::Pod;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

pub use crate::lab::readback::{padded_row_bytes, read_buffer, readback_buffer, unpad_rows};

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
    pub voxels: GrowBuffer,
    pub bricks: GrowBuffer,
    pub cards: GrowBuffer,
    pub pages: GrowBuffer,
    pub rays: GrowBuffer,
    pub probes: GrowBuffer,
    pub radiance: GrowBuffer,
    /// The teapot's BVH and triangles (static, local space).
    pub mesh_nodes: wgpu::Buffer,
    pub mesh_triangles: wgpu::Buffer,
}

pub struct Renderer {
    pub gpu: Gpu,
    pub pipelines: Pipelines,
    pub statics: Statics,
    pub records: RecordBuffers,
    mesh_vertices: wgpu::Buffer,
    mesh_indices: wgpu::Buffer,
    object_ids: wgpu::Buffer,
    _brick_atlas: wgpu::Texture,
    brick_atlas_view: wgpu::TextureView,
    _surface_cache: wgpu::Texture,
    surface_cache_view: wgpu::TextureView,
    _surface_display: wgpu::Texture,
    surface_display_view: wgpu::TextureView,
    /// The lab's environment (environment.rs), gEnvironment.
    _environment: wgpu::Texture,
    environment_view: wgpu::TextureView,
    /// The Surface Cache passes (built once, then the per-frame picture).
    #[cfg(commercial)]
    cache_gpu: SurfaceCacheGpu,
    #[cfg(commercial)]
    cache_inputs: wgpu::BindGroup,
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
    samples: GrowBuffer,
    cards: GrowBuffer,
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
    /// The Surface Cache picture (what the card quads show), for the atlas inset.
    pub atlas_texture: egui::TextureId,
    /// Readout formulas, uploaded the first time each is shown and kept.
    formulas: HashMap<&'static str, (egui::TextureHandle, egui::Vec2)>,
    last_layout: Layout,
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
        let resource = |name: &str| -> wgpu::BindingResource {
            if let Some((_, buffer)) = buffers.iter().find(|(n, _)| *n == name) {
                return buffer.as_entire_binding();
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
        ChainTargets { source, size, views, buffers, groups, scene_group }
    }

    fn view(&self, name: &str) -> &wgpu::TextureView {
        &self.views.iter().find(|(n, _)| *n == name).expect("a scene output is a chain target").1
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
        let steps: Vec<&crate::lab::investigation::ShadingPass> = self.source.early.iter().chain(self.source.passes).collect();
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

/// Everything a frame needs besides the shot state.
pub struct FrameRequest<'a> {
    pub state: &'a ShotState,
    pub beat_kind: BeatKind,
    pub image: Option<(egui::TextureId, egui::Vec2)>,
    pub beat_sources: &'a [String],
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
    /// `statics` from `Statics::prepare`: the Surface Cache is captured and
    /// lit here on the GPU, read back into `statics`, and the screen probes
    /// are traced from it. A cache already lit on the CPU is uploaded as is.
    pub fn new(gpu: Gpu, #[cfg_attr(not(commercial), allow(unused_mut))] mut statics: Statics, width: u32, height: u32) -> anyhow::Result<Renderer> {
        let device = &gpu.device;
        let queue = &gpu.queue;
        let pipelines = Pipelines::new(device)?;
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
        let mesh_nodes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("teapot BVH nodes"), contents: bytemuck::cast_slice(&crate::lab::mesh::teapot().nodes), usage: wgpu::BufferUsages::STORAGE });
        let mesh_triangles = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("teapot triangles"), contents: bytemuck::cast_slice(&crate::lab::mesh::teapot().triangles), usage: wgpu::BufferUsages::STORAGE });
        // The lab's environment (environment.rs), gEnvironment.
        let env = crate::lab::environment::Environment::get();
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

        // The Surface Cache: the atlases as stored (u32 per texel, the engine's
        // encodings) plus the radiosity snapshots, and the picture of them.
        #[cfg(commercial)]
        let (surface_cache, surface_display, cache_gpu) = {
            let atlas_size = statics.fixtures.surface_cache.atlas_size;
            let surface_cache = surface_cache_gpu::atlas_texture(device, atlas_size);
            let surface_display = surface_cache_gpu::display_texture(device, atlas_size);
            let cache_gpu = SurfaceCacheGpu::new(device, &statics.fixtures.surface_cache, &surface_display);
            if statics.fixtures.surface_cache.lit {
                upload_lit_cache(queue, &surface_cache, &statics.fixtures.surface_cache);
            } else {
                let records = statics.scene.records();
                let start = std::time::Instant::now();
                let texels = cache_gpu.build(device, queue, &statics.fixtures.surface_cache, &records, &statics.scene.lighting, [&mesh_nodes, &mesh_triangles], &environment_view, &surface_cache);
                statics.fixtures.surface_cache.set_lit_texels(texels, start.elapsed().as_secs_f32());
                statics.trace_probes();
            }
            (surface_cache, surface_display, cache_gpu)
        };
        // Without src/commercial there is no Surface Cache: one-texel stand-ins.
        #[cfg(not(commercial))]
        let (surface_cache, surface_display) = (
            placeholder(device, "surface cache (none)", wgpu::TextureDimension::D2, wgpu::TextureFormat::R32Uint),
            placeholder(device, "surface display (none)", wgpu::TextureDimension::D2, wgpu::TextureFormat::Rgba8Unorm),
        );
        #[cfg(commercial)]
        let (df, cache, probes) = (&statics.fixtures.distance_field, &statics.fixtures.surface_cache, &statics.fixtures.probes);
        let capacity = |count: usize, size: usize| (count * size) as u64;
        #[cfg(commercial)]
        let [voxels, bricks, cards, pages, probe_records, radiance] = [
            capacity(df.voxels.len(), std::mem::size_of::<VoxelSample>()),
            capacity(df.bricks.len(), std::mem::size_of::<BrickEntry>()),
            capacity(cache.cards.len(), std::mem::size_of::<CardData>()),
            capacity(cache.pages.len(), std::mem::size_of::<PageEntry>()),
            capacity(probes.records.len(), std::mem::size_of::<ProbeRecord>()),
            capacity(probes.samples.len(), std::mem::size_of::<RadianceSample>()),
        ];
        #[cfg(not(commercial))]
        let [voxels, bricks, cards, pages, probe_records, radiance] = [0u64; 6];
        #[cfg_attr(not(commercial), allow(unused_mut))]
        let mut records = RecordBuffers {
            uniforms: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("frame uniforms"),
                size: std::mem::size_of::<FrameUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            objects: GrowBuffer::new(device, "objects", storage, capacity(statics.scene.objects.len(), std::mem::size_of::<ObjectData>())),
            voxels: GrowBuffer::new(device, "voxels", storage, voxels),
            bricks: GrowBuffer::new(device, "bricks", storage, bricks),
            cards: GrowBuffer::new(device, "cards", storage, cards),
            pages: GrowBuffer::new(device, "pages", storage, pages),
            rays: GrowBuffer::new(device, "rays", storage, std::mem::size_of::<RayDebugData>() as u64),
            probes: GrowBuffer::new(device, "probes", storage, probe_records),
            radiance: GrowBuffer::new(device, "radiance", storage, radiance),
            mesh_nodes,
            mesh_triangles,
        };
        #[cfg(commercial)]
        {
            records.voxels.write(device, queue, &df.voxels);
            records.bricks.write(device, queue, &df.bricks);
            records.pages.write(device, queue, &cache.pages);
        }

        let meshes = &statics.meshes;
        let mesh_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("mesh vertices"), contents: bytemuck::cast_slice(&meshes.vertices), usage: wgpu::BufferUsages::VERTEX });
        let mesh_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("mesh indices"), contents: bytemuck::cast_slice(&meshes.indices), usage: wgpu::BufferUsages::INDEX });
        let ids: Vec<u32> = (0..statics.scene.objects.len() as u32).collect();
        let object_ids = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("object ids"), contents: bytemuck::cast_slice(&ids), usage: wgpu::BufferUsages::VERTEX });

        #[cfg(commercial)]
        let atlas_dims = [df.atlas_bricks[0] * 8, df.atlas_bricks[1] * 8, df.atlas_bricks[2] * 8];
        #[cfg(commercial)]
        let brick_atlas = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("brick atlas (R8)"),
                size: wgpu::Extent3d { width: atlas_dims[0], height: atlas_dims[1], depth_or_array_layers: atlas_dims[2] },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &df.atlas,
        );
        #[cfg(not(commercial))]
        let brick_atlas = placeholder(device, "brick atlas (none)", wgpu::TextureDimension::D3, wgpu::TextureFormat::R8Unorm);
        let brick_atlas_view = brick_atlas.create_view(&Default::default());
        let surface_cache_view = surface_cache.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
        // Sampled as sRGB: the card shader and egui read linear colour.
        let surface_display_view = surface_display.create_view(&wgpu::TextureViewDescriptor { format: Some(wgpu::TextureFormat::Rgba8UnormSrgb), usage: Some(wgpu::TextureUsages::TEXTURE_BINDING), ..Default::default() });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let vertex = wgpu::BufferUsages::VERTEX;
        let egui_ctx = egui::Context::default();
        ui::install_fonts(&egui_ctx);
        let mut egui_renderer = egui_wgpu::Renderer::new(device, FRAME_FORMAT, egui_wgpu::RendererOptions { msaa_samples: 1, depth_stencil_format: None, dithering: false, predictable_texture_filtering: true });
        let atlas_texture = egui_renderer.register_native_texture(device, &surface_display_view, wgpu::FilterMode::Linear);
        let frame = Self::frame_texture(device, width, height);
        let frame_view = frame.create_view(&Default::default());
        let shading_uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene shading uniforms"),
            size: gpu::SHADING_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut renderer = Renderer {
            frame_bind_group: Self::frame_bind_group(device, &pipelines, &records, &brick_atlas_view, &surface_display_view, &surface_cache_view, &environment_view, &sampler),
            #[cfg(commercial)]
            cache_inputs: Self::cache_inputs(device, &cache_gpu, &records, &surface_cache_view, &environment_view),
            #[cfg(commercial)]
            cache_gpu,
            shading_uniforms,
            shadings: Vec::new(),
            chain: None,
            pipelines,
            records,
            mesh_vertices,
            mesh_indices,
            object_ids,
            _brick_atlas: brick_atlas,
            brick_atlas_view,
            _surface_cache: surface_cache,
            surface_cache_view,
            _surface_display: surface_display,
            surface_display_view,
            _environment: environment,
            environment_view,
            sampler,
            segments: GrowBuffer::new(device, "segments", vertex, 1 << 16),
            segments_xray: GrowBuffer::new(device, "segments x-ray", vertex, 1 << 16),
            boxes: GrowBuffer::new(device, "boxes", vertex, 1 << 16),
            boxes_xray: GrowBuffer::new(device, "boxes x-ray", vertex, 1 << 16),
            samples: GrowBuffer::new(device, "samples", vertex, 1 << 20),
            cards: GrowBuffer::new(device, "card instances", vertex, 1 << 12),
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
            atlas_texture,
            formulas: HashMap::new(),
            last_layout: Layout::new(width, height),
            statics,
            gpu,
        };
        renderer.resize(width, height);
        Ok(renderer)
    }

    fn frame_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        target(device, "frame (RGBA8)", [width, height], FRAME_FORMAT, 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING)
    }

    fn frame_bind_group(device: &wgpu::Device, pipelines: &Pipelines, r: &RecordBuffers, bricks: &wgpu::TextureView, pictures: &wgpu::TextureView, cache: &wgpu::TextureView, environment: &wgpu::TextureView, sampler: &wgpu::Sampler) -> wgpu::BindGroup {
        fn buffer(binding: u32, b: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry { binding, resource: b.as_entire_binding() }
        }
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout: &pipelines.frame_layout,
            entries: &[
                buffer(0, &r.uniforms),
                buffer(1, &r.objects.buffer),
                buffer(2, &r.voxels.buffer),
                buffer(3, &r.bricks.buffer),
                buffer(4, &r.cards.buffer),
                buffer(5, &r.pages.buffer),
                buffer(6, &r.rays.buffer),
                buffer(7, &r.probes.buffer),
                buffer(8, &r.radiance.buffer),
                wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::TextureView(bricks) },
                wgpu::BindGroupEntry { binding: 10, resource: wgpu::BindingResource::TextureView(pictures) },
                wgpu::BindGroupEntry { binding: 11, resource: wgpu::BindingResource::Sampler(sampler) },
                buffer(12, &r.mesh_nodes),
                buffer(13, &r.mesh_triangles),
                wgpu::BindGroupEntry { binding: 14, resource: wgpu::BindingResource::TextureView(cache) },
                wgpu::BindGroupEntry { binding: 15, resource: wgpu::BindingResource::TextureView(environment) },
            ],
        })
    }

    #[cfg(commercial)]
    fn cache_inputs(device: &wgpu::Device, cache_gpu: &SurfaceCacheGpu, r: &RecordBuffers, atlas: &wgpu::TextureView, environment: &wgpu::TextureView) -> wgpu::BindGroup {
        cache_gpu.inputs(device, &CacheInputs {
            uniforms: &r.uniforms,
            objects: &r.objects.buffer,
            cards: &r.cards.buffer,
            pages: &r.pages.buffer,
            mesh_nodes: &r.mesh_nodes,
            mesh_triangles: &r.mesh_triangles,
            atlas,
            environment,
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

    /// `dock`: the interactive viewer's layout with the slider dock.
    pub fn layout_for(&self, state: &ShotState, beat_kind: BeatKind, dock: bool) -> Layout {
        let panel = beat_kind != BeatKind::Scene || state.panel != crate::lab::shot::PanelMode::Hidden;
        let mut layout = self.layout();
        if dock {
            layout = layout.with_dock();
        }
        if beat_kind == BeatKind::Scene && state.panel == crate::lab::shot::PanelMode::Code {
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
        let r = &mut self.records;
        let mut grown = r.objects.write(device, queue, &content.objects);
        grown |= r.cards.write(device, queue, &content.cards);
        grown |= r.rays.write(device, queue, std::slice::from_ref(&content.ray));
        grown |= r.probes.write(device, queue, &content.probes);
        grown |= r.radiance.write(device, queue, &content.radiance);
        if grown {
            self.frame_bind_group = Self::frame_bind_group(device, &self.pipelines, &self.records, &self.brick_atlas_view, &self.surface_display_view, &self.surface_cache_view, &self.environment_view, &self.sampler);
            #[cfg(commercial)]
            {
                self.cache_inputs = Self::cache_inputs(device, &self.cache_gpu, &self.records, &self.surface_cache_view, &self.environment_view);
                self.cache_gpu.invalidate();
            }
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
        self.samples.write(device, queue, &d.samples);
        self.cards.write(device, queue, &d.cards);
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
        let dock = matches!(&request.chrome, Chrome::Viewer(v) if !(v.timeline_mode && v.playing));
        let layout = self.layout_for(request.state, request.beat_kind, dock);
        self.last_layout = layout;
        let vp = layout.viewport_px();
        let viewport_size = [vp[2], vp[3]];
        if self.viewport.as_ref().is_none_or(|v| v.size != viewport_size) {
            self.chain = None;
            self.viewport = Some(ViewportTargets::new(&self.gpu.device, &self.pipelines, viewport_size));
        }
        let scene_beat = request.beat_kind == BeatKind::Scene;
        #[cfg(commercial)]
        if let Some(setting) = investigation::current().prepared_setting(request.state) {
            self.cache_gpu.rebuild(&self.gpu.device, &self.gpu.queue, &mut self.statics, [&self.records.mesh_nodes, &self.records.mesh_triangles], &self.environment_view, &self._surface_cache, setting);
        }
        let content = match prepared {
            Some(c) => Some(c),
            None => scene_beat.then(|| investigation::current().build_frame(request.state, &self.statics, [vp[2] as f32, vp[3] as f32], layout.scale, request.time)),
        };
        if let Some(content) = &content {
            self.upload(content);
            self.upload_draws(content);
        }
        let formula = content.as_ref().and_then(|c| c.readout.as_ref()?.formula).and_then(|key| self.formula_texture(key));

        // egui pass content.
        let mut raw = request.raw_input;
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, layout.canvas));
        raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(layout.scale);
        let ui_input = UiInput {
            layout,
            state: request.state,
            content: content.as_ref(),
            statics: &self.statics,
            atlas_texture: self.atlas_texture,
            beat_kind: request.beat_kind,
            image: request.image,
            formula,
            beat_sources: request.beat_sources,
            chapter_number: request.chapter_number,
            fade: request.fade,
            stamp: request.stamp,
        };
        let mut chrome = request.chrome;
        let mut report = UiReport::default();
        // Start every frame with an empty glyph atlas. Otherwise the atlas
        // grows and repacks as new characters appear (hundreds in Chinese),
        // which moves glyph texels and changes text pixels by a level, so a
        // frame would depend on the frames rendered before it.
        self.egui_ctx.set_glyph_rasterizers(Vec::new());
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
            self.encode_scene(&mut encoder, content);
        }
        let targets = self.viewport.as_ref().unwrap();

        {
            let bg = crate::lab::palette::BACKGROUND;
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("composite + ui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.frame_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            // The frame is UNORM and holds sRGB-encoded bytes, so the
                            // clear colour is the encoded background colour itself.
                            load: wgpu::LoadOp::Clear(wgpu::Color { r: bg.r() as f64 / 255.0, g: bg.g() as f64 / 255.0, b: bg.b() as f64 / 255.0, a: 1.0 }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            if scene_beat {
                pass.set_viewport(vp[0] as f32, vp[1] as f32, vp[2] as f32, vp[3] as f32, 0.0, 1.0);
                pass.set_pipeline(&self.pipelines.composite);
                pass.set_bind_group(0, &self.frame_bind_group, &[]);
                pass.set_bind_group(1, &targets.composite, &[]);
                pass.draw(0..3, 0..1);
                pass.set_viewport(0.0, 0.0, self.width as f32, self.height as f32, 0.0, 1.0);
            }
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

    /// A readout's formula as a texture. Its upload is part of the first frame
    /// that shows it, and the texture never changes, so frames stay deterministic.
    fn formula_texture(&mut self, key: &'static str) -> Option<(egui::TextureId, egui::Vec2)> {
        if !self.formulas.contains_key(key) {
            let image = crate::lab::record::decode_png(std::io::Cursor::new(investigation::current().formula(key)?)).ok()?;
            let size = egui::vec2(image.size[0] as f32, image.size[1] as f32);
            let handle = self.egui_ctx.load_texture(format!("formula {key}"), image, egui::TextureOptions::LINEAR);
            self.formulas.insert(key, (handle, size));
        }
        self.formulas.get(key).map(|(h, s)| (h.id(), *s))
    }

    /// The Surface Cache picture, then the scene and overlay passes into the
    /// viewport targets (the composite reads them).
    fn encode_scene(&mut self, encoder: &mut wgpu::CommandEncoder, content: &FrameContent) {
        // The Surface Cache picture the card quads and the atlas inset show.
        #[cfg(commercial)]
        {
            let params = content.cache_display;
            let view = [content.uniforms.overlay_params[0], content.uniforms.cache_view[0]];
            self.cache_gpu.display(&self.gpu.queue, encoder, &self.cache_inputs, params, view);
        }
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
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Discard }),
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
        draw(&mut pass, &self.pipelines.card, &self.cards, 6);
        draw(&mut pass, &self.pipelines.sample, &self.samples, 36);
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
        let vp = layout.viewport_px();
        if self.viewport.as_ref().is_none_or(|v| v.size != [vp[2], vp[3]]) {
            self.chain = None;
            self.viewport = Some(ViewportTargets::new(&self.gpu.device, &self.pipelines, [vp[2], vp[3]]));
        }
        let content = state.map(|s| investigation::current().build_frame(s, &self.statics, [vp[2] as f32, vp[3] as f32], layout.scale, time));
        if let Some(content) = &content {
            self.upload(content);
            self.upload_draws(content);
        }
        let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("backdrop") });
        if let Some(content) = &content {
            self.encode_scene(&mut encoder, content);
        }
        {
            let bg = crate::lab::palette::BACKGROUND;
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
            "voxels" => &self.records.voxels.buffer,
            "bricks" => &self.records.bricks.buffer,
            "cards" => &self.records.cards.buffer,
            "pages" => &self.records.pages.buffer,
            "rays" => &self.records.rays.buffer,
            "probes" => &self.records.probes.buffer,
            "radiance" => &self.records.radiance.buffer,
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

/// A one-texel texture for a binding nothing reads in this build.
#[cfg(not(commercial))]
fn placeholder(device: &wgpu::Device, label: &str, dimension: wgpu::TextureDimension, format: wgpu::TextureFormat) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: if format == wgpu::TextureFormat::Rgba8Unorm { &[wgpu::TextureFormat::Rgba8UnormSrgb] } else { &[] },
    })
}

/// Upload a cache lit on the CPU; every radiosity snapshot is the finished layer.
#[cfg(commercial)]
fn upload_lit_cache(queue: &wgpu::Queue, atlas: &wgpu::Texture, cache: &crate::lab::surface_cache::SurfaceCache) {
    use crate::lab::surface_cache::Layer;
    let [w, h] = cache.atlas_size;
    let write = |layer: u32, data: &[u32]| {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: atlas, mip_level: 0, origin: wgpu::Origin3d { x: 0, y: 0, z: layer }, aspect: wgpu::TextureAspect::All },
            bytemuck::cast_slice(data),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
    };
    for (i, &layer) in Layer::ALL.iter().enumerate() {
        write(i as u32, cache.layer(layer));
    }
    for frame in 0..=crate::lab::surface_cache::RADIOSITY_FRAMES {
        write(surface_cache_gpu::HISTORY_FINAL + frame, cache.layer(Layer::FinalLighting));
        write(surface_cache_gpu::HISTORY_INDIRECT + frame, cache.layer(Layer::IndirectLighting));
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
