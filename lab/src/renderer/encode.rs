//! Encoding a frame's passes: the scene in its graph's order, the composites
//! of the views, an experiment's reference, and the homepage's backdrop.

use super::*;

impl Renderer {
    /// The frame cleared to the background colour.
    pub(super) fn background_clear(&self) -> wgpu::LoadOp<wgpu::Color> {
        // The frame is UNORM and holds sRGB-encoded bytes, so the clear
        // colour is the encoded background colour itself.
        wgpu::LoadOp::Clear(crate::palette::background_clear())
    }

    /// The viewport targets tone mapped into the frame at `at` (x, y, width,
    /// height in pixels), the frame cleared first when `clear`.
    pub(super) fn composite(&self, encoder: &mut wgpu::CommandEncoder, at: [u32; 4], clear: bool) {
        self.composite_timed(encoder, at, clear, None);
    }

    /// `composite`, its pass timed (src/timing.rs).
    pub(super) fn composite_timed(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        at: [u32; 4],
        clear: bool,
        timestamps: Option<wgpu::RenderPassTimestampWrites>,
    ) {
        self.composite_pass(
            encoder,
            self.viewport.as_ref().unwrap(),
            at,
            clear,
            None,
            timestamps,
        );
    }

    /// `composite` of `targets`, drawn only inside `scissor` (pixels) when given.
    pub(super) fn composite_from(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        targets: &ViewportTargets,
        at: [u32; 4],
        clear: bool,
        scissor: Option<[u32; 4]>,
    ) {
        self.composite_pass(encoder, targets, at, clear, scissor, None);
    }

    pub(super) fn composite_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        targets: &ViewportTargets,
        at: [u32; 4],
        clear: bool,
        scissor: Option<[u32; 4]>,
        timestamps: Option<wgpu::RenderPassTimestampWrites>,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.frame_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if clear {
                        self.background_clear()
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timestamps,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(
            at[0] as f32,
            at[1] as f32,
            at[2] as f32,
            at[3] as f32,
            0.0,
            1.0,
        );
        if let Some(r) = scissor {
            pass.set_scissor_rect(r[0], r[1], r[2].max(1), r[3].max(1));
        }
        pass.set_pipeline(&self.pipelines.composite);
        pass.set_bind_group(0, &self.frame_bind_group, &[]);
        pass.set_bind_group(1, &targets.composite, &[]);
        pass.draw(0..3, 0..1);
    }

