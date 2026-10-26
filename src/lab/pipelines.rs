//! Every pipeline of the scene renderer, built from the WGSL modules in
//! src/lab/shaders.rs. Bind group 0 mirrors shaders/wgsl/bindings.wgsl.

use crate::lab::frame::{BoxInstance, CardInstance, PointInstance, SampleInstance, SegmentInstance};
use crate::lab::gpu::{DEPTH_FORMAT, FRAME_FORMAT, HDR_FORMAT, MSAA, OVERLAY_FORMAT};
use crate::lab::investigation::SceneShading;
use crate::lab::scene::Vertex;
use anyhow::{anyhow, Result};
use std::borrow::Cow;
use std::collections::BTreeMap;

pub const VERIFY_ENTRIES: &[&str] = &[
    "dumpObjects",
    "dumpVoxels",
    "dumpBricks",
    "dumpCards",
    "dumpPages",
    "dumpRays",
    "dumpProbes",
    "dumpRadiance",
    "dumpFrame",
    "traceTests",
    "projectTests",
    "lightingTests",
    "surfaceCacheTests",
    "ggxTests",
];

/// Size of the uniform buffer behind `gShading` (`FrameContent::shading`).
pub const SHADING_UNIFORM_BYTES: u64 = (crate::lab::frame::SHADING_UNIFORMS * 16) as u64;

pub struct Pipelines {
    pub frame_layout: wgpu::BindGroupLayout,
    pub texture_pair_layout: wgpu::BindGroupLayout,
    pub verify_layout: wgpu::BindGroupLayout,
    /// Group 1 of an investigation's own scene shading (`SceneShading`).
    pub shading_layout: wgpu::BindGroupLayout,
    pub scene: wgpu::RenderPipeline,
    pub ghost: wgpu::RenderPipeline,
    pub card: wgpu::RenderPipeline,
    pub sample: wgpu::RenderPipeline,
    pub segment: wgpu::RenderPipeline,
    pub segment_xray: wgpu::RenderPipeline,
    pub boxes: wgpu::RenderPipeline,
    pub boxes_xray: wgpu::RenderPipeline,
    pub point: wgpu::RenderPipeline,
    pub composite: wgpu::RenderPipeline,
    pub verify: BTreeMap<&'static str, wgpu::ComputePipeline>,
    scene_module: wgpu::ShaderModule,
}

/// An investigation's shading: its scene pass, its chain's compute passes
/// (each with its group-2 layout), and its verification kernels.
pub struct ShadingPipelines {
    pub scene: wgpu::RenderPipeline,
    /// The depth pre-pass, when the shading has one.
    pub prepass: Option<wgpu::RenderPipeline>,
    /// Group 2 of the scene pass (`SceneShading::scene_reads`).
    pub scene_group: wgpu::BindGroupLayout,
    /// The early passes, then the passes after the scene pass, in order.
    pub passes: Vec<(wgpu::ComputePipeline, wgpu::BindGroupLayout)>,
    pub verify: BTreeMap<&'static str, wgpu::ComputePipeline>,
}

/// Where a pass binds a chain target or buffer in group 2 (`ShadingPass`).
pub fn chain_binding(shading: &SceneShading, name: &str, written: bool) -> u32 {
    let n = shading.targets.len() as u32;
    if let Some(k) = shading.buffers.iter().position(|b| b.name == name) {
        return 2 * n + 2 + k as u32;
    }
    match (name, written) {
        ("depth", false) => 2 * n,
        ("hdr", true) => 2 * n + 1,
        _ => {
            let i = shading.targets.iter().position(|t| t.name == name).unwrap_or_else(|| panic!("{}: no target {name} to {}", shading.fragment, if written { "write" } else { "read" })) as u32;
            2 * i + u32::from(written)
        }
    }
}

/// The format of a target of an investigation's pass chain.
pub fn target_format(shading: &SceneShading, name: &str) -> wgpu::TextureFormat {
    match name {
        "hdr" => HDR_FORMAT,
        _ => shading.targets.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("{}: no target {name}", shading.fragment)).format,
    }
}

fn storage(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        // Writable storage is only used by the compute verification kernels.
        visibility: if read_only { wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE } else { wgpu::ShaderStages::COMPUTE },
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

fn texture(binding: u32, dimension: wgpu::TextureViewDimension) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: dimension, multisampled: false },
        count: None,
    }
}

const fn attr(location: u32, offset: u64, format: wgpu::VertexFormat) -> wgpu::VertexAttribute {
    wgpu::VertexAttribute { format, offset, shader_location: location }
}

