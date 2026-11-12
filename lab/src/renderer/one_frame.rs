//! Rendering one frame: its layout, the views it draws besides the main one (an experiment's reference, a split
//! area's panes, the free camera's, the views over the main one), the main view's content, the egui pass, and the
//! frame's encoding and submission.

use super::*;
use crate::investigation::Pane;

/// A frame's layout and the views it shows besides the main one, as `plan_frame` sets them up.
struct FramePlan {
    layout: Layout,
    /// The split scene area's other views, and the main view's own (the last of `Investigation::panes`).
    panes: Vec<Pane>,
    main: Option<Pane>,
    /// Each pane's rectangle, the main view's last.
    rects: Vec<egui::Rect>,
    /// An experiment's reference, and its rectangle when it is beside the live view.
    compare: Option<CompareRequest>,
    reference_rect: egui::Rect,
    /// The main view's content, when the panel's layout needed it built.
    built: Option<FrameContent>,
}

/// The views a frame drew besides the main one.
struct Drawn {
    /// The split area's panes, each in its rectangle.
    panes: Vec<(egui::Rect, FrameContent)>,
    /// The free camera's view.
    free: Option<FrameContent>,
    /// The views over the main one: their slots, alphas, and contents.
    overs: Vec<(ViewId, f32, FrameContent)>,
    /// The main camera's own view in the viewport's corner, while the free camera is on.
    main_view: Option<egui::Rect>,
}

/// A recording (no live frames before it, or a single snapshot) times a few renders of the same shot first, so its
/// panel shows measurements.
const PERFORMANCE_WARMUP: usize = 12;

impl FrameResult {
    /// A frame's result: what is on screen is the free camera's view while it is on (`free`), else the main view's.
    fn of(
        report: UiReport,
        content: Option<FrameContent>,
        free: Option<FrameContent>,
    ) -> FrameResult {
        match free {
            Some(free) => FrameResult {
                report,
                content: Some(free),
                main_content: content,
            },
            None => FrameResult {
                report,
                content,
                main_content: None,
            },
        }
    }
}

/// The groups of a 2D beat's frame graph scene that are lit, and so show the frame's pictures.
fn lit_boxes(request: &FrameRequest) -> Vec<(String, String)> {
    request
        .diagram
        .filter(|_| request.scene_type != SceneType::ThreeD)
        .map(|d| d.lit_groups())
        .unwrap_or_default()
}

/// The main camera's own view, in the viewport's lower right corner, at its shape.
fn main_corner(layout: &Layout) -> egui::Rect {
    let v = layout.viewport;
    let size = v.size() * 0.26;
    egui::Rect::from_min_size(v.max - size - egui::vec2(16.0, 16.0), size)
}

