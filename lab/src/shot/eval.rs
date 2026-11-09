//! A timeline read: loaded and validated, its channels eased between keys, the
//! notes and images the keys move, and the shot's state at a frame.

use super::*;

/// A channel's value over time: eased from its value at a key to the key's
/// target, or a motion's.
#[derive(Clone, Debug)]
pub(super) enum Channel {
    Blend(Segment),
    /// The `index`th channel of a motion that started at `from` from `start`.
    Motion {
        motion: Rc<Motion>,
        index: usize,
        from: u64,
        fps: f32,
        start: Rc<Vec<f32>>,
    },
}

impl Channel {
    pub(super) fn hold(from: u64, value: f32) -> Channel {
        Channel::Blend(Segment::hold(from, value))
    }

    pub(super) fn value(&self, frame: u64) -> f32 {
        match self {
            Channel::Blend(segment) => segment.value(frame),
            Channel::Motion {
                motion,
                index,
                from,
                fps,
                start,
            } => motion.sample(frame.saturating_sub(*from) as f32 / fps, start)[*index],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Segment {
    pub(super) from: u64,
    pub(super) blend: u64,
    pub(super) ease: Ease,
    pub(super) start: f32,
    pub(super) target: f32,
}

impl Segment {
    pub(super) fn hold(from: u64, value: f32) -> Segment {
        Segment {
            from,
            blend: 0,
            ease: Ease::Smooth,
            start: value,
            target: value,
        }
    }

    pub(super) fn value(&self, frame: u64) -> f32 {
        if self.blend == 0 || frame >= self.from + self.blend {
            return self.target;
        }
        if frame <= self.from {
            return self.start;
        }
        let x = (frame - self.from) as f32 / self.blend as f32;
        self.start + (self.target - self.start) * self.ease.apply(x)
    }
}

/// The channel of the formula card's lit term's weight.
pub(super) const FORMULA_LIT: &str = "formula.lit";
/// The channel of the fade of the story's image `name`.
pub(super) fn image_alpha(name: &str) -> String {
    format!("image.{name}.alpha")
}

/// The channel of component `k` of parameter `param` of the story's image instance `name`.
pub(super) fn image_mark(name: &str, k: usize) -> String {
    format!("image.{name}.mark#{k}")
}

pub(super) fn image_mark_alpha(name: &str) -> String {
    format!("image.{name}.markAlpha")
}

pub(super) fn image_param(name: &str, param: &str, k: usize) -> String {
    format!("image.{name}.param.{param}#{k}")
}

/// A parameter's value as a key holds it: a number, a flag, or numbers.
pub(super) fn components(value: &serde_json::Value) -> Result<Vec<f32>, String> {
    match value {
        serde_json::Value::Bool(b) => Ok(vec![if *b {
            1.0
        } else {
            0.0
        }]),
        serde_json::Value::Array(a) => a
            .iter()
            .map(|v| {
                v.as_f64()
                    .map(|n| n as f32)
                    .ok_or_else(|| format!("{value} is not numbers"))
            })
            .collect(),
        v => v
            .as_f64()
            .map(|n| vec![n as f32])
            .ok_or_else(|| format!("{value} is not a number or numbers")),
    }
}

/// The channel of the fade of the story's note `name`.
pub(super) fn note_alpha(name: &str) -> String {
    format!("note.{name}.alpha")
}

/// The channel of component `k` of a fixed point `end` (`from`, `to`, `at`) of note `name`.
pub(super) fn note_point(name: &str, end: &str, k: usize) -> String {
    format!("note.{name}.{end}#{k}")
}

/// A point as a key holds it: three numbers, or a published point's key.
pub(super) enum PointValue {
    At([f32; 3]),
    Named(String),
}

pub(super) fn point_value(value: &serde_json::Value) -> Result<PointValue, String> {
    if let Some(key) = value.as_str() {
        let declared = investigation::current()
            .annotations()
            .iter()
            .any(|a| a.key == key && a.kind == AnnotationKind::Point);
        if !declared {
            let known: Vec<&str> = investigation::current()
                .annotations()
                .iter()
                .filter(|a| a.kind == AnnotationKind::Point)
                .map(|a| a.key)
                .collect();
            return Err(format!(
                "no point `{key}` (the investigation's: {})",
                crate::story_refs::names_or_none(&known)
            ));
        }
        return Ok(PointValue::Named(key.to_string()));
    }
    let n = components(value)?;
    let [x, y, z] = n[..] else {
        return Err(format!(
            "{value} is not a point (three numbers, or a point's key)"
        ));
    };
    Ok(PointValue::At([x, y, z]))
}

/// The arrows and labels `key` hides (all of them, by its empty `arrow` or `label`, or those it names) fade
/// out.
fn hide_notes(key: &Key, state: &ShotState, targets: &mut Vec<(String, f32)>) {
    let hide = |state: &ShotState,
                targets: &mut Vec<(String, f32)>,
                which: &dyn Fn(&StoryNote) -> bool| {
        targets.extend(
            state
                .notes
                .iter()
                .filter(|n| which(n))
                .map(|n| (note_alpha(&n.name), 0.0)),
        );
    };
    if key.arrow.as_deref() == Some("") {
        hide(state, targets, &|n| {
            !matches!(n.kind, NoteKind::Label { .. })
        });
    }
    if key.label.as_deref() == Some("") {
        hide(state, targets, &|n| {
            matches!(n.kind, NoteKind::Label { .. })
        });
    }
    targets.extend(
        state
            .notes
            .iter()
            .filter(|n| match n.kind {
                NoteKind::Label { .. } => key.label_hide.contains(&n.name),
                _ => key.arrow_hide.contains(&n.name),
            })
            .map(|n| (note_alpha(&n.name), 0.0)),
    );
}

/// A point's anchor at `key` (its note `name`'s `end`): published, or fixed and blended over its channels.
fn set_point(
    key: &Key,
    name: &str,
    end: &str,
    value: &serde_json::Value,
    segments: &mut BTreeMap<String, Channel>,
    targets: &mut Vec<(String, f32)>,
) -> Anchor {
    match point_value(value).unwrap_or_else(|e| panic!("{e}, in key at frame {}", key.frame)) {
        PointValue::Named(k) => Anchor::Named(k),
        PointValue::At(p) => {
            for (k, v) in p.into_iter().enumerate() {
                let channel = note_point(name, end, k);
                if !segments.contains_key(&channel) {
                    segments.insert(channel.clone(), Channel::hold(key.frame, v));
                }
                targets.push((channel, v));
            }
            Anchor::At(p)
        }
    }
}

/// The note `name` shown at `key` as `kind` (in `color`, when given): made or changed, faded in from where it is.
fn show_note(
    key: &Key,
    name: &str,
    kind: NoteKind,
    color: Option<[u8; 3]>,
    state: &mut ShotState,
    segments: &mut BTreeMap<String, Channel>,
    targets: &mut Vec<(String, f32)>,
) {
    let shown = segments
        .get(&note_alpha(name))
        .is_some_and(|s| s.value(key.frame) > 0.0);
    match state.notes.iter_mut().find(|n| n.name == name) {
        Some(n) => {
            n.kind = kind;
            if color.is_some() {
                n.color = color;
            }
        }
        None => state.notes.push(StoryNote {
            name: name.to_string(),
            kind,
            color,
            alpha: 0.0,
        }),
    }
    if !shown {
        segments.insert(note_alpha(name), Channel::hold(key.frame, 0.0));
    }
    targets.push((note_alpha(name), 1.0));
}

/// Applies `key`'s arrows and labels to `state`: shown ones fade in over the
/// key's blend, hidden ones out, fixed points blend to their new place.
pub(super) fn apply_notes(
    key: &Key,
    state: &mut ShotState,
    segments: &mut BTreeMap<String, Channel>,
    targets: &mut Vec<(String, f32)>,
) {
    hide_notes(key, state, targets);
    let color = |c: &Option<String>| {
        c.as_deref().map(|c| {
            crate::utility::annotations::parse_color(c)
                .unwrap_or_else(|e| panic!("{e}, in key at frame {}", key.frame))
        })
    };
    if let Some(name) = key.arrow.as_deref().filter(|n| !n.is_empty()) {
        let old = state
            .notes
            .iter()
            .find(|n| n.name == name)
            .map(|n| n.kind.clone());
        let (old_from, old_to, old_width) = match old {
            Some(NoteKind::Arrow { from, to, width }) => (Some(from), Some(to), width),
            _ => (None, None, 3.0),
        };
        let from = key
            .arrow_from
            .as_ref()
            .map(|v| set_point(key, name, "from", v, segments, targets))
            .or(old_from)
            .expect("a new arrow names its ends (checked)");
        let to = key
            .arrow_to
            .as_ref()
            .map(|v| set_point(key, name, "to", v, segments, targets))
            .or(old_to)
            .expect("a new arrow names its ends (checked)");
        show_note(
            key,
            name,
            NoteKind::Arrow {
                from,
                to,
                width: key.arrow_width.unwrap_or(old_width),
            },
            color(&key.arrow_color),
            state,
            segments,
            targets,
        );
    }
    if let Some(set) = &key.arrows {
        show_note(
            key,
            set,
            NoteKind::Arrows,
            color(&key.arrow_color),
            state,
            segments,
            targets,
        );
    }
    if let Some(name) = key.label.as_deref().filter(|n| !n.is_empty()) {
        let old = state
            .notes
            .iter()
            .find(|n| n.name == name)
            .map(|n| n.kind.clone());
        let (old_at, old_text) = match old {
            Some(NoteKind::Label { at, text }) => (Some(at), Some(text)),
            _ => (None, None),
        };
        let at = key
            .label_at
            .as_ref()
            .map(|v| set_point(key, name, "at", v, segments, targets))
            .or(old_at)
            .expect("a new label names its point (checked)");
        let text = key
            .label_text
            .clone()
            .or(old_text)
            .expect("a new label names its text (checked)");
        show_note(
            key,
            name,
            NoteKind::Label { at, text },
            color(&key.label_color),
            state,
            segments,
            targets,
        );
    }
}

/// The notes at `frame`: each one's fade and its fixed points from their
/// channels; the faded-out ones are gone.
pub(super) fn finish_notes(
    state: &mut ShotState,
    segments: &BTreeMap<String, Channel>,
    frame: u64,
) {
    let point = |name: &str, end: &str, anchor: &mut Anchor| {
        if let Anchor::At(p) = anchor {
            for k in 0..3 {
                if let Some(s) = segments.get(&note_point(name, end, k)) {
                    p[k] = s.value(frame);
                }
            }
        }
    };
    for note in &mut state.notes {
        note.alpha = segments
            .get(&note_alpha(&note.name))
            .map_or(0.0, |s| s.value(frame));
        let name = note.name.clone();
        match &mut note.kind {
            NoteKind::Arrow { from, to, .. } => {
                point(&name, "from", from);
                point(&name, "to", to);
            }
            NoteKind::Label { at, .. } => point(&name, "at", at),
            NoteKind::Arrows => {}
        }
    }
    state.notes.retain(|n| n.alpha > 0.0);
}

/// The camera's channels, as keys' `camera` and motions name them: its
/// values' (src/camera.rs), by component (`camera.x`, …, `camera.yaw`, …).
pub(super) fn camera_channels() -> Vec<&'static str> {
    OrbitCamera::schema()
        .iter()
        .flat_map(|e| e.channels())
        .collect()
}

/// The free camera's channels (`freeCamera.on`, `freeCamera.yaw`, …).
pub(super) fn free_camera_channels() -> Vec<&'static str> {
    FreeCamera::schema()
        .iter()
        .flat_map(|e| e.channels())
        .collect()
}

/// The scene's size's channels (`frame.width`, `frame.height`).
pub(super) fn frame_channels() -> Vec<&'static str> {
    super::FrameSize::schema()
        .iter()
        .flat_map(|e| e.channels())
        .collect()
}

