//! A scene shading's program (`ShadingProgram`; the `render-graph` agent skill): the
//! investigation's own images, buffers, and passes, and the commands its plan
//! returns each frame, recorded in order where their passes' nodes are in the
//! frame (`SceneShading::program_runs`). Group 2 of
//! a pass holds the command's 16 words of parameters (binding 0, at a dynamic
//! offset; wgpu keeps dynamic offsets out of a group with binding arrays),
//! group 3 its uses; group 3 of the scene pass holds `scene_uses`.

use crate::investigation::{
    Access, Element, ImageKind, PassKind, ProgramCommand, ProgramFrame, ProgramImage, ProgramUse,
    SceneShading, ShadingProgram, Which,
};
use std::collections::HashMap;
use std::num::NonZeroU32;

/// Bytes between two commands' parameters (a dynamic offset the device accepts).
pub const PARAMS_STRIDE: u64 = 256;
/// The most dispatches and draws one frame's plan may return.
pub const COMMANDS_MAX: u64 = 4096;
/// The run of a plan's commands after its last pass command: recorded right
/// before the scene pass.
pub const BEFORE_SCENE: u32 = u32::MAX;
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

/// One of the frame's textures a program pass may read (`ProgramUse`): its
/// format and whether it is multisampled; None for a program's own resource.
pub fn external(shading: &SceneShading, name: &str) -> Option<(wgpu::TextureFormat, bool)> {
    let program = shading.program?;
    if program.images.iter().any(|i| i.name == name)
        || program.buffers.iter().any(|b| b.name == name)
    {
        return None;
    }
    match name {
        "depth" => Some((crate::gpu::DEPTH_FORMAT, true)),
        "hdr" => Some((crate::gpu::HDR_FORMAT, false)),
        _ => shading
            .targets
            .iter()
            .find(|t| t.name == name)
            .map(|t| (t.format, shading.attachment(t.name))),
    }
}

/// Whether a program's groups wait for the frame's textures (`ProgramRuntime::bind`):
/// it has images the size of the view, or uses the frame's textures.
fn needs_bind(shading: &SceneShading, program: &ShadingProgram) -> bool {
    program.images.iter().any(|i| i.viewport > 0)
        || program
            .passes
            .iter()
            .flat_map(|p| p.uses.iter())
            .any(|u| external(shading, u.resource).is_some())
}

/// An image's mip levels at `size`.
fn levels(i: &ProgramImage, size: [u32; 3]) -> u32 {
    let largest = if i.kind == ImageKind::D3 {
        size[0].max(size[1]).max(size[2])
    } else {
        size[0].max(size[1])
    };
    i.mips.max(1).min(32 - largest.max(1).leading_zeros())
}

/// An image's size for a view of `viewport` pixels.
#[allow(clippy::manual_checked_ops)] // a fixed size or a fraction of the view, not a failed division
fn image_size(i: &ProgramImage, viewport: [u32; 2]) -> [u32; 3] {
    if i.viewport == 0 {
        [i.size[0], i.size[1], i.size[2].max(1)]
    } else {
        [
            (viewport[0] / i.viewport).max(1),
            (viewport[1] / i.viewport).max(1),
            i.size[2].max(1),
        ]
    }
}

