//! A scene shading's program (`ShadingProgram`, todo/pass-programs.md): the
//! investigation's own images, buffers, and passes, and the commands its plan
//! returns each frame, recorded in order at their passes' stages. Group 2 of
//! a pass holds the command's 16 words of parameters (binding 0, at a dynamic
//! offset; wgpu keeps dynamic offsets out of a group with binding arrays),
//! group 3 its uses; group 3 of the scene pass holds `scene_uses`.

use crate::investigation::{Access, Element, ImageKind, PassKind, ProgramCommand, ProgramFrame, ProgramStage, ProgramUse, ShadingProgram, Which};
use std::collections::HashMap;
use std::num::NonZeroU32;

/// Bytes between two commands' parameters (a dynamic offset the device accepts).
pub const PARAMS_STRIDE: u64 = 256;
/// The most dispatches and draws one frame's plan may return.
pub const COMMANDS_MAX: u64 = 4096;
/// The format of a raster pass's attachment, which it never writes.
const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

pub enum PassPipeline {
    Compute(wgpu::ComputePipeline),
    Raster(wgpu::RenderPipeline),
}

pub struct ProgramPipelines {
    /// Each pass's pipeline and its group-3 layout, in `passes` order.
    pub passes: Vec<(PassPipeline, wgpu::BindGroupLayout)>,
    /// Group 2: the command's parameters.
    pub params: wgpu::BindGroupLayout,
    /// Group 3 of the scene pass, when it reads the program.
    pub scene: Option<wgpu::BindGroupLayout>,
}

fn view_dimension(kind: ImageKind) -> wgpu::TextureViewDimension {
    match kind {
        ImageKind::D2 => wgpu::TextureViewDimension::D2,
        ImageKind::D2Array => wgpu::TextureViewDimension::D2Array,
        ImageKind::D3 => wgpu::TextureViewDimension::D3,
    }
}

fn binding_type(program: &ShadingProgram, u: &ProgramUse) -> (wgpu::BindingType, Option<NonZeroU32>) {
    if let Some(image) = program.images.iter().find(|i| i.name == u.resource) {
        let view_dimension = view_dimension(image.kind);
        let count = (u.which == Which::All).then(|| NonZeroU32::new(image.count.max(1)).expect("a count"));
        let storage = |access| (wgpu::BindingType::StorageTexture { access, format: image.format, view_dimension }, count);
        return match u.access {
            Access::Sampled => (wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension, multisampled: false }, count),
            Access::Load => {
                let sample_type = match image.format.sample_type(None, None) {
                    Some(wgpu::TextureSampleType::Float { .. }) | None => wgpu::TextureSampleType::Float { filterable: false },
                    Some(t) => t,
                };
                (wgpu::BindingType::Texture { sample_type, view_dimension, multisampled: false }, count)
            }
            Access::Read => storage(wgpu::StorageTextureAccess::ReadOnly),
            Access::Write => storage(wgpu::StorageTextureAccess::WriteOnly),
            Access::ReadWrite => storage(wgpu::StorageTextureAccess::ReadWrite),
            Access::Atomic => storage(wgpu::StorageTextureAccess::Atomic),
            Access::Uniform | Access::Sampler => panic!("image `{}` used as {:?}", u.resource, u.access),
        };
    }
    if program.buffers.iter().any(|b| b.name == u.resource) {
        let ty = match u.access {
            Access::Uniform => wgpu::BufferBindingType::Uniform,
            Access::Read => wgpu::BufferBindingType::Storage { read_only: true },
            Access::Write | Access::ReadWrite | Access::Atomic => wgpu::BufferBindingType::Storage { read_only: false },
            _ => panic!("buffer `{}` used as {:?}", u.resource, u.access),
        };
        return (wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None }, None);
    }
    (wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), None)
}

