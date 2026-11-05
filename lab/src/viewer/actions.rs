//! The viewer's actions (`ViewerAction`, from the dock, the panels, the keys,
//! and the pointer) applied, and those that save or measure.

use super::*;

impl App {
    pub fn apply(&mut self, action: ViewerAction) {
        let interactive = !self.timeline_playing();
        match action {
            ViewerAction::SelectDemo(d) => self.select_demo(d),
            ViewerAction::TogglePlay => {
                if self.playing {
                    self.stop();
                } else {
                    self.play();
                }
            }
            ViewerAction::Step => {
                self.stop();
                if self.timeline_mode {
                    self.seek(self.timeline_frame + 1);
                } else {
                    self.drift_from = self.demo_time;
                    self.demo_time += 1.0 / 30.0;
                    self.bake_animation();
                }
            }
            ViewerAction::ToggleFreeCamera => {
                let shown = self.render_state();
                let mut free = shown.free_camera;
                free.on = !free.on;
                // Turned on, it starts beside the main camera, seeing it and its view.
                if free.on {
                    free.place(&crate::camera::FreeCamera::beside(&shown.camera));
                }
                self.free_view = Some(free);
            }
            ViewerAction::Reset => {
                self.free_view = None;
                if self.timeline_mode {
                    self.seek(0);
                } else {
                    self.state = self.default_state(self.state.demo);
                    self.demo_time = 0.0;
                    self.drift_from = 0.0;
                    self.playing = false;
                }
            }
            ViewerAction::ToggleTimeline => {
                if self.timeline.is_some() {
                    self.stop_audio();
                    self.playing = false;
                    self.timeline_mode = !self.timeline_mode;
                    self.free_view = None;
                    if self.timeline_mode {
                        self.take_timeline_shot();
                    }
                    self.show_tab();
                }
            }
            ViewerAction::Seek(fraction) => {
                let total = self.timeline.as_ref().map_or(0, |t| t.total_frames);
                self.seek((fraction.clamp(0.0, 1.0) as f64 * total as f64) as u64);
            }
            // The experiment's reference focused: the slider sets the reference's value (its camera is the live one's).
            ViewerAction::SetParam(name, value) if interactive && self.focus == ViewId::REFERENCE && self.experiment.reference.is_some() => {
                let reference = self.experiment.reference.as_mut().expect("a reference");
                reference.set(name, value);
                for (k, v) in investigation::current().linked_params(name, value) {
                    reference.set(k, v);
                }
            }
            // A view stacked over the main one focused: the slider sets that view's value (src/view.rs).
            ViewerAction::SetParam(name, value) if interactive && self.over_camera(self.focus).is_some() => {
                let (view, _) = self.over_camera(self.focus).expect("a view over the main one");
                self.state.set(&format!("view.{view}.{name}"), value);
                for (k, v) in investigation::current().linked_params(name, value) {
                    self.state.set(&format!("view.{view}.{k}"), v);
                }
            }
            ViewerAction::SetParam(name, value) if interactive => {
                // Taking hold of an animated value stops the animation where it is.
                if self.playing && self.animated(name) {
                    self.stop();
                }
                self.state.set(name, value);
                for (k, v) in investigation::current().linked_params(name, value) {
                    self.state.set(k, v);
                }
            }
            // The free camera's view focused: the camera sliders move the free camera.
            ViewerAction::SetCamera(camera) if self.focus == ViewId::FREE && self.free_camera_on() => {
                self.free_view().place(&camera);
            }
            ViewerAction::SetCamera(camera) if self.over_camera(self.focus).is_some() => {
                let (view, _) = self.over_camera(self.focus).expect("a view over the main one");
                self.state.set_own_camera(view, &camera);
            }
            ViewerAction::SetCamera(camera) if interactive => {
                // The camera sliders show the swayed view; hold it before changing it.
                self.end_intro();
                self.stop();
                self.state.camera = camera;
            }
            ViewerAction::SetRecord(record) if interactive => {
                if std::mem::discriminant(&record) != std::mem::discriminant(&self.state.record) {
                    self.state.highlight = shot::default_highlight(record);
                }
                self.state.record = record;
            }
            ViewerAction::SetHighlight(fields) if interactive => self.state.highlight = fields,
            ViewerAction::SetPanel(mode) if interactive => self.state.panel = mode,
            ViewerAction::Inspect(key) if interactive => inspect(&mut self.state, key),
            ViewerAction::ResetParameters if interactive => {
                // The camera stays unless the dock shows it (the overview's camera sliders).
                self.stop();
                let demo = self.state.demo;
                let defaults = self.default_state(demo);
                let camera = if investigation::current().resets_camera(demo) { defaults.camera } else { self.state.camera };
                self.state = ShotState { camera, ..defaults };
                self.demo_time = 0.0;
                self.drift_from = 0.0;
                self.playing = false;
            }
            ViewerAction::SelectTab(tab) => {
                self.tab = tab;
                if tab == DockTab::Experiments {
                    self.experiment.presets = crate::experiment::presets();
                }
                self.show_tab();
            }
            ViewerAction::SetCode(name) if interactive => {
                self.state.code = Some(name.to_string());
                self.state.highlight.clear();
            }
            ViewerAction::Home => {
                self.stop_audio();
                self.exit = ViewerExit::Back;
                self.leaving = true;
            }
            ViewerAction::ResetParam(key) if interactive => {
                let defaults = self.default_state(self.state.demo);
                if let Some(value) = defaults.try_get(key) {
                    if crate::camera::OrbitCamera::schema().iter().any(|e| e.key == key || e.components.contains(&key)) {
                        self.end_intro();
                        self.state.camera.set(key, value);
                    } else {
                        self.state.set(key, value.f32());
                        for (k, v) in investigation::current().linked_params(key, value.f32()) {
                            self.state.set(k, v);
                        }
                    }
                }
            }
            ViewerAction::SetReference => {
                self.experiment.reference = Some(self.state.clone());
                if self.experiment.compare == crate::experiment::Compare::Off {
                    self.experiment.compare = crate::experiment::Compare::Wipe;
                }
                self.experiment.measurement = None;
                self.experiment.message = Some(crate::i18n::tr("experiments-reference_set").to_string());
            }
            ViewerAction::ClearReference => {
                self.experiment.reference = None;
                self.experiment.compare = crate::experiment::Compare::Off;
                self.experiment.measurement = None;
            }
            ViewerAction::SwapReference if interactive => {
                if let Some(reference) = self.experiment.reference.take() {
                    let camera = self.state.camera;
                    let live = std::mem::replace(&mut self.state, ShotState { camera, ..reference });
                    self.experiment.reference = Some(live);
                    self.experiment.measurement = None;
                }
            }
            ViewerAction::SetCompare(mode) => self.experiment.compare = mode,
            ViewerAction::SetSplit(split) => self.experiment.split = split.clamp(0.0, 1.0),
            ViewerAction::SetGain(gain) => self.experiment.gain = gain.clamp(1.0, 64.0),
            ViewerAction::Measure => self.measure(),
            ViewerAction::SavePreset(name) => {
                let defaults = self.default_state(self.state.demo);
                let experiment = crate::experiment::Experiment::capture(&name, &self.state, &defaults, self.experiment.reference.as_ref());
                self.experiment.message = Some(match crate::experiment::save_user(&experiment) {
                    Ok(path) => crate::i18n::tr_with("experiments-saved", crate::i18n::args!(path = path.display().to_string())),
                    Err(e) => format!("{e:#}"),
                });
                self.experiment.presets = crate::experiment::presets();
            }
            ViewerAction::LoadPreset(i) if interactive => {
                if let Some(preset) = self.experiment.presets.get(i).cloned() {
                    self.experiment.message = Some(match self.load_experiment(&preset) {
                        Ok(()) => crate::i18n::tr_with("experiments-loaded", crate::i18n::args!(name = preset.name.clone())),
                        Err(e) => format!("{e:#}"),
                    });
                }
            }
            ViewerAction::ResizeLayout(part, size) => {
                let size = size.map(|s| s.round().max(1.0) as u32);
                match part {
                    crate::ui::LayoutPart::Panel => self.layout.panel_width = size,
                    crate::ui::LayoutPart::CodePanel => self.layout.code_panel_width = size,
                    crate::ui::LayoutPart::Dock => self.layout.dock_height = size,
                }
            }
            ViewerAction::SaveLayout => self.save_layout(),
            ViewerAction::SetKnowledgeOverlay(on) => self.knowledge_overlay = on,
            ViewerAction::SetDagOverlay(on) => self.dag_overlay = on,
            ViewerAction::ShowSource(on) => self.graph_source = on,
            ViewerAction::ResetLayout => {
                self.layout = Default::default();
                self.save_layout();
            }
            ViewerAction::SetNarration(volume, finished) => {
                self.narration = volume;
                if let Some(a) = self.audio.as_mut() {
                    a.set_gain(volume.gain());
                }
                // FFplay takes a volume when it starts: a playing narration restarts where it is.
                if finished && self.timeline_playing() {
                    self.start_audio();
                }
            }
            ViewerAction::SetSpeed(speed) => {
                // Playing, the timeline goes on from where it is at the new speed, the narration with it.
                if self.timeline_playing() {
                    let total = self.timeline.as_ref().map_or(0, |t| t.total_frames);
                    self.timeline_frame = self.anchored_frame(Instant::now()).min(total.saturating_sub(1));
                    self.speed = speed;
                    self.start_audio();
                } else {
                    self.speed = speed;
                }
            }
            _ => {}
        }
    }

