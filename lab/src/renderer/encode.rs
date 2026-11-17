//! Encoding a frame's passes: the scene in its graph's order, the composites
//! of the views, an experiment's reference, and the homepage's backdrop.

use super::*;
use crate::investigation::{RasterColor, RasterDepth, RasterPass};

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
    /// raster passes (the depth views, a pre-pass, the scene pass), the
    /// chain's compute passes, the program's passes, and the overlay. The composite is the caller's.
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
        // Every view runs its program, from its own slot (its images and history; `Renderer::enter`).
        self.bind_program(shading, chain, targets);
        // A program that traces the scene gets its TLAS rebuilt from this frame's records.
        if let Some(program) = shading.and_then(|s| s.program.as_ref())
            && let Some(tlas) = program.borrow_mut().tlas.as_mut()
        {
            let rays = self.scene_rays.get_or_insert_with(|| {
                crate::ray_tracing::SceneRays::new(&self.gpu.device, &self.statics.meshes)
            });
            rays.encode(
                encoder,
                (&self.mesh_vertices, &self.mesh_indices),
                &content.objects,
                tlas,
            );
        }
        let frame = SceneFrame {
            targets: FrameTargets {
                chain,
                shading,
                targets,
            },
            chained,
            depth_written: std::cell::Cell::new(false),
            // Each program pass's run of nodes.
            runs: shading.map_or(Vec::new(), |s| s.source.program_runs()),
            graph: &graph,
            content,
        };
        // The runs recorded so far.
        let mut recorded: Vec<u32> = Vec::new();
        // The chain's compute passes waiting to be encoded: consecutive ones share a compute pass.
        let mut pending: Vec<usize> = Vec::new();
        for &n in &graph.order {
            // Each live node in order: chain passes gather into one compute pass, the others encode their own.
            let node = &graph.nodes[n];
            if !graph.live[n] {
                continue;
            }
            // The inspector draws what a node wrote right after it, in the main view only.
            let inspected = timed && self.inspections.values().any(|x| x.node == n);
            if let Step::Compute(i) = node.step {
                // A chain pass waits to share a compute pass, unless it is inspected: then the waiting ones go now.
                pending.push(i);
                if inspected {
                    frame.dispatch(self, encoder, &mut pending, timing.as_mut());
                    self.capture(encoder, n, &frame.targets);
                }
                continue;
            }
            frame.dispatch(self, encoder, &mut pending, timing.as_mut());
            match node.step {
                Step::Update(_) | Step::Compute(_) | Step::Composite => {}
                // A run of program nodes: its commands, where its first node is.
                Step::Program(i) => {
                    let run = frame.runs[i];
                    if !recorded.contains(&run) {
                        recorded.push(run);
                        self.record_program(
                            encoder,
                            crate::program::ProgramRun {
                                runs: &frame.runs,
                                run,
                            },
                            &graph,
                            &frame.targets,
                            timing.as_mut(),
                        );
                    }
                }
                Step::Raster(i) => self.encode_raster(encoder, &frame, i, timing.as_mut()),
                Step::Scene => self.encode_lab_scene(encoder, &frame, timing.as_mut()),
                Step::Overlay => self.encode_overlay(encoder, &frame, timing.as_mut()),
            }
            if inspected {
                self.capture(encoder, n, &frame.targets);
            }
        }
        frame.dispatch(self, encoder, &mut pending, timing.as_mut());
        if let Some(program) = shading.and_then(|s| s.program.as_ref()) {
            program.borrow_mut().finish();
        }
        self.timing = timing.or(self.timing.take());
    }

    /// The program's groups, for this view's size and the frame's textures it reads.
    fn bind_program(
        &self,
        shading: Option<&Shading>,
        chain: Option<&ChainTargets>,
        targets: &ViewportTargets,
    ) {
        if let Some(s) = shading
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
    }

    /// Raster pass `i` (`SceneShading::rasters`): the depth views' layers, or one render pass of the scene's meshes
    /// into its colour (its targets, or the HDR target with the sky behind) and the frame's depth, which it clears
    /// and writes, or tests against what a raster pass before it wrote. Before the scene pass, the program's
    /// commands after its plan's passes; besides the scene pass, it draws only in a chain.
    fn encode_raster(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &SceneFrame,
        i: usize,
        mut timing: Option<&mut crate::timing::Timing>,
    ) {
        let FrameTargets {
            chain,
            shading,
            targets,
        } = frame.targets;
        let Some(s) = shading else { return };
        let r = &s.source.rasters[i];
        if r.depth == RasterDepth::Layers {
            return self.encode_depth_views(encoder, frame, i, timing);
        }
        let scene = i == s.source.scene;
        if scene {
            self.record_program(
                encoder,
                crate::program::ProgramRun {
                    runs: &frame.runs,
                    run: crate::program::BEFORE_SCENE,
                },
                frame.graph,
                &frame.targets,
                timing.as_deref_mut(),
            );
        } else if chain.is_none() {
            return;
        }
        let color_attachments = raster_attachments(r, chain, targets);
        // A pass that tests against the depth loads what a pass before it wrote; one that writes it clears it.
        let load = if r.depth == RasterDepth::Test && frame.depth_written.get() {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(0.0)
        };
        if r.depth == RasterDepth::Write {
            frame.depth_written.set(true);
        }
        let span = timing.as_mut().and_then(|t| t.render_writes(r.timing));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(r.timing),
            color_attachments: &color_attachments,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
                depth_ops: Some(wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: span.as_ref().map(|t| t.writes()),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.frame_bind_group, &[]);
        // Into the HDR target, the sky is the background; a chain's passes draw their own.
        if r.color == RasterColor::Hdr {
            pass.set_pipeline(&self.pipelines.sky);
            pass.draw(0..3, 0..1);
        }
        pass.set_pipeline(&s.pipelines.rasters[i].pipelines[0]);
        pass.set_bind_group(1, &s.group, &[]);
        if r.fragment.is_some() {
            // Its fragment's groups: its reads in the chain, or (the scene pass without one) an empty stand-in (2),
            // and the program's scene uses (3).
            let reads = chain.and_then(|c| c.raster_groups[i].as_ref());
            if let Some(group) = reads.or(s.empty_scene_group.as_ref().filter(|_| scene)) {
                pass.set_bind_group(2, group, &[]);
            }
            if let Some(group) = s
                .program
                .as_ref()
                .and_then(|p| p.borrow().scene_group.clone())
            {
                pass.set_bind_group(3, &group, &[]);
            }
        }
        self.draw_meshes(&mut pass);
    }

    /// The scene's meshes, each object once (its instance index is its id).
    fn draw_meshes(&self, pass: &mut wgpu::RenderPass) {
        pass.set_vertex_buffer(0, self.mesh_vertices.slice(..));
        pass.set_vertex_buffer(1, self.object_ids.slice(..));
        pass.set_index_buffer(self.mesh_indices.slice(..), wgpu::IndexFormat::Uint32);
        for (i, range) in self.statics.meshes.ranges.iter().enumerate() {
            pass.draw_indexed(range.clone(), 0, i as u32..i as u32 + 1);
        }
    }

    /// The depth views (shadow maps, raster pass `i`), one layer at a time, in a chain, when what they see (the
    /// objects, the light, the shading values) changed.
    fn encode_depth_views(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &SceneFrame,
        i: usize,
        mut timing: Option<&mut crate::timing::Timing>,
    ) {
        let (Some(s), Some(c)) = (frame.chained, frame.targets.chain) else {
            return;
        };
        let key = depth_views_key(frame.content);
        if let Some((_, layers)) = c
            .depth_views
            .as_ref()
            .filter(|_| c.depth_views_drawn.replace(Some(key)) != Some(key))
        {
            let r = &s.source.rasters[i];
            let span = timing.as_mut().and_then(|t| t.render_writes(r.timing));
            let last = layers.len().saturating_sub(1);
            for (k, (pipeline, layer)) in s.pipelines.rasters[i]
                .pipelines
                .iter()
                .zip(layers)
                .enumerate()
            {
                // Each layer clears and draws the scene; the timer brackets the first and the last.
                let writes = span
                    .as_ref()
                    .filter(|_| k == 0 || k == last)
                    .map(|t| t.span(k == 0, k == last));
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("depth view"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: layer,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: writes,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &self.frame_bind_group, &[]);
                pass.set_bind_group(1, &s.group, &[]);
                self.draw_meshes(&mut pass);
            }
        }
    }

    /// The scene pass of a frame without a shading: the sky, then the meshes with the lab's own pipeline, into the
    /// HDR target.
    fn encode_lab_scene(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &SceneFrame,
        mut timing: Option<&mut crate::timing::Timing>,
    ) {
        let targets = frame.targets.targets;
        let color_attachments = [Some(hdr_attachment(targets))];
        let span = timing.as_mut().and_then(|t| t.render_writes("scene pass"));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: span.as_ref().map(|t| t.writes()),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.frame_bind_group, &[]);
        pass.set_pipeline(&self.pipelines.sky);
        pass.draw(0..3, 0..1);
        pass.set_pipeline(&self.pipelines.scene);
        self.draw_meshes(&mut pass);
    }

    /// The overlay pass: ghosts, boxes, segments, and points over the scene.
    fn encode_overlay(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &SceneFrame,
        mut timing: Option<&mut crate::timing::Timing>,
    ) {
        let (targets, content) = (frame.targets.targets, frame.content);
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
            pass.set_index_buffer(self.mesh_indices.slice(..), wgpu::IndexFormat::Uint32);
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

/// What a frame's scene passes are encoded with: the shading, its chain, and the viewport's targets; the shading
/// when it has a chain; whether a pre-pass wrote the depth; the program's runs; the frame's graph; its content.
struct SceneFrame<'a> {
    targets: FrameTargets<'a>,
    chained: Option<&'a Shading>,
    /// Whether a raster pass wrote the frame's depth yet (a pass that tests against it loads it then).
    depth_written: std::cell::Cell<bool>,
    runs: Vec<u32>,
    graph: &'a crate::graph::FrameGraph,
    content: &'a FrameContent,
}

