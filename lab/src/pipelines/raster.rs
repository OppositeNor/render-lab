//! A shading's raster passes' pipelines (`SceneShading::rasters`): the scene's meshes, each a pipeline (the depth
//! views one per layer), with the group-2 layout of what its fragment entry reads.

use super::*;
use crate::investigation::{RasterColor, RasterDepth, RasterPass};

/// A raster pass's pipelines (one per layer of the depth views, else one) and its group-2 layout (none for a pass
/// that draws depth only).
pub struct RasterPipelines {
    pub pipelines: Vec<wgpu::RenderPipeline>,
    pub group: Option<wgpu::BindGroupLayout>,
}

/// What every raster pipeline of a shading shares: the device, the module, the shading, and the layouts of the
/// frame (group 0), the shading (1), and the program's scene uses (3).
pub(super) struct RasterContext<'a> {
    pub device: &'a wgpu::Device,
    pub module: &'a wgpu::ShaderModule,
    pub shading: &'a SceneShading,
    pub frame: &'a wgpu::BindGroupLayout,
    pub own: &'a wgpu::BindGroupLayout,
    pub program_scene: Option<&'a wgpu::BindGroupLayout>,
}

impl RasterContext<'_> {
    /// Raster pass `r`'s pipelines. With a fragment entry, it binds the frame, the shading, its reads (group 2),
    /// and the program's scene uses (3), and draws multisampled into its targets (or the HDR target); depth only,
    /// the frame and the shading. Its depth: written (`Greater`), tested (`GreaterEqual`, not written), or the
    /// depth views' layers (single-sample, with their bias).
    pub fn pipelines(&self, r: &RasterPass) -> RasterPipelines {
        let group = r.fragment.map(|_| {
            chain_layout(
                self.device,
                self.shading,
                r.name,
                r.reads,
                &[],
                wgpu::ShaderStages::FRAGMENT,
            )
        });
        let groups: Vec<Option<&wgpu::BindGroupLayout>> = match &group {
            Some(g) => vec![
                Some(self.frame),
                Some(self.own),
                Some(g),
                self.program_scene,
            ],
            None => vec![Some(self.frame), Some(self.own)],
        };
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(r.name),
                bind_group_layouts: &groups,
                immediate_size: 0,
            });
        let pipelines = match (r.depth, self.shading.depth_views) {
            // The depth views: one pipeline per layer, its own vertex entry, the views' bias, single-sample.
            (RasterDepth::Layers, Some(d)) => d
                .vertices
                .iter()
                .map(|&vertex| {
                    let depth = depth_state(true, wgpu::CompareFunction::Greater, d.bias);
                    self.pipeline(r, &layout, vertex, depth, 1)
                })
                .collect(),
            (RasterDepth::Layers, None) => Vec::new(),
            (RasterDepth::Write, _) => {
                let depth = depth_state(true, wgpu::CompareFunction::Greater, (0, 0.0));
                vec![self.pipeline(r, &layout, r.vertex, depth, MSAA)]
            }
            (RasterDepth::Test, _) => {
                let depth = depth_state(false, wgpu::CompareFunction::GreaterEqual, (0, 0.0));
                vec![self.pipeline(r, &layout, r.vertex, depth, MSAA)]
            }
        };
        RasterPipelines { pipelines, group }
    }

    /// One pipeline of raster pass `r`: the meshes placed by `vertex`, its fragment entry's colour targets (the HDR
    /// target, or its targets' formats), `depth`, and `samples` per pixel.
    fn pipeline(
        &self,
        r: &RasterPass,
        layout: &wgpu::PipelineLayout,
        vertex: &str,
        depth: wgpu::DepthStencilState,
        samples: u32,
    ) -> wgpu::RenderPipeline {
        let formats: Vec<wgpu::TextureFormat> = match r.color {
            RasterColor::Hdr => vec![HDR_FORMAT],
            RasterColor::Targets(t) => t.iter().map(|o| target_format(self.shading, o)).collect(),
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
        let [mesh, objects] = mesh_layouts();
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(r.fragment.unwrap_or(vertex)),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: self.module,
                    entry_point: Some(vertex),
                    compilation_options: Default::default(),
                    buffers: &[Some(mesh), Some(objects)],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(depth),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: r.fragment.map(|entry| wgpu::FragmentState {
                    module: self.module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            })
    }
}

/// A raster pipeline's depth: written or not, its comparison (reversed: nearer is greater), and its bias.
fn depth_state(
    write: bool,
    compare: wgpu::CompareFunction,
    bias: (i32, f32),
) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(write),
        depth_compare: Some(compare),
        stencil: Default::default(),
        bias: wgpu::DepthBiasState {
            constant: bias.0,
            slope_scale: bias.1,
            clamp: 0.0,
        },
    }
}
