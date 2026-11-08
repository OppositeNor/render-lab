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
            ViewerAction::SetParam(name, value)
                if interactive
                    && self.focus != ViewId::MAIN
                    && self.features.view_state(self.focus, &self.state).is_some() =>
            {
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
            ViewerAction::SetParam(name, value)
                if interactive && self.over_camera(self.focus).is_some() =>
            {
                let (view, _) = self
                    .over_camera(self.focus)
                    .expect("a view over the main one");
                self.state.set(&format!("view.{view}.{name}"), value);
                for (k, v) in investigation::current().linked_params(name, value) {
                    self.state.set(&format!("view.{view}.{k}"), v);
                }
            }
            ViewerAction::SetParam(name, value) if interactive => self.set_live_value(name, value),
            // The free camera's view focused: the camera sliders move the free camera.
            ViewerAction::SetCamera(camera)
                if self.focus == ViewId::FREE && self.free_camera_on() =>
            {
                self.free_view().place(&camera);
            }
            ViewerAction::SetCamera(camera) if self.over_camera(self.focus).is_some() => {
                let (view, _) = self
                    .over_camera(self.focus)
                    .expect("a view over the main one");
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
                let camera = if investigation::current().resets_camera(demo) {
                    defaults.camera
                } else {
                    self.state.camera
                };
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
            ViewerAction::Window(id, change) => self.change_window(id, change),
            ViewerAction::Select(id) => {
                self.selected = id;
                self.gizmo_hover = None;
            }
            ViewerAction::SetGizmo(mode) => self.gizmo_mode = mode,
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
                    self.timeline_frame = self
                        .anchored_frame(Instant::now())
                        .min(total.saturating_sub(1));
                    self.speed = speed;
                    self.start_audio();
                } else {
                    self.speed = speed;
                }
            }
            _ => {}
        }
    }

    /// Move, dock, resize, collapse, open, or close window `id` (src/ui/windows.rs), from where the last
    /// frame drew it.
    fn change_window(&mut self, id: crate::ui::windows::WindowId, change: crate::ui::WindowChange) {
        use crate::ui::WindowChange;
        use crate::ui::windows::{Dock, MIN_FLOAT};
        let drawn = self
            .renderer
            .as_ref()
            .and_then(|r| r.frame_layout().window(id));
        let area = self.renderer.as_ref().map(|r| r.frame_layout().area);
        let place = self.layout.window_mut(id);
        match change {
            WindowChange::Move(delta) => {
                let Some(drawn) = drawn else { return };
                // A docked window floats from where it is drawn, its size kept (a collapsed one its float size).
                if place.dock != Dock::Float {
                    let size = if drawn.collapsed {
                        egui::vec2(place.float[2], place.float[3])
                    } else {
                        drawn.frame.size()
                    };
                    place.float = [drawn.frame.min.x, drawn.frame.min.y, size.x, size.y];
                    place.dock = Dock::Float;
                }
                place.float[0] += delta.x;
                place.float[1] += delta.y;
            }
            WindowChange::Drop(dock) => place.dock = dock,
            WindowChange::Resize(to) => {
                let Some(drawn) = drawn else { return };
                let f = drawn.frame;
                let size = match place.dock {
                    Dock::Left => to.x - f.min.x,
                    Dock::Right => f.max.x - to.x,
                    Dock::Bottom => f.max.y - to.y,
                    Dock::Float => {
                        let most = area.map_or(egui::Vec2::INFINITY, |a| a.max - f.min);
                        let size = (to - f.min).max(MIN_FLOAT).min(most);
                        place.float[2] = size.x;
                        place.float[3] = size.y;
                        return;
                    }
                };
                // The Panel window keeps a width of its own for wide content (the pseudo-code).
                if drawn.wide {
                    place.wide = size.max(1.0);
                } else {
                    place.size = size.max(1.0);
                }
            }
            WindowChange::Collapse(collapsed) => place.collapsed = collapsed,
            WindowChange::Open(open) => place.open = open,
            WindowChange::Default => *place = id.default_place(),
        }
    }

    /// Write the windows' places to the settings file, the other settings as it has them; docked sizes as
    /// the layout kept them (within the window).
    pub(super) fn save_layout(&mut self) {
        if let Some(r) = self.renderer.as_ref() {
            let l = r.frame_layout();
            for w in l.windows.iter().flatten().filter(|w| !w.collapsed) {
                let place = self.layout.window_mut(w.id);
                let size = match w.dock {
                    crate::ui::windows::Dock::Bottom => w.frame.height(),
                    crate::ui::windows::Dock::Float => continue,
                    _ => w.frame.width(),
                };
                *(if w.wide {
                    &mut place.wide
                } else {
                    &mut place.size
                }) = size.round();
            }
            for place in &mut self.layout.windows {
                place.float = place.float.map(f32::round);
            }
        }
        let Some(path) = self.options.settings.clone() else {
            return;
        };
        if let Err(e) = crate::config::Config::save_layout(&path, self.layout) {
            self.popups.push(
                Popup::new(crate::i18n::tr("ui-layout_not_saved"), format!("{e:#}"))
                    .closing_after(8.0),
            );
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
        self.with_features(|features, host| {
            features.states_mut().for_each(|state| state.update(host))
        });
    }

    /// Run `f` on the features with the viewer as their host, then apply what they asked of it.
    pub(super) fn with_features(
        &mut self,
        f: impl FnOnce(&mut crate::feature::Features, &mut dyn crate::feature::Host),
    ) {
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
        self.timeline
            .as_ref()
            .filter(|_| self.timeline_mode)
            .map(|t| t.chapter_at(self.timeline_frame).number)
    }

    /// Paused: the caption at or last before the frame, and the one before that.
    fn captions(&self) -> (Option<String>, Option<String>) {
        let Some(t) = self
            .timeline
            .as_ref()
            .filter(|_| self.timeline_mode && !self.playing)
        else {
            return (None, None);
        };
        let (_, beat) = t.beat_at(self.timeline_frame);
        let at = beat
            .captions
            .iter()
            .rposition(|c| c.start <= self.timeline_frame);
        (
            at.and_then(|i| i.checked_sub(1))
                .map(|i| beat.captions[i].text.clone()),
            at.map(|i| beat.captions[i].text.clone()),
        )
    }
}
