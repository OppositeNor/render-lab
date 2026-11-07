//! What the viewer shows and when: the demonstration and the tab, the Play
//! animation, the timeline's clock, its narration, and seeking.

use super::*;

impl App {
    pub fn select_demo(&mut self, demo: impl Into<DemoId>) {
        let demo = demo.into();
        self.intro = None;
        self.stop_audio();
        // Opened on a feature's tab, the feature sees the change (the experiment carries the changed values over).
        let before = self.options.tab.map(|tab| {
            let old = self.state.clone();
            let old_defaults = self.default_state(old.demo);
            (tab, old, old_defaults)
        });
        self.state = self.default_state(demo);
        if let Some((tab, old, old_defaults)) = before {
            let defaults = self.state.clone();
            self.with_features(|features, host| {
                if let Some(state) = features.state_mut(tab.0) {
                    state.demo_selected(&crate::feature::DemoChange { old: &old, old_defaults: &old_defaults, defaults: &defaults }, host);
                }
            });
        }
        self.demo_time = 0.0;
        self.drift_from = 0.0;
        self.playing = false;
        self.timeline_mode = false;
        self.show_tab();
    }

    /// The tab shown puts its panel up (src/feature/: the Code tab the demonstration's first listing), and the
    /// others give theirs back: the demonstration's panel. A timeline's tab (the Captions) is the timeline's only.
    pub(super) fn show_tab(&mut self) {
        if !self.timeline_mode && crate::feature::find(self.tab.0).is_some_and(|f| f.timeline()) {
            self.tab = DockTab::default();
        }
        let defaults = self.default_state(self.state.demo);
        // The features' tabs put their panels up, and give them back when left (src/feature/: the
        // Prerequisites tab its knowledge graph, the Frame graph tab its DAG, the Code tab its listing).
        let mut actions = Vec::new();
        for feature in crate::feature::all() {
            feature.show(&mut crate::feature::ShowTab { state: &mut self.state, defaults: &defaults, shown: self.tab.0 == feature.id(), actions: &mut actions });
        }
        for action in actions {
            self.apply(action);
        }
    }

    /// Whether the demonstration's animation drives `param`.
    pub(super) fn animated(&self, param: &str) -> bool {
        investigation::current().animation(self.state.demo, self.demo_time).iter().any(|p| p.0 == param)
            || crate::script::runtime::animate(self.state.demo, self.demo_time, &mut self.state.clone()).is_some_and(|keys| keys.iter().any(|k| k == param))
    }

    /// Keep the animation's current values and the swayed camera, so that
    /// the sliders and the view hold the pose it stopped at.
    pub(super) fn bake_animation(&mut self) {
        for (k, v) in investigation::current().animation(self.state.demo, self.demo_time) {
            self.state.set(k, v);
        }
        crate::script::runtime::animate(self.state.demo, self.demo_time, &mut self.state);
        self.state.camera = shot::camera_drift(self.state.camera, self.drift_from, self.demo_time, drift(&self.state));
        self.drift_from = self.demo_time;
    }

    pub(super) fn play(&mut self) {
        self.playing = true;
        // The camera sways from where it is now.
        self.drift_from = self.demo_time;
        if self.timeline_mode {
            // The timeline takes the shot back from where it paused, with its narration.
            self.start_audio();
        }
    }

    pub(super) fn stop(&mut self) {
        if self.playing && !self.timeline_mode {
            self.bake_animation();
        }
        if self.playing && self.timeline_mode {
            self.stop_audio();
            self.take_timeline_shot();
            // Paused, the dock shows the timeline's tab (the caption, to read again).
            if let Some(tab) = crate::feature::all().into_iter().find(|f| f.timeline()) {
                self.tab = DockTab(tab.id());
            }
            self.show_tab();
        }
        self.playing = false;
    }

    /// A paused timeline hands its shot to the user: the interactive state
    /// starts as the timeline's state at the current frame.
    pub(super) fn take_timeline_shot(&mut self) {
        if let Some(t) = &self.timeline {
            self.state = t.state_at(self.timeline_frame);
            self.demo_time = 0.0;
            self.drift_from = 0.0;
        }
    }

    /// The playing timeline's frame at `now`, from the wall clock since its anchor.
    pub fn anchored_frame(&self, now: Instant) -> u64 {
        let (at, from) = self.timeline_anchor;
        from + (now.saturating_duration_since(at).as_secs_f64() * self.fps() * self.speed as f64) as u64
    }

    /// The instant the playing timeline's clock counts from.
    pub fn timeline_anchor(&self) -> Instant {
        self.timeline_anchor.0
    }

    pub(super) fn fps(&self) -> f64 {
        self.timeline.as_ref().map_or(30.0, |t| t.fps as f64)
    }

    /// Start the narration at the current frame, and anchor the timeline's
    /// clock there. FFplay takes about a tenth of a second to start sounding,
    /// so the picture holds this frame that long.
    pub(super) fn start_audio(&mut self) {
        let seconds = self.timeline_frame as f64 / self.fps();
        let speed = self.speed;
        let started = self.audio.as_mut().is_some_and(|a| {
            a.set_speed(speed);
            a.play_from(seconds)
        });
        let delay = if started { AUDIO_START_DELAY } else { std::time::Duration::ZERO };
        self.timeline_anchor = (Instant::now() + delay, self.timeline_frame);
    }

    pub fn stop_audio(&mut self) {
        if let Some(a) = self.audio.as_mut() {
            a.stop();
        }
    }

    /// Move the timeline to `frame`: playing, the narration follows; paused,
    /// the new frame's shot is handed over.
    pub(super) fn seek(&mut self, frame: u64) {
        let Some(total) = self.timeline.as_ref().map(|t| t.total_frames) else { return };
        self.timeline_frame = frame.min(total.saturating_sub(1));
        if self.timeline_playing() {
            self.start_audio();
        } else if self.timeline_mode {
            self.take_timeline_shot();
        }
    }

    /// Skip the timeline `seconds` back or forward; presses in the same
    /// direction while the notice shows add up in it.
    pub(super) fn skip(&mut self, seconds: i64) {
        let Some(fps) = self.timeline.as_ref().map(|t| t.fps as i64) else { return };
        self.seek((self.timeline_frame as i64 + seconds * fps).max(0) as u64);
        let now = Instant::now();
        let total = match self.skip {
            Some((s, at)) if now - at < SKIP_NOTICE && s.signum() == seconds.signum() => s + seconds,
            _ => seconds,
        };
        self.skip = Some((total, now));
    }
}
