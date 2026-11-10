//! Every pipeline of the scene renderer, built from the WGSL modules in
//! src/shaders.rs. Bind group 0 mirrors packages/render_lab/shaders/bindings.wgsl.

use crate::frame::{BoxInstance, CuboidInstance, PointInstance, SegmentInstance};
use crate::gpu::{DEPTH_FORMAT, FRAME_FORMAT, HDR_FORMAT, MSAA, OVERLAY_FORMAT};
use crate::investigation::SceneShading;
use crate::scene::Vertex;
use crate::shaders;
use anyhow::{Result, anyhow};
use std::borrow::Cow;
use std::collections::BTreeMap;

pub const VERIFY_ENTRIES: &[&str] = &[
    "dumpObjects",
    "dumpFrame",
    "traceTests",
    "projectTests",
    "lightingTests",
];

/// Size of the uniform buffer behind `gShading` (`FrameContent::shading`).
pub const SHADING_UNIFORM_BYTES: u64 = (crate::frame::SHADING_UNIFORMS * 16) as u64;

pub struct Pipelines {
    pub frame_layout: wgpu::BindGroupLayout,
    pub texture_pair_layout: wgpu::BindGroupLayout,
    pub verify_layout: wgpu::BindGroupLayout,
    /// Group 1 of an investigation's own scene shading (`SceneShading`).
    pub shading_layout: wgpu::BindGroupLayout,
    pub scene: wgpu::RenderPipeline,
    /// The backdrop, drawn first in a scene pass that has no pass chain.
    pub sky: wgpu::RenderPipeline,
    pub ghost: wgpu::RenderPipeline,
    pub segment: wgpu::RenderPipeline,
    pub segment_xray: wgpu::RenderPipeline,
    pub boxes: wgpu::RenderPipeline,
    pub boxes_xray: wgpu::RenderPipeline,
    pub point: wgpu::RenderPipeline,
    pub cuboid: wgpu::RenderPipeline,
    pub composite: wgpu::RenderPipeline,
    /// The scene without the overlay (the images an experiment measures).
    pub composite_scene: wgpu::RenderPipeline,
    pub composite_over: wgpu::RenderPipeline,
    /// The experiment's difference view (`compositeDiffFS`): group 2 holds its gain.
    pub composite_diff: wgpu::RenderPipeline,
    pub compare_layout: wgpu::BindGroupLayout,
    pub verify: BTreeMap<&'static str, wgpu::ComputePipeline>,
    scene_module: wgpu::ShaderModule,
}

/// An investigation's shading: its scene pass, its chain's compute passes
/// (each with its group-2 layout), and its verification kernels.
pub struct ShadingPipelines {
    pub scene: wgpu::RenderPipeline,
    /// The depth pre-pass, when the shading has one.
    pub prepass: Option<wgpu::RenderPipeline>,
    /// One pipeline per layer of the depth views (`SceneShading::depth_views`).
    pub depth_views: Vec<wgpu::RenderPipeline>,
    /// Group 2 of the scene pass (`SceneShading::scene_reads`).
    pub scene_group: wgpu::BindGroupLayout,
    /// The shading's compute passes, in `SceneShading::passes` order.
    pub passes: Vec<(wgpu::ComputePipeline, wgpu::BindGroupLayout)>,
    pub verify: BTreeMap<&'static str, wgpu::ComputePipeline>,
    /// Group 3, the volumes sampled (`SceneShading::volumes`), when it has any.
    pub volumes: Option<wgpu::BindGroupLayout>,
    /// The update passes (`SceneShading::update`), in its order, each with its group-2 layout.
    pub updates: Vec<(wgpu::ComputePipeline, wgpu::BindGroupLayout)>,
    /// The program's passes and the scene pass's group 3 (`SceneShading::program`).
    pub program: Option<crate::program::ProgramPipelines>,
}

/// Bytes between two dispatches' parameters in an update's uniform buffer
/// (`UpdateDispatch::params`, 16 words, at an offset the device accepts).
pub const UPDATE_PARAMS_STRIDE: u64 = 256;

