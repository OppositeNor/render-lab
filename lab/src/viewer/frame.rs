//! A frame: what the renderer is asked to draw and the chrome around it,
//! the actions its interface returns, and the frame presented to the window.

use super::*;

/// The window's cursor for one egui asks for.
pub(super) fn window_cursor(icon: egui::CursorIcon) -> winit::window::CursorIcon {
    use winit::window::CursorIcon as W;
    match icon {
        egui::CursorIcon::ResizeHorizontal | egui::CursorIcon::ResizeColumn => W::EwResize,
        egui::CursorIcon::ResizeVertical | egui::CursorIcon::ResizeRow => W::NsResize,
        egui::CursorIcon::PointingHand => W::Pointer,
        egui::CursorIcon::Text => W::Text,
        egui::CursorIcon::Grab => W::Grab,
        egui::CursorIcon::Grabbing => W::Grabbing,
        _ => W::Default,
    }
}

/// Every beat of a timeline with its chapter, for the bar's hover preview.
pub(super) fn bar_sections(t: &Timeline) -> Vec<crate::ui::BarSection> {
    let total = t.total_frames.max(1) as f32;
    t.beats
        .iter()
        .map(|b| crate::ui::BarSection {
            start: b.start_frame as f32 / total,
            chapter: b.chapter,
            chapter_title: t.chapters.iter().find(|c| c.number == b.chapter).map(|c| c.title.clone()).unwrap_or_default(),
            headline: b.headline.clone(),
        })
        .collect()
}

/// The camera sway of a state (0 when the investigation has no `cameraDrift`).
pub(super) fn drift(state: &ShotState) -> f32 {
    state.try_get("cameraDrift").map_or(0.0, |v| v.f32())
}

pub(super) fn blit_group(renderer: &Renderer) -> wgpu::BindGroup {
    renderer.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("blit"),
        layout: &renderer.pipelines.texture_pair_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&renderer.frame_view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&renderer.frame_view) },
        ],
    })
}

impl App {
    pub(super) fn status(&self) -> String {
        use crate::i18n::{args, tr, tr_with};
        let mode = if let (true, Some(tl)) = (self.timeline_mode, &self.timeline) {
            let clock = |frame: u64| {
                let s = frame / tl.fps as u64;
                format!("{}:{:02}", s / 60, s % 60)
            };
            let chapter = tl.chapter_at(self.timeline_frame);
            let what = if self.playing {
                tr("viewer-playing")
            } else {
                tr("viewer-paused_hint")
            };
            let audio = if self.audio.is_some() { "" } else { tr("viewer-no_narration_track") };
            format!("{} / {} · {} · {what}{audio}", clock(self.timeline_frame), clock(tl.total_frames), tr_with("viewer-chapter", args!(number = chapter.number)))
        } else {
            format!("{} · t = {:.2} s", if self.playing { tr("viewer-playing") } else { tr("viewer-paused") }, self.demo_time)
        };
        let hint = investigation::current().demo_hint(self.state.demo).unwrap_or_else(|| tr("viewer-drag_hint"));
        format!("{mode} · {hint}")
    }

    pub fn configure(&mut self, size: PhysicalSize<u32>) {
        let (Some(surface), Some(renderer)) = (self.surface.as_mut(), self.renderer.as_mut()) else { return };
        surface.config.width = size.width.max(1);
        surface.config.height = size.height.max(1);
        surface.surface.configure(&renderer.gpu.device, &surface.config);
        renderer.resize(size.width.max(64), size.height.max(64));
        surface.blit_group = blit_group(renderer);
    }