const MESH_ATTRS: [wgpu::VertexAttribute; 2] = [attr(0, 0, wgpu::VertexFormat::Float32x3), attr(1, 12, wgpu::VertexFormat::Float32x3)];
const OBJECT_ATTRS: [wgpu::VertexAttribute; 1] = [attr(2, 0, wgpu::VertexFormat::Uint32)];
const SEGMENT_ATTRS: [wgpu::VertexAttribute; 6] = [
    attr(0, 0, wgpu::VertexFormat::Float32x3),
    attr(1, 12, wgpu::VertexFormat::Float32),
    attr(2, 16, wgpu::VertexFormat::Float32x3),
    attr(3, 28, wgpu::VertexFormat::Uint32),
    attr(4, 32, wgpu::VertexFormat::Float32x4),
    attr(5, 48, wgpu::VertexFormat::Uint32x4),
];
const BOX_ATTRS: [wgpu::VertexAttribute; 3] = [attr(0, 0, wgpu::VertexFormat::Uint32x4), attr(1, 16, wgpu::VertexFormat::Float32x4), attr(2, 32, wgpu::VertexFormat::Float32x4)];
const PAIR_ATTRS: [wgpu::VertexAttribute; 2] = [attr(0, 0, wgpu::VertexFormat::Uint32x4), attr(1, 16, wgpu::VertexFormat::Float32x4)];
const POINT_ATTRS: [wgpu::VertexAttribute; 4] = [
    attr(0, 0, wgpu::VertexFormat::Float32x3),
    attr(1, 12, wgpu::VertexFormat::Float32),
    attr(2, 16, wgpu::VertexFormat::Float32x4),
    attr(3, 32, wgpu::VertexFormat::Uint32x4),
];

fn instance_layout<T>(attributes: &'static [wgpu::VertexAttribute]) -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<T>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes }
}

pub fn mesh_layouts() -> [wgpu::VertexBufferLayout<'static>; 2] {
    [
        wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &MESH_ATTRS },
        wgpu::VertexBufferLayout { array_stride: 4, step_mode: wgpu::VertexStepMode::Instance, attributes: &OBJECT_ATTRS },
    ]
}

fn module(device: &wgpu::Device, label: &str, source: impl Into<Cow<'static, str>>) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) })
}

struct PipelineSpec<'a> {
    label: &'a str,
    module: &'a wgpu::ShaderModule,
    vs: &'a str,
    fs: &'a str,
    buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
    format: wgpu::TextureFormat,
    samples: u32,
    depth: Option<(wgpu::CompareFunction, bool)>,
    blend: Option<wgpu::BlendState>,
}