/// Group 3 of a shading with volumes: a filtering sampler at 0, then each volume.
pub fn volume_layout(device: &wgpu::Device, shading: &SceneShading) -> wgpu::BindGroupLayout {
    let mut entries = vec![wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }];
    for (v, _) in shading.volumes.iter().enumerate() {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 1 + v as u32,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D3,
                multisampled: false,
            },
            count: None,
        });
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("volumes"),
        entries: &entries,
    })
}

/// Group 2 of an update pass: its parameters, the update's buffers, and the volumes it writes.
pub fn update_layout(
    device: &wgpu::Device,
    shading: &SceneShading,
    pass: &crate::investigation::UpdatePass,
) -> wgpu::BindGroupLayout {
    let update = shading.update.expect("an update");
    let mut entries = vec![wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: wgpu::BufferSize::new(64),
        },
        count: None,
    }];
    for (k, _) in update.buffers.iter().enumerate() {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 1 + k as u32,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
    }
    for (v, volume) in shading.volumes.iter().enumerate() {
        if pass.writes.contains(&volume.name) {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: 1 + update.buffers.len() as u32 + v as u32,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: volume.format,
                    view_dimension: wgpu::TextureViewDimension::D3,
                },
                count: None,
            });
        }
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(pass.entry),
        entries: &entries,
    })
}

/// Where a pass binds a chain target or buffer in group 2 (`ShadingPass`).
pub fn chain_binding(shading: &SceneShading, name: &str, written: bool) -> u32 {
    let n = shading.targets.len() as u32;
    if let Some(k) = shading.buffers.iter().position(|b| b.name == name) {
        return 2 * n + 2 + k as u32;
    }
    if shading.depth_views.is_some_and(|d| d.name == name) {
        return 2 * n + 2 + shading.buffers.len() as u32;
    }
    // After the targets' pairs come the depth to read (2n), the HDR target to write (2n + 1), then buffers.
    match (name, written) {
        ("depth", false) => 2 * n,
        ("hdr", true) => 2 * n + 1,
        _ => {
            // Each target has two bindings in a row: 2i to read it, 2i + 1 to write it.
            let i = shading
                .targets
                .iter()
                .position(|t| t.name == name)
                .unwrap_or_else(|| {
                    panic!(
                        "{}: no target {name} to {}",
                        shading.fragment,
                        if written {
                            "write"
                        } else {
                            "read"
                        }
                    )
                }) as u32;
            2 * i + u32::from(written)
        }
    }
}

/// The format of a target of an investigation's pass chain.
pub fn target_format(shading: &SceneShading, name: &str) -> wgpu::TextureFormat {
    match name {
        "hdr" => HDR_FORMAT,
        _ => {
            shading
                .targets
                .iter()
                .find(|t| t.name == name)
                .unwrap_or_else(|| panic!("{}: no target {name}", shading.fragment))
                .format
        }
    }
}

fn storage(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        // Writable storage is only used by the compute verification kernels.
        visibility: if read_only {
            wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE
        } else {
            wgpu::ShaderStages::COMPUTE
        },
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture(binding: u32, dimension: wgpu::TextureViewDimension) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: dimension,
            multisampled: false,
        },
        count: None,
    }
}

const fn attr(location: u32, offset: u64, format: wgpu::VertexFormat) -> wgpu::VertexAttribute {
    wgpu::VertexAttribute {
        format,
        offset,
        shader_location: location,
    }
}

