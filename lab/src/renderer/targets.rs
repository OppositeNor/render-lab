//! What the renderer draws into and draws from: the viewport's targets, an
//! investigation's pass chain's targets and buffers, the record buffers, a
//! view's saved slot, and the keys that tell when a frame can stay still.

use super::*;

/// A vertex/storage buffer that grows to fit its contents.
pub struct GrowBuffer {
    pub buffer: wgpu::Buffer,
    capacity: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
    pub count: u32,
}

impl GrowBuffer {
    pub(super) fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages, capacity: u64) -> Self {
        let capacity = capacity.max(256);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: capacity, usage: usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        GrowBuffer { buffer, capacity, usage, label, count: 0 }
    }

    /// Returns true when the buffer was reallocated.
    pub(super) fn write<T: Pod>(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[T]) -> bool {
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

/// A number for each set of targets made (`ViewportTargets`, `ChainTargets`):
/// what a program's groups were made for (`ProgramRuntime::bind`).
pub(super) static TARGETS_MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

pub(super) fn targets_made() -> usize {
    TARGETS_MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub(super) struct ViewportTargets {
    /// Its number (`targets_made`).
    pub(super) made: usize,
    pub(super) size: [u32; 2],
    pub(super) hdr_texture: wgpu::Texture,
    pub(super) hdr_msaa: wgpu::TextureView,
    pub(super) hdr: wgpu::TextureView,
    pub(super) overlay_msaa: wgpu::TextureView,
    pub(super) overlay: wgpu::TextureView,
    pub(super) depth: wgpu::TextureView,
    pub(super) composite: wgpu::BindGroup,
}

pub(super) fn target(device: &wgpu::Device, label: &str, size: [u32; 2], format: wgpu::TextureFormat, samples: u32, usage: wgpu::TextureUsages) -> wgpu::Texture {
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
    pub(super) fn new(device: &wgpu::Device, pipelines: &Pipelines, size: [u32; 2]) -> Self {
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let sampled = attach | wgpu::TextureUsages::TEXTURE_BINDING;
        let view = |t: wgpu::Texture| t.create_view(&Default::default());
        // An investigation's pass chain may write the HDR target from a compute pass.
        // An experiment reads it back (`Renderer::read_view`).
        let hdr_texture = target(device, "scene hdr", size, HDR_FORMAT, 1, sampled | wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC);
        let hdr = view(hdr_texture.clone());
        let overlay = view(target(device, "overlay", size, OVERLAY_FORMAT, 1, sampled));
        let composite = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("composite"),
            layout: &pipelines.texture_pair_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&hdr) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&overlay) }],
        });
        ViewportTargets {
            made: targets_made(),
            size,
            hdr_texture,
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
pub(super) struct SceneResources {
    pub(super) mesh_vertices: wgpu::Buffer,
    pub(super) mesh_indices: wgpu::Buffer,
    pub(super) object_ids: wgpu::Buffer,
}

/// What a view keeps of its own between frames (src/view.rs): its targets,
/// its chain, how long it has held still, and its scene shading's program
/// (images, history, plan), by shading. The view being drawn has its slot's
/// contents in the renderer's own fields (`Renderer::enter`).
#[derive(Default)]
pub(super) struct ViewSlot {
    pub(super) viewport: Option<ViewportTargets>,
    pub(super) chain: Option<ChainTargets>,
    pub(super) still: (Option<u64>, u32),
    pub(super) still_scene: (Option<u64>, u32),
    /// Each program's runtime and handles, by its shading's source.
    pub(super) programs: Vec<(usize, std::cell::RefCell<crate::program::ProgramRuntime>, std::cell::RefCell<crate::program::ProgramHandles>)>,
}

/// The targets of an investigation's pass chain at one viewport size, with
/// each pass's group-2 bind group.
pub(super) struct ChainTargets {
    /// Its number (`targets_made`).
    pub(super) made: usize,
    pub(super) source: &'static SceneShading,
    pub(super) size: [u32; 2],
    /// `Shading::scale` when it was made.
    pub(super) scale: u32,
    /// Each target's divisor of the viewport's size.
    pub(super) divisors: Vec<(&'static str, u32)>,
    /// The targets by name: the scene pass's outputs multisampled, the rest single-sample.
    pub(super) views: Vec<(&'static str, wgpu::TextureView)>,
    pub(super) buffers: Vec<(&'static str, wgpu::Buffer)>,
    /// The depth views' texture array as one view (bound by name), and each of its layers (drawn into).
    pub(super) depth_views: Option<(wgpu::TextureView, Vec<wgpu::TextureView>)>,
    /// What the depth views were last drawn from (`depth_views_key`): they
    /// are drawn again only when that changes.
    pub(super) depth_views_drawn: std::cell::Cell<Option<u64>>,
    /// Group 2 of each compute pass (`SceneShading::passes`).
    pub(super) groups: Vec<wgpu::BindGroup>,
    /// Group 2 of the scene pass.
    pub(super) scene_group: wgpu::BindGroup,
}

impl ChainTargets {
    pub(super) fn new(device: &wgpu::Device, shading: &Shading, viewport: &ViewportTargets) -> ChainTargets {
        let source = shading.source;
        let size = viewport.size;
        let written: Vec<&str> = source.passes.iter().flat_map(|p| p.writes.iter().copied()).collect();
        let divisors: Vec<(&'static str, u32)> = source.targets.iter().map(|t| (t.name, if source.resolution.is_some_and(|r| r.targets.contains(&t.name)) { shading.scale } else { t.divisor })).collect();
        let views: Vec<(&'static str, wgpu::TextureView)> = source
            .targets
            .iter()
            .zip(&divisors)
            .map(|(t, &(_, divisor))| {
                let output = source.attachment(t.name);
                let mut usage = wgpu::TextureUsages::TEXTURE_BINDING;
                if output {
                    usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
                }
                if written.contains(&t.name) {
                    usage |= wgpu::TextureUsages::STORAGE_BINDING;
                }
                let s = if divisor == 1 { size } else { [(size[0] / divisor).max(1), (size[1] / divisor).max(1)] };
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
            if let (Some(d), Some((all, _))) = (source.depth_views, &depth_views) && d.name == name {
                return wgpu::BindingResource::TextureView(all);
            }
            wgpu::BindingResource::TextureView(match name {
                "hdr" => &viewport.hdr,
                "depth" => &viewport.depth,
                _ => &views.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("{}: no target {name}", source.fragment)).1,
            })
        };
        let group = |label: &str, layout: &wgpu::BindGroupLayout, reads: &[&str], writes: &[&str]| {
            let mut entries: Vec<wgpu::BindGroupEntry> = Vec::new();
            for (name, written) in reads.iter().map(|n| (*n, false)).chain(writes.iter().map(|n| (*n, true))).filter(|(n, _)| !crate::graph::is_group3(source, n)) {
                let binding = gpu::chain_binding(source, name, written);
                if !entries.iter().any(|e| e.binding == binding) {
                    entries.push(wgpu::BindGroupEntry { binding, resource: resource(name) });
                }
            }
            device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(label), layout, entries: &entries })
        };
        let groups = source.passes.iter().zip(&shading.pipelines.passes).map(|(pass, (_, layout))| group(pass.entry, layout, pass.reads, pass.writes)).collect();
        let scene_group = group("scene reads", &shading.pipelines.scene_group, source.scene_reads, &[]);
        ChainTargets { made: targets_made(), source, size, scale: shading.scale, divisors, views, buffers, depth_views, depth_views_drawn: std::cell::Cell::new(None), groups, scene_group }
    }

    pub(super) fn view(&self, name: &str) -> &wgpu::TextureView {
        &self.views.iter().find(|(n, _)| *n == name).expect("a scene output is a chain target").1
    }

    /// A group-2 bind group for `layout` that reads `reads` (targets, `hdr`,
    /// `depth`, buffers, or the depth views), as a chain pass binds them: an
    /// image kind that reads the chain (src/views.rs).
    pub(super) fn read_group(&self, device: &wgpu::Device, viewport: &ViewportTargets, layout: &wgpu::BindGroupLayout, reads: &[&str]) -> wgpu::BindGroup {
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

    pub(super) fn target<'a>(&'a self, viewport: &'a ViewportTargets, name: &str) -> &'a wgpu::TextureView {
        match name {
            "hdr" => &viewport.hdr,
            "depth" => &viewport.depth,
            _ => &self.views.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("{}: no target {name}", self.source.fragment)).1,
        }
    }

    /// The size in pixels a pass writing `name` covers: a target's, or the viewport's for a buffer.
    pub(super) fn size_of(&self, name: &str) -> [u32; 2] {
        match self.divisors.iter().find(|(n, _)| *n == name).map_or(1, |d| d.1) {
            1 => self.size,
            divisor => [(self.size[0] / divisor).max(1), (self.size[1] / divisor).max(1)],
        }
    }

    /// Encode compute passes `steps` (indices into `SceneShading::passes`),
    /// in order, with their bind groups. Timed (src/timing.rs), each pass
    /// runs in a compute pass of its own that writes its timestamps.
    pub(super) fn dispatch(&self, encoder: &mut wgpu::CommandEncoder, frame: &wgpu::BindGroup, shading: &Shading, steps: &[usize], timing: Option<&mut crate::timing::Timing>) {
        if steps.is_empty() {
            return;
        }
        let all: Vec<&crate::investigation::ShadingPass> = self.source.passes.iter().collect();
        if let Some(timing) = timing {
            for &i in steps {
                let step = all[i];
                let size = self.size_of(step.writes[0]);
                let span = timing.compute_writes(step.entry);
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(step.entry), timestamp_writes: span.as_ref().map(|t| t.compute()) });
                pass.set_bind_group(0, frame, &[]);
                pass.set_bind_group(1, &shading.group, &[]);
                pass.set_pipeline(&shading.pipelines.passes[i].0);
                pass.set_bind_group(2, &self.groups[i], &[]);
                if let Some(v) = shading.volume_sets.get(shading.volume_set) {
                    pass.set_bind_group(3, &v.group, &[]);
                } else if let Some(g) = shading.program.as_ref().and_then(|p| p.borrow().scene_group.clone()) {
                    pass.set_bind_group(3, &g, &[]);
                }
                pass.dispatch_workgroups(size[0].div_ceil(step.workgroup[0]), size[1].div_ceil(step.workgroup[1]), 1);
            }
            return;
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("shading chain"), timestamp_writes: None });
        pass.set_bind_group(0, frame, &[]);
        pass.set_bind_group(1, &shading.group, &[]);
        if let Some(v) = shading.volume_sets.get(shading.volume_set) {
            pass.set_bind_group(3, &v.group, &[]);
        } else if let Some(g) = shading.program.as_ref().and_then(|p| p.borrow().scene_group.clone()) {
            pass.set_bind_group(3, &g, &[]);
        }
        for &i in steps {
            let step = all[i];
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
pub(super) fn still_key(content: &FrameContent, viewport: [u32; 2], camera: bool) -> u64 {
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
pub(super) fn depth_views_key(content: &FrameContent) -> u64 {
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