fn render_pipeline(device: &wgpu::Device, layout: &wgpu::PipelineLayout, spec: PipelineSpec) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(spec.label),
        layout: Some(layout),
        vertex: wgpu::VertexState { module: spec.module, entry_point: Some(spec.vs), compilation_options: Default::default(), buffers: spec.buffers },
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
        depth_stencil: spec.depth.map(|(compare, write)| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(write),
            depth_compare: Some(compare),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState { count: spec.samples, mask: !0, alpha_to_coverage_enabled: false },
        fragment: Some(wgpu::FragmentState {
            module: spec.module,
            entry_point: Some(spec.fs),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format: spec.format, blend: spec.blend, write_mask: wgpu::ColorWrites::ALL })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

impl Pipelines {
    pub fn new(device: &wgpu::Device) -> Result<Pipelines> {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut frame_entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<crate::lab::layout::FrameUniforms>() as u64),
            },
            count: None,
        }];
        frame_entries.extend((1..=8).map(|b| storage(b, true)));
        frame_entries.push(texture(9, wgpu::TextureViewDimension::D3));
        frame_entries.push(texture(10, wgpu::TextureViewDimension::D2));
        frame_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 11,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        frame_entries.extend([storage(12, true), storage(13, true)]);
        frame_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 14,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Uint, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
            count: None,
        });
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("frame"), entries: &frame_entries });
        let texture_pair_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture pair"),
            entries: &[texture(0, wgpu::TextureViewDimension::D2), texture(1, wgpu::TextureViewDimension::D2)],
        });
        let verify_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("verify"), entries: &[storage(0, false), storage(1, true)] });
        let shading_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene shading"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: wgpu::BufferSize::new(SHADING_UNIFORM_BYTES) },
                    count: None,
                },
                storage(1, true),
                storage(2, true),
            ],
        });

        let frame_only = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("frame only"), bind_group_layouts: &[Some(&frame_layout)], immediate_size: 0 });
        let with_pair = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame + textures"),
            bind_group_layouts: &[Some(&frame_layout), Some(&texture_pair_layout)],
            immediate_size: 0,
        });
        let with_verify = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame + verify"),
            bind_group_layouts: &[Some(&frame_layout), Some(&verify_layout)],
            immediate_size: 0,
        });

        let scene_module = module(device, "scene.wgsl", crate::lab::shaders::SCENE);
        let overlay_module = module(device, "overlay.wgsl", crate::lab::shaders::OVERLAY);
        let verify_module = module(device, "verify.wgsl", crate::lab::shaders::VERIFY);
        let [mesh, objects] = mesh_layouts();
        let mesh_buffers = [Some(mesh), Some(objects)];
        let premultiplied = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        // Reversed depth (camera.rs): nearer is greater.
        use wgpu::CompareFunction::{Always, Greater, GreaterEqual};

        let scene = render_pipeline(device, &frame_only, PipelineSpec {
            label: "scene",
            module: &scene_module,
            vs: "sceneVS",
            fs: "sceneFS",
            buffers: &mesh_buffers,
            format: HDR_FORMAT,
            samples: MSAA,
            depth: Some((Greater, true)),
            blend: None,
        });
        let overlay = |label: &'static str, vs: &'static str, fs: &'static str, buffers: &[Option<wgpu::VertexBufferLayout>], depth: (wgpu::CompareFunction, bool)| {
            render_pipeline(device, &frame_only, PipelineSpec { label, module: &overlay_module, vs, fs, buffers, format: OVERLAY_FORMAT, samples: MSAA, depth: Some(depth), blend: premultiplied })
        };
        let segments = [Some(instance_layout::<SegmentInstance>(&SEGMENT_ATTRS))];
        let boxes = [Some(instance_layout::<BoxInstance>(&BOX_ATTRS))];
        let cards = [Some(instance_layout::<CardInstance>(&PAIR_ATTRS))];
        let samples = [Some(instance_layout::<SampleInstance>(&PAIR_ATTRS))];
        let points = [Some(instance_layout::<PointInstance>(&POINT_ATTRS))];
        let pipelines = Pipelines {
            ghost: overlay("ghost", "ghostVS", "ghostFS", &mesh_buffers, (GreaterEqual, false)),
            card: overlay("card", "cardVS", "cardFS", &cards, (GreaterEqual, false)),
            sample: overlay("sample", "sampleVS", "sampleFS", &samples, (GreaterEqual, true)),
            segment: overlay("segment", "segmentVS", "lineFS", &segments, (GreaterEqual, false)),
            segment_xray: overlay("segment x-ray", "segmentVS", "lineFS", &segments, (Always, false)),
            boxes: overlay("box", "boxVS", "lineFS", &boxes, (GreaterEqual, false)),
            boxes_xray: overlay("box x-ray", "boxVS", "lineFS", &boxes, (Always, false)),
            point: overlay("point", "pointVS", "pointFS", &points, (Always, false)),
            composite: render_pipeline(device, &with_pair, PipelineSpec {
                label: "composite",
                module: &scene_module,
                vs: "fullscreenVS",
                fs: "compositeFS",
                buffers: &[],
                format: FRAME_FORMAT,
                samples: 1,
                depth: None,
                blend: None,
            }),
            verify: VERIFY_ENTRIES
                .iter()
                .map(|&entry| {
                    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some(entry),
                        layout: Some(&with_verify),
                        module: &verify_module,
                        entry_point: Some(entry),
                        compilation_options: Default::default(),
                        cache: None,
                    });
                    (entry, pipeline)
                })
                .collect(),
            scene,
            frame_layout,
            texture_pair_layout,
            verify_layout,
            shading_layout,
            scene_module,
        };
        if let Some(error) = pollster::block_on(scope.pop()) {
            return Err(anyhow!("pipeline creation failed: {error}"));
        }
        Ok(pipelines)
    }

    /// The pipelines of an investigation's own shading: its scene pass, its
    /// chain's compute passes with their group-2 layouts, and its
    /// verification kernels by entry point.
    pub fn shading(&self, device: &wgpu::Device, shading: &SceneShading) -> Result<ShadingPipelines> {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = module(device, shading.fragment, format!("{}{}", crate::lab::shaders::SCENE, shading.source));
        let chain_layout = |label: &str, reads: &[&str], writes: &[&str], visibility: wgpu::ShaderStages| {
            let mut entries: Vec<wgpu::BindGroupLayoutEntry> = Vec::new();
            for (name, written) in reads.iter().map(|n| (*n, false)).chain(writes.iter().map(|n| (*n, true))) {
                let binding = chain_binding(shading, name, written);
                if entries.iter().any(|e| e.binding == binding) {
                    continue;
                }
                let ty = if shading.buffers.iter().any(|b| b.name == name) {
                    wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }
                } else if written {
                    wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: target_format(shading, name), view_dimension: wgpu::TextureViewDimension::D2 }
                } else if name == "depth" {
                    wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: true }
                } else {
                    wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: shading.outputs.contains(&name) }
                };
                entries.push(wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None });
            }
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries: &entries })
        };
        let scene_group = chain_layout("scene reads", shading.scene_reads, &[], wgpu::ShaderStages::FRAGMENT);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame + shading"),
            bind_group_layouts: &[Some(&self.frame_layout), Some(&self.shading_layout), Some(&scene_group)],
            immediate_size: 0,
        });
        let [mesh, objects] = mesh_layouts();
        // After a depth pre-pass, the scene pass only tests against its depth (reversed: nearer is greater).
        let (compare, write) = if shading.prepass.is_some() { (wgpu::CompareFunction::GreaterEqual, false) } else { (wgpu::CompareFunction::Greater, true) };
        let formats: Vec<wgpu::TextureFormat> = if shading.outputs.is_empty() { vec![HDR_FORMAT] } else { shading.outputs.iter().map(|o| target_format(shading, o)).collect() };
        let targets: Vec<Option<wgpu::ColorTargetState>> = formats.iter().map(|&format| Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })).collect();
        let scene = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(shading.fragment),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("sceneVS"), compilation_options: Default::default(), buffers: &[Some(mesh), Some(objects)] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(write),
                depth_compare: Some(compare),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState { count: MSAA, mask: !0, alpha_to_coverage_enabled: false },
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some(shading.fragment), compilation_options: Default::default(), targets: &targets }),
            multiview_mask: None,
            cache: None,
        });
        let prepass = shading.prepass.map(|entry| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("sceneVS"), compilation_options: Default::default(), buffers: &[Some(mesh_layouts()[0].clone()), Some(mesh_layouts()[1].clone())] },
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: Some(true), depth_compare: Some(wgpu::CompareFunction::Greater), stencil: Default::default(), bias: Default::default() }),
                multisample: wgpu::MultisampleState { count: MSAA, mask: !0, alpha_to_coverage_enabled: false },
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some(entry), compilation_options: Default::default(), targets: &[] }),
                multiview_mask: None,
                cache: None,
            })
        });
        let passes = shading
            .early
            .iter()
            .chain(shading.passes)
            .map(|pass| {
                let group = chain_layout(pass.entry, pass.reads, pass.writes, wgpu::ShaderStages::COMPUTE);
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(pass.entry),
                    bind_group_layouts: &[Some(&self.frame_layout), Some(&self.shading_layout), Some(&group)],
                    immediate_size: 0,
                });
                let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(pass.entry),
                    layout: Some(&layout),
                    module: &module,
                    entry_point: Some(pass.entry),
                    compilation_options: Default::default(),
                    cache: None,
                });
                (pipeline, group)
            })
            .collect();
        let verify_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame + shading + verify"),
            bind_group_layouts: &[Some(&self.frame_layout), Some(&self.shading_layout), Some(&self.verify_layout)],
            immediate_size: 0,
        });
        let verify = shading
            .verify
            .iter()
            .map(|&entry| {
                let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&verify_layout),
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                });
                (entry, pipeline)
            })
            .collect();
        if let Some(error) = pollster::block_on(scope.pop()) {
            return Err(anyhow!("{} pipeline creation failed: {error}", shading.fragment));
        }
        Ok(ShadingPipelines { scene, prepass, scene_group, passes, verify })
    }

    /// The viewer's swapchain blit. sRGB swapchains decode before writing.
    pub fn blit(&self, device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit"),
            bind_group_layouts: &[Some(&self.frame_layout), Some(&self.texture_pair_layout)],
            immediate_size: 0,
        });
        render_pipeline(device, &layout, PipelineSpec {
            label: "blit",
            module: &self.scene_module,
            vs: "fullscreenVS",
            fs: if format.is_srgb() { "blitDecodeFS" } else { "blitRawFS" },
            buffers: &[],
            format,
            samples: 1,
            depth: None,
            blend: None,
        })
    }
}