    /// A view's picture laid over the frame at `at`, blended at `alpha` (src/view.rs).
    pub(super) fn composite_over(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        targets: &ViewportTargets,
        at: [u32; 4],
        alpha: f32,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite over"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.frame_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(
            at[0] as f32,
            at[1] as f32,
            at[2] as f32,
            at[3] as f32,
            0.0,
            1.0,
        );
        pass.set_pipeline(&self.pipelines.composite_over);
        pass.set_blend_constant(wgpu::Color {
            r: alpha as f64,
            g: alpha as f64,
            b: alpha as f64,
            a: alpha as f64,
        });
        pass.set_bind_group(0, &self.frame_bind_group, &[]);
        pass.set_bind_group(1, &targets.composite, &[]);
        pass.draw(0..3, 0..1);
    }

    /// The experiment's difference view of the live and reference HDR targets into `at`.
    pub(super) fn composite_difference(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        at: [u32; 4],
        gain: f32,
    ) {
        let (Some(live), Some(reference)) = (
            self.viewport.as_ref(),
            self.slots
                .get(&ViewId::REFERENCE)
                .and_then(|s| s.viewport.as_ref()),
        ) else {
            return;
        };
        self.gpu.queue.write_buffer(
            &self.compare_params,
            0,
            bytemuck::cast_slice(&[gain, 0.0, 0.0, 0.0]),
        );
        let pair = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("compare pair"),
                layout: &self.pipelines.texture_pair_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&live.hdr),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&reference.hdr),
                    },
                ],
            });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite difference"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.frame_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(
            at[0] as f32,
            at[1] as f32,
            at[2] as f32,
            at[3] as f32,
            0.0,
            1.0,
        );
        pass.set_pipeline(&self.pipelines.composite_diff);
        pass.set_bind_group(0, &self.frame_bind_group, &[]);
        pass.set_bind_group(1, &pair, &[]);
        pass.set_bind_group(2, &self.compare_group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Records the program's commands of a run, the inspector drawing what
    /// each pass wrote (in `frame`'s targets) right after it.
    pub(super) fn record_program(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        run: crate::program::ProgramRun,
        graph: &crate::graph::FrameGraph,
        frame: &FrameTargets,
        timing: Option<&mut crate::timing::Timing>,
    ) {
        let Some(s) = frame.shading else { return };
        let (Some(program), Some(pipelines)) = (&s.program, s.pipelines.program.as_ref()) else {
            return;
        };
        let meshes = crate::program::Meshes {
            vertices: &self.mesh_vertices,
            objects: &self.object_ids,
            indices: &self.mesh_indices,
            ranges: &self.statics.meshes.ranges,
        };
        let mut after = |encoder: &mut wgpu::CommandEncoder, pass: &'static str| {
            if let Some(n) = graph
                .nodes
                .iter()
                .position(|n| n.name == pass && matches!(n.step, Step::Program(_)))
                && self.inspections.values().any(|x| x.node == n)
            {
                self.capture(encoder, n, frame);
            }
        };
        let bindings = crate::program::ProgramBindings {
            pipelines,
            frame: &self.frame_bind_group,
            shading: &s.group,
            meshes,
        };
        program.borrow_mut().record(
            &self.gpu.device,
            encoder,
            run,
            &bindings,
            timing,
            &mut after,
        );
    }

    /// Encode a frame's scene in its graph's order (src/graph.rs): the
    /// update passes (their plan's dispatches), then each live node: the
    /// depth views, the pre-pass, the chain's compute passes, the scene pass,
    /// and the overlay. The composite is the caller's.
    /// The scene and overlay passes into the viewport targets (the composite
    /// reads them).
    /// With `timed`, its passes write GPU timestamps (src/timing.rs).
    pub(super) fn encode_scene(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        content: &FrameContent,
        timed: bool,
    ) {
        self.shading();
        let graph = self.frame_graph();
        let mut timing = if timed {
            self.timing.take()
        } else {
            None
        };
        // The update passes run first, in their plan's order (they read nothing this frame writes).
        self.encode_updates(encoder, timing.as_mut());
        let shading = investigation::current().scene_shading().and_then(|source| {
            self.shadings
                .iter()
                .find(|s| std::ptr::eq(s.source, source))
        });
        let targets = self.viewport.as_ref().unwrap();
        // An investigation's pass chain: its targets follow the viewport.
        let chained = shading.filter(|s| s.source.chained());
        if let Some(s) = chained
            && self.chain.as_ref().is_none_or(|c| {
                c.size != targets.size || c.scale != s.scale || !std::ptr::eq(c.source, s.source)
            })
        {
            self.chain = Some(ChainTargets::new(&self.gpu.device, s, targets));
        }
        let chain = chained.and(self.chain.as_ref());
        let color_attachments: Vec<Option<wgpu::RenderPassColorAttachment>> =
            match chain.filter(|c| !c.source.outputs.is_empty()) {
                // The chain's targets start at zero: its passes decide what the background is.
                Some(c) => c
                    .source
                    .outputs
                    .iter()
                    .map(|name| {
                        Some(wgpu::RenderPassColorAttachment {
                            view: c.view(name),
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })
                    })
                    .collect(),
                None => vec![Some(wgpu::RenderPassColorAttachment {
                    view: &targets.hdr_msaa,
                    depth_slice: None,
                    resolve_target: Some(&targets.hdr),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
            };
        let prepassed = chain.is_some() && chained.is_some_and(|s| s.pipelines.prepass.is_some());
        // Every view runs its program, from its own slot (its images and history; `Renderer::enter`).
        let program_shading = shading;
        // Each program pass's run of nodes, and the runs recorded so far.
        let runs = shading.map_or(Vec::new(), |s| s.source.program_runs());
        let mut recorded: Vec<u32> = Vec::new();
        // Its groups, for this view's size and the frame's textures it reads.
        if let Some(s) = program_shading
            && let (Some(program), Some(pipelines)) = (&s.program, s.pipelines.program.as_ref())
        {
            let key = targets.made ^ chain.map_or(0, |c| c.made << 32);
            let external = |name: &str| -> Option<wgpu::TextureView> {
                crate::program::external(s.source, name)?;
                match name {
                    "depth" => Some(targets.depth.clone()),
                    "hdr" => Some(targets.hdr.clone()),
                    _ => chain.and_then(|c| {
                        c.views
                            .iter()
                            .find(|(n, _)| *n == name)
                            .map(|(_, v)| v.clone())
                    }),
                }
            };
            if program
                .borrow_mut()
                .bind(&self.gpu.device, pipelines, targets.size, key, &external)
            {
                *s.program_handles.borrow_mut() = program.borrow().handles();
            }
        }
        // The chain's compute passes waiting to be encoded: consecutive ones share a compute pass.
        let mut pending: Vec<usize> = Vec::new();
        for &n in &graph.order {
            let node = &graph.nodes[n];
            if !graph.live[n] {
                continue;
            }
            // The inspector draws what a node wrote right after it, in the main view only.
            let inspected = timed && self.inspections.values().any(|x| x.node == n);
            if let Step::Compute(i) = node.step {
                pending.push(i);
                if inspected {
                    if let (Some(s), Some(c)) = (chained, chain) {
                        c.dispatch(
                            encoder,
                            &self.frame_bind_group,
                            s,
                            &std::mem::take(&mut pending),
                            timing.as_mut(),
                        );
                    }
                    self.capture(
                        encoder,
                        n,
                        &FrameTargets {
                            chain,
                            shading,
                            targets,
                        },
                    );
                }
                continue;
            }
            if let (Some(s), Some(c)) = (chained, chain) {
                c.dispatch(
                    encoder,
                    &self.frame_bind_group,
                    s,
                    &std::mem::take(&mut pending),
                    timing.as_mut(),
                );
            }
            match node.step {
                Step::Update(_) | Step::Compute(_) | Step::Composite => {}
                // A run of program nodes: its commands, where its first node is.
                Step::Program(i) => {
                    let run = runs[i];
                    if !recorded.contains(&run) {
                        recorded.push(run);
                        self.record_program(
                            encoder,
                            crate::program::ProgramRun { runs: &runs, run },
                            &graph,
                            &FrameTargets {
                                chain,
                                shading: program_shading,
                                targets,
                            },
                            timing.as_mut(),
                        );
                    }
                }
                Step::DepthViews => {
                    let (Some(s), Some(c)) = (chained, chain) else {
                        continue;
                    };
                    // The depth views (shadow maps), one layer at a time, when what
                    // they see (the objects, the light, the shading values) changed.
                    let key = depth_views_key(content);
                    if let Some((_, layers)) = c
                        .depth_views
                        .as_ref()
                        .filter(|_| c.depth_views_drawn.replace(Some(key)) != Some(key))
                    {
                        let span = timing.as_mut().and_then(|t| t.render_writes("depth views"));
                        let last = layers.len().saturating_sub(1);
                        for (i, (pipeline, layer)) in
                            s.pipelines.depth_views.iter().zip(layers).enumerate()
                        {
                            let writes = span
                                .as_ref()
                                .filter(|_| i == 0 || i == last)
                                .map(|t| t.span(i == 0, i == last));
                            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("depth view"),
                                color_attachments: &[],
                                depth_stencil_attachment: Some(
                                    wgpu::RenderPassDepthStencilAttachment {
                                        view: layer,
                                        depth_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(0.0),
                                            store: wgpu::StoreOp::Store,
                                        }),
                                        stencil_ops: None,
                                    },
                                ),
                                timestamp_writes: writes,
                                occlusion_query_set: None,
                                multiview_mask: None,
                            });
                            pass.set_pipeline(pipeline);
                            pass.set_bind_group(0, &self.frame_bind_group, &[]);
                            pass.set_bind_group(1, &s.group, &[]);
                            pass.set_vertex_buffer(0, self.mesh_vertices.slice(..));
                            pass.set_vertex_buffer(1, self.object_ids.slice(..));
                            pass.set_index_buffer(
                                self.mesh_indices.slice(..),
                                wgpu::IndexFormat::Uint32,
                            );
                            for (i, range) in self.statics.meshes.ranges.iter().enumerate() {
                                pass.draw_indexed(range.clone(), 0, i as u32..i as u32 + 1);
                            }
                        }
                    }
                }
                Step::Prepass => {
                    let (Some(s), Some(c)) = (chained, chain) else {
                        continue;
                    };
                    if let Some(prepass) = &s.pipelines.prepass {
                        let span = timing
                            .as_mut()
                            .and_then(|t| t.render_writes("depth pre-pass"));
                        // What the pre-pass writes besides the depth (`prepass_outputs`).
                        let outputs: Vec<Option<wgpu::RenderPassColorAttachment>> = s
                            .source
                            .prepass_outputs
                            .iter()
                            .map(|name| {
                                Some(wgpu::RenderPassColorAttachment {
                                    view: c.view(name),
                                    depth_slice: None,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })
                            })
                            .collect();
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("depth pre-pass"),
                            color_attachments: &outputs,
                            depth_stencil_attachment: Some(
                                wgpu::RenderPassDepthStencilAttachment {
                                    view: &targets.depth,
                                    depth_ops: Some(wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(0.0),
                                        store: wgpu::StoreOp::Store,
                                    }),
                                    stencil_ops: None,
                                },
                            ),
                            timestamp_writes: span.as_ref().map(|t| t.writes()),
                            occlusion_query_set: None,
                            multiview_mask: None,
                        });
                        pass.set_pipeline(prepass);
                        pass.set_bind_group(0, &self.frame_bind_group, &[]);
                        pass.set_bind_group(1, &s.group, &[]);
                        pass.set_bind_group(2, &c.scene_group, &[]);
                        if let Some(group) = s
                            .program
                            .as_ref()
                            .and_then(|p| p.borrow().scene_group.clone())
                        {
                            pass.set_bind_group(3, &group, &[]);
                        }
                        pass.set_vertex_buffer(0, self.mesh_vertices.slice(..));
                        pass.set_vertex_buffer(1, self.object_ids.slice(..));
                        pass.set_index_buffer(
                            self.mesh_indices.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        for (i, range) in self.statics.meshes.ranges.iter().enumerate() {
                            pass.draw_indexed(range.clone(), 0, i as u32..i as u32 + 1);
                        }
                    }
                }
                Step::Scene => {
                    self.record_program(
                        encoder,
                        crate::program::ProgramRun {
                            runs: &runs,
                            run: crate::program::BEFORE_SCENE,
                        },
                        &graph,
                        &FrameTargets {
                            chain,
                            shading: program_shading,
                            targets,
                        },
                        timing.as_mut(),
                    );
                    let span = timing.as_mut().and_then(|t| t.render_writes("scene pass"));
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("scene"),
                        color_attachments: &color_attachments,
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &targets.depth,
                            depth_ops: Some(wgpu::Operations {
                                load: if prepassed {
                                    wgpu::LoadOp::Load
                                } else {
                                    wgpu::LoadOp::Clear(0.0)
                                },
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: span.as_ref().map(|t| t.writes()),
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_bind_group(0, &self.frame_bind_group, &[]);
                    // Without a chain (or one that ends before the scene pass), the sky is the background; a chain's passes draw their own.
                    if chain.is_none_or(|c| c.source.outputs.is_empty()) {
                        pass.set_pipeline(&self.pipelines.sky);
                        pass.draw(0..3, 0..1);
                    }
                    pass.set_pipeline(
                        shading.map_or(&self.pipelines.scene, |s| &s.pipelines.scene),
                    );
                    if let Some(s) = shading {
                        pass.set_bind_group(1, &s.group, &[]);
                        if let Some(c) = chain {
                            pass.set_bind_group(2, &c.scene_group, &[]);
                        } else if let Some(empty) = &s.empty_scene_group {
                            pass.set_bind_group(2, empty, &[]);
                        }
                        if let Some(group) = s
                            .program
                            .as_ref()
                            .and_then(|p| p.borrow().scene_group.clone())
                        {
                            pass.set_bind_group(3, &group, &[]);
                        }
                    }
                    pass.set_vertex_buffer(0, self.mesh_vertices.slice(..));
                    pass.set_vertex_buffer(1, self.object_ids.slice(..));
                    pass.set_index_buffer(self.mesh_indices.slice(..), wgpu::IndexFormat::Uint32);
                    for (i, range) in self.statics.meshes.ranges.iter().enumerate() {
                        pass.draw_indexed(range.clone(), 0, i as u32..i as u32 + 1);
                    }
                }
                Step::Overlay => {
                    let span = timing.as_mut().and_then(|t| t.render_writes("overlay"));

                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("overlay"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &targets.overlay_msaa,
                            depth_slice: None,
                            resolve_target: Some(&targets.overlay),
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Discard,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &targets.depth,
                            // Kept when an image kind reads the chain's depth after this pass (src/views.rs).
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: if self.views.values().any(|v| !v.kind.reads.is_empty()) {
                                    wgpu::StoreOp::Store
                                } else {
                                    wgpu::StoreOp::Discard
                                },
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: span.as_ref().map(|t| t.writes()),
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_bind_group(0, &self.frame_bind_group, &[]);
                    let draw = |pass: &mut wgpu::RenderPass,
                                pipeline: &wgpu::RenderPipeline,
                                buffer: &GrowBuffer,
                                vertices: u32| {
                        if buffer.count > 0 {
                            pass.set_pipeline(pipeline);
                            pass.set_vertex_buffer(0, buffer.buffer.slice(..));
                            pass.draw(0..vertices, 0..buffer.count);
                        }
                    };
                    if self.ghost_ids.count > 0 {
                        pass.set_pipeline(&self.pipelines.ghost);
                        pass.set_vertex_buffer(0, self.mesh_vertices.slice(..));
                        pass.set_vertex_buffer(1, self.object_ids.slice(..));
                        pass.set_index_buffer(
                            self.mesh_indices.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        for &object in &content.draw.ghosts {
                            let range = self.statics.meshes.ranges[object as usize].clone();
                            pass.draw_indexed(range, 0, object..object + 1);
                        }
                    }
                    // Solid boxes first: they write depth, so the lines after them are hidden behind them (and dashed, x-ray).
                    draw(&mut pass, &self.pipelines.cuboid, &self.cuboids, 36);
                    draw(&mut pass, &self.pipelines.boxes_xray, &self.boxes_xray, 108);
                    draw(
                        &mut pass,
                        &self.pipelines.segment_xray,
                        &self.segments_xray,
                        9,
                    );
                    draw(&mut pass, &self.pipelines.boxes, &self.boxes, 108);
                    draw(&mut pass, &self.pipelines.segment, &self.segments, 9);
                    draw(&mut pass, &self.pipelines.point, &self.points, 6);
                }
            }
            if inspected {
                self.capture(
                    encoder,
                    n,
                    &FrameTargets {
                        chain,
                        shading,
                        targets,
                    },
                );
            }
        }
        if let (Some(s), Some(c)) = (chained, chain) {
            c.dispatch(
                encoder,
                &self.frame_bind_group,
                s,
                &pending,
                timing.as_mut(),
            );
        }
        if let Some(program) = program_shading.and_then(|s| s.program.as_ref()) {
            program.borrow_mut().finish();
        }
        self.timing = timing.or(self.timing.take());
    }

    /// A research run's frame (src/research.rs): `state` filling the frame
    /// target with no chrome, building on the frames before it as the viewer's
    /// still frames do (`FrameRequest::history`), its scene data built first,
    /// its passes timed while timing is on (`set_timing`).
    pub fn render_experiment(&mut self, state: &ShotState) {
        let mut layout = Layout::new(self.width, self.height);
        layout.viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, layout.canvas);
        self.last_layout = layout;
        let vp = layout.viewport_px();
        let size = [vp[2], vp[3]];
        if self.viewport.as_ref().is_none_or(|v| v.size != size) {
            self.chain = None;
            self.viewport = Some(ViewportTargets::new(
                &self.gpu.device,
                &self.pipelines,
                size,
            ));
        }
        self.switch_scene(investigation::current().scene(state));
        self.select_data(state, true);
        self.plan_program(state, true, size);
        let mut content = crate::shader_data::build_frame(
            state,
            &self.statics,
            [vp[2] as f32, vp[3] as f32],
            layout.scale,
            0.0,
        );
        let count = |last: (Option<u64>, u32), key: u64| {
            if last.0 == Some(key) {
                (Some(key), last.1.saturating_add(1))
            } else {
                (Some(key), 0)
            }
        };
        self.still = count(self.still, still_key(&content, size, true));
        self.still_scene = count(self.still_scene, still_key(&content, size, false));
        content.uniforms.view_params[3] =
            (1 + self.still.1.min(255) + 256 * self.still_scene.1.min(32767)) as f32;
        self.upload(&content);
        self.upload_draws(&content);
        // An experiment that measures a resource of the frame's graph (`Experiment::inspect`).
        self.prepare_inspections(state, &[]);
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("experiment"),
            });
        self.encode_scene(&mut encoder, &content, true);
        let timed = self
            .timing
            .as_mut()
            .and_then(|t| t.render_writes("composite"));
        self.composite_timed(&mut encoder, vp, true, timed.as_ref().map(|t| t.writes()));
        if let Some(t) = self.timing.as_mut() {
            t.resolve(&mut encoder);
        }
        self.gpu.queue.submit([encoder.finish()]);
        if let Some(t) = self.timing.as_mut() {
            t.submitted(&self.gpu.device);
        }
    }

    /// The next frame starts a new history (a research run's next variant).
    pub fn reset_history(&mut self) {
        self.still = (None, 0);
        self.still_scene = (None, 0);
    }

    /// The scene of `state` filling the whole frame target, without any chrome
    /// (the homepage's backdrop); with no state, only the background colour.
    pub fn render_backdrop(&mut self, state: Option<&ShotState>, time: f32) {
        let mut layout = Layout::new(self.width, self.height);
        layout.viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, layout.canvas);
        self.last_layout = layout;
        let vp = layout.viewport_px();
        if self
            .viewport
            .as_ref()
            .is_none_or(|v| v.size != [vp[2], vp[3]])
        {
            self.chain = None;
            self.viewport = Some(ViewportTargets::new(
                &self.gpu.device,
                &self.pipelines,
                [vp[2], vp[3]],
            ));
        }
        if let Some(s) = state {
            self.switch_scene(investigation::current().scene(s));
        }
        let content = state.map(|s| {
            crate::shader_data::build_frame(
                s,
                &self.statics,
                [vp[2] as f32, vp[3] as f32],
                layout.scale,
                time,
            )
        });
        if let Some(content) = &content {
            self.upload(content);
            self.upload_draws(content);
        }
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("backdrop"),
            });
        if let Some(content) = &content {
            self.encode_scene(&mut encoder, content, false);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("backdrop composite"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.frame_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: self.background_clear(),
                        store: wgpu::StoreOp::Store,
                    },
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
}
