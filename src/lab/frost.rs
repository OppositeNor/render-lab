//! Frosted glass for the homepage (shaders/wgsl/frost.wgsl): the renderer's
//! frame target is halved twice, blurred at quarter resolution, and drawn
//! tinted, grained, and vignetted behind the page.

use crate::lab::shaders;
use std::borrow::Cow;

/// The format of the intermediate targets: the frame's, sRGB-encoded bytes.
const BLUR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Separable blur rounds at quarter resolution.
const BLUR_ROUNDS: usize = 2;

struct Target {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl Target {
    fn new(device: &wgpu::Device, label: &str, size: [u32; 2]) -> Target {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: size[0].max(1), height: size[1].max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: BLUR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Target { _texture: texture, view }
    }
}

/// The half and quarter targets for one frame size.
struct Targets {
    size: [u32; 2],
    half: Target,
    quarter: [Target; 2],
    half_group: wgpu::BindGroup,
    quarter_groups: [wgpu::BindGroup; 2],
}

pub struct Frost {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    downsample: wgpu::RenderPipeline,
    blur: [wgpu::RenderPipeline; 2],
    frost: wgpu::RenderPipeline,
    targets: Option<Targets>,
}

impl Frost {
    /// `output`: the format of the target the page is drawn into.
    pub fn new(device: &wgpu::Device, output: wgpu::TextureFormat) -> anyhow::Result<Frost> {
        // `shader/frost` has no prelude.
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("frost"), source: wgpu::ShaderSource::Wgsl(Cow::Owned(shaders::module(shaders::FROST)?)) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frost"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("frost"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = |entry: &str, format: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("fullscreenVS"), compilation_options: Default::default(), buffers: &[] },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("frost"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        Ok(Frost {
            downsample: pipeline("downsampleFS", BLUR_FORMAT),
            blur: [pipeline("blurHorizontalFS", BLUR_FORMAT), pipeline("blurVerticalFS", BLUR_FORMAT)],
            frost: pipeline("frostFS", output),
            layout,
            sampler,
            targets: None,
        })
    }

    fn group(&self, device: &wgpu::Device, view: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frost"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) }],
        })
    }

    fn pass(encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, pipeline: &wgpu::RenderPipeline, group: &wgpu::BindGroup) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("frost blur"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Blur `frame` (`size` pixels, the renderer's frame target) into the
    /// quarter-resolution target `draw` reads.
    pub fn blur(&mut self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, frame: &wgpu::TextureView, size: [u32; 2]) {
        if self.targets.as_ref().is_none_or(|t| t.size != size) {
            let half = Target::new(device, "frost half", [size[0] / 2, size[1] / 2]);
            let quarter = [Target::new(device, "frost quarter A", [size[0] / 4, size[1] / 4]), Target::new(device, "frost quarter B", [size[0] / 4, size[1] / 4])];
            let half_group = self.group(device, &half.view);
            let quarter_groups = [self.group(device, &quarter[0].view), self.group(device, &quarter[1].view)];
            self.targets = Some(Targets { size, half, quarter, half_group, quarter_groups });
        }
        // The frame view changes when the renderer resizes, so its group is made each time.
        let frame_group = self.group(device, frame);
        let t = self.targets.as_ref().unwrap();
        Self::pass(encoder, &t.half.view, &self.downsample, &frame_group);
        Self::pass(encoder, &t.quarter[0].view, &self.downsample, &t.half_group);
        for _ in 0..BLUR_ROUNDS {
            Self::pass(encoder, &t.quarter[1].view, &self.blur[0], &t.quarter_groups[0]);
            Self::pass(encoder, &t.quarter[0].view, &self.blur[1], &t.quarter_groups[1]);
        }
    }

    /// Draw the frosted backdrop over the whole target of `pass`.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(t) = &self.targets else { return };
        pass.set_pipeline(&self.frost);
        pass.set_bind_group(0, &t.quarter_groups[0], &[]);
        pass.draw(0..3, 0..1);
    }
}