impl SceneFrame<'_> {
    /// The chain's compute passes `pending`, encoded (as one compute pass) and cleared.
    fn dispatch(
        &self,
        renderer: &Renderer,
        encoder: &mut wgpu::CommandEncoder,
        pending: &mut Vec<usize>,
        timing: Option<&mut crate::timing::Timing>,
    ) {
        if let (Some(s), Some(c)) = (self.chained, self.targets.chain) {
            c.dispatch(
                encoder,
                &renderer.frame_bind_group,
                s,
                &std::mem::take(pending),
                timing,
            );
        }
    }
}

/// A raster pass's colour attachments: its targets (from transparent: the passes after it decide what the background
/// is), else the viewport's HDR target.
fn raster_attachments<'a>(
    r: &RasterPass,
    chain: Option<&'a ChainTargets>,
    targets: &'a ViewportTargets,
) -> Vec<Option<wgpu::RenderPassColorAttachment<'a>>> {
    match (r.color, chain) {
        (RasterColor::Hdr, _) => vec![Some(hdr_attachment(targets))],
        (RasterColor::Targets(names), Some(c)) => names
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
        (RasterColor::Targets(_), None) => Vec::new(),
    }
}

/// The viewport's HDR target, multisampled and resolved, from black.
fn hdr_attachment(targets: &ViewportTargets) -> wgpu::RenderPassColorAttachment<'_> {
    wgpu::RenderPassColorAttachment {
        view: &targets.hdr_msaa,
        depth_slice: None,
        resolve_target: Some(&targets.hdr),
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            store: wgpu::StoreOp::Discard,
        },
    }
}
