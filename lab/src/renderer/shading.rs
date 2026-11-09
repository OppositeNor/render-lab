//! An investigation's scene shading on the GPU: its pipelines and data, the
//! volume sets its update passes write, the scene data a shot selects, and
//! its program's plan.

use super::*;

/// The pipelines and data of one investigation's `SceneShading`.
pub(super) struct Shading {
    pub(super) source: &'static SceneShading,
    pub(super) pipelines: gpu::ShadingPipelines,
    /// Group 1 with the data of the state being drawn (`select_data`).
    pub(super) group: wgpu::BindGroup,
    /// The data built so far, most recently used first: `SceneShading::data`
    /// (key None), and `Investigation::scene_data`'s by key, each with its group.
    pub(super) data: Vec<(Option<u64>, wgpu::Buffer, wgpu::BindGroup)>,
    /// A `scene_data` build on a worker thread: its key, its result, and when it started.
    pub(super) pending: Option<(u64, DataBuild, std::time::Instant)>,
    /// `FrameContent::shading_frame`.
    pub(super) frame_data: wgpu::Buffer,
    /// The volumes (`SceneShading::volumes`), one set per update key kept.
    pub(super) volume_sets: Vec<VolumeSet>,
    /// The set the state being drawn reads (`select_data`).
    pub(super) volume_set: usize,
    /// Update dispatches `select_data` planned and the next `encode_scene`
    /// encodes: into which set, and what.
    pub(super) update_runs: Vec<(usize, Vec<crate::investigation::UpdateDispatch>)>,
    pub(super) update_buffers: Vec<wgpu::Buffer>,
    /// The dispatches' parameters, `UPDATE_PARAMS_STRIDE` apart.
    pub(super) update_params: Option<wgpu::Buffer>,
    pub(super) sampler: wgpu::Sampler,
    /// The divisor of the targets `SceneShading::resolution` names, for the state being drawn.
    pub(super) scale: u32,
    /// A count that orders the volume sets by when they were last used.
    pub(super) clock: u64,
    /// The program's resources, plan, and commands (`SceneShading::program`).
    pub(super) program: Option<std::cell::RefCell<crate::program::ProgramRuntime>>,
    /// An empty group 2 for a scene pass that reads the program at group 3 but no chain.
    pub(super) empty_scene_group: Option<wgpu::BindGroup>,
    /// The program's resources by name (handles, for the inspector, which
    /// draws while the program records): made again when `bind` makes its
    /// images the size of the view again.
    pub(super) program_handles: std::cell::RefCell<crate::program::ProgramHandles>,
}

impl Shading {
    /// The program's resource `name`, for the inspector and the selftests' readbacks.
    pub(super) fn program_handle(&self, name: &str) -> Option<crate::program::ProgramHandle> {
        self.program_handles
            .borrow()
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, h)| h.clone())
    }
}

/// The volumes of one update key: their textures, group 3 that samples them,
/// and each update pass's group 2 for each mip it may write.
pub(super) struct VolumeSet {
    pub(super) key: Option<u64>,
    pub(super) used: u64,
    pub(super) textures: Vec<wgpu::Texture>,
    pub(super) group: wgpu::BindGroup,
    pub(super) updates: Vec<Vec<wgpu::BindGroup>>,
}

/// How many update keys' volumes a shading keeps.
pub(super) const VOLUME_SETS_KEPT: usize = 4;

/// The most dispatches one update plan may have.
pub(super) const UPDATE_DISPATCHES_MAX: u64 = 256;

