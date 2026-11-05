//! What the viewer shows and when: the demonstration and the tab, the Play
//! animation, the timeline's clock, its narration, and seeking.

use super::*;

impl App {
    pub fn select_demo(&mut self, demo: impl Into<DemoId>) {
        let demo = demo.into();
        self.intro = None;
        self.stop_audio();
        // Experimenting, the values changed so far carry over to the next demonstration.
        // So does the reference's: its changes from the old demonstration's defaults, over the new one's.
        let (carried, reference) = if self.options.experiments {
            let defaults = self.default_state(self.state.demo);
            (crate::experiment::changes(&self.state, &defaults), self.experiment.reference.as_ref().map(|r| crate::experiment::changes(r, &defaults)))
        } else {
            (Default::default(), None)
        };
        self.state = self.default_state(demo);
        if let Err(e) = crate::experiment::apply(&mut self.state, &carried) {
            self.experiment.message = Some(format!("{e:#}"));
        }
        if let Some(changes) = reference {
            let mut r = self.default_state(demo);
            match crate::experiment::apply(&mut r, &changes) {
                Ok(()) => self.experiment.reference = Some(r),
                Err(e) => self.experiment.message = Some(format!("{e:#}")),
            }
            self.experiment.measurement = None;
        }
        self.demo_time = 0.0;
        self.drift_from = 0.0;
        self.playing = false;
        self.timeline_mode = false;
        self.show_tab();
    }

    /// The Code tab shows the demonstration's first listing in the panel;
    /// the other tabs show its struct panel.
    pub(super) fn show_tab(&mut self) {
        if self.tab == DockTab::Captions && !self.timeline_mode {
            self.tab = DockTab::Parameters;
        }
        let defaults = self.default_state(self.state.demo);
        // The Prerequisites tab shows the knowledge graph in the panel.
        if self.tab == DockTab::Prerequisites {
            self.state.panel = PanelMode::Knowledge;
        } else if self.state.panel == PanelMode::Knowledge {
            self.state.panel = defaults.panel;
        }
        // The Frame graph tab shows the frame graph (its DAG, unless a view of it shows).
        let graph = matches!(self.state.panel, PanelMode::Graph | PanelMode::Dag | PanelMode::Resources);
        if self.tab == DockTab::Graph {
            if !graph {
                self.state.panel = PanelMode::Dag;
            }
        } else if graph {
            self.state.panel = defaults.panel;
            self.dag_overlay = false;
        }
        if self.tab == DockTab::Code {
            if self.state.panel != PanelMode::Code {
                self.state.panel = PanelMode::Code;
                self.state.code = investigation::current().listing_for_demo(self.state.demo).map(|l| l.name.to_string());
                self.state.highlight.clear();
            }
        } else if self.state.panel == PanelMode::Code {
            self.state.panel = defaults.panel;
            self.state.highlight = defaults.highlight;
            self.state.code = None;
        }
    }

    /// The chapters the Prerequisites tab is about: on a timeline, the
    /// current one; else those whose demonstrations include this one.
    pub(super) fn knowledge_chapters(&self) -> Vec<u32> {
        match (&self.timeline, self.timeline_mode) {
            (Some(t), true) => vec![t.chapter_at(self.timeline_frame).number],
            _ => crate::knowledge::current().map(|g| g.chapters_of_demo(self.state.demo.key())).unwrap_or_default(),
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
            // Paused, the dock shows the caption, to read again.
            self.tab = DockTab::Captions;
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