const MESH_ATTRS: [wgpu::VertexAttribute; 2] = [
    attr(0, 0, wgpu::VertexFormat::Float32x3),
    attr(1, 12, wgpu::VertexFormat::Float32x3),
];
const OBJECT_ATTRS: [wgpu::VertexAttribute; 1] = [attr(2, 0, wgpu::VertexFormat::Uint32)];
const SEGMENT_ATTRS: [wgpu::VertexAttribute; 6] = [
    attr(0, 0, wgpu::VertexFormat::Float32x3),
    attr(1, 12, wgpu::VertexFormat::Float32),
    attr(2, 16, wgpu::VertexFormat::Float32x3),
    attr(3, 28, wgpu::VertexFormat::Uint32),
    attr(4, 32, wgpu::VertexFormat::Float32x4),
    attr(5, 48, wgpu::VertexFormat::Uint32x4),
];
const BOX_ATTRS: [wgpu::VertexAttribute; 3] = [
    attr(0, 0, wgpu::VertexFormat::Uint32x4),
    attr(1, 16, wgpu::VertexFormat::Float32x4),
    attr(2, 32, wgpu::VertexFormat::Float32x4),
];
const CUBOID_ATTRS: [wgpu::VertexAttribute; 5] = [
    attr(0, 0, wgpu::VertexFormat::Float32x3),
    attr(1, 16, wgpu::VertexFormat::Float32x4),
    attr(2, 32, wgpu::VertexFormat::Float32x4),
    attr(3, 48, wgpu::VertexFormat::Float32x4),
    attr(4, 64, wgpu::VertexFormat::Float32x4),
];
const POINT_ATTRS: [wgpu::VertexAttribute; 4] = [
    attr(0, 0, wgpu::VertexFormat::Float32x3),
    attr(1, 12, wgpu::VertexFormat::Float32),
    attr(2, 16, wgpu::VertexFormat::Float32x4),
    attr(3, 32, wgpu::VertexFormat::Uint32x4),
];

fn instance_layout<T>(
    attributes: &'static [wgpu::VertexAttribute],
) -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<T>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes,
    }
}

pub fn mesh_layouts() -> [wgpu::VertexBufferLayout<'static>; 2] {
    [
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &MESH_ATTRS,
        },
        wgpu::VertexBufferLayout {
            array_stride: 4,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &OBJECT_ATTRS,
        },
    ]
}

fn module(
    device: &wgpu::Device,
    label: &str,
    source: impl Into<Cow<'static, str>>,
) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
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