impl Renderer {
    /// Like `render_frame`, but with prepared frame content (used by the
    /// selftest to draw known markers).
    pub fn render_frame_with(
        &mut self,
        request: FrameRequest,
        readback: Option<&wgpu::Buffer>,
        prepared: Option<FrameContent>,
    ) -> FrameResult {
        self.begin_frame(&request);
        let mut plan = self.plan_frame(&request, prepared.is_some());
        let layout = plan.layout;
        self.last_layout = layout;
        let viewport_size = layout.render_size_px();
        let frame = ViewFrame {
            scale: layout.scale,
            time: request.time,
            history: request.history,
            wait: request.wait,
        };
        self.targets_at(viewport_size);
        let size = [viewport_size[0] as f32, viewport_size[1] as f32];
        let mut drawn = Drawn {
            panes: self.draw_beside(&plan, viewport_size, frame),
            free: None,
            overs: Vec::new(),
            main_view: None,
        };
        let scene_beat = request.scene_type == SceneType::ThreeD;
        let scene_state = plan.main.as_ref().map_or(request.state, |m| &m.state);
        // The frame graph scene (a 2D beat's `graph`) renders the frame too, unseen, for its lit boxes' pictures.
        let boxes = lit_boxes(&request);
        let renders = scene_beat || !boxes.is_empty();
        // A recording waits for the data, so every frame is the same each time.
        if renders {
            self.select_data(scene_state, request.wait || !request.history);
            self.plan_program(scene_state, request.history, viewport_size);
        }
        let settle_prepared = if request.history {
            None
        } else {
            prepared.clone()
        };
        let given = prepared.is_some();
        let panel_built = plan.built.is_some();
        let mut content = match prepared.or(plan.built.take()) {
            Some(c) => Some(c),
            None => renders.then(|| {
                crate::shader_data::build_frame(
                    scene_state,
                    &self.statics,
                    size,
                    layout.scale,
                    request.time,
                )
            }),
        };
        if let Some(content) = content.as_mut() {
            // The frames the view has held still: the same records, uniforms
            // (but the clock), shading values, and viewport as the last frame.
            self.hold_still(content, viewport_size, request.history);
        }
        drawn.free = self.draw_free(&request, content.as_ref(), &plan, &drawn, frame);
        drawn.overs = self.draw_overs(&request, given, &plan, &drawn, frame);
        // The shading's data is the main view's again (an over view selected its own).
        if renders && !drawn.overs.is_empty() {
            self.select_data(scene_state, request.wait || !request.history);
        }
        // The main camera's own view, in the viewport's lower right corner, at its shape.
        drawn.main_view = drawn.free.as_ref().map(|_| main_corner(&layout));
        self.last_main_view = drawn.main_view;
        self.place_views(&plan, &drawn);
        if let Some(content) = &content {
            self.upload(content);
            self.upload_draws(content);
            self.prepare_views(request.state);
            self.prepare_inspections(request.state, &boxes);
        }
        if let (false, Some(c)) = (request.history, content.as_ref()) {
            // A frame that stands alone: its program warmed up, and the frame built again from its own reads.
            self.warm_up_program(scene_state, c, viewport_size);
            let time = request.time;
            if let Some(rebuilt) =
                self.fetch_alone(c, scene_state, viewport_size, layout.scale, time)
            {
                // Drawn again from the start only when that changes whether its panel shows (and so the
                // view's size).
                let empty = |c: &FrameContent| c.panel_record.is_none() && c.readout.is_none();
                if panel_built && empty(&rebuilt) != empty(c) && self.settling < 2 {
                    return self.render_again(request, readback, settle_prepared);
                }
                self.upload(&rebuilt);
                self.upload_draws(&rebuilt);
                content = Some(rebuilt);
            }
        }
        if let (true, false, Some(c)) = (
            request.state.panel.shows_passes(),
            request.history,
            &content,
        ) {
            self.warm_up_performance(c);
        }
        let (output, report) = self.ui_pass(request, &plan, &drawn, content.as_ref(), &boxes);
        self.encode_frame(
            output,
            content.as_ref(),
            &plan,
            &drawn,
            scene_beat,
            readback,
        );
        FrameResult::of(report, content, drawn.free)
    }

    /// The frame drawn again from the start (its panel's showing changed once its reads were in hand).
    fn render_again(
        &mut self,
        request: FrameRequest,
        readback: Option<&wgpu::Buffer>,
        prepared: Option<FrameContent>,
    ) -> FrameResult {
        self.settling += 1;
        let frame = self.render_frame_with(request, readback, prepared);
        self.settling -= 1;
        frame
    }

    /// A frame's start: the view and scene it is of, the reads that have landed, the clock of its frames.
    fn begin_frame(&mut self, request: &FrameRequest) {
        self.enter(ViewId::MAIN);
        self.view_scene(investigation::current().scene(request.state));
        // The reads earlier frames asked for that have landed (src/fetch.rs);
        // a recording's frame with history waits for the last frame's.
        self.fetches
            .collect(&self.gpu.device, request.wait && request.history);
        // The performance panel measures the frames it is shown with.
        if request.state.panel.shows_passes() && request.scene_type == SceneType::ThreeD {
            self.set_timing(true);
        }
        self.frame_starts.push_back(std::time::Instant::now());
        if self.frame_starts.len() > 121 {
            self.frame_starts.pop_front();
        }
    }

