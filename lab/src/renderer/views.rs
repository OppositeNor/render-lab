//! A story's image kinds: the GPU images an investigation's shaders define,
//! instanced and dispatched for the frames that show them.

use super::*;

/// A story's image of an image kind (src/views.rs): its texture, written
/// each frame by the kind's entry point, and shown through egui.
pub(super) struct ViewInstance {
    pub(super) kind: &'static crate::views::ViewKind,
    pub(super) _texture: wgpu::Texture,
    /// Its texture's size in texels (`ViewKind::texels`).
    pub(super) size: [u32; 2],
    pub(super) params: wgpu::Buffer,
    pub(super) group: wgpu::BindGroup,
    pub(super) id: egui::TextureId,
}

/// An image kind's compute pipeline (src/views.rs): the frame and the scene
/// shading at groups 0 and 1, the chain reads at group 2 when it reads any,
/// and its texture and parameters at group 3.
pub(super) fn view_pipeline(device: &wgpu::Device, frame: &wgpu::BindGroupLayout, shading: &wgpu::BindGroupLayout, chain: Option<&wgpu::BindGroupLayout>, view: &wgpu::BindGroupLayout, kind: &crate::views::ViewKind) -> wgpu::ComputePipeline {
    let source = crate::shaders::module(&kind.module).unwrap_or_else(|e| panic!("{e:#}"));
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(&kind.module), source: wgpu::ShaderSource::Wgsl(source.into()) });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(&kind.entry), bind_group_layouts: &[Some(frame), Some(shading), chain, Some(view)], immediate_size: 0 });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(&kind.entry), layout: Some(&layout), module: &module, entry_point: Some(&kind.entry), compilation_options: Default::default(), cache: None })
}

impl Renderer {
    /// The textures of the image instances `state` shows (src/views.rs): made
    /// for a new one and registered with egui, their parameters written, and
    /// those it no longer shows let go.
    pub(super) fn prepare_views(&mut self, state: &ShotState) {
        use crate::utility::inset::{view_texture, ImageSource};
        let package = investigation::current().package().unwrap_or("render_lab");
        let shown: Vec<(&str, &str, &crate::utility::inset::ImageParams)> = state.images.iter().filter_map(|i| match &i.source {
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
        let viewport = self.viewport.as_ref().map_or([0, 0], |v| v.size);
        for (name, kind, params) in shown {
            let kind = crate::views::kind(package, kind).unwrap_or_else(|e| panic!("{e}"));
            let size = kind.texels(viewport);
            // One that takes the viewport's shape is made again when that changes.
            if let Some(view) = self.views.remove(name) {
                if view.size == size {
                    self.views.insert(name.to_string(), view);
                } else {
                    self.egui_renderer.free_texture(&view.id);
                    self.textures.remove(&view_texture(name));
                }
            }
            if !self.views.contains_key(name) {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(name),
                    size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
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
                self.textures.insert(view_texture(name), (id, size));
                self.views.insert(name.to_string(), ViewInstance { kind, _texture: texture, size, params, group, id });
            }
            let view = &self.views[name];
            self.gpu.queue.write_buffer(&view.params, 0, &view.kind.pack(params));
        }
    }

    /// Runs each image instance's entry point into its texture: before the
    /// scene those that read nothing of the pass chain, after it
    /// (`after_chain`) those that do (src/views.rs `reads`).
    pub(super) fn dispatch_views(&mut self, encoder: &mut wgpu::CommandEncoder, after_chain: bool) {
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
                pass.dispatch_workgroups(view.size[0].div_ceil(kind.workgroup[0].max(1)), view.size[1].div_ceil(kind.workgroup[1].max(1)), 1);
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
            pass.dispatch_workgroups(view.size[0].div_ceil(kind.workgroup[0].max(1)), view.size[1].div_ceil(kind.workgroup[1].max(1)), 1);
        }
    }
}