    /// Advance the clocks and render the frame with its chrome into the
    /// renderer's target (and `readback`), applying what its controls did.
    pub(super) fn render(&mut self, readback: Option<&wgpu::Buffer>) -> Result<()> {
        let now = Instant::now();
        let dt = (now - self.last_tick).as_secs_f32().min(0.1);
        self.last_tick = now;
        // Live reload: about twice a second, the checkout's WGSL files changed since are read
        // again and every pipeline made from them (a broken edit keeps the last good ones).
        if self.frames.is_multiple_of(30) && let Some(renderer) = self.renderer.as_mut() {
            let changed = crate::resources::reload_changed();
            if !changed.is_empty() {
                use crate::i18n::{args, tr_with};
                let files: Vec<String> = changed.iter().map(|id| crate::resources::get(id).source().rsplit('/').next().unwrap_or_default().to_string()).collect();
                self.shader_reload = Some(match renderer.reload_shaders() {
                    Ok(()) => (tr_with("graph-reloaded", args!(files = files.join(", "))), false, now),
                    Err(e) => (tr_with("graph-reload_failed", args!(error = e)), true, now),
                });
            }
        }
        if self.playing {
            if self.timeline_mode {
                if let Some(t) = &self.timeline {
                    // The wall clock since the anchor, so the narration stays in
                    // step; the autotest advances one frame per redraw.
                    let frame = if self.autotest.is_some() {
                        self.timeline_frame + 1
                    } else {
                        self.anchored_frame(now)
                    };
                    self.timeline_frame = frame.min(t.total_frames - 1);
                    if self.timeline_frame + 1 >= t.total_frames {
                        // The end: pause on the last frame, or on to the next part.
                        self.stop();
                        if self.options.next && self.autotest.is_none() {
                            self.exit = ViewerExit::Next;
                            self.leaving = true;
                        }
                    }
                }
            } else {
                self.demo_time += dt;
            }
        }
        if let Some(t) = &self.timeline {
            self.timeline_frame = self.timeline_frame.min(t.total_frames - 1);
        }
        let state = self.render_state();
        let state = self.with_intro(state, now);
        // Playing, the caption on screen; paused, the one at or last before
        // the frame, and the one before that.
        let captions = self.timeline.as_ref().filter(|_| self.timeline_mode).map_or((None, None), |t| {
            let (_, beat) = t.beat_at(self.timeline_frame);
            let frame = self.timeline_frame;
            if self.playing {
                (None, beat.caption_at(frame).map(|c| c.text.clone()))
            } else {
                let at = beat.captions.iter().rposition(|c| c.start <= frame);
                (at.and_then(|i| i.checked_sub(1)).map(|i| beat.captions[i].text.clone()), at.map(|i| beat.captions[i].text.clone()))
            }
        });
        let mut chrome = ViewerChrome {
            playing: self.playing,
            timeline: self.timeline.as_ref().map(|t| (self.timeline_frame, t.total_frames)),
            timeline_mode: self.timeline_mode,
            chapters: self.timeline.as_ref().map(|t| t.chapters.iter().map(|c| c.start_frame as f32 / t.total_frames.max(1) as f32).collect()).unwrap_or_default(),
            sections: self.bar_sections.clone(),
            fps: self.timeline.as_ref().map_or(30, |t| t.fps),
            hover: None,
            caption: captions.1,
            previous_caption: captions.0,
            skip: self.skip.filter(|_| self.timeline_mode).and_then(|(s, at)| {
                let left = 1.0 - (now - at).as_secs_f32() / SKIP_NOTICE.as_secs_f32();
                (left > 0.0).then(|| (s, (left * 3.0).min(1.0)))
            }),
            status: self.status(),
            tab: self.tab,
            actions: Vec::new(),
            ui_has_pointer: false,
            widgets: Vec::new(),
            popups: std::mem::take(&mut self.popups),
            narration: self.narration,
            speed: self.speed,
            diagram: None,
            experiment: crate::ui::ExperimentView::default(),
            knowledge: crate::ui::KnowledgeChrome { chapters: self.knowledge_chapters(), focus: self.knowledge_focus.clone(), overlay: self.knowledge_overlay },
            dag_overlay: self.dag_overlay && matches!(self.state.panel, PanelMode::Dag | PanelMode::Resources),
            graph_source: self.graph_source,
            shader_reload: self.shader_reload.as_ref().filter(|(_, failed, at)| *failed || at.elapsed().as_secs_f32() < 4.0).map(|(t, f, _)| (t.clone(), *f)),
            focus_rect: self.renderer.as_ref().map(|r| r.frame_views()).filter(|v| v.len() > 1).and_then(|v| v.iter().find(|(id, _)| *id == self.focus).map(|(_, r)| *r)),
        };
        // The experiment: the reference drawn while the live state is (not on a timeline), and timings while its tab shows.
        let experimenting = !self.timeline_mode && (self.options.experiments || self.tab == DockTab::Experiments);
        self.frame_times.push_back(dt as f64 * 1000.0);
        if self.frame_times.len() > 120 {
            self.frame_times.pop_front();
        }
        if let Some(renderer) = self.renderer.as_mut() {
            let compare = self.experiment.reference_state(&state).filter(|_| !self.timeline_mode).map(|reference| crate::renderer::CompareRequest { reference, mode: self.experiment.compare, split: self.experiment.split, gain: self.experiment.gain });
            renderer.set_compare(compare);
            // The performance panel measures as it shows, here or on a timeline.
            renderer.set_timing(experimenting || matches!(state.panel, PanelMode::Performance | PanelMode::Graph | PanelMode::Dag | PanelMode::Resources));
            let times: Vec<f64> = self.frame_times.iter().copied().collect();
            let (mean, _, p95) = crate::metrics::summary(&times);
            chrome.experiment = crate::ui::ExperimentView {
                reference: self.experiment.reference.is_some(),
                compare: self.experiment.compare,
                split: self.experiment.split,
                gain: self.experiment.gain,
                measurement: self.experiment.measurement,
                message: self.experiment.message.clone(),
                presets: self.experiment.presets.clone(),
                timings: renderer.timing.as_ref().map(|t| t.summary()).unwrap_or_default(),
                frame_ms: (mean, p95),
                rebuilding: renderer.rebuilding(),
            };
        }
        // The frame a screenshot is taken of shows no popup.
        chrome.popups.hidden = self.screenshot;
        // Paused on a diagram beat, the diagram stays, in its panel, zoomed and panned by the pointer.
        let paused_diagram = self.paused_diagram();
        let (kind, diagram, sources, chapter, fade, notices) = match (&self.timeline, self.timeline_playing() || self.paused_page()) {
            (Some(t), true) => {
                let (_, beat) = t.beat_at(self.timeline_frame);
                // Playing, a diagram moves from the beat before (or builds up); paused, it is settled.
                let shown = if self.timeline_playing() { t.diagram_at(self.timeline_frame) } else { beat.beat_diagram() };
                let diagram = shown.map(|d| (d.graph.clone(), d.focus.to_vec(), d.scene.cloned(), (d.transition.from.map(<[String]>::to_vec), d.transition.seconds, d.transition.built)));
                let fade = if self.paused_page() { 0.0 } else { beat_fade(t, self.timeline_frame) };
                (beat.kind, diagram, beat.sources.clone(), beat.chapter, fade, beat.notices)
            }
            _ => (SceneType::ThreeD, None, Vec::new(), 0, 0.0, false),
        };
        if paused_diagram {
            // The view is set up afresh for another beat, or another first view.
            let beat = self.timeline.as_ref().map_or(0, |t| t.beat_at(self.timeline_frame).1.start_frame);
            let preset = self.diagram_preset.map_or(0, |(z, c)| (z.to_bits() as u64) << 32 ^ (c.x.to_bits() as u64) << 16 ^ c.y.to_bits() as u64);
            chrome.diagram = Some(crate::ui::DiagramPage { key: beat ^ preset.rotate_left(7), preset: self.diagram_preset });
        }
        let raw_input = egui::RawInput { time: Some(self.frames as f64 / 60.0), events: std::mem::take(&mut self.egui_events), ..Default::default() };
        // The focused view's state, when it is not the main view's (src/view.rs): the dock shows and sets it.
        let focus = match self.focus {
            ViewId::FREE => state.free_camera.on.then(|| ShotState { camera: state.free_camera.orbit(), ..state.clone() }),
            ViewId::REFERENCE => self.experiment.reference_state(&state),
            view => self.over_camera(view).map(|(name, camera)| ShotState { camera, ..state.view_state(name) }),
        };
        let renderer = self.renderer.as_mut().expect("renderer");
        renderer.user_layout = self.layout;
        let result = renderer.render_frame(
            FrameRequest { state: &state, scene_type: kind, diagram: diagram.as_ref().map(|(graph, focus, scene, (from, seconds, built))| crate::diagram::BeatDiagram { graph, focus, scene: scene.as_ref(), transition: crate::diagram::Transition { from: from.as_deref(), seconds: *seconds, built: *built } }), beat_sources: &sources, notices, chapter_number: chapter, fade, time: self.demo_time, raw_input, chrome: Chrome::Viewer(&mut chrome), stamp: None, history: true, wait: false, focus: focus.as_ref() },
            readback,
        );
        self.ui_has_pointer = chrome.ui_has_pointer;
        self.bar_hover = chrome.hover.take();
        let cursor = self.renderer().cursor;
        if cursor != self.cursor_icon {
            self.cursor_icon = cursor;
            if let Some(surface) = &self.surface {
                surface.window.set_cursor(window_cursor(cursor));
            }
        }
        self.dock_widgets = std::mem::take(&mut chrome.widgets);
        self.popups = std::mem::take(&mut chrome.popups);
        for action in self.popups.take_actions() {
            popup::perform(action);
        }
        if result.content.is_some() {
            self.last_content = result.content;
            self.last_main_content = result.main_content;
            // A focused view that is gone gives the focus back to the main view.
            if !self.renderer().frame_views().iter().any(|(v, _)| *v == self.focus) {
                self.focus = ViewId::MAIN;
            }
            self.last_insets = result.report.insets;
        }
        for action in chrome.actions {
            self.apply(action);
        }
        Ok(())
    }

    pub(super) fn redraw(&mut self) -> Result<()> {
        self.render(None)?;
        // Present: blit the RGBA frame to the swapchain.
        let renderer = self.renderer.as_ref().unwrap();
        let surface = self.surface.as_ref().unwrap();
        let frame = match surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                let size = surface.window.inner_size();
                self.configure(size);
                return Ok(());
            }
            _ => return Ok(()),
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = renderer.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&surface.blit);
            pass.set_bind_group(0, renderer.frame_bind_group_ref(), &[]);
            pass.set_bind_group(1, &surface.blit_group, &[]);
            pass.draw(0..3, 0..1);
        }
        renderer.gpu.queue.submit([encoder.finish()]);
        if std::mem::take(&mut self.screenshot) {
            let saved = crate::readback::screenshot(&renderer.gpu.device, &renderer.gpu.queue, &frame.texture, &self.options.manifest_dir);
            self.popups.push(Popup::screenshot(&saved));
        }
        surface.window.pre_present_notify();
        renderer.gpu.queue.present(frame);
        self.frames += 1;
        Ok(())
    }
}