fn render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    spec: PipelineSpec,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(spec.label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: spec.module,
            entry_point: Some(spec.vs),
            compilation_options: Default::default(),
            buffers: spec.buffers,
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: spec.depth.map(|(compare, write)| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(write),
            depth_compare: Some(compare),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: spec.samples,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(wgpu::FragmentState {
            module: spec.module,
            entry_point: Some(spec.fs),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: spec.format,
                blend: spec.blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

impl Pipelines {
    pub fn new(device: &wgpu::Device) -> Result<Pipelines> {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let base = BaseLayouts::new(device);
        let [frame_only, with_pair, with_compare, with_verify] = base.pipeline_layouts(device);
        let (frame_layout, texture_pair_layout, verify_layout, shading_layout, compare_layout) = (
            base.frame,
            base.texture_pair,
            base.verify,
            base.shading,
            base.compare,
        );

        let scene_module = module(device, shaders::SCENE, shaders::module(shaders::SCENE)?);
        let overlay_module = module(device, shaders::OVERLAY, shaders::module(shaders::OVERLAY)?);
        let verify_module = module(device, shaders::VERIFY, shaders::module(shaders::VERIFY)?);
        let [mesh, objects] = mesh_layouts();
        let mesh_buffers = [Some(mesh), Some(objects)];
        // Reversed depth (camera.rs): nearer is greater.
        use wgpu::CompareFunction::{Always, Greater};

        let scene = render_pipeline(
            device,
            &frame_only,
            PipelineSpec {
                label: "scene",
                module: &scene_module,
                vs: "sceneVS",
                fs: "sceneFS",
                buffers: &mesh_buffers,
                format: HDR_FORMAT,
                samples: MSAA,
                depth: Some((Greater, true)),
                blend: None,
            },
        );
        let [
            ghost,
            segment,
            segment_xray,
            boxes,
            boxes_xray,
            point,
            cuboid,
        ] = overlay_pipelines(device, &frame_only, &overlay_module);
        let [composite, composite_over, composite_scene, composite_diff] =
            composite_pipelines(device, &with_pair, &with_compare, &scene_module);
        let sky = render_pipeline(
            device,
            &frame_only,
            PipelineSpec {
                label: "sky",
                module: &scene_module,
                vs: "fullscreenVS",
                fs: "skyFS",
                buffers: &[],
                format: HDR_FORMAT,
                samples: MSAA,
                depth: Some((Always, false)),
                blend: None,
            },
        );
        let pipelines = Pipelines {
            sky,
            ghost,
            segment,
            segment_xray,
            boxes,
            boxes_xray,
            point,
            cuboid,
            composite,
            composite_over,
            composite_scene,
            composite_diff,
            compare_layout,
            verify: verify_pipelines(device, &with_verify, &verify_module),
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
    pub fn shading(
        &self,
        device: &wgpu::Device,
        shading: &SceneShading,
    ) -> Result<ShadingPipelines> {
        // The declarations are the graph's edges: they must be what the shaders do.
        let problems = crate::graph::lint(shading)?;
        if !problems.is_empty() {
            return Err(anyhow!(
                "{}: the passes' reads and writes are not what their WGSL does (src/graph/lint.rs):\n  {}",
                shading.module,
                problems.join("\n  ")
            ));
        }
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = module(device, shading.module, shaders::module(shading.module)?);
        let chain_layout =
            |label: &str, reads: &[&str], writes: &[&str], visibility: wgpu::ShaderStages| {
                chain_layout(device, shading, label, reads, writes, visibility)
            };
        let scene_group = chain_layout(
            "scene reads",
            shading.scene_reads,
            &[],
            wgpu::ShaderStages::FRAGMENT,
        );
        // What the scene pass reads of the program: group 3.
        let program_scene = shading
            .program
            .and_then(|p| crate::program::scene_layout(device, shading, p));
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame + shading"),
            bind_group_layouts: &[
                Some(&self.frame_layout),
                Some(&self.shading_layout),
                Some(&scene_group),
                program_scene.as_ref(),
            ],
            immediate_size: 0,
        });
        let scene = Self::scene_pipeline(device, &module, shading, &layout);
        let prepass = Self::prepass_pipeline(device, &module, shading, &layout);
        // The depth views: the scene's meshes, depth only, one pipeline per layer.
        let depth_views = self.depth_view_pipelines(device, &module, shading);
        let volumes = (!shading.volumes.is_empty()).then(|| volume_layout(device, shading));
        let passes = self.pass_pipelines(
            device,
            &module,
            shading,
            volumes.as_ref().or(program_scene.as_ref()),
        );
        let verify = self.verify_pipelines(device, &module, shading, volumes.as_ref());
        let updates = self.update_pipelines(device, &module, shading, volumes.as_ref());
        let program = shading.program.map(|p| {
            crate::program::pipelines(
                device,
                &module,
                shading,
                p,
                &self.frame_layout,
                &self.shading_layout,
                program_scene,
            )
        });
        if let Some(error) = pollster::block_on(scope.pop()) {
            return Err(anyhow!(
                "{} pipeline creation failed: {error}",
                shading.fragment
            ));
        }
        Ok(ShadingPipelines {
            scene,
            prepass,
            depth_views,
            scene_group,
            passes,
            verify,
            volumes,
            updates,
            program,
        })
    }

    /// The scene pass: the meshes, into the chain's outputs (or the HDR target); after a depth pre-pass it only tests
    /// against its depth (reversed: nearer is greater).
    fn scene_pipeline(
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        shading: &SceneShading,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        let [mesh, objects] = mesh_layouts();
        // After a depth pre-pass, the scene pass only tests against its depth (reversed: nearer is greater).
        let (compare, write) = if shading.prepass.is_some() {
            (wgpu::CompareFunction::GreaterEqual, false)
        } else {
            (wgpu::CompareFunction::Greater, true)
        };
        let formats: Vec<wgpu::TextureFormat> = if shading.outputs.is_empty() {
            vec![HDR_FORMAT]
        } else {
            shading
                .outputs
                .iter()
                .map(|o| target_format(shading, o))
                .collect()
        };
        let targets: Vec<Option<wgpu::ColorTargetState>> = formats
            .iter()
            .map(|&format| {
                Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })
            })
            .collect();

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(shading.fragment),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("sceneVS"),
                compilation_options: Default::default(),
                buffers: &[Some(mesh), Some(objects)],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(write),
                depth_compare: Some(compare),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: MSAA,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(shading.fragment),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        })
    }

    /// The depth pre-pass, and what it writes besides the depth.
    fn prepass_pipeline(
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        shading: &SceneShading,
        layout: &wgpu::PipelineLayout,
    ) -> Option<wgpu::RenderPipeline> {
        let prepass_targets: Vec<Option<wgpu::ColorTargetState>> = shading
            .prepass_outputs
            .iter()
            .map(|o| {
                Some(wgpu::ColorTargetState {
                    format: target_format(shading, o),
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })
            })
            .collect();

        shading.prepass.map(|entry| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some("sceneVS"),
                    compilation_options: Default::default(),
                    buffers: &[
                        Some(mesh_layouts()[0].clone()),
                        Some(mesh_layouts()[1].clone()),
                    ],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: MSAA,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &prepass_targets,
                }),
                multiview_mask: None,
                cache: None,
            })
        })
    }

    /// The depth views: the scene's meshes, depth only, one pipeline per layer.
    fn depth_view_pipelines(
        &self,
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        shading: &SceneShading,
    ) -> Vec<wgpu::RenderPipeline> {
        let views_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame + shading (depth views)"),
            bind_group_layouts: &[Some(&self.frame_layout), Some(&self.shading_layout)],
            immediate_size: 0,
        });

        shading.depth_views.map_or_else(Vec::new, |d| {
            d.vertices
                .iter()
                .map(|&entry| {
                    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(entry),
                        layout: Some(&views_layout),
                        vertex: wgpu::VertexState {
                            module,
                            entry_point: Some(entry),
                            compilation_options: Default::default(),
                            buffers: &[
                                Some(mesh_layouts()[0].clone()),
                                Some(mesh_layouts()[1].clone()),
                            ],
                        },
                        primitive: wgpu::PrimitiveState {
                            topology: wgpu::PrimitiveTopology::TriangleList,
                            cull_mode: None,
                            ..Default::default()
                        },
                        depth_stencil: Some(wgpu::DepthStencilState {
                            format: DEPTH_FORMAT,
                            depth_write_enabled: Some(true),
                            depth_compare: Some(wgpu::CompareFunction::Greater),
                            stencil: Default::default(),
                            bias: wgpu::DepthBiasState {
                                constant: d.bias.0,
                                slope_scale: d.bias.1,
                                clamp: 0.0,
                            },
                        }),
                        multisample: wgpu::MultisampleState::default(),
                        fragment: None,
                        multiview_mask: None,
                        cache: None,
                    })
                })
                .collect()
        })
    }

    /// The chain's compute passes' pipelines and group layouts (`group3`: the volumes', else the program's scene group).
    fn pass_pipelines(
        &self,
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        shading: &SceneShading,
        group3: Option<&wgpu::BindGroupLayout>,
    ) -> Vec<(wgpu::ComputePipeline, wgpu::BindGroupLayout)> {
        shading
            .passes
            .iter()
            .map(|pass| {
                let group = chain_layout(
                    device,
                    shading,
                    pass.entry,
                    pass.reads,
                    pass.writes,
                    wgpu::ShaderStages::COMPUTE,
                );
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(pass.entry),
                    bind_group_layouts: &[
                        Some(&self.frame_layout),
                        Some(&self.shading_layout),
                        Some(&group),
                        group3,
                    ],
                    immediate_size: 0,
                });
                let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(pass.entry),
                    layout: Some(&layout),
                    module,
                    entry_point: Some(pass.entry),
                    compilation_options: Default::default(),
                    cache: None,
                });
                (pipeline, group)
            })
            .collect()
    }

    /// The selftest's verification entry points' pipelines, by entry point.
    fn verify_pipelines(
        &self,
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        shading: &SceneShading,
        volumes: Option<&wgpu::BindGroupLayout>,
    ) -> BTreeMap<&'static str, wgpu::ComputePipeline> {
        let verify_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame + shading + verify"),
            bind_group_layouts: &[
                Some(&self.frame_layout),
                Some(&self.shading_layout),
                Some(&self.verify_layout),
                volumes,
            ],
            immediate_size: 0,
        });

        shading
            .verify
            .iter()
            .map(|&entry| {
                let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&verify_layout),
                    module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                });
                (entry, pipeline)
            })
            .collect()
    }

    /// The update passes' pipelines and group layouts (`volumes`: the volumes' group, bound for a pass that reads one).
    fn update_pipelines(
        &self,
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        shading: &SceneShading,
        volumes: Option<&wgpu::BindGroupLayout>,
    ) -> Vec<(wgpu::ComputePipeline, wgpu::BindGroupLayout)> {
        shading.update.map_or_else(Vec::new, |update| {
            update
                .passes
                .iter()
                .map(|pass| {
                    let group = update_layout(device, shading, pass);
                    let reads = if pass
                        .reads
                        .iter()
                        .any(|r| crate::graph::is_volume(shading, r))
                    {
                        volumes
                    } else {
                        None
                    };
                    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some(pass.entry),
                        bind_group_layouts: &[
                            Some(&self.frame_layout),
                            Some(&self.shading_layout),
                            Some(&group),
                            reads,
                        ],
                        immediate_size: 0,
                    });
                    let pipeline =
                        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                            label: Some(pass.entry),
                            layout: Some(&layout),
                            module,
                            entry_point: Some(pass.entry),
                            compilation_options: Default::default(),
                            cache: None,
                        });
                    (pipeline, group)
                })
                .collect()
        })
    }

    /// The viewer's swapchain blit. sRGB swapchains decode before writing.
    pub fn blit(&self, device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit"),
            bind_group_layouts: &[Some(&self.frame_layout), Some(&self.texture_pair_layout)],
            immediate_size: 0,
        });
        render_pipeline(
            device,
            &layout,
            PipelineSpec {
                label: "blit",
                module: &self.scene_module,
                vs: "fullscreenVS",
                fs: if format.is_srgb() {
                    "blitDecodeFS"
                } else {
                    "blitRawFS"
                },
                buffers: &[],
                format,
                samples: 1,
                depth: None,
                blend: None,
            },
        )
    }
}