impl Timeline {
    pub fn load(path: &std::path::Path) -> Result<Timeline> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read timeline {}", path.display()))?;
        let mut timeline: Timeline = serde_json::from_str(&text)
            .with_context(|| format!("parse timeline {}", path.display()))?;
        timeline.keys.sort_by_key(|k| k.frame);
        timeline.validate()?;
        Ok(timeline)
    }

    pub fn validate(&self) -> Result<()> {
        self.validate_diagnostics().map(|_| ())
    }

    pub(crate) fn validate_diagnostics(&self) -> Result<Vec<(usize, CueProblem)>> {
        let mut checks = CueChecks::default();
        let mut warnings = Vec::new();
        for key in &self.keys {
            checks.declare(key);
        }
        for (index, key) in self.keys.iter().enumerate() {
            let at = |problem| TimelineCueProblem {
                index,
                frame: key.frame,
                source: key.source.clone(),
                problem,
            };
            checks.check(key).map_err(at)?;
            warnings.extend(
                checks
                    .take_warnings()
                    .into_iter()
                    .map(|warning| (index, warning)),
            );
            key.check_motions(&key.animate).map_err(at)?;
            for motion in &key.animate {
                if let Some(problem) = motion
                    .seconds()
                    .and_then(|s| crate::animation::seconds_problem(*s))
                {
                    return Err(at(CueProblem::new(
                        "story.motion.length",
                        "animate.seconds",
                        problem.into(),
                    ))
                    .into());
                }
            }
        }
        let mut frame = 0;
        for beat in &self.beats {
            if beat.start_frame != frame {
                bail!(
                    "beat {} starts at {} but the previous beat ended at {frame}",
                    beat.id,
                    beat.start_frame
                );
            }
            frame += beat.frames;
            if let Some(graph) = beat.diagram_graph() {
                let what = if beat.graph.is_some() {
                    "its frame graph"
                } else {
                    "its diagram"
                };
                if let Err(e) = graph
                    .map_err(str::to_string)
                    .and_then(|g| g.check_focus(&beat.focus))
                {
                    bail!("beat {}: {what}: {e}", beat.id);
                }
            }
        }
        if frame != self.total_frames {
            bail!(
                "beats cover {frame} frames, timeline declares {}",
                self.total_frames
            );
        }
        Ok(warnings)
    }

    pub fn beat_at(&self, frame: u64) -> (usize, &TimelineBeat) {
        let i = self
            .beats
            .partition_point(|b| b.start_frame + b.frames <= frame)
            .min(self.beats.len() - 1);
        (i, &self.beats[i])
    }

    /// The diagram frame `frame` shows, on its way from the last beat's: a
    /// beat after one of the same diagram moves from that beat's focus; the
    /// first beat of a diagram builds it up (src/diagram.rs `Transition`).
    pub fn diagram_at(&self, frame: u64) -> Option<crate::diagram::BeatDiagram<'_>> {
        let (i, beat) = self.beat_at(frame);
        let mut d = beat.beat_diagram()?;
        let seconds = frame.saturating_sub(beat.start_frame) as f32 / self.fps.max(1) as f32;
        d.transition = match i
            .checked_sub(1)
            .map(|k| &self.beats[k])
            .filter(|b| b.same_diagram(beat))
        {
            Some(before) => crate::diagram::Transition {
                from: Some(before.focus.as_slice()),
                seconds,
                built: true,
            },
            None => crate::diagram::Transition {
                from: None,
                seconds,
                built: false,
            },
        };
        Some(d)
    }

    pub fn chapter_at(&self, frame: u64) -> &TimelineChapter {
        self.chapters
            .iter()
            .rev()
            .find(|c| c.start_frame <= frame)
            .unwrap_or(&self.chapters[0])
    }

    /// Evaluate the shot state at a frame. Pure function of (timeline, frame).
    pub fn state_at(&self, frame: u64) -> ShotState {
        // Before the first key, the investigation's first demonstration.
        let mut state = ShotState::for_demo(investigation::current().demos()[0]);
        let mut segments: BTreeMap<String, Channel> = BTreeMap::new();
        let camera = camera_channels();
        Self::reset(&mut segments, &state, 0);
        for key in self.keys.iter().take_while(|k| k.frame <= frame) {
            Self::switch_demo(key, &mut state, &mut segments, &camera);
            Self::apply_cues(key, &mut state);
            let mut targets: Vec<(String, f32)> = key
                .targets()
                .unwrap_or_else(|e| panic!("{e}, in key at frame {}", key.frame))
                .to_vec();
            if key.image.as_deref() == Some("") {
                targets.extend(state.images.iter().map(|i| (image_alpha(&i.name), 0.0)));
            }
            targets.extend(key.image_hide.iter().map(|name| (image_alpha(name), 0.0)));
            Self::show_image(key, &mut state, &mut segments, &mut targets);
            // A newly lit term fades in from dark; unlighting fades the term out.
            apply_notes(key, &mut state, &mut segments, &mut targets);
            if let Some(term) = key.term {
                if term > 0 {
                    state.formula_term = term;
                    segments.insert(FORMULA_LIT.into(), Channel::hold(key.frame, 0.0));
                }
                targets.push((
                    FORMULA_LIT.into(),
                    if term > 0 {
                        1.0
                    } else {
                        0.0
                    },
                ));
            }
            Self::camera_targets(key, &mut targets);
            Self::blend_to(key, targets, &mut segments);
            self.animate(key, &mut segments);
        }
        self.channel_values(&mut state, &segments, &camera, frame);
        state.formula_lit = segments.get(FORMULA_LIT).map_or(0.0, |s| s.value(frame));
        if state.formula_lit <= 0.0 {
            state.formula_term = 0;
        }
        // Each image's fade and an instance's parameters; the faded-out ones are gone.
        Self::image_values(&mut state, &segments, frame);
        state.images.retain(|i| i.alpha > 0.0);
        finish_notes(&mut state, &segments, frame);
        // The running actions set the state last, in the order they started.
        self.run_actions(&mut state, frame);
        state
    }

    /// Every channel of `state` held at its value from frame `at` on.
    fn reset(segments: &mut BTreeMap<String, Channel>, state: &ShotState, at: u64) {
        for (name, value) in state.channels() {
            segments.insert(name, Channel::hold(at, value));
        }
    }

    /// A key that switches the demonstration: a new state, keeping the callout, the narration's images and formula
    /// card, and the camera's motion when the key itself moves the camera.
    fn switch_demo(
        key: &Key,
        state: &mut ShotState,
        segments: &mut BTreeMap<String, Channel>,
        camera: &[&'static str],
    ) {
        if let Some(demo) = key.demo {
            let callout = state.callout.clone();
            let (formula, term) = (state.formula.clone(), state.formula_term);
            let images = std::mem::take(&mut state.images);
            let notes = std::mem::take(&mut state.notes);
            let camera_now: Vec<f32> = camera
                .iter()
                .map(|c| segments[*c].value(key.frame))
                .collect();
            *state = ShotState::for_demo(demo);
            state.callout = callout;
            // The images and the formula card belong to the narration, not the demonstration.
            state.images = images;
            state.notes = notes;
            (state.formula, state.formula_term) = (formula, term);
            Self::reset(segments, state, key.frame);
            // A demo switch cuts parameters but keeps the camera continuous
            // when the key itself moves the camera.
            if key.camera.is_some() {
                for (i, c) in camera.iter().enumerate() {
                    segments.insert(c.to_string(), Channel::hold(key.frame, camera_now[i]));
                }
            }
        }
    }

    /// `key`'s cues that set the state at once: the record, the highlight, the panel, the callout, the listing, the
    /// formula, and the actions it stops and runs.
    fn apply_cues(key: &Key, state: &mut ShotState) {
        if let Some(record) = key.record {
            state.record = record;
        }
        if let Some(highlight) = &key.highlight {
            state.highlight = highlight.iter().map(|h| h.name().to_string()).collect();
        }
        if let Some(panel) = key.panel {
            state.panel = panel;
        }
        if let Some(callout) = &key.callout {
            state.callout = if callout.is_empty() {
                None
            } else {
                Some(callout.clone())
            };
        }
        if let Some(code) = &key.code {
            state.code = if code.is_empty() {
                None
            } else {
                Some(code.clone())
            };
        }
        if let Some(formula) = &key.formula {
            state.formula = if formula.is_empty() {
                None
            } else {
                Some(formula.clone())
            };
        }
        // A demonstration's switch above stops every action (its state is new).
        state.actions.retain(|a| !key.stop.contains(&a.key));
        if let Some(call) = &key.run {
            state.actions.retain(|a| a.key != call.name);
            state.actions.push(RunningAction {
                key: call.name.clone(),
                args: call.args.clone(),
                started: key.frame,
                elapsed: 0.0,
            });
        }
    }

    /// The image `key` shows: made (or given a new kind, its parameters starting over), titled, placed, marked, and
    /// faded in with its parameters.
    fn show_image(
        key: &Key,
        state: &mut ShotState,
        segments: &mut BTreeMap<String, Channel>,
        targets: &mut Vec<(String, f32)>,
    ) {
        if let Some(name) = key.image.as_deref().filter(|n| !n.is_empty()) {
            let made = key
                .image_from
                .as_ref()
                .map(|kind| ImageSource::View {
                    kind: kind.clone(),
                    params: BTreeMap::new(),
                })
                .or_else(|| {
                    key.image_graph.as_ref().map(|graph| ImageSource::Graph {
                        key: graph.clone(),
                        show: key.image_show.clone(),
                        params: BTreeMap::new(),
                    })
                });
            let shown = segments
                .get(&image_alpha(name))
                .is_some_and(|s| s.value(key.frame) > 0.0);
            match state.images.iter().position(|i| i.name == name) {
                // A new kind for an instance starts its parameters over.
                Some(i) => {
                    if let Some(made) = made.filter(|m| *m != state.images[i].source) {
                        let prefix = format!("image.{name}.param.");
                        segments.retain(|channel, _| !channel.starts_with(&prefix));
                        state.images[i].source = made;
                    }
                }
                None => {
                    let source = made.unwrap_or_else(|| {
                        image_source(name)
                            .unwrap_or_else(|e| panic!("{e}, in key at frame {}", key.frame))
                    });
                    state.images.push(StoryImage {
                        name: name.to_string(),
                        source,
                        title: None,
                        corner: Corner::TopRight,
                        alpha: 0.0,
                        click: None,
                        mark: [0.0; 4],
                        mark_alpha: 0.0,
                        mark_text: None,
                    });
                }
            }
            let image = state
                .images
                .iter_mut()
                .find(|i| i.name == name)
                .expect("the image just shown");
            if let Some(title) = &key.image_title {
                image.title = Some(title.clone());
            }
            // A later key may draw the same resource another way.
            if let (Some(show), ImageSource::Graph { show: shown, .. }) =
                (&key.image_show, &mut image.source)
            {
                *shown = Some(show.clone());
            }
            if let Some(corner) = key.image_corner {
                image.corner = corner;
            }
            if let Some(click) = &key.image_click {
                image.click = Some(click.clone());
            }
            if let Some(text) = &key.image_mark_text {
                image.mark_text = Some(text.clone()).filter(|t| !t.is_empty());
            }
            if !shown {
                segments.insert(image_alpha(name), Channel::hold(key.frame, 0.0));
                segments.insert(image_mark_alpha(name), Channel::hold(key.frame, 0.0));
            }
            Self::mark_image(key, name, segments, targets);
            targets.push((image_alpha(name), 1.0));
            for (param, value) in &key.image_params {
                let value = components(value)
                    .unwrap_or_else(|e| panic!("{e}, in key at frame {}", key.frame));
                targets.extend(
                    value
                        .into_iter()
                        .enumerate()
                        .map(|(k, v)| (image_param(name, param, k), v)),
                );
            }
        }
    }

    /// The mark `key` sets on the image `name`: none hides it; one not yet shown appears where it points, a shown one
    /// moves there.
    fn mark_image(
        key: &Key,
        name: &str,
        segments: &mut BTreeMap<String, Channel>,
        targets: &mut Vec<(String, f32)>,
    ) {
        match key.image_mark.as_deref() {
            Some([]) => targets.push((image_mark_alpha(name), 0.0)),
            Some(&[u0, v0, u1, v1]) => {
                // A mark not yet shown appears where it points; a shown one moves there.
                let marked = segments
                    .get(&image_mark_alpha(name))
                    .is_some_and(|s| s.value(key.frame) > 0.0);
                for (k, v) in [u0, v0, u1, v1].into_iter().enumerate() {
                    if !marked {
                        segments.insert(image_mark(name, k), Channel::hold(key.frame, v));
                    }
                    targets.push((image_mark(name, k), v));
                }
                targets.push((image_mark_alpha(name), 1.0));
            }
            Some(other) => panic!(
                "imageMark takes four numbers (u0,v0,u1,v1) or none, not {other:?}, in key at frame {}",
                key.frame
            ),
            None => {}
        }
    }

    /// The camera's channels `key` moves: its target, yaw, pitch, distance, and field of view.
    fn camera_targets(key: &Key, targets: &mut Vec<(String, f32)>) {
        if let Some(c) = &key.camera {
            if let Some(t) = c.target {
                let target = OrbitCamera::schema()
                    .iter()
                    .find(|e| e.key == "camera.target")
                    .expect("the camera's target");
                targets.extend(
                    target
                        .channels()
                        .into_iter()
                        .zip(t)
                        .map(|(n, v)| (n.to_string(), v)),
                );
            }
            for (name, v) in [
                ("camera.yaw", c.yaw),
                ("camera.pitch", c.pitch),
                ("camera.distance", c.distance),
                ("camera.fov", c.fov),
            ] {
                if let Some(v) = v {
                    targets.push((name.into(), v));
                }
            }
        }
    }

    /// Each target blends from its channel's value at `key` over the key's blend.
    fn blend_to(key: &Key, targets: Vec<(String, f32)>, segments: &mut BTreeMap<String, Channel>) {
        for (name, target) in targets {
            let start = segments
                .get(&name)
                .map(|s| s.value(key.frame))
                .unwrap_or(target);
            segments.insert(
                name,
                Channel::Blend(Segment {
                    from: key.frame,
                    blend: key.blend,
                    ease: key.ease,
                    start,
                    target,
                }),
            );
        }
    }

    /// `key`'s motions: each moves its parameters from where they are at the key.
    fn animate(&self, key: &Key, segments: &mut BTreeMap<String, Channel>) {
        for motion in &key.animate {
            let params = motion.params();
            let start = Rc::new(
                params
                    .iter()
                    .map(|p| segments[*p].value(key.frame))
                    .collect::<Vec<f32>>(),
            );
            let motion = Rc::new(motion.clone());
            for (index, name) in params.iter().enumerate() {
                segments.insert(
                    name.to_string(),
                    Channel::Motion {
                        motion: motion.clone(),
                        index,
                        from: key.frame,
                        fps: self.fps as f32,
                        start: start.clone(),
                    },
                );
            }
        }
    }

    /// The channels' values at `frame`: the cameras (the sway moving the one the viewport shows), the frame's size,
    /// the values, and other views' values.
    fn channel_values(
        &self,
        state: &mut ShotState,
        segments: &BTreeMap<String, Channel>,
        camera: &[&'static str],
        frame: u64,
    ) {
        let value = |name: &str| segments[name].value(frame);
        let mut keyed = state.camera;
        for name in camera {
            keyed.set(name, Value::F32(value(name)));
        }
        for name in free_camera_channels() {
            state.free_camera.set(name, Value::F32(value(name)));
        }
        for name in frame_channels() {
            state.frame.set(name, Value::F32(value(name)));
        }
        // The sway moves the camera the viewport shows: the free one while it is on
        // (so the main camera's cone holds still unless a cue moves it).
        let drift = segments.get("cameraDrift").map_or(0.0, |s| s.value(frame));
        let seconds = frame as f32 / self.fps as f32;
        if state.free_camera.on {
            state.camera = keyed;
            let free = camera_drift(state.free_camera.orbit(), 0.0, seconds, drift);
            state.free_camera.place(&free);
        } else {
            state.camera = camera_drift(keyed, 0.0, seconds, drift);
        }
        for (name, _) in state.value_channels() {
            state.set(&name, value(&name));
        }
        // Other views' values (src/view.rs), which a state starts without.
        for (name, segment) in segments
            .range("view.".to_string()..)
            .take_while(|(c, _)| c.starts_with("view."))
        {
            state.set(name, segment.value(frame));
        }
    }

    /// Each image's fade, its mark, and an instance's parameters at `frame`.
    fn image_values(state: &mut ShotState, segments: &BTreeMap<String, Channel>, frame: u64) {
        for image in &mut state.images {
            image.alpha = segments
                .get(&image_alpha(&image.name))
                .map_or(0.0, |s| s.value(frame));
            image.mark_alpha = segments
                .get(&image_mark_alpha(&image.name))
                .map_or(0.0, |s| s.value(frame));
            for k in 0..4 {
                image.mark[k] = segments
                    .get(&image_mark(&image.name, k))
                    .map_or(0.0, |s| s.value(frame));
            }
            if let ImageSource::View { params, .. } | ImageSource::Graph { params, .. } =
                &mut image.source
            {
                let prefix = format!("image.{}.param.", image.name);
                for (channel, segment) in segments
                    .range(prefix.clone()..)
                    .take_while(|(c, _)| c.starts_with(&prefix))
                {
                    let (param, k) = channel[prefix.len()..]
                        .rsplit_once('#')
                        .expect("a parameter's channel");
                    let k: usize = k.parse().expect("a component's index");
                    let value = params.entry(param.to_string()).or_default();
                    value.resize(value.len().max(k + 1), 0.0);
                    value[k] = segment.value(frame);
                }
            }
        }
    }

    /// The running actions set the state last, in the order they started: a script's action, else the
    /// investigation's code's.
    fn run_actions(&self, state: &mut ShotState, frame: u64) {
        let actions = investigation::current().actions();
        for a in &mut state.actions {
            a.elapsed = (frame - a.started) as f32 / self.fps as f32;
        }
        for action in state.actions.clone() {
            // A script's action, else the investigation's code's.
            if crate::script::runtime::run_action(&action, state).is_some() {
                continue;
            }
            let elapsed = action.elapsed;
            (actions.run)(&action, &mut ActionFrame { state, elapsed })
                .unwrap_or_else(|e| panic!("{e}, at frame {frame}"));
        }
    }
}