/// Group 3 of a pass or of the scene pass.
fn group_layout(device: &wgpu::Device, program: &ShadingProgram, label: &str, uses: &[ProgramUse], visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayout {
    let mut entries = Vec::new();
    for u in uses {
        let (ty, count) = binding_type(program, u);
        // A raster pass writes storage from its fragments (vertex stages may not, without a feature).
        let visibility = if u.access.writes() && visibility.contains(wgpu::ShaderStages::FRAGMENT) { wgpu::ShaderStages::FRAGMENT } else { visibility };
        entries.push(wgpu::BindGroupLayoutEntry { binding: u.binding, visibility, ty, count });
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries: &entries })
}

/// The scene pass's group 3, for its pipeline layout (`Pipelines::shading`).
pub fn scene_layout(device: &wgpu::Device, program: &ShadingProgram) -> Option<wgpu::BindGroupLayout> {
    // The scene pass and the chain's compute passes (a shading without volumes) bind it.
    (!program.scene_uses.is_empty()).then(|| group_layout(device, program, "program (scene)", program.scene_uses, wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE))
}

/// Every pass's pipeline, from the shading's module.
pub fn pipelines(device: &wgpu::Device, module: &wgpu::ShaderModule, program: &ShadingProgram, frame: &wgpu::BindGroupLayout, shading: &wgpu::BindGroupLayout, scene: Option<wgpu::BindGroupLayout>) -> ProgramPipelines {
    let params = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("program parameters"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: wgpu::BufferSize::new(64) },
            count: None,
        }],
    });
    let passes = program
        .passes
        .iter()
        .map(|pass| {
            let visibility = match pass.kind {
                PassKind::Compute { .. } => wgpu::ShaderStages::COMPUTE,
                PassKind::Raster { .. } => wgpu::ShaderStages::VERTEX_FRAGMENT,
            };
            let group = group_layout(device, program, pass.name, pass.uses, visibility);
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(pass.name), bind_group_layouts: &[Some(frame), Some(shading), Some(&params), Some(&group)], immediate_size: 0 });
            let pipeline = match pass.kind {
                PassKind::Compute { entry, .. } => PassPipeline::Compute(device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(pass.name),
                    layout: Some(&layout),
                    module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })),
                PassKind::Raster { vertex, fragment } => {
                    let [mesh, objects] = crate::pipelines::mesh_layouts();
                    PassPipeline::Raster(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(pass.name),
                        layout: Some(&layout),
                        vertex: wgpu::VertexState { module, entry_point: Some(vertex), compilation_options: Default::default(), buffers: &[Some(mesh), Some(objects)] },
                        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
                        depth_stencil: None,
                        multisample: wgpu::MultisampleState::default(),
                        fragment: Some(wgpu::FragmentState {
                            module,
                            entry_point: Some(fragment),
                            compilation_options: Default::default(),
                            targets: &[Some(wgpu::ColorTargetState { format: TARGET_FORMAT, blend: None, write_mask: wgpu::ColorWrites::empty() })],
                        }),
                        multiview_mask: None,
                        cache: None,
                    }))
                }
            };
            (pipeline, group)
        })
        .collect();
    ProgramPipelines { passes, params, scene }
}

/// The scene's meshes, as the renderer draws them.
pub struct Meshes<'a> {
    pub vertices: &'a wgpu::Buffer,
    pub objects: &'a wgpu::Buffer,
    pub indices: &'a wgpu::Buffer,
    pub ranges: &'a [std::ops::Range<u32>],
}

/// A program's resource, for the inspector (cheap handles, cloned).
#[derive(Clone)]
pub enum ProgramHandle {
    Image { kind: ImageKind, textures: Vec<wgpu::Texture> },
    Buffer(Vec<wgpu::Buffer>),
}

impl ProgramHandle {
    /// Image `index`'s whole view, as the inspector binds it (a 2D image of an integer format as a one-layer array).
    pub fn view(&self, index: u32) -> Option<wgpu::TextureView> {
        let ProgramHandle::Image { kind, textures } = self else { return None };
        let t = textures.get(index as usize).or(textures.first())?;
        let integer = crate::graph::image_class(t.format()) != 0;
        let dimension = match kind {
            ImageKind::D2 if integer => wgpu::TextureViewDimension::D2Array,
            k => view_dimension(*k),
        };
        Some(t.create_view(&wgpu::TextureViewDescriptor { dimension: Some(dimension), ..Default::default() }))
    }
}