    /// A recording's code panel is as wide as its listing needs: its width, measured once per listing, language,
    /// and style; None when the frame shows no listing.
    fn code_width(&mut self, request: &FrameRequest) -> Option<f32> {
        let listing = match &request.chrome {
            Chrome::Recording { .. } if request.state.panel == crate::shot::PanelMode::Code => {
                request
                    .state
                    .code
                    .as_deref()
                    .and_then(|c| investigation::current().listing(c))
            }
            _ => None,
        }?;
        let key = (
            listing.name,
            crate::i18n::current(),
            crate::pseudocode::style(),
        );
        if let Some(width) = self.code_fit.get(&key) {
            return Some(*width);
        }
        // The fonts are there from the first pass on.
        if self.egui_ctx.cumulative_pass_nr() == 0 {
            let mut warm = self.egui_ctx.run_pass(egui::RawInput::default(), |_| {});
            // Its texture changes (the font atlas) are uploaded like any pass's.
            for (id, deltas) in warm.textures_delta.set.drain() {
                for delta in &deltas {
                    self.egui_renderer
                        .update_texture(&self.gpu.device, &self.gpu.queue, id, delta);
                }
            }
            for id in warm.textures_delta.free.drain() {
                self.egui_renderer.free_texture(&id);
            }
        }
        let width = ui::code_panel_fit(&self.egui_ctx, listing);
        self.code_fit.insert(key, width);
        Some(width)
    }

    /// The frame's layout, its split area's panes, and the experiment's reference beside it; `given` when the
    /// frame's content is prepared (the selftest's).
    fn plan_frame(&mut self, request: &FrameRequest, given: bool) -> FramePlan {
        let dock = matches!(&request.chrome, Chrome::Viewer(v) if !(v.timeline_mode && v.playing));
        let values =
            dock && matches!(&request.chrome, Chrome::Viewer(v) if v.feature_panel().is_some());
        let code_width = self.code_width(request);
        let viewer = matches!(&request.chrome, Chrome::Viewer(_));
        let layout = self.layout_for(
            request.state,
            request.scene_type,
            viewer,
            dock,
            values,
            code_width,
        );
        let scene_beat = request.scene_type == SceneType::ThreeD;
        // A split scene area: the views beside the main one, each in its own
        // rectangle; the main view takes the last (Investigation::panes).
        let mut panes = if scene_beat && !given {
            investigation::current().panes(request.state)
        } else {
            Vec::new()
        };
        let main = panes.pop();
        // A scene with a size of its own (src/shot/mod.rs `FrameSize`), fitted into the viewport: not split, nor compared.
        let fixed = request
            .state
            .frame
            .size()
            .filter(|_| scene_beat && !given && main.is_none() && self.compare.is_none());
        let fit = |layout: Layout| match fixed {
            Some(size) => layout.with_frame(size),
            None => layout,
        };
        let (layout, built) = if layout.panel_shown
            && scene_beat
            && !given
            && main.is_none()
            && !values
            && self.compare.is_none()
        {
            self.fit_panel(request, layout, &fit)
        } else {
            (layout, None)
        };
        let mut layout = fit(layout);
        let rects = if main.is_some() {
            layout.split(panes.len() + 1)
        } else {
            Vec::new()
        };
        if let Some(rect) = rects.last() {
            layout.viewport = *rect;
        }
        // An experiment's reference (src/experiment.rs): in a view of its own,
        // beside the live one, or behind it (a wipe, a difference, alone).
        let compare = self
            .compare
            .clone()
            .filter(|_| scene_beat && !given && main.is_none());
        let mut reference_rect = layout.viewport;
        if compare
            .as_ref()
            .is_some_and(|c| c.mode == crate::experiment::Compare::SideBySide)
        {
            let halves = layout.split(2);
            reference_rect = halves[0];
            layout.viewport = halves[1];
        }
        FramePlan {
            layout,
            panes,
            main,
            rects,
            compare,
            reference_rect,
            built,
        }
    }

