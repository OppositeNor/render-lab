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
            // A view a feature adds focused (the experiment's reference): the slider sets that view's value.
            ViewerAction::SetParam(name, value) if interactive && self.focus != ViewId::MAIN && self.features.view_state(self.focus, &self.state).is_some() => {
                let focus = self.focus;
                self.with_features(|features, _| {
                    for state in features.states_mut() {
                        if state.set_value(focus, name, value) {
                            break;
                        }
                    }
                });
            }
            // A view stacked over the main one focused: the slider sets that view's value (src/view.rs).
            ViewerAction::SetParam(name, value) if interactive && self.over_camera(self.focus).is_some() => {
                let (view, _) = self.over_camera(self.focus).expect("a view over the main one");
                self.state.set(&format!("view.{view}.{name}"), value);
                for (k, v) in investigation::current().linked_params(name, value) {
                    self.state.set(&format!("view.{view}.{k}"), v);
                }
            }
            ViewerAction::SetParam(name, value) if interactive => self.set_live_value(name, value),
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
                self.show_tab();
                self.select_feature();
            }
            ViewerAction::Home => {
                self.stop_audio();
                self.exit = ViewerExit::Back;
                self.leaving = true;
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
            let code = self.state.panel.wide();
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

    /// Tell the current tab's feature (src/feature/) it was selected.
    pub(super) fn select_feature(&mut self) {
        let tab = self.tab;
        self.with_features(|features, host| {
            if let Some(state) = features.state_mut(tab.0) {
                state.selected(host);
            }
        });
    }

    /// Update every feature: what its UI asked, done with the viewer.
    pub(super) fn update_features(&mut self) {
        self.with_features(|features, host| features.states_mut().for_each(|state| state.update(host)));
    }

    /// Run `f` on the features with the viewer as their host, then apply what they asked of it.
    pub(super) fn with_features(&mut self, f: impl FnOnce(&mut crate::feature::Features, &mut dyn crate::feature::Host)) {
        let mut features = std::mem::take(&mut self.features);
        f(&mut features, self);
        self.features = features;
        for action in std::mem::take(&mut self.feature_actions) {
            self.apply(action);
        }
    }

    /// Set a value of the live state, as its dock slider does: taking hold of an animated value stops the
    /// animation where it is; the values linked to it follow.
    pub(super) fn set_live_value(&mut self, name: &'static str, value: f32) {
        if self.playing && self.animated(name) {
            self.stop();
        }
        self.state.set(name, value);
        for (k, v) in investigation::current().linked_params(name, value) {
            self.state.set(k, v);
        }
    }
}

impl crate::feature::Host for App {
    fn state(&self) -> &ShotState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut ShotState {
        &mut self.state
    }

    fn interactive(&self) -> bool {
        !self.timeline_playing()
    }

    fn ready(&self) -> bool {
        self.renderer.is_some() && !self.timeline_mode
    }

    fn default_state(&mut self, demo: DemoId) -> ShotState {
        App::default_state(self, demo)
    }

    fn set_state(&mut self, state: ShotState) {
        self.stop();
        self.intro = None;
        self.state = state;
    }

    fn show(&mut self, state: ShotState) {
        self.set_state(state);
        self.demo_time = 0.0;
        self.drift_from = 0.0;
        self.show_tab();
    }

    fn set_value(&mut self, key: &'static str, value: f32) {
        self.set_live_value(key, value);
    }

    fn renderer(&mut self) -> Option<&mut Renderer> {
        self.renderer.as_mut()
    }

    fn act(&mut self, action: ViewerAction) {
        self.feature_actions.push(action);
    }

    fn tab(&self) -> DockTab {
        self.tab
    }

    fn timeline_chapter(&self) -> Option<u32> {
        self.timeline.as_ref().filter(|_| self.timeline_mode).map(|t| t.chapter_at(self.timeline_frame).number)
    }

    /// Paused: the caption at or last before the frame, and the one before that.
    fn captions(&self) -> (Option<String>, Option<String>) {
        let Some(t) = self.timeline.as_ref().filter(|_| self.timeline_mode && !self.playing) else { return (None, None) };
        let (_, beat) = t.beat_at(self.timeline_frame);
        let at = beat.captions.iter().rposition(|c| c.start <= self.timeline_frame);
        (at.and_then(|i| i.checked_sub(1)).map(|i| beat.captions[i].text.clone()), at.map(|i| beat.captions[i].text.clone()))
    }
}