/// A program's resources, bind groups, plan state, and this frame's commands.
pub struct ProgramRuntime {
    pub source: &'static ShadingProgram,
    images: Vec<Vec<wgpu::Texture>>,
    buffers: Vec<Vec<wgpu::Buffer>>,
    params: wgpu::Buffer,
    params_group: wgpu::BindGroup,
    /// Each pass's bind group per instance.
    groups: Vec<Vec<wgpu::BindGroup>>,
    pub scene_group: Option<wgpu::BindGroup>,
    targets: HashMap<[u32; 2], wgpu::TextureView>,
    state: Box<dyn std::any::Any + Send>,
    /// The commands the last plan returned, until they are recorded.
    commands: Vec<ProgramCommand>,
    /// The upload data of those commands, in one buffer, and each upload's offset in it.
    uploads: Option<(wgpu::Buffer, Vec<u64>)>,
}

fn usage_of(program: &ShadingProgram, name: &str) -> wgpu::TextureUsages {
    let mut usage = wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING;
    for u in program.passes.iter().flat_map(|p| p.uses.iter()).chain(program.scene_uses).filter(|u| u.resource == name) {
        match u.access {
            Access::Read | Access::Write | Access::ReadWrite => usage |= wgpu::TextureUsages::STORAGE_BINDING,
            Access::Atomic => usage |= wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::STORAGE_ATOMIC,
            _ => {}
        }
    }
    usage
}