    /// A panel with nothing to show gives its room to the viewport: a listing
    /// the investigation does not have, or a struct panel whose frame has
    /// neither a record nor a readout. That is known once the frame is built,
    /// at the viewport's size: it is built at the last frame's layout, and
    /// again at the other if that was wrong. The layout, and the content when
    /// it was built (`fit` fits a layout to the scene's own size).
    fn fit_panel(
        &mut self,
        request: &FrameRequest,
        layout: Layout,
        fit: &dyn Fn(Layout) -> Layout,
    ) -> (Layout, Option<FrameContent>) {
        use crate::shot::PanelMode;
        match request.state.panel {
            PanelMode::Code
                if request
                    .state
                    .code
                    .as_deref()
                    .and_then(|c| investigation::current().listing(c))
                    .is_none() =>
            {
                (layout.for_panel(false), None)
            }
            PanelMode::Declaration | PanelMode::Values | PanelMode::Memory => {
                self.select_data(request.state, request.wait || !request.history);
                let build = |layout: &Layout| {
                    let size = fit(*layout).render_size_px();
                    crate::shader_data::build_frame(
                        request.state,
                        &self.statics,
                        [size[0] as f32, size[1] as f32],
                        layout.scale,
                        request.time,
                    )
                };
                let shown = layout;
                let hidden = layout.for_panel(false);
                let mut content = build(if self.panel_empty {
                    &hidden
                } else {
                    &shown
                });
                let empty = content.panel_record.is_none() && content.readout.is_none();
                if empty != self.panel_empty {
                    content = build(if empty {
                        &hidden
                    } else {
                        &shown
                    });
                }
                self.panel_empty = empty;
                let layout = if empty {
                    hidden
                } else {
                    shown
                };
                (layout, Some(content))
            }
            _ => (layout, None),
        }
    }

    /// The experiment's reference, then each other view of a split area, from their own slots: the panes drawn, in
    /// their rectangles.
    fn draw_beside(
        &mut self,
        plan: &FramePlan,
        viewport_size: [u32; 2],
        frame: ViewFrame,
    ) -> Vec<(egui::Rect, FrameContent)> {
        self.reference_drawn = false;
        if let Some(c) = &plan.compare {
            // The reference first, from its own slot (src/view.rs), so its history builds up as the live view's does.
            self.draw_view(
                ViewId::REFERENCE,
                &c.reference,
                None,
                viewport_size,
                frame,
                None,
            );
            self.reference_drawn = true;
        }
        // Each other view of a split area: from its own slot, composited into its rectangle.
        let mut drawn = Vec::new();
        for (n, (pane, rect)) in plan.panes.iter().zip(&plan.rects).enumerate() {
            let at = Layout {
                viewport: *rect,
                ..plan.layout
            }
            .viewport_px();
            let content = self.draw_view(
                ViewId::pane(n as u32),
                &pane.state,
                None,
                [at[2], at[3]],
                frame,
                Some((at, drawn.is_empty())),
            );
            drawn.push((*rect, content));
        }
        for n in plan.panes.len() as u32..8 {
            self.slots.remove(&ViewId::pane(n));
        }
        drawn
    }

    /// The free camera's view (src/free_view.rs): the main frame drawn from
    /// the free camera, in its own submission and targets (so its history
    /// builds up as the main view's does), before the main frame's uniforms
    /// are written. The main frame is still drawn after it: the pictures of
    /// its buffers and what follows its camera are read from it, and it is
    /// shown in a corner.
    fn draw_free(
        &mut self,
        request: &FrameRequest,
        content: Option<&FrameContent>,
        plan: &FramePlan,
        drawn: &Drawn,
        frame: ViewFrame,
    ) -> Option<FrameContent> {
        let scene_beat = request.scene_type == SceneType::ThreeD;
        let free = request.state.free_camera;
        let free_content = content
            .filter(|_| scene_beat && free.on && drawn.panes.is_empty() && plan.compare.is_none())
            .map(|main| {
                let content = crate::free_view::content(main, request.state, &free);
                // The program plans for the camera this view is drawn from.
                let seen = ShotState {
                    camera: free.orbit(),
                    ..request.state.clone()
                };
                self.draw_view(
                    ViewId::FREE,
                    &seen,
                    Some(content),
                    plan.layout.render_size_px(),
                    frame,
                    None,
                )
            });
        if free_content.is_none() {
            self.slots.remove(&ViewId::FREE);
        }
        free_content
    }