fn binding_type(
    shading: &SceneShading,
    program: &ShadingProgram,
    u: &ProgramUse,
) -> (wgpu::BindingType, Option<NonZeroU32>) {
    if let Some((format, multisampled)) = external(shading, u.resource) {
        let sample_type = if format == crate::gpu::DEPTH_FORMAT {
            wgpu::TextureSampleType::Depth
        } else {
            match format.sample_type(None, None) {
                Some(wgpu::TextureSampleType::Float { .. }) | None => {
                    wgpu::TextureSampleType::Float { filterable: false }
                }
                Some(t) => t,
            }
        };
        assert!(
            u.access == Access::Load,
            "a program reads the frame's `{}` with Load, not {:?}",
            u.resource,
            u.access
        );
        return (
            wgpu::BindingType::Texture {
                sample_type,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled,
            },
            None,
        );
    }
    if let Some(image) = program.images.iter().find(|i| i.name == u.resource) {
        let view_dimension = view_dimension(image.kind);
        let count =
            (u.which == Which::All).then(|| NonZeroU32::new(image.count.max(1)).expect("a count"));
        let storage = |access| {
            (
                wgpu::BindingType::StorageTexture {
                    access,
                    format: image.format,
                    view_dimension,
                },
                count,
            )
        };
        return match u.access {
            Access::Sampled => (
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension,
                    multisampled: false,
                },
                count,
            ),
            Access::Load => {
                let sample_type = match image.format.sample_type(None, None) {
                    Some(wgpu::TextureSampleType::Float { .. }) | None => {
                        wgpu::TextureSampleType::Float { filterable: false }
                    }
                    Some(t) => t,
                };
                (
                    wgpu::BindingType::Texture {
                        sample_type,
                        view_dimension,
                        multisampled: false,
                    },
                    count,
                )
            }
            Access::Read => storage(wgpu::StorageTextureAccess::ReadOnly),
            Access::Write => storage(wgpu::StorageTextureAccess::WriteOnly),
            Access::ReadWrite => storage(wgpu::StorageTextureAccess::ReadWrite),
            Access::Atomic => storage(wgpu::StorageTextureAccess::Atomic),
            Access::Uniform | Access::Sampler => {
                panic!("image `{}` used as {:?}", u.resource, u.access)
            }
        };
    }
    if program.buffers.iter().any(|b| b.name == u.resource) {
        let ty = match u.access {
            Access::Uniform => wgpu::BufferBindingType::Uniform,
            Access::Read => wgpu::BufferBindingType::Storage { read_only: true },
            Access::Write | Access::ReadWrite | Access::Atomic => {
                wgpu::BufferBindingType::Storage { read_only: false }
            }
            _ => panic!("buffer `{}` used as {:?}", u.resource, u.access),
        };
        return (
            wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            None,
        );
    }
    (
        wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        None,
    )
}

/// Group 3 of a pass or of the scene pass.
fn group_layout(
    device: &wgpu::Device,
    shading: &SceneShading,
    program: &ShadingProgram,
    label: &str,
    uses: &[ProgramUse],
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayout {
    let mut entries = Vec::new();
    for u in uses {
        let (ty, count) = binding_type(shading, program, u);
        // A raster pass writes storage from its fragments (vertex stages may not, without a feature).
        let visibility = if u.access.writes() && visibility.contains(wgpu::ShaderStages::FRAGMENT) {
            wgpu::ShaderStages::FRAGMENT
        } else {
            visibility
        };
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: u.binding,
            visibility,
            ty,
            count,
        });
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    })
}

/// The scene pass's group 3, for its pipeline layout (`Pipelines::shading`).
pub fn scene_layout(
    device: &wgpu::Device,
    shading: &SceneShading,
    program: &ShadingProgram,
) -> Option<wgpu::BindGroupLayout> {
    // The scene pass and the chain's compute passes (a shading without volumes) bind it.
    (!program.scene_uses.is_empty()).then(|| {
        group_layout(
            device,
            shading,
            program,
            "program (scene)",
            program.scene_uses,
            wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
        )
    })
}

/// Every pass's pipeline, from the shading's module.
pub fn pipelines(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    source: &SceneShading,
    program: &ShadingProgram,
    frame: &wgpu::BindGroupLayout,
    shading: &wgpu::BindGroupLayout,
    scene: Option<wgpu::BindGroupLayout>,
) -> ProgramPipelines {
    let params = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("program parameters"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(64),
            },
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
            let group = group_layout(device, source, program, pass.name, pass.uses, visibility);
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(pass.name),
                bind_group_layouts: &[Some(frame), Some(shading), Some(&params), Some(&group)],
                immediate_size: 0,
            });
            let pipeline = match pass.kind {
                PassKind::Compute { entry, .. } => PassPipeline::Compute(
                    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some(pass.name),
                        layout: Some(&layout),
                        module,
                        entry_point: Some(entry),
                        compilation_options: Default::default(),
                        cache: None,
                    }),
                ),
                PassKind::Raster { vertex, fragment } => {
                    let [mesh, objects] = crate::pipelines::mesh_layouts();
                    PassPipeline::Raster(device.create_render_pipeline(
                        &wgpu::RenderPipelineDescriptor {
                            label: Some(pass.name),
                            layout: Some(&layout),
                            vertex: wgpu::VertexState {
                                module,
                                entry_point: Some(vertex),
                                compilation_options: Default::default(),
                                buffers: &[Some(mesh), Some(objects)],
                            },
                            primitive: wgpu::PrimitiveState {
                                topology: wgpu::PrimitiveTopology::TriangleList,
                                cull_mode: None,
                                ..Default::default()
                            },
                            depth_stencil: None,
                            multisample: wgpu::MultisampleState::default(),
                            fragment: Some(wgpu::FragmentState {
                                module,
                                entry_point: Some(fragment),
                                compilation_options: Default::default(),
                                targets: &[Some(wgpu::ColorTargetState {
                                    format: TARGET_FORMAT,
                                    blend: None,
                                    write_mask: wgpu::ColorWrites::empty(),
                                })],
                            }),
                            multiview_mask: None,
                            cache: None,
                        },
                    ))
                }
            };
            (pipeline, group)
        })
        .collect();
    ProgramPipelines {
        passes,
        params,
        scene,
    }
}