impl VolumeSet {
    pub(super) fn new(
        device: &wgpu::Device,
        source: &SceneShading,
        pipelines: &gpu::ShadingPipelines,
        buffers: &[wgpu::Buffer],
        params: Option<&wgpu::Buffer>,
        sampler: &wgpu::Sampler,
    ) -> VolumeSet {
        let textures = Self::textures(device, source);
        let views: Vec<wgpu::TextureView> = textures
            .iter()
            .map(|t| t.create_view(&Default::default()))
            .collect();
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Sampler(sampler),
        }];
        entries.extend(
            views
                .iter()
                .enumerate()
                .map(|(v, view)| wgpu::BindGroupEntry {
                    binding: 1 + v as u32,
                    resource: wgpu::BindingResource::TextureView(view),
                }),
        );
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("volumes"),
            layout: pipelines.volumes.as_ref().expect("a volume layout"),
            entries: &entries,
        });
        let updates = source.update.map_or_else(Vec::new, |update| {
            update
                .passes
                .iter()
                .zip(&pipelines.updates)
                .map(|(pass, (_, layout))| {
                    let mips = source
                        .volumes
                        .iter()
                        .filter(|v| pass.writes.contains(&v.name))
                        .map(|v| v.mips)
                        .max()
                        .unwrap_or(1);
                    (0..mips)
                        .map(|mip| {
                            let mip_views: Vec<(u32, wgpu::TextureView)> = source
                                .volumes
                                .iter()
                                .zip(&textures)
                                .enumerate()
                                .filter(|(_, (v, _))| pass.writes.contains(&v.name))
                                .map(|(k, (v, t))| {
                                    (
                                        1 + update.buffers.len() as u32 + k as u32,
                                        t.create_view(&wgpu::TextureViewDescriptor {
                                            base_mip_level: mip.min(v.mips - 1),
                                            mip_level_count: Some(1),
                                            ..Default::default()
                                        }),
                                    )
                                })
                                .collect();
                            let mut entries = vec![wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                    buffer: params.expect("update parameters"),
                                    offset: 0,
                                    size: wgpu::BufferSize::new(64),
                                }),
                            }];
                            entries.extend(buffers.iter().enumerate().map(|(k, b)| {
                                wgpu::BindGroupEntry {
                                    binding: 1 + k as u32,
                                    resource: b.as_entire_binding(),
                                }
                            }));
                            entries.extend(mip_views.iter().map(|(binding, view)| {
                                wgpu::BindGroupEntry {
                                    binding: *binding,
                                    resource: wgpu::BindingResource::TextureView(view),
                                }
                            }));
                            device.create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some(pass.entry),
                                layout,
                                entries: &entries,
                            })
                        })
                        .collect()
                })
                .collect()
        });
        VolumeSet {
            key: None,
            used: 0,
            textures,
            group,
            updates,
        }
    }

    /// The shading's volumes' 3D textures, with their mips.
    fn textures(device: &wgpu::Device, source: &SceneShading) -> Vec<wgpu::Texture> {
        let textures: Vec<wgpu::Texture> = source
            .volumes
            .iter()
            .map(|v| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(v.name),
                    size: wgpu::Extent3d {
                        width: v.size[0],
                        height: v.size[1],
                        depth_or_array_layers: v.size[2],
                    },
                    mip_level_count: v.mips,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D3,
                    format: v.format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::STORAGE_BINDING
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
            })
            .collect();
        textures
    }
}

/// How many `scene_data` builds a shading keeps.
pub(super) const SCENE_DATA_KEPT: usize = 4;