    /// The views the investigation stacks over the main one (src/view.rs):
    /// each from its own slot, in its own scene, laid over the main view below.
    fn draw_overs(
        &mut self,
        request: &FrameRequest,
        given: bool,
        plan: &FramePlan,
        drawn: &Drawn,
        frame: ViewFrame,
    ) -> Vec<(ViewId, f32, FrameContent)> {
        let scene_beat = request.scene_type == SceneType::ThreeD;
        let overs = if scene_beat && !given && plan.main.is_none() && drawn.free.is_none() {
            investigation::current().views(request.state)
        } else {
            Vec::new()
        };
        let mut over_drawn: Vec<(ViewId, f32, FrameContent)> = Vec::new();
        for (n, over) in overs.iter().enumerate() {
            if over.alpha <= 0.0 {
                continue;
            }
            // Each view over the main one that is showing draws from its own slot; slots of views gone are freed below.
            let id = ViewId::over(n as u32);
            crate::view::set_name(Some(over.name));
            let size = plan.layout.render_size_px();
            let content = self.draw_view(id, &over.state, None, size, frame, None);
            crate::view::set_name(None);
            over_drawn.push((id, over.alpha.min(1.0), content));
        }
        for n in 0..8u32 {
            if !over_drawn.iter().any(|(id, _, _)| *id == ViewId::over(n)) {
                self.slots.remove(&ViewId::over(n));
            }
        }
        self.last_overs = overs
            .iter()
            .enumerate()
            .filter(|(_, o)| o.alpha > 0.0)
            .map(|(n, o)| (ViewId::over(n as u32), o.name, o.camera))
            .collect();
        over_drawn
    }

    /// Where each view is, back to front, for the pointer (src/view.rs).
    fn place_views(&mut self, plan: &FramePlan, drawn: &Drawn) {
        let layout = plan.layout;
        self.last_views = match (&drawn.free, drawn.main_view) {
            (Some(_), Some(corner)) => {
                vec![(ViewId::FREE, layout.viewport), (ViewId::MAIN, corner)]
            }
            _ => {
                // Without the free camera: the split panes, the reference beside the main view, then the main view.
                let mut views: Vec<(ViewId, egui::Rect)> = drawn
                    .panes
                    .iter()
                    .enumerate()
                    .map(|(n, (rect, _))| (ViewId::pane(n as u32), *rect))
                    .collect();
                if plan
                    .compare
                    .as_ref()
                    .is_some_and(|c| c.mode == crate::experiment::Compare::SideBySide)
                {
                    views.push((ViewId::REFERENCE, plan.reference_rect));
                }
                views.push((ViewId::MAIN, layout.viewport));
                views
            }
        };
        // The views over the main one take the pointer where they are mostly shown.
        self.last_views.extend(
            drawn
                .overs
                .iter()
                .filter(|(_, a, _)| *a > 0.5)
                .map(|(id, _, _)| (*id, layout.viewport)),
        );
    }