/// Which of a program's runs to record: `run`, among `runs` (each pass's
/// run: `SceneShading::program_runs`).
#[derive(Clone, Copy)]
pub struct ProgramRun<'a> {
    pub runs: &'a [u32],
    pub run: u32,
}

/// What a program's passes draw with: its pipelines, the frame's and the
/// shading's bind groups, and the scene's meshes.
pub struct ProgramBindings<'a> {
    pub pipelines: &'a ProgramPipelines,
    pub frame: &'a wgpu::BindGroup,
    pub shading: &'a wgpu::BindGroup,
    pub meshes: Meshes<'a>,
}

/// The scene's meshes, as the renderer draws them.
pub struct Meshes<'a> {
    pub vertices: &'a wgpu::Buffer,
    pub objects: &'a wgpu::Buffer,
    pub indices: &'a wgpu::Buffer,
    pub ranges: &'a [std::ops::Range<u32>],
}

/// A program's resources by name, for the inspector.
pub type ProgramHandles = Vec<(&'static str, ProgramHandle)>;

/// A program's resource, for the inspector (cheap handles, cloned).
#[derive(Clone)]
pub enum ProgramHandle {
    Image {
        kind: ImageKind,
        textures: Vec<wgpu::Texture>,
    },
    Buffer(Vec<wgpu::Buffer>),
}

impl ProgramHandle {
    /// Image `index`'s view, as the inspector binds it (a 2D image of an integer format as a one-layer array):
    /// a 2D image's mip `level` alone (clamped to its levels), else the whole image.
    pub fn view(&self, index: u32, level: u32) -> Option<wgpu::TextureView> {
        let ProgramHandle::Image { kind, textures } = self else {
            return None;
        };
        let t = textures.get(index as usize).or(textures.first())?;
        let integer = crate::graph::image_class(t.format()) != 0;
        let dimension = match kind {
            ImageKind::D2 if integer => wgpu::TextureViewDimension::D2Array,
            k => view_dimension(*k),
        };
        let one = matches!(kind, ImageKind::D2) && t.mip_level_count() > 1;
        let base_mip_level = if one {
            level.min(t.mip_level_count() - 1)
        } else {
            0
        };
        Some(t.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(dimension),
            base_mip_level,
            mip_level_count: one.then_some(1),
            ..Default::default()
        }))
    }
}

/// A program's resources, bind groups, plan state, and this frame's commands.
pub struct ProgramRuntime {
    pub source: &'static ShadingProgram,
    shading: &'static SceneShading,
    images: Vec<Vec<wgpu::Texture>>,
    /// Each image's views: every element whole, and element 0's mip levels one by one.
    views: Vec<(Vec<wgpu::TextureView>, Vec<wgpu::TextureView>)>,
    samplers: [wgpu::Sampler; 3],
    /// What the groups were made for (`bind`): the view's size and the frame's textures.
    bound: Option<([u32; 2], usize)>,
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
    let mut usage = wgpu::TextureUsages::COPY_SRC
        | wgpu::TextureUsages::COPY_DST
        | wgpu::TextureUsages::TEXTURE_BINDING;
    for u in program
        .passes
        .iter()
        .flat_map(|p| p.uses.iter())
        .chain(program.scene_uses)
        .filter(|u| u.resource == name)
    {
        match u.access {
            Access::Read | Access::Write | Access::ReadWrite => {
                usage |= wgpu::TextureUsages::STORAGE_BINDING
            }
            Access::Atomic => {
                usage |= wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::STORAGE_ATOMIC
            }
            _ => {}
        }
    }
    usage
}