/// The group-2 layout of a pass of `shading`'s chain that reads `reads` and
/// writes `writes` (`chain_binding` places each): its passes', its scene
/// pass's, and an image kind's that reads the chain (src/views.rs).
pub fn chain_layout(
    device: &wgpu::Device,
    shading: &SceneShading,
    label: &str,
    reads: &[&str],
    writes: &[&str],
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayout {
    let mut entries: Vec<wgpu::BindGroupLayoutEntry> = Vec::new();
    // The volumes a pass samples (`volume_layout`), or the program's scene group, are group 3's.
    for (name, written) in reads
        .iter()
        .map(|n| (*n, false))
        .chain(writes.iter().map(|n| (*n, true)))
        .filter(|(n, _)| !crate::graph::is_group3(shading, n))
    {
        let binding = chain_binding(shading, name, written);
        if entries.iter().any(|e| e.binding == binding) {
            continue;
        }
        let ty = if shading.depth_views.is_some_and(|d| d.name == name) {
            wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2Array,
                multisampled: false,
            }
        } else if shading.buffers.iter().any(|b| b.name == name) {
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            }
        } else if written {
            wgpu::BindingType::StorageTexture {
                access: wgpu::StorageTextureAccess::WriteOnly,
                format: target_format(shading, name),
                view_dimension: wgpu::TextureViewDimension::D2,
            }
        } else if name == "depth" {
            wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: true,
            }
        } else {
            wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: shading.attachment(name),
            }
        };
        entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty,
            count: None,
        });
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    })
}