    /// Write the layout's sizes to the settings file, the other settings as it has them.
    pub(super) fn save_layout(&mut self) {
        // What the user sees: the sizes as the layout kept them (clamped to the window).
        if let Some(r) = self.renderer.as_ref() {
            let l = r.frame_layout();
            let dock = (l.captions.max.y - l.captions.min.y).round() as u32;
            let panel = (l.panel.max.x - l.panel.min.x).round() as u32;
            if self.layout.dock_height.is_some() {
                self.layout.dock_height = Some(dock);
            }
            let code = matches!(self.state.panel, PanelMode::Code | PanelMode::Knowledge | PanelMode::Dag | PanelMode::Resources);
            let width = if code { &mut self.layout.code_panel_width } else { &mut self.layout.panel_width };
            if width.is_some() && l.viewport.max.x < l.panel.min.x {
                *width = Some(panel);
            }
        }
        let Some(path) = self.options.settings.clone() else { return };
        if let Err(e) = crate::config::Config::save_layout(&path, self.layout) {
            self.popups.push(Popup::new(crate::i18n::tr("ui-layout_not_saved"), format!("{e:#}")).closing_after(8.0));
        }
    }

    /// Measure the live view against the reference's, as the last frame drew them (src/metrics.rs).
    pub(super) fn measure(&mut self) {
        let Some(renderer) = self.renderer.as_mut() else { return };
        let views = renderer.read_view(crate::renderer::ViewKind::Reference).zip(renderer.read_view(crate::renderer::ViewKind::Live));
        match views {
            Some(((ref_display, ref_linear), (display, linear))) if ref_display.width == display.width && ref_display.height == display.height => {
                self.experiment.measurement = Some(crate::metrics::measure(&ref_display, &display, Some((&ref_linear, &linear))));
                self.experiment.message = None;
            }
            _ => self.experiment.message = Some(crate::i18n::tr("experiments-measure_needs_reference").to_string()),
        }
    }

    /// Open an experiment file: its demonstration, camera, and base values,
    /// and its reference (with a comparison) when it has one.
    pub(super) fn load_experiment(&mut self, preset: &crate::experiment::Preset) -> Result<()> {
        let experiment = preset.load()?;
        experiment.check()?;
        self.stop();
        self.intro = None;
        let demo = experiment.start_demo()?;
        let mut state = self.default_state(demo);
        experiment.apply(&mut state, &experiment.base)?;
        self.experiment.reference = match experiment.reference_values() {
            Some(values) => {
                let mut reference = state.clone();
                experiment.apply(&mut reference, &values)?;
                Some(reference)
            }
            None => None,
        };
        if self.experiment.reference.is_some() && self.experiment.compare == crate::experiment::Compare::Off {
            self.experiment.compare = crate::experiment::Compare::Wipe;
        }
        self.experiment.measurement = None;
        self.state = state;
        Ok(())
    }
}