/// Image `i`'s textures for a view of `viewport` pixels, and their views.
fn make_image(
    device: &wgpu::Device,
    program: &ShadingProgram,
    i: &ProgramImage,
    viewport: [u32; 2],
) -> (
    Vec<wgpu::Texture>,
    (Vec<wgpu::TextureView>, Vec<wgpu::TextureView>),
) {
    let size = image_size(i, viewport);
    let mips = levels(i, size);
    let textures: Vec<wgpu::Texture> = (0..i.count.max(1))
        .map(|_| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(i.name),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: size[2],
                },
                mip_level_count: mips,
                sample_count: 1,
                dimension: if i.kind == ImageKind::D3 {
                    wgpu::TextureDimension::D3
                } else {
                    wgpu::TextureDimension::D2
                },
                format: i.format,
                usage: usage_of(program, i.name),
                view_formats: &[],
            })
        })
        .collect();
    let whole = textures
        .iter()
        .map(|t| {
            t.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(view_dimension(i.kind)),
                ..Default::default()
            })
        })
        .collect();
    let levels = if mips > 1 {
        (0..mips)
            .map(|m| {
                textures[0].create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(view_dimension(i.kind)),
                    base_mip_level: m,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    (textures, (whole, levels))
}

impl ProgramRuntime {
    pub fn new(
        device: &wgpu::Device,
        shading: &'static SceneShading,
        program: &'static ShadingProgram,
        pipelines: &ProgramPipelines,
    ) -> ProgramRuntime {
        let (images, views): (Vec<Vec<wgpu::Texture>>, Vec<_>) = program
            .images
            .iter()
            .map(|i| make_image(device, program, i, [1, 1]))
            .unzip();
        let buffers: Vec<Vec<wgpu::Buffer>> = program
            .buffers
            .iter()
            .map(|b| {
                let mut usage = wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST;
                if b.indirect {
                    usage |= wgpu::BufferUsages::INDIRECT;
                }
                if program
                    .passes
                    .iter()
                    .flat_map(|p| p.uses.iter())
                    .any(|u| u.resource == b.name && u.access == Access::Uniform)
                {
                    usage |= wgpu::BufferUsages::UNIFORM;
                }
                (0..b.count.max(1))
                    .map(|_| {
                        device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some(b.name),
                            size: b.bytes.max(16).next_multiple_of(4),
                            usage,
                            mapped_at_creation: false,
                        })
                    })
                    .collect()
            })
            .collect();
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("program parameters"),
            size: COMMANDS_MAX * PARAMS_STRIDE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = |filter, mipmap_filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("program sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter,
                min_filter: filter,
                mipmap_filter,
                ..Default::default()
            })
        };
        let samplers = [
            sampler(wgpu::FilterMode::Linear, wgpu::MipmapFilterMode::Nearest),
            sampler(wgpu::FilterMode::Nearest, wgpu::MipmapFilterMode::Nearest),
            sampler(wgpu::FilterMode::Linear, wgpu::MipmapFilterMode::Linear),
        ];
        let params_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("program parameters"),
            layout: &pipelines.params,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &params,
                    offset: 0,
                    size: wgpu::BufferSize::new(64),
                }),
            }],
        });
        let mut runtime = ProgramRuntime {
            source: program,
            shading,
            images,
            views,
            samplers,
            bound: None,
            buffers,
            params,
            params_group,
            groups: Vec::new(),
            scene_group: None,
            targets: HashMap::new(),
            state: (program.init)(),
            commands: Vec::new(),
            uploads: None,
        };
        // A program of its own resources only is bound now; the others when the frame's textures exist.
        if !needs_bind(shading, program) {
            runtime.make_groups(device, pipelines, &|_| None);
        }
        runtime
    }

    /// Makes the groups again for a view of `viewport` pixels and the frame's
    /// textures (`external` gives each by name; `key` stands for them): the
    /// images the size of the view are made again, at zero, when it changes.
    /// Nothing happens when neither changed, or when the program needs neither.
    /// Returns whether its images were made again.
    pub fn bind(
        &mut self,
        device: &wgpu::Device,
        pipelines: &ProgramPipelines,
        viewport: [u32; 2],
        key: usize,
        external: &dyn Fn(&str) -> Option<wgpu::TextureView>,
    ) -> bool {
        if !needs_bind(self.shading, self.source) || self.bound == Some((viewport, key)) {
            return false;
        }
        let remade = self.bound.is_none_or(|(v, _)| v != viewport);
        if remade {
            for (k, i) in self
                .source
                .images
                .iter()
                .enumerate()
                .filter(|(_, i)| i.viewport > 0)
            {
                (self.images[k], self.views[k]) = make_image(device, self.source, i, viewport);
            }
        }
        self.make_groups(device, pipelines, external);
        self.bound = Some((viewport, key));
        remade
    }

    fn make_groups(
        &mut self,
        device: &wgpu::Device,
        pipelines: &ProgramPipelines,
        external: &dyn Fn(&str) -> Option<wgpu::TextureView>,
    ) {
        let program = self.source;
        let (views, buffers, samplers) = (&self.views, &self.buffers, &self.samplers);
        let group = |label: &str,
                     layout: &wgpu::BindGroupLayout,
                     uses: &[ProgramUse],
                     instance: u32|
         -> wgpu::BindGroup {
            let element = |which: Which| match which {
                Which::First | Which::All | Which::Mip(_) => 0,
                Which::Instance => instance as usize,
                Which::Fixed(k) => k as usize,
                Which::Pick(table) => table.get(instance as usize).copied().unwrap_or(0) as usize,
            };
            let refs: Vec<Vec<&wgpu::TextureView>> =
                views.iter().map(|v| v.0.iter().collect()).collect();
            let frame: Vec<(u32, wgpu::TextureView)> = uses
                .iter()
                .filter_map(|u| external(u.resource).map(|v| (u.binding, v)))
                .collect();
            let mut entries = Vec::new();
            for u in uses {
                let resource = if let Some((_, v)) = frame.iter().find(|(b, _)| *b == u.binding) {
                    wgpu::BindingResource::TextureView(v)
                } else if let Some(k) = program.images.iter().position(|i| i.name == u.resource) {
                    match u.which {
                        Which::All => wgpu::BindingResource::TextureViewArray(&refs[k]),
                        Which::Mip(offset) if !views[k].1.is_empty() => {
                            wgpu::BindingResource::TextureView(
                                &views[k].1[(instance as i64 + offset as i64)
                                    .clamp(0, views[k].1.len() as i64 - 1)
                                    as usize],
                            )
                        }
                        w => wgpu::BindingResource::TextureView(
                            &views[k].0[element(w).min(views[k].0.len() - 1)],
                        ),
                    }
                } else if let Some(k) = program.buffers.iter().position(|b| b.name == u.resource) {
                    buffers[k][element(u.which).min(buffers[k].len() - 1)].as_entire_binding()
                } else if u.resource == "nearestSampler" {
                    wgpu::BindingResource::Sampler(&samplers[1])
                } else if u.resource == "trilinearSampler" {
                    wgpu::BindingResource::Sampler(&samplers[2])
                } else {
                    wgpu::BindingResource::Sampler(&samplers[0])
                };
                entries.push(wgpu::BindGroupEntry {
                    binding: u.binding,
                    resource,
                });
            }
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries: &entries,
            })
        };
        self.groups = program
            .passes
            .iter()
            .zip(&pipelines.passes)
            .map(|(p, (_, layout))| {
                (0..p.instances.max(1))
                    .map(|i| group(p.name, layout, p.uses, i))
                    .collect()
            })
            .collect();
        self.scene_group = pipelines
            .scene
            .as_ref()
            .map(|layout| group("program (scene)", layout, program.scene_uses, 0));
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
                ProgramCommand::Dispatch { params, .. }
                | ProgramCommand::DispatchIndirect { params, .. }
                | ProgramCommand::Draw { params, .. } => {
                    assert!(
                        n < COMMANDS_MAX,
                        "a program's plan returned more than {COMMANDS_MAX} dispatches and draws"
                    );
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
        self.uploads = (!upload.is_empty()).then(|| {
            (
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("program uploads"),
                    contents: bytemuck::cast_slice(&upload),
                    usage: wgpu::BufferUsages::COPY_SRC,
                }),
                offsets,
            )
        });
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
        let k = self
            .source
            .buffers
            .iter()
            .position(|b| b.name == e.name)
            .unwrap_or_else(|| panic!("a program has no buffer `{}`", e.name));
        self.buffers[k]
            .get(e.index as usize)
            .unwrap_or_else(|| panic!("buffer `{}` has no element {}", e.name, e.index))
    }

    /// Every resource by name, for the inspector and the selftests' readbacks.
    pub fn handles(&self) -> ProgramHandles {
        let images = self.source.images.iter().zip(&self.images).map(|(i, t)| {
            (
                i.name,
                ProgramHandle::Image {
                    kind: i.kind,
                    textures: t.clone(),
                },
            )
        });
        let buffers = self
            .source
            .buffers
            .iter()
            .zip(&self.buffers)
            .map(|(b, x)| (b.name, ProgramHandle::Buffer(x.clone())));
        images.chain(buffers).collect()
    }

    /// The bytes of a buffer or an image, read back (a selftest): the texels
    /// of image `index` row by row, layer by layer.
    pub fn texture(&self, name: &str, index: u32) -> Option<&wgpu::Texture> {
        let k = self.image(name)?;
        self.images[k].get(index as usize)
    }

    /// Records the commands of a run (`ProgramRun`; each upload, clear, and
    /// copy in the run of the next pass command after it, those after the
    /// last in `BEFORE_SCENE`), calling `after` with each pass's name once its
    /// consecutive commands are recorded (the inspector).
    pub fn record(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        run: ProgramRun,
        bindings: &ProgramBindings,
        mut timing: Option<&mut crate::timing::Timing>,
        after: &mut dyn FnMut(&mut wgpu::CommandEncoder, &'static str),
    ) {
        let ProgramRun { runs, run } = run;
        let (pipelines, frame, shading, meshes) = (
            bindings.pipelines,
            bindings.frame,
            bindings.shading,
            &bindings.meshes,
        );
        if self.commands.is_empty() {
            return;
        }
        assert!(
            !self.groups.is_empty() || self.source.passes.is_empty(),
            "a program's groups are made when the frame's textures exist (ProgramRuntime::bind)"
        );
        let commands = std::mem::take(&mut self.commands);
        let source = self.source;
        let stage = run;
        let pass_index = |name: &str| {
            source
                .passes
                .iter()
                .position(|p| p.name == name)
                .unwrap_or_else(|| panic!("a program has no pass `{name}`"))
        };
        // Each command's run, and its parameters' slot.
        let mut stages = vec![BEFORE_SCENE; commands.len()];
        let mut next = None;
        for (i, c) in commands.iter().enumerate().rev() {
            if let Some(p) = c.pass() {
                next = Some(runs[pass_index(p)]);
            }
            stages[i] = next.unwrap_or(BEFORE_SCENE);
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
                ProgramCommand::Upload {
                    buffer,
                    offset,
                    words,
                } => {
                    let (staging, offsets) = self.uploads.as_ref().expect("the uploads");
                    let to = self.buffer(*buffer);
                    encoder.copy_buffer_to_buffer(
                        staging,
                        offsets[uploads[i]],
                        to,
                        *offset,
                        words.len() as u64 * 4,
                    );
                    i += 1;
                }
                ProgramCommand::Clear(e) => {
                    let pick = |k: usize| e.index == u32::MAX || k == e.index as usize;
                    if let Some(b) = self.source.buffers.iter().position(|b| b.name == e.name) {
                        for (_, x) in self.buffers[b].iter().enumerate().filter(|(k, _)| pick(*k)) {
                            encoder.clear_buffer(x, 0, None);
                        }
                    } else {
                        let k = self.image(e.name).unwrap_or_else(|| {
                            panic!("a program has no image or buffer `{}`", e.name)
                        });
                        for (_, t) in self.images[k].iter().enumerate().filter(|(k, _)| pick(*k)) {
                            encoder.clear_texture(t, &wgpu::ImageSubresourceRange::default());
                        }
                    }
                    i += 1;
                }
                ProgramCommand::Copy { from, to, bytes } => {
                    encoder.copy_buffer_to_buffer(
                        self.buffer(*from),
                        0,
                        self.buffer(*to),
                        0,
                        *bytes,
                    );
                    i += 1;
                }
                ProgramCommand::Dispatch { pass, .. }
                | ProgramCommand::DispatchIndirect { pass, .. } => {
                    let p = pass_index(pass);
                    let PassPipeline::Compute(pipeline) = &pipelines.passes[p].0 else {
                        panic!("`{pass}` is a raster pass: draw it")
                    };
                    let mut end = i;
                    while end < count
                        && stages[end] == stage
                        && commands[end].pass() == Some(*pass)
                        && !matches!(commands[end], ProgramCommand::Draw { .. })
                    {
                        end += 1;
                    }
                    let span = timing.as_mut().and_then(|t| t.compute_writes(pass));
                    let mut cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some(pass),
                        timestamp_writes: span.as_ref().map(|t| t.compute()),
                    });
                    cp.set_pipeline(pipeline);
                    cp.set_bind_group(0, frame, &[]);
                    cp.set_bind_group(1, shading, &[]);
                    for k in i..end {
                        match &commands[k] {
                            ProgramCommand::Dispatch {
                                groups, instance, ..
                            } => {
                                cp.set_bind_group(
                                    2,
                                    &self.params_group,
                                    &[(slots[k] * PARAMS_STRIDE) as u32],
                                );
                                cp.set_bind_group(3, &self.groups[p][*instance as usize], &[]);
                                if groups.iter().all(|g| *g > 0) {
                                    cp.dispatch_workgroups(groups[0], groups[1], groups[2]);
                                }
                            }
                            ProgramCommand::DispatchIndirect {
                                buffer,
                                offset,
                                instance,
                                ..
                            } => {
                                cp.set_bind_group(
                                    2,
                                    &self.params_group,
                                    &[(slots[k] * PARAMS_STRIDE) as u32],
                                );
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
                    let PassPipeline::Raster(pipeline) = &pipelines.passes[p].0 else {
                        panic!("`{pass}` is a compute pass: dispatch it")
                    };
                    let mut end = i;
                    while end < count
                        && stages[end] == stage
                        && commands[end].pass() == Some(*pass)
                        && matches!(commands[end], ProgramCommand::Draw { .. })
                    {
                        end += 1;
                    }
                    let span = timing.as_mut().and_then(|t| t.render_writes(pass));
                    for k in i..end {
                        let ProgramCommand::Draw { size, instance, .. } = &commands[k] else {
                            unreachable!()
                        };
                        let size = [size[0].max(1), size[1].max(1)];
                        let target = self.targets.entry(size).or_insert_with(|| {
                            device
                                .create_texture(&wgpu::TextureDescriptor {
                                    label: Some("program raster target"),
                                    size: wgpu::Extent3d {
                                        width: size[0],
                                        height: size[1],
                                        depth_or_array_layers: 1,
                                    },
                                    mip_level_count: 1,
                                    sample_count: 1,
                                    dimension: wgpu::TextureDimension::D2,
                                    format: TARGET_FORMAT,
                                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                                    view_formats: &[],
                                })
                                .create_view(&Default::default())
                        });
                        let writes = span
                            .as_ref()
                            .filter(|_| k == i || k + 1 == end)
                            .map(|t| t.span(k == i, k + 1 == end));
                        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some(pass),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: target,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                    store: wgpu::StoreOp::Discard,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: writes,
                            occlusion_query_set: None,
                            multiview_mask: None,
                        });
                        rp.set_pipeline(pipeline);
                        rp.set_bind_group(0, frame, &[]);
                        rp.set_bind_group(1, shading, &[]);
                        rp.set_bind_group(
                            2,
                            &self.params_group,
                            &[(slots[k] * PARAMS_STRIDE) as u32],
                        );
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
        self.commands = commands;
    }

    /// Ends the frame's recording: its commands and uploads are spent.
    pub fn finish(&mut self) {
        self.commands.clear();
        self.uploads = None;
    }
}