/// The lab's own bind group layouts: the frame's (group 0), a texture pair's, the verification's, a scene
/// shading's (group 1), and the comparison's.
struct BaseLayouts {
    frame: wgpu::BindGroupLayout,
    texture_pair: wgpu::BindGroupLayout,
    verify: wgpu::BindGroupLayout,
    shading: wgpu::BindGroupLayout,
    compare: wgpu::BindGroupLayout,
}

impl BaseLayouts {
    fn new(device: &wgpu::Device) -> BaseLayouts {
        let mut frame_entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<
                    crate::layout::FrameUniforms,
                >() as u64),
            },
            count: None,
        }];
        frame_entries.push(storage(1, true));
        frame_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        frame_entries.extend([storage(3, true), storage(4, true)]);
        frame_entries.push(texture(5, wgpu::TextureViewDimension::D2));
        frame_entries.push(storage(6, true));
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &frame_entries,
        });
        let texture_pair_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture pair"),
                entries: &[
                    texture(0, wgpu::TextureViewDimension::D2),
                    texture(1, wgpu::TextureViewDimension::D2),
                ],
            });
        let verify_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verify"),
            entries: &[storage(0, false), storage(1, true)],
        });
        let shading_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene shading"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(SHADING_UNIFORM_BYTES),
                    },
                    count: None,
                },
                storage(1, true),
                storage(2, true),
            ],
        });

        let compare_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compare"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(16),
                },
                count: None,
            }],
        });
        BaseLayouts {
            frame: frame_layout,
            texture_pair: texture_pair_layout,
            verify: verify_layout,
            shading: shading_layout,
            compare: compare_layout,
        }
    }

    /// The lab's pipeline layouts: the frame's group alone, with a texture pair, with a pair and the comparison, and
    /// with the verification's.
    fn pipeline_layouts(&self, device: &wgpu::Device) -> [wgpu::PipelineLayout; 4] {
        let (frame_layout, texture_pair_layout, verify_layout, compare_layout) =
            (&self.frame, &self.texture_pair, &self.verify, &self.compare);
        let frame_only = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame only"),
            bind_group_layouts: &[Some(frame_layout)],
            immediate_size: 0,
        });
        let with_pair = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame + textures"),
            bind_group_layouts: &[Some(frame_layout), Some(texture_pair_layout)],
            immediate_size: 0,
        });
        let with_compare = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame + textures + compare"),
            bind_group_layouts: &[
                Some(frame_layout),
                Some(texture_pair_layout),
                Some(compare_layout),
            ],
            immediate_size: 0,
        });
        let with_verify = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame + verify"),
            bind_group_layouts: &[Some(frame_layout), Some(verify_layout)],
            immediate_size: 0,
        });
        [frame_only, with_pair, with_compare, with_verify]
    }
}