impl ProgramRuntime {
    pub fn new(device: &wgpu::Device, program: &'static ShadingProgram, pipelines: &ProgramPipelines) -> ProgramRuntime {
        let images: Vec<Vec<wgpu::Texture>> = program
            .images
            .iter()
            .map(|i| {
                (0..i.count.max(1))
                    .map(|_| {
                        device.create_texture(&wgpu::TextureDescriptor {
                            label: Some(i.name),
                            size: wgpu::Extent3d { width: i.size[0], height: i.size[1], depth_or_array_layers: i.size[2].max(1) },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: if i.kind == ImageKind::D3 { wgpu::TextureDimension::D3 } else { wgpu::TextureDimension::D2 },
                            format: i.format,
                            usage: usage_of(program, i.name),
                            view_formats: &[],
                        })
                    })
                    .collect()
            })
            .collect();
        let buffers: Vec<Vec<wgpu::Buffer>> = program
            .buffers
            .iter()
            .map(|b| {
                let mut usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
                if b.indirect {
                    usage |= wgpu::BufferUsages::INDIRECT;
                }
                if program.passes.iter().flat_map(|p| p.uses.iter()).any(|u| u.resource == b.name && u.access == Access::Uniform) {
                    usage |= wgpu::BufferUsages::UNIFORM;
                }
                (0..b.count.max(1)).map(|_| device.create_buffer(&wgpu::BufferDescriptor { label: Some(b.name), size: b.bytes.max(16).next_multiple_of(4), usage, mapped_at_creation: false })).collect()
            })
            .collect();
        let params = device.create_buffer(&wgpu::BufferDescriptor { label: Some("program parameters"), size: COMMANDS_MAX * PARAMS_STRIDE, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let sampler = |filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("program sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        };
        let (linear, nearest) = (sampler(wgpu::FilterMode::Linear), sampler(wgpu::FilterMode::Nearest));
        // Every image's views, each as a whole of its kind.
        let views: Vec<Vec<wgpu::TextureView>> = program
            .images
            .iter()
            .zip(&images)
            .map(|(i, textures)| textures.iter().map(|t| t.create_view(&wgpu::TextureViewDescriptor { dimension: Some(view_dimension(i.kind)), ..Default::default() })).collect())
            .collect();
        let group = |label: &str, layout: &wgpu::BindGroupLayout, uses: &[ProgramUse], instance: u32| -> wgpu::BindGroup {
            let element = |which: Which| match which {
                Which::First | Which::All => 0,
                Which::Instance => instance as usize,
                Which::Fixed(k) => k as usize,
                Which::Pick(table) => table.get(instance as usize).copied().unwrap_or(0) as usize,
            };
            let refs: Vec<Vec<&wgpu::TextureView>> = views.iter().map(|v| v.iter().collect()).collect();
            let mut entries = Vec::new();
            for u in uses {
                let resource = if let Some(k) = program.images.iter().position(|i| i.name == u.resource) {
                    if u.which == Which::All { wgpu::BindingResource::TextureViewArray(&refs[k]) } else { wgpu::BindingResource::TextureView(&views[k][element(u.which).min(views[k].len() - 1)]) }
                } else if let Some(k) = program.buffers.iter().position(|b| b.name == u.resource) {
                    buffers[k][element(u.which).min(buffers[k].len() - 1)].as_entire_binding()
                } else if u.resource == "nearestSampler" {
                    wgpu::BindingResource::Sampler(&nearest)
                } else {
                    wgpu::BindingResource::Sampler(&linear)
                };
                entries.push(wgpu::BindGroupEntry { binding: u.binding, resource });
            }
            device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(label), layout, entries: &entries })
        };
        let groups = program.passes.iter().zip(&pipelines.passes).map(|(p, (_, layout))| (0..p.instances.max(1)).map(|i| group(p.name, layout, p.uses, i)).collect()).collect();
        let scene_group = pipelines.scene.as_ref().map(|layout| group("program (scene)", layout, program.scene_uses, 0));
        let params_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("program parameters"),
            layout: &pipelines.params,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &params, offset: 0, size: wgpu::BufferSize::new(64) }) }],
        });
        ProgramRuntime { source: program, images, buffers, params, params_group, groups, scene_group, targets: HashMap::new(), state: (program.init)(), commands: Vec::new(), uploads: None }
    }

    /// Runs the plan for this frame: its commands, their parameters, and their uploads, ready to record.
    pub fn plan(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &ProgramFrame) {
        use wgpu::util::DeviceExt;
        let commands = (self.source.plan)(&mut *self.state, frame);
        let mut bytes = Vec::new();
        let mut n = 0u64;
        let mut upload: Vec<u32> = Vec::new();
        let mut offsets = Vec::new();
        for c in &commands {
            match c {
                ProgramCommand::Dispatch { params, .. } | ProgramCommand::DispatchIndirect { params, .. } | ProgramCommand::Draw { params, .. } => {
                    assert!(n < COMMANDS_MAX, "a program's plan returned more than {COMMANDS_MAX} dispatches and draws");
                    bytes.resize((n * PARAMS_STRIDE) as usize, 0);
                    bytes.extend_from_slice(bytemuck::cast_slice(params));
                    n += 1;
                }
                ProgramCommand::Upload { words, .. } => {
                    offsets.push(upload.len() as u64 * 4);
                    upload.extend_from_slice(words);
                }
                _ => {}
            }
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.params, 0, &bytes);
        }
        self.uploads = (!upload.is_empty()).then(|| (device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("program uploads"), contents: bytemuck::cast_slice(&upload), usage: wgpu::BufferUsages::COPY_SRC }), offsets));
        self.commands = commands;
    }

    /// Whether the plan left commands not yet recorded.
    pub fn pending(&self) -> bool {
        !self.commands.is_empty()
    }

    fn image(&self, name: &str) -> Option<usize> {
        self.source.images.iter().position(|i| i.name == name)
    }

    fn buffer(&self, e: Element) -> &wgpu::Buffer {
        let k = self.source.buffers.iter().position(|b| b.name == e.name).unwrap_or_else(|| panic!("a program has no buffer `{}`", e.name));
        self.buffers[k].get(e.index as usize).unwrap_or_else(|| panic!("buffer `{}` has no element {}", e.name, e.index))
    }

    /// Every resource by name, for the inspector and the selftests' readbacks.
    pub fn handles(&self) -> Vec<(&'static str, ProgramHandle)> {
        let images = self.source.images.iter().zip(&self.images).map(|(i, t)| (i.name, ProgramHandle::Image { kind: i.kind, textures: t.clone() }));
        let buffers = self.source.buffers.iter().zip(&self.buffers).map(|(b, x)| (b.name, ProgramHandle::Buffer(x.clone())));
        images.chain(buffers).collect()
    }

    /// The bytes of a buffer or an image, read back (a selftest): the texels
    /// of image `index` row by row, layer by layer.
    pub fn texture(&self, name: &str, index: u32) -> Option<&wgpu::Texture> {
        let k = self.image(name)?;
        self.images[k].get(index as usize)
    }

    /// Records the commands of `stage` (each upload, clear, and copy at the
    /// stage of the next pass command after it), calling `after` with each
    /// pass's name once its consecutive commands are recorded (the inspector).
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &ProgramPipelines,
        stage: ProgramStage,
        frame: &wgpu::BindGroup,
        shading: &wgpu::BindGroup,
        meshes: &Meshes,
        mut timing: Option<&mut crate::timing::Timing>,
        after: &mut dyn FnMut(&mut wgpu::CommandEncoder, &'static str),
    ) {
        if self.commands.is_empty() {
            return;
        }
        let commands = std::mem::take(&mut self.commands);
        let source = self.source;
        let pass_index = |name: &str| source.passes.iter().position(|p| p.name == name).unwrap_or_else(|| panic!("a program has no pass `{name}`"));
        // Each command's stage, and its parameters' slot.
        let mut stages = vec![ProgramStage::Start; commands.len()];
        let mut next = None;
        for (i, c) in commands.iter().enumerate().rev() {
            if let Some(p) = c.pass() {
                next = Some(source.passes[pass_index(p)].stage);
            }
            stages[i] = next.unwrap_or(ProgramStage::BeforeScene);
        }
        let mut slot = 0u64;
        let mut slots = vec![0u64; commands.len()];
        let mut upload = 0usize;
        let mut uploads = vec![0usize; commands.len()];
        for (i, c) in commands.iter().enumerate() {
            slots[i] = slot;
            uploads[i] = upload;
            if c.pass().is_some() {
                slot += 1;
            }
            if matches!(c, ProgramCommand::Upload { .. }) {
                upload += 1;
            }
        }
        let mut i = 0;
        let count = commands.len();
        while i < count {
            if stages[i] != stage {
                i += 1;
                continue;
            }
            match &commands[i] {
                ProgramCommand::Upload { buffer, offset, words } => {
                    let (staging, offsets) = self.uploads.as_ref().expect("the uploads");
                    let to = self.buffer(*buffer);
                    encoder.copy_buffer_to_buffer(staging, offsets[uploads[i]], to, *offset, words.len() as u64 * 4);
                    i += 1;
                }
                ProgramCommand::Clear(e) => {
                    let pick = |k: usize| e.index == u32::MAX || k == e.index as usize;
                    if let Some(b) = self.source.buffers.iter().position(|b| b.name == e.name) {
                        for (_, x) in self.buffers[b].iter().enumerate().filter(|(k, _)| pick(*k)) {
                            encoder.clear_buffer(x, 0, None);
                        }
                    } else {
                        let k = self.image(e.name).unwrap_or_else(|| panic!("a program has no image or buffer `{}`", e.name));
                        for (_, t) in self.images[k].iter().enumerate().filter(|(k, _)| pick(*k)) {
                            encoder.clear_texture(t, &wgpu::ImageSubresourceRange::default());
                        }
                    }
                    i += 1;
                }
                ProgramCommand::Copy { from, to, bytes } => {
                    encoder.copy_buffer_to_buffer(self.buffer(*from), 0, self.buffer(*to), 0, *bytes);
                    i += 1;
                }
                ProgramCommand::Dispatch { pass, .. } | ProgramCommand::DispatchIndirect { pass, .. } => {
                    let p = pass_index(pass);
                    let PassPipeline::Compute(pipeline) = &pipelines.passes[p].0 else { panic!("`{pass}` is a raster pass: draw it") };
                    let mut end = i;
                    while end < count && stages[end] == stage && commands[end].pass() == Some(*pass) && !matches!(commands[end], ProgramCommand::Draw { .. }) {
                        end += 1;
                    }
                    let span = timing.as_mut().and_then(|t| t.compute_writes(pass));
                    let mut cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(pass), timestamp_writes: span.as_ref().map(|t| t.compute()) });
                    cp.set_pipeline(pipeline);
                    cp.set_bind_group(0, frame, &[]);
                    cp.set_bind_group(1, shading, &[]);
                    for k in i..end {
                        match &commands[k] {
                            ProgramCommand::Dispatch { groups, instance, .. } => {
                                cp.set_bind_group(2, &self.params_group, &[(slots[k] * PARAMS_STRIDE) as u32]);
                                cp.set_bind_group(3, &self.groups[p][*instance as usize], &[]);
                                if groups.iter().all(|g| *g > 0) {
                                    cp.dispatch_workgroups(groups[0], groups[1], groups[2]);
                                }
                            }
                            ProgramCommand::DispatchIndirect { buffer, offset, instance, .. } => {
                                cp.set_bind_group(2, &self.params_group, &[(slots[k] * PARAMS_STRIDE) as u32]);
                                cp.set_bind_group(3, &self.groups[p][*instance as usize], &[]);
                                cp.dispatch_workgroups_indirect(self.buffer(*buffer), *offset);
                            }
                            _ => unreachable!(),
                        }
                    }
                    drop(cp);
                    after(encoder, pass);
                    i = end;
                }
                ProgramCommand::Draw { pass, .. } => {
                    let p = pass_index(pass);
                    let PassPipeline::Raster(pipeline) = &pipelines.passes[p].0 else { panic!("`{pass}` is a compute pass: dispatch it") };
                    let mut end = i;
                    while end < count && stages[end] == stage && commands[end].pass() == Some(*pass) && matches!(commands[end], ProgramCommand::Draw { .. }) {
                        end += 1;
                    }
                    let span = timing.as_mut().and_then(|t| t.render_writes(pass));
                    for k in i..end {
                        let ProgramCommand::Draw { size, instance, .. } = &commands[k] else { unreachable!() };
                        let size = [size[0].max(1), size[1].max(1)];
                        let target = self.targets.entry(size).or_insert_with(|| {
                            device
                                .create_texture(&wgpu::TextureDescriptor {
                                    label: Some("program raster target"),
                                    size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                                    mip_level_count: 1,
                                    sample_count: 1,
                                    dimension: wgpu::TextureDimension::D2,
                                    format: TARGET_FORMAT,
                                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                                    view_formats: &[],
                                })
                                .create_view(&Default::default())
                        });
                        let writes = span.as_ref().filter(|_| k == i || k + 1 == end).map(|t| t.span(k == i, k + 1 == end));
                        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some(pass),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Discard } })],
                            depth_stencil_attachment: None,
                            timestamp_writes: writes,
                            occlusion_query_set: None,
                            multiview_mask: None,
                        });
                        rp.set_pipeline(pipeline);
                        rp.set_bind_group(0, frame, &[]);
                        rp.set_bind_group(1, shading, &[]);
                        rp.set_bind_group(2, &self.params_group, &[(slots[k] * PARAMS_STRIDE) as u32]);
                        rp.set_bind_group(3, &self.groups[p][*instance as usize], &[]);
                        rp.set_vertex_buffer(0, meshes.vertices.slice(..));
                        rp.set_vertex_buffer(1, meshes.objects.slice(..));
                        rp.set_index_buffer(meshes.indices.slice(..), wgpu::IndexFormat::Uint32);
                        for (o, range) in meshes.ranges.iter().enumerate() {
                            rp.draw_indexed(range.clone(), 0, o as u32..o as u32 + 1);
                        }
                    }
                    after(encoder, pass);
                    i = end;
                }
            }
        }
        if stage == ProgramStage::BeforeScene {
            self.uploads = None;
        } else {
            self.commands = commands;
        }
    }
}