impl Renderer {
    /// The current investigation's own scene shading, built on first use.
    pub(super) fn shading(&mut self) -> Option<&Shading> {
        let source = investigation::current().scene_shading()?;
        if !self.shadings.iter().any(|s| std::ptr::eq(s.source, source)) {
            let device = &self.gpu.device;
            let pipelines = self
                .pipelines
                .shading(device, source)
                .unwrap_or_else(|e| panic!("{e:#}"));
            let (data, frame_data, group) = self.shading_data(source);
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("volumes"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            });
            let (update_buffers, update_params) = Self::update_buffers(device, source);
            let volume_sets = if source.volumes.is_empty() {
                Vec::new()
            } else {
                vec![VolumeSet::new(
                    device,
                    source,
                    &pipelines,
                    &update_buffers,
                    update_params.as_ref(),
                    &sampler,
                )]
            };
            let program = source
                .program
                .zip(pipelines.program.as_ref())
                .map(|(p, pp)| {
                    std::cell::RefCell::new(crate::program::ProgramRuntime::new(
                        device, source, p, pp,
                    ))
                });
            let program_handles = std::cell::RefCell::new(
                program
                    .as_ref()
                    .map_or(Vec::new(), |p| p.borrow().handles()),
            );
            let empty_scene_group = (source.program.is_some_and(|p| !p.scene_uses.is_empty())
                && source.scene_reads.is_empty())
            .then(|| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("no scene reads"),
                    layout: &pipelines.scene_group,
                    entries: &[],
                })
            });
            self.shadings.push(Shading {
                source,
                pipelines,
                group: group.clone(),
                data: vec![(None, data, group)],
                pending: None,
                frame_data,
                volume_sets,
                volume_set: 0,
                update_runs: Vec::new(),
                update_buffers,
                update_params,
                sampler,
                scale: 1,
                clock: 0,
                program,
                empty_scene_group,
                program_handles,
            });
        }
        self.shadings
            .iter()
            .find(|s| std::ptr::eq(s.source, source))
    }

    /// Group 1 of a new shading: its uniforms, its data as built once (`SceneShading::data`), and its frame's data;
    /// the data's buffer, the frame data's, and the group.
    fn shading_data(&self, source: &SceneShading) -> (wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup) {
        let device = &self.gpu.device;
        let mut data = (source.data)();
        if data.is_empty() {
            data.push([0.0; 4]);
        }
        let data = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene shading data"),
            contents: bytemuck::cast_slice(&data),
            usage: wgpu::BufferUsages::STORAGE,
        });
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
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.shading_uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: data.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: frame_data.as_entire_binding(),
                },
            ],
        });
        (data, frame_data, group)
    }

    /// The shading's update buffers, and its update passes' parameters (`UPDATE_DISPATCHES_MAX` of them).
    fn update_buffers(
        device: &wgpu::Device,
        source: &SceneShading,
    ) -> (Vec<wgpu::Buffer>, Option<wgpu::Buffer>) {
        let update_buffers: Vec<wgpu::Buffer> = source.update.map_or_else(Vec::new, |u| {
            u.buffers
                .iter()
                .map(|b| {
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(b.name),
                        size: ((b.words)().max(1) * 4),
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: false,
                    })
                })
                .collect()
        });
        let update_params = source.update.map(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("update parameters"),
                size: UPDATE_DISPATCHES_MAX * crate::pipelines::UPDATE_PARAMS_STRIDE,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        (update_buffers, update_params)
    }

    /// Group 1 of the current shading with the data `state` needs
    /// (`Investigation::scene_data`): a kept one, or, while a new one builds
    /// on a worker thread, the last one drawn. With `wait`, the build is
    /// waited for (a research run). A finished build restarts the history.
    pub(super) fn select_data(&mut self, state: &ShotState, wait: bool) {
        if self.shading().is_none() {
            return;
        }
        let wanted = investigation::current().scene_data(state);
        let source = investigation::current()
            .scene_shading()
            .expect("a scene shading");
        let device = self.gpu.device.clone();
        let layout = self.pipelines.shading_layout.clone();
        let uniforms = self.shading_uniforms.clone();
        let shading = self
            .shadings
            .iter_mut()
            .find(|s| std::ptr::eq(s.source, source))
            .expect("built");
        let key = wanted.as_ref().map(|w| w.key);
        if let Some(w) = wanted {
            let kept = shading.data.iter().any(|d| d.0 == Some(w.key));
            let building = shading.pending.as_ref().is_some_and(|p| p.0 == w.key);
            if !kept && !building {
                let (tx, rx) = std::sync::mpsc::channel();
                let build = w.build;
                std::thread::Builder::new()
                    .name("scene data".into())
                    .spawn(move || {
                        let _ = tx.send(build());
                    })
                    .expect("a thread for the scene data");
                shading.pending = Some((w.key, rx, std::time::Instant::now()));
            }
        }
        let mut finished = false;
        finished |= Self::land_data(shading, &device, &layout, &uniforms, wait, key);
        if let Some(i) = shading.data.iter().position(|d| d.0 == key) {
            shading.group = shading.data[i].2.clone();
        }
        if let Some(r) = source.resolution {
            shading.scale = (r.divisor)(state).max(1);
        }
        finished |= Self::select_volume_set(shading, &device, source, state);
        if finished {
            self.still = (None, 0);
            self.still_scene = (None, 0);
            for slot in self.slots.values_mut() {
                slot.still = (None, 0);
                slot.still_scene = (None, 0);
            }
        }
    }

    /// The scene data building for `shading`, once it has landed (waited for with `wait` when it is the data `key`
    /// names): its group made and kept beside the others. Whether it landed.
    fn land_data(
        shading: &mut Shading,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniforms: &wgpu::Buffer,
        wait: bool,
        key: Option<u64>,
    ) -> bool {
        let mut finished = false;
        if let Some((pending_key, rx, _)) = &shading.pending {
            let result = if wait && key == Some(*pending_key) {
                rx.recv().ok()
            } else {
                rx.try_recv().ok()
            };
            if let Some(mut data) = result {
                if data.is_empty() {
                    data.push([0.0; 4]);
                }
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("scene shading data"),
                    contents: bytemuck::cast_slice(&data),
                    usage: wgpu::BufferUsages::STORAGE,
                });
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("scene shading"),
                    layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: uniforms.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: shading.frame_data.as_entire_binding(),
                        },
                    ],
                });
                shading.data.insert(
                    1.min(shading.data.len()),
                    (Some(*pending_key), buffer, group),
                );
                // Keep the built-once data and the latest builds.
                while shading.data.len() > SCENE_DATA_KEPT + 1 {
                    let last = shading
                        .data
                        .iter()
                        .rposition(|d| d.0.is_some())
                        .expect("a build to let go");
                    shading.data.remove(last);
                }
                shading.pending = None;
                finished = true;
            }
        }
        finished
    }

    /// The volume set `state`'s update plan fills (kept, a new one, or the one used longest ago), and its run of
    /// updates when it is fresh or always runs: whether the set is fresh.
    fn select_volume_set(
        shading: &mut Shading,
        device: &wgpu::Device,
        source: &SceneShading,
        state: &ShotState,
    ) -> bool {
        let Some(update) = source.update else {
            return false;
        };
        let plan = (update.plan)(state);
        shading.clock += 1;
        let clock = shading.clock;
        let set = match shading
            .volume_sets
            .iter()
            .position(|v| v.key == Some(plan.key))
        {
            Some(i) => i,
            None => {
                if shading.volume_sets.len() < VOLUME_SETS_KEPT {
                    let set = VolumeSet::new(
                        device,
                        source,
                        &shading.pipelines,
                        &shading.update_buffers,
                        shading.update_params.as_ref(),
                        &shading.sampler,
                    );
                    shading.volume_sets.push(set);
                    shading.volume_sets.len() - 1
                } else {
                    // The set used longest ago, unless a run still waits to fill it.
                    let waiting: Vec<usize> = shading.update_runs.iter().map(|r| r.0).collect();
                    (0..shading.volume_sets.len())
                        .filter(|i| !waiting.contains(i))
                        .min_by_key(|&i| shading.volume_sets[i].used)
                        .unwrap_or(0)
                }
            }
        };
        let fresh = shading.volume_sets[set].key != Some(plan.key);
        if fresh || plan.always {
            assert!(
                plan.dispatches.len() as u64 <= UPDATE_DISPATCHES_MAX,
                "{}: {} update dispatches, at most {UPDATE_DISPATCHES_MAX}",
                source.fragment,
                plan.dispatches.len()
            );
            shading.update_runs.retain(|r| r.0 != set);
            shading.update_runs.push((set, plan.dispatches));
            shading.volume_sets[set].key = Some(plan.key);
        }
        shading.volume_sets[set].used = clock;
        shading.volume_set = set;
        fresh
    }

    /// Seconds the current shading's scene data has been building, while it does.
    /// Live reload (src/resources.rs `reload_changed`): every pipeline made
    /// again from the shaders as they are now. On an error nothing changes
    /// (the last good pipelines keep running) and the error is returned.
    /// The bind groups stay: wgpu shares a layout among layouts of the same
    /// entries, so the new pipelines take the groups made for the old.
    pub fn reload_shaders(&mut self) -> Result<(), String> {
        let device = &self.gpu.device;
        let pipelines = gpu::Pipelines::new(device).map_err(|e| format!("{e:#}"))?;
        let shadings = self
            .shadings
            .iter()
            .map(|s| {
                pipelines
                    .shading(device, s.source)
                    .map_err(|e| format!("{e:#}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.pipelines = pipelines;
        for (s, p) in self.shadings.iter_mut().zip(shadings) {
            s.pipelines = p;
        }
        // Made again when next used.
        self.view_pipelines.clear();
        self.inspect_pipelines.clear();
        Ok(())
    }

    pub fn rebuilding(&self) -> Option<f32> {
        let source = investigation::current().scene_shading()?;
        let shading = self
            .shadings
            .iter()
            .find(|s| std::ptr::eq(s.source, source))?;
        shading
            .pending
            .as_ref()
            .map(|p| p.2.elapsed().as_secs_f32())
    }

    /// Build (and wait for) the scene data `state` needs (a research run).
    pub fn prepare_scene_data(&mut self, state: &ShotState) {
        self.switch_scene(investigation::current().scene(state));
        self.select_data(state, true);
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("update"),
            });
        self.encode_updates(&mut encoder, None);
        self.gpu.queue.submit([encoder.finish()]);
    }

    /// Runs the current shading's program's plan for the main view's frame
    /// (`ShadingProgram::plan`): the next timed `encode_scene` records its commands.
    pub(super) fn plan_program(&mut self, state: &ShotState, history: bool, viewport: [u32; 2]) {
        self.shading();
        let Some(source) = investigation::current().scene_shading() else {
            return;
        };
        let Some(shading) = self
            .shadings
            .iter()
            .find(|s| std::ptr::eq(s.source, source))
        else {
            return;
        };
        let Some(program) = &shading.program else {
            return;
        };
        let frame = crate::investigation::ProgramFrame {
            state,
            history,
            features: self.gpu.device.features(),
            viewport,
        };
        program
            .borrow_mut()
            .plan(&self.gpu.device, &self.gpu.queue, &frame);
    }

    /// The update dispatches `select_data` planned (`SceneShading::update`):
    /// their parameters written, each run of one pass in a compute pass of
    /// its own (timed while `timing` is given, under the pass's entry).
    pub(super) fn encode_updates(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        mut timing: Option<&mut crate::timing::Timing>,
    ) {
        let Some(source) = investigation::current().scene_shading() else {
            return;
        };
        let Some(update) = source.update else { return };
        let Some(shading) = self
            .shadings
            .iter_mut()
            .find(|s| std::ptr::eq(s.source, source))
        else {
            return;
        };
        let runs = std::mem::take(&mut shading.update_runs);
        let shading = &*shading;
        let stride = crate::pipelines::UPDATE_PARAMS_STRIDE;
        for (set, dispatches) in runs {
            let params = shading.update_params.as_ref().expect("update parameters");
            let mut bytes = vec![0u8; dispatches.len() * stride as usize];
            for (i, d) in dispatches.iter().enumerate() {
                bytes[i * stride as usize..i * stride as usize + 64]
                    .copy_from_slice(bytemuck::cast_slice(&d.params));
            }
            // One run per submission: a second run writes the parameters after the first is submitted.
            self.gpu.queue.write_buffer(params, 0, &bytes);
            let volumes = &shading.volume_sets[set];
            let mut i = 0;
            while i < dispatches.len() {
                let p = update
                    .passes
                    .iter()
                    .position(|pass| pass.entry == dispatches[i].pass)
                    .unwrap_or_else(|| {
                        panic!("{}: no update pass {}", source.fragment, dispatches[i].pass)
                    });
                let mut end = i + 1;
                while end < dispatches.len() && dispatches[end].pass == dispatches[i].pass {
                    end += 1;
                }
                let span = timing
                    .as_mut()
                    .and_then(|t| t.compute_writes(update.passes[p].entry));
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some(update.passes[p].entry),
                    timestamp_writes: span.as_ref().map(|t| t.compute()),
                });
                pass.set_pipeline(&shading.pipelines.updates[p].0);
                pass.set_bind_group(0, &self.frame_bind_group, &[]);
                pass.set_bind_group(1, &shading.group, &[]);
                if update.passes[p]
                    .reads
                    .iter()
                    .any(|r| crate::graph::is_volume(source, r))
                {
                    pass.set_bind_group(3, &volumes.group, &[]);
                }
                for (k, d) in dispatches[i..end].iter().enumerate() {
                    let groups = &volumes.updates[p];
                    pass.set_bind_group(
                        2,
                        &groups[(d.mip as usize).min(groups.len() - 1)],
                        &[((i + k) as u64 * stride) as u32],
                    );
                    if d.groups > 0 {
                        pass.dispatch_workgroups(d.groups, 1, 1);
                    }
                }
                drop(pass);
                i = end;
            }
        }
    }

    /// Run a verification kernel of the current investigation's scene shading
    /// (`SceneShading::verify`) with `count` workgroups and read `words` back;
    /// the shading's uniforms are those of the last uploaded frame.
    pub fn run_shading_verify(
        &mut self,
        entry: &str,
        count: u32,
        tests: &[[f32; 4]],
        words: usize,
    ) -> Vec<u32> {
        self.shading()
            .expect("the current investigation has no scene shading");
        let source = investigation::current().scene_shading().unwrap();
        let shading = self
            .shadings
            .iter()
            .find(|s| std::ptr::eq(s.source, source))
            .unwrap();
        let device = &self.gpu.device;
        let out = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verify out"),
            size: (words.max(1) * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let test_data: Vec<[f32; 4]> = if tests.is_empty() {
            vec![[0.0; 4]]
        } else {
            tests.to_vec()
        };
        let input = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("verify tests"),
            contents: bytemuck::cast_slice(&test_data),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verify"),
            layout: &self.pipelines.verify_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: out.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: input.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&shading.pipelines.verify[entry]);
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            pass.set_bind_group(1, &shading.group, &[]);
            pass.set_bind_group(2, &bind, &[]);
            if let Some(v) = shading.volume_sets.get(shading.volume_set) {
                pass.set_bind_group(3, &v.group, &[]);
            }
            pass.dispatch_workgroups(count, 1, 1);
        }
        self.gpu.queue.submit([encoder.finish()]);
        read_buffer(device, &self.gpu.queue, &out, 0, (words * 4) as u64)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect()
    }
}

/// A `scene_data` build's result, from its worker thread.
pub(super) type DataBuild = std::sync::mpsc::Receiver<Vec<[f32; 4]>>;