/// The overlay's pipelines (src/overlay.wgsl): the ghosts, the segments and boxes (each also seen through
/// everything, x-ray), the points, and the cuboids; in the order of `Pipelines`' fields.
fn overlay_pipelines(
    device: &wgpu::Device,
    frame_only: &wgpu::PipelineLayout,
    overlay_module: &wgpu::ShaderModule,
) -> [wgpu::RenderPipeline; 7] {
    use wgpu::CompareFunction::{Always, GreaterEqual};
    let [mesh, objects] = mesh_layouts();
    let mesh_buffers = [Some(mesh), Some(objects)];
    let premultiplied = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
    let overlay = |label: &'static str,
                   vs: &'static str,
                   fs: &'static str,
                   buffers: &[Option<wgpu::VertexBufferLayout>],
                   depth: (wgpu::CompareFunction, bool)| {
        render_pipeline(
            device,
            frame_only,
            PipelineSpec {
                label,
                module: overlay_module,
                vs,
                fs,
                buffers,
                format: OVERLAY_FORMAT,
                samples: MSAA,
                depth: Some(depth),
                blend: premultiplied,
            },
        )
    };
    let segments = [Some(instance_layout::<SegmentInstance>(&SEGMENT_ATTRS))];
    let boxes = [Some(instance_layout::<BoxInstance>(&BOX_ATTRS))];
    let points = [Some(instance_layout::<PointInstance>(&POINT_ATTRS))];
    let cuboids = [Some(instance_layout::<CuboidInstance>(&CUBOID_ATTRS))];
    [
        overlay(
            "ghost",
            "ghostVS",
            "ghostFS",
            &mesh_buffers,
            (GreaterEqual, false),
        ),
        overlay(
            "segment",
            "segmentVS",
            "lineFS",
            &segments,
            (GreaterEqual, false),
        ),
        overlay(
            "segment x-ray",
            "segmentVS",
            "lineFS",
            &segments,
            (Always, false),
        ),
        overlay("box", "boxVS", "lineFS", &boxes, (GreaterEqual, false)),
        overlay("box x-ray", "boxVS", "lineFS", &boxes, (Always, false)),
        overlay("point", "pointVS", "pointFS", &points, (Always, false)),
        overlay(
            "cuboid",
            "cuboidVS",
            "cuboidFS",
            &cuboids,
            (GreaterEqual, true),
        ),
    ]
}