    /// A frame that stands alone, of a program that reads what earlier
    /// frames left: the same shot drawn first, each from the one before,
    /// the first from nothing (`ShadingProgram::warmup`), untimed.
    fn warm_up_program(
        &mut self,
        scene_state: &ShotState,
        content: &FrameContent,
        viewport_size: [u32; 2],
    ) {
        let warmup = investigation::current()
            .scene_shading()
            .and_then(|s| s.program)
            .map_or(0, |p| p.warmup);
        if warmup == 0 {
            return;
        }
        let timing = self.timing.take();
        for k in 0..warmup {
            self.plan_program(scene_state, k > 0, viewport_size);
            let mut encoder =
                self.gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("program warm-up"),
                    });
            self.encode_scene(&mut encoder, content, true);
            self.gpu.queue.submit([encoder.finish()]);
        }
        self.timing = timing;
        self.plan_program(scene_state, true, viewport_size);
    }

    /// A frame that stands alone shows its own frame's reads (src/fetch.rs):
    /// when one is not of this frame's inputs, the frame is drawn (the
    /// warm-up has drawn it already), read, and built again (at `scale`, at `time`): the frame built again, or
    /// None when it had no reads to wait for or they were of its inputs already.
    fn fetch_alone(
        &mut self,
        c: &FrameContent,
        scene_state: &ShotState,
        viewport_size: [u32; 2],
        scale: f32,
        time: f32,
    ) -> Option<FrameContent> {
        let tag = still_key(c, viewport_size, true);
        if c.fetches.is_empty() || crate::fetch::Queue::answered(&c.fetches, tag) {
            return None;
        }
        let warmup = investigation::current()
            .scene_shading()
            .and_then(|s| s.program)
            .map_or(0, |p| p.warmup);
        let timing = self.timing.take();
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("fetch"),
            });
        if warmup == 0 {
            self.encode_scene(&mut encoder, c, true);
        }
        let requests = c.fetches.clone();
        let mut queue = std::mem::take(&mut self.fetches);
        queue.collect(&self.gpu.device, true);
        queue.issue(
            &self.gpu.device,
            &mut encoder,
            &requests,
            tag,
            u64::MAX,
            &|source| self.fetch_target(source),
        );
        self.gpu.queue.submit([encoder.finish()]);
        queue.submitted();
        queue.collect(&self.gpu.device, true);
        self.fetches = queue;
        self.timing = timing;
        // Built again with the reads in hand.
        let view_params = c.uniforms.view_params;
        let size = [viewport_size[0] as f32, viewport_size[1] as f32];
        let mut rebuilt =
            crate::shader_data::build_frame(scene_state, &self.statics, size, scale, time);
        rebuilt.uniforms.view_params = view_params;
        Some(rebuilt)
    }

    /// A recording's performance panel: a few renders of the same shot timed first (`PERFORMANCE_WARMUP`).
    fn warm_up_performance(&mut self, content: &FrameContent) {
        let measured = self
            .timing
            .as_ref()
            .map_or(usize::MAX, |t| t.totals().len());
        for _ in measured..PERFORMANCE_WARMUP {
            self.frame_starts.push_back(std::time::Instant::now());
            let mut encoder =
                self.gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("performance warm-up"),
                    });
            self.encode_scene(&mut encoder, content, true);
            if let Some(t) = self.timing.as_mut() {
                t.resolve(&mut encoder);
            }
            self.gpu.queue.submit([encoder.finish()]);
            if let Some(t) = self.timing.as_mut() {
                t.submitted(&self.gpu.device);
                t.finish(&self.gpu.device);
            }
        }
        self.frame_starts.push_back(std::time::Instant::now());
        while self.frame_starts.len() > 121 {
            self.frame_starts.pop_front();
        }
    }

    /// The frame's egui pass: its windows, panels, and overlays over the views drawn.
    fn ui_pass(
        &mut self,
        request: FrameRequest,
        plan: &FramePlan,
        drawn: &Drawn,
        content: Option<&FrameContent>,
        boxes: &[(String, String)],
    ) -> (egui::FullOutput, UiReport) {
        let layout = plan.layout;
        let diagram_pictures: Vec<crate::diagram::Picture> = boxes
            .iter()
            .filter_map(|(group, _)| {
                self.inspections
                    .get(&format!("{BOX_IMAGE}{group}"))
                    .map(|x| (group.clone(), x.id, x.size))
            })
            .collect();
        let performance = request
            .state
            .panel
            .shows_passes()
            .then(|| self.performance_view());
        let graph_view = request.state.panel.shows_graph().then(|| self.graph_view());
        let pane_views: Vec<ui::PaneView> = drawn
            .panes
            .iter()
            .zip(&plan.panes)
            .map(|((rect, content), pane)| ui::PaneView {
                rect: *rect,
                content,
                title: pane.title.as_deref(),
            })
            .collect();
        let over_views: Vec<ui::PaneView> = drawn
            .overs
            .iter()
            .filter(|(_, a, _)| *a > 0.5)
            .map(|(_, _, content)| ui::PaneView {
                rect: layout.viewport,
                content,
                title: None,
            })
            .collect();
        // egui pass content.
        let mut raw = request.raw_input;
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, layout.canvas));
        raw.viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(layout.scale);
        let ui_input = UiInput {
            layout,
            state: request.state,
            focus: request.focus,
            content: drawn.free.as_ref().or(content),
            main_view: drawn.main_view,
            statics: &self.statics,
            textures: &self.textures,
            scene_type: request.scene_type,
            diagram: request.diagram,
            diagram_pictures: &diagram_pictures,
            beat_sources: request.beat_sources,
            notices: request.notices,
            references: request.references,
            chapter_number: request.chapter_number,
            fade: request.fade,
            stamp: request.stamp,
            panes: &pane_views,
            overs: &over_views,
            title: plan.main.as_ref().and_then(|m| m.title.as_deref()),
            compare: plan
                .compare
                .as_ref()
                .filter(|_| self.reference_drawn)
                .map(|c| ui::CompareOverlay {
                    mode: c.mode,
                    reference: plan.reference_rect,
                    split: c.split,
                    gain: c.gain,
                }),
            performance: performance.as_ref(),
            graph: graph_view.as_ref(),
        };
        let mut chrome = request.chrome;
        let mut report = UiReport::default();
        // Start every frame with an empty glyph atlas. Otherwise the atlas
        // grows and repacks as new characters appear (hundreds in Chinese),
        // which moves glyph texels and changes text pixels by a level, so a
        // frame would depend on the frames rendered before it.
        self.egui_ctx.set_glyph_rasterizers(Vec::new());
        ui::restyle(&self.egui_ctx);
        let recording = match &chrome {
            Chrome::Recording { .. } => self
                .recording_features
                .get_or_insert_with(|| crate::feature::Features::new(investigation::current())),
            Chrome::Viewer(_) => self.recording_features.get_or_insert_with(Default::default),
        };
        let output = self.egui_ctx.run_pass(raw, |ctx| {
            let chrome = match &mut chrome {
                Chrome::Recording { timeline, frame } => Chrome::Recording {
                    timeline,
                    frame: *frame,
                },
                Chrome::Viewer(v) => Chrome::Viewer(v),
            };
            report = ui::draw(ctx, &ui_input, chrome, recording);
        });
        (output, report)
    }

    /// The frame encoded and submitted: the views' passes, the scene's, the composites, the egui pass over them,
    /// and the copy into `readback`.
    fn encode_frame(
        &mut self,
        mut output: egui::FullOutput,
        content: Option<&FrameContent>,
        plan: &FramePlan,
        drawn: &Drawn,
        scene_beat: bool,
        readback: Option<&wgpu::Buffer>,
    ) {
        self.cursor = output.platform_output.cursor_icon;
        let paint_jobs = self
            .egui_ctx
            .tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.width, self.height],
            pixels_per_point: output.pixels_per_point,
        };
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        // Drain the deltas: epaint asserts (in debug builds) that a
        // TexturesDelta is empty when it is dropped.
        for (id, deltas) in output.textures_delta.set.drain() {
            for delta in &deltas {
                self.egui_renderer.update_texture(device, queue, id, delta);
            }
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame"),
        });
        let mut commands =
            self.egui_renderer
                .update_buffers(device, queue, &mut encoder, &paint_jobs, &screen);
        if let Some(content) = content {
            self.dispatch_views(&mut encoder, false);
            self.encode_scene(&mut encoder, content, true);
            self.dispatch_views(&mut encoder, true);
            // The reads it asks for, of what its passes left (src/fetch.rs).
            if !content.fetches.is_empty() {
                let tag = still_key(content, plan.layout.render_size_px(), true);
                let mut queue = std::mem::take(&mut self.fetches);
                queue.issue(
                    &self.gpu.device,
                    &mut encoder,
                    &content.fetches,
                    tag,
                    crate::fetch::BUDGET,
                    &|source| self.fetch_target(source),
                );
                self.fetches = queue;
            }
        }
        self.composite_views(&mut encoder, plan, drawn, scene_beat);
        self.encode_ui(&mut encoder, &paint_jobs, &screen, scene_beat);
        if let Some(buffer) = readback {
            crate::readback::copy_to_readback(
                &mut encoder,
                &self.frame,
                buffer,
                [self.width, self.height],
            );
        }
        if let Some(t) = self.timing.as_mut() {
            t.resolve(&mut encoder);
        }
        commands.push(encoder.finish());
        self.gpu.queue.submit(commands);
        self.fetches.submitted();
        if let Some(t) = self.timing.as_mut() {
            t.submitted(&self.gpu.device);
        }
        for id in output.textures_delta.free.drain() {
            self.egui_renderer.free_texture(&id);
        }
    }

    /// The views composited into the frame: the free view with the main one in its corner, or the main view (with
    /// the panes drawn already), then the views over it, then the experiment's reference.
    fn composite_views(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        plan: &FramePlan,
        drawn: &Drawn,
        scene_beat: bool,
    ) {
        let layout = plan.layout;
        let vp = layout.viewport_px();
        if let (Some(free), Some(rect)) = (
            self.slots
                .get(&ViewId::FREE)
                .and_then(|s| s.viewport.as_ref())
                .filter(|_| drawn.free.is_some()),
            drawn.main_view,
        ) {
            // The free view fills the viewport; the main view sits in its corner.
            self.composite_from(encoder, free, vp, true, None);
            let at = Layout {
                viewport: rect,
                ..layout
            }
            .viewport_px();
            let timed = self
                .timing
                .as_mut()
                .and_then(|t| t.render_writes("composite"));
            self.composite_pass(
                encoder,
                self.viewport.as_ref().unwrap(),
                at,
                false,
                None,
                timed.as_ref().map(|t| t.writes()),
            );
        } else if scene_beat {
            let timed = self
                .timing
                .as_mut()
                .and_then(|t| t.render_writes("composite"));
            self.composite_timed(
                encoder,
                vp,
                drawn.panes.is_empty(),
                timed.as_ref().map(|t| t.writes()),
            );
        }
        // The views stacked over the main one, each at its alpha.
        for (id, alpha, _) in &drawn.overs {
            if let Some(targets) = self.slots.get(id).and_then(|s| s.viewport.as_ref()) {
                self.composite_over(encoder, targets, vp, *alpha);
            }
        }
        if let Some(c) = &plan.compare {
            self.composite_compare(encoder, c, plan);
        }
    }

    /// An experiment's reference composited as its mode says: a wipe, beside the live view, alone, or the
    /// difference.
    fn composite_compare(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        c: &CompareRequest,
        plan: &FramePlan,
    ) {
        use crate::experiment::Compare;
        let Some(reference) = self
            .slots
            .get(&ViewId::REFERENCE)
            .and_then(|s| s.viewport.as_ref())
            .filter(|_| self.reference_drawn)
        else {
            return;
        };
        let vp = plan.layout.viewport_px();
        match c.mode {
            Compare::Off => {}
            Compare::Wipe => {
                // The wipe draws the reference over the left part of the viewport, up to the split.
                let width = (vp[2] as f32 * c.split.clamp(0.0, 1.0)).round() as u32;
                if width > 0 {
                    self.composite_from(
                        encoder,
                        reference,
                        vp,
                        false,
                        Some([vp[0], vp[1], width, vp[3]]),
                    );
                }
            }
            Compare::SideBySide => self.composite_from(
                encoder,
                reference,
                Layout {
                    viewport: plan.reference_rect,
                    ..plan.layout
                }
                .viewport_px(),
                false,
                None,
            ),
            Compare::Reference => self.composite_from(encoder, reference, vp, false, None),
            Compare::Difference => self.composite_difference(encoder, vp, c.gain),
        }
    }

    /// The egui pass drawn over the frame (over the scene's composite on a scene beat; on the background else).
    fn encode_ui(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        paint_jobs: &[egui::ClippedPrimitive],
        screen: &egui_wgpu::ScreenDescriptor,
        scene_beat: bool,
    ) {
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.frame_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if scene_beat {
                            wgpu::LoadOp::Load
                        } else {
                            self.background_clear()
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
            .forget_lifetime();
        self.egui_renderer.render(&mut pass, paint_jobs, screen);
    }
}