/// The composites into the frame: the viewport's, the scene's alone, one over another at an alpha, and an
/// experiment's difference; in the order of `Pipelines`' fields.
fn composite_pipelines(
    device: &wgpu::Device,
    with_pair: &wgpu::PipelineLayout,
    with_compare: &wgpu::PipelineLayout,
    scene_module: &wgpu::ShaderModule,
) -> [wgpu::RenderPipeline; 4] {
    [
        render_pipeline(
            device,
            with_pair,
            PipelineSpec {
                label: "composite",
                module: scene_module,
                vs: "fullscreenVS",
                fs: "compositeFS",
                buffers: &[],
                format: FRAME_FORMAT,
                samples: 1,
                depth: None,
                blend: None,
            },
        ),
        // A view laid over another (src/view.rs): blended by a constant (its alpha).
        render_pipeline(
            device,
            with_pair,
            PipelineSpec {
                label: "composite over",
                module: scene_module,
                vs: "fullscreenVS",
                fs: "compositeFS",
                buffers: &[],
                format: FRAME_FORMAT,
                samples: 1,
                depth: None,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Constant,
                        dst_factor: wgpu::BlendFactor::OneMinusConstant,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Constant,
                        dst_factor: wgpu::BlendFactor::OneMinusConstant,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
            },
        ),
        render_pipeline(
            device,
            with_pair,
            PipelineSpec {
                label: "composite scene",
                module: scene_module,
                vs: "fullscreenVS",
                fs: "compositeSceneFS",
                buffers: &[],
                format: FRAME_FORMAT,
                samples: 1,
                depth: None,
                blend: None,
            },
        ),
        render_pipeline(
            device,
            with_compare,
            PipelineSpec {
                label: "composite difference",
                module: scene_module,
                vs: "fullscreenVS",
                fs: "compositeDiffFS",
                buffers: &[],
                format: FRAME_FORMAT,
                samples: 1,
                depth: None,
                blend: None,
            },
        ),
    ]
}

/// The lab's verification entry points' pipelines (`VERIFY_ENTRIES`), by entry point.
fn verify_pipelines(
    device: &wgpu::Device,
    with_verify: &wgpu::PipelineLayout,
    verify_module: &wgpu::ShaderModule,
) -> BTreeMap<&'static str, wgpu::ComputePipeline> {
    VERIFY_ENTRIES
        .iter()
        .map(|&entry| {
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(with_verify),
                module: verify_module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            });
            (entry, pipeline)
        })
        .collect()
}
