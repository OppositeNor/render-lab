//! The shared shot state and the timeline that drives it.
//!
//! One `ShotState` decides everything a frame shows: the demonstration, the
//! camera, the selected record, the highlighted struct fields, the panel
//! mode, and every numeric emphasis parameter (overlay opacity, object
//! motion, roughness, ...). The recorder evaluates it from the timeline by
//! frame index; the viewer edits it interactively. Both render it the same way.
//!
//! Which demonstrations and parameters exist is the current investigation's
//! choice (src/investigation.rs); the lab only names a demonstration by
//! its key and reads parameters by name.

use crate::actions::{ActionCall, ActionFrame, RunningAction};
use crate::animation::{Ease, Motion};
use crate::utility::inset::{Corner, ImageSource, StoryImage};
use crate::camera::OrbitCamera;
use crate::investigation;
use crate::values::{self, Reflect, Value, ValueMap};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::rc::Rc;

/// A demonstration of the current investigation, by its key (`"cards"`).
/// Investigations define their own demonstrations and hand out these ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DemoId(&'static str);

impl DemoId {
    pub const fn new(key: &'static str) -> DemoId {
        DemoId(key)
    }

    pub fn key(self) -> &'static str {
        self.0
    }

    /// The demonstration's title in the current locale.
    pub fn title(self) -> &'static str {
        investigation::current().demo_title(self)
    }

    /// The current investigation's demonstration with this key.
    pub fn find(key: &str) -> Option<DemoId> {
        investigation::current().demos().iter().copied().find(|d| d.key() == key)
    }
}

impl Serialize for DemoId {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for DemoId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<DemoId, D::Error> {
        let key = String::deserialize(deserializer)?;
        DemoId::find(&key).ok_or_else(|| serde::de::Error::custom(format!("unknown demonstration `{key}` for {}", investigation::current().title())))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RecordRef {
    None,
    Object { index: u32 },
    Voxel { index: u32 },
    Brick { index: u32 },
    Card { index: u32 },
    Page { index: u32 },
    Ray,
    Camera,
    Probe { index: u32 },
    Radiance { index: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelMode {
    Declaration,
    Values,
    Memory,
    /// A Python pseudo-code listing (`ShotState::code`) in a wider panel.
    Code,
    Hidden,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ShotState {
    pub demo: DemoId,
    pub camera: OrbitCamera,
    pub record: RecordRef,
    pub highlight: Vec<String>,
    pub panel: PanelMode,
    /// Every value of the investigation by key (src/values.rs): a declared
    /// one in its type (a vector as one value), any other parameter as a
    /// number. `state["key"]` reads or writes one; `p`, `set`, `get`, and
    /// `set_value` also reach a vector's components; `view` and `store`
    /// convert them to and from a typed struct.
    pub values: ValueMap,
    pub callout: Option<String>,
    /// The pseudo-code listing (src/pseudocode.rs) shown in PanelMode::Code;
    /// `highlight` then names the tags of the lit lines.
    pub code: Option<String>,
    /// The formula card in the viewport: a formula tag (src/rich_text.rs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    /// The card's lit term (0: none), and how lit it is (0..1).
    #[serde(skip_serializing_if = "is_zero")]
    pub formula_term: usize,
    #[serde(skip_serializing_if = "is_zero_f32")]
    pub formula_lit: f32,
    /// The actions the story has started and not stopped (src/actions.rs),
    /// which have already set this state.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<RunningAction>,
    /// The image the story shows (`Key::image`), and how far it has faded in (0..1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<StoryImage>,
    #[serde(skip_serializing_if = "is_zero_f32")]
    pub image_alpha: f32,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

fn is_zero_f32(x: &f32) -> bool {
    *x == 0.0
}

impl ShotState {
    /// The current investigation's starting state for a demonstration.
    pub fn for_demo(demo: impl Into<DemoId>) -> ShotState {
        investigation::current().shot_state(demo.into())
    }

    /// Whether the story shows the image the investigation publishes as `key`
    /// (so its frame publishes it, in `FrameContent::images`).
    pub fn shows_image(&self, key: &str) -> bool {
        matches!(&self.image, Some(StoryImage { source: ImageSource::Published(k), .. }) if k == key)
    }

    /// A number by name: a value's (a flag is 0 or 1), a component's, a
    /// parameter's, or the camera's. Panics for a name the investigation lacks.
    pub fn p(&self, name: &str) -> f32 {
        if let Some(v) = self.values.get(name) {
            return v.f32();
        }
        self.try_get(name).unwrap_or_else(|| panic!("unknown shot parameter {name}")).f32()
    }

    /// Sets a number by name: a value (in its type: a flag on above 0.5, an
    /// index the nearest whole number), a component, a parameter, or the camera's.
    pub fn set(&mut self, name: &str, value: f32) {
        if self.camera.set(name, Value::F32(value)) {
            return;
        }
        let investigation = investigation::current();
        if let Some(demos) = investigation.demo_specs() {
            assert!(demos.set(&mut self.values, name, value), "unknown shot parameter {name}");
            return;
        }
        assert!(investigation.param_default(name).is_some(), "unknown shot parameter {name}");
        self.values.insert(name.to_string(), Value::F32(value));
    }

    /// The values as a typed struct (`#[derive(serde::Deserialize)]`, its
    /// fields named as the keys), converted on each call: for code that reads
    /// many values at once, not for a hot loop.
    pub fn view<T: serde::de::DeserializeOwned>(&self) -> T {
        // A choice by its option's name, so a field can be its enum.
        let json_of = |k: &str, v: &Value| match values::entry(k).and_then(|e| e.choices) {
            Some(c) => serde_json::json!(c.options.get(v.u32() as usize).map_or("", |o| o.name)),
            None => v.to_json(),
        };
        let json: serde_json::Map<String, serde_json::Value> = self.values.iter().map(|(k, v)| (k.clone(), json_of(k, v))).collect();
        serde_json::from_value(serde_json::Value::Object(json)).unwrap_or_else(|e| panic!("the shot's values as {}: {e}", std::any::type_name::<T>()))
    }

    /// Sets every value a typed struct holds (`#[derive(serde::Serialize)]`),
    /// by its fields' names (a choice's enum by its option's name).
    pub fn store<T: serde::Serialize>(&mut self, typed: &T) {
        let serde_json::Value::Object(fields) = serde_json::to_value(typed).expect("a struct of values") else { panic!("{} is not a struct", std::any::type_name::<T>()) };
        for (key, json) in fields {
            for (channel, number) in values::resolve(&key, &json).unwrap_or_else(|e| panic!("{e}")) {
                self.set(&channel, number);
            }
        }
    }

    /// The numbers of the values a timeline blends, by channel: each
    /// declared value's components, and each other parameter.
    fn value_channels(&self) -> Vec<(String, f32)> {
        let mut out = Vec::new();
        for (key, value) in self.values.iter() {
            match values::entry(key).filter(|e| e.key == key && !e.components.is_empty()) {
                Some(e) => out.extend(e.components.iter().zip(value.components()).map(|(c, n)| (c.to_string(), n))),
                None => out.push((key.clone(), value.f32())),
            }
        }
        out
    }

    #[cfg(test)]
    pub(crate) fn channels_for_tests(&self) -> Vec<(String, f32)> {
        self.channels()
    }

    /// Every channel a timeline blends: the camera's, then the values'.
    fn channels(&self) -> Vec<(String, f32)> {
        let camera = camera_channels().into_iter().map(|c| (c.to_string(), self.camera.get(c).expect("a camera channel").f32()));
        camera.chain(self.value_channels()).collect()
    }

    /// The value of `key` (src/values.rs): the camera's, a stored one, a
    /// vector's component (an `F32`), or None for no such key.
    pub fn try_get(&self, key: &str) -> Option<Value> {
        if let Some(v) = self.camera.get(key) {
            return Some(v);
        }
        if let Some(v) = self.values.get(key) {
            return Some(*v);
        }
        let entry = values::entry(key)?;
        let stored = self.values.get(entry.key)?;
        Some(Value::F32(stored.components()[values::component(entry, key)]))
    }

    /// The value of `key`; panics for a key the investigation does not have.
    pub fn get(&self, key: &str) -> Value {
        self.try_get(key).unwrap_or_else(|| panic!("unknown value {key}"))
    }

    /// Sets `key` to `value`, in the key's declared type: a camera value, a
    /// stored value (a vector whole), a component, or a parameter.
    pub fn set_value(&mut self, key: &str, value: impl Into<Value>) {
        let value = value.into();
        if self.camera.set(key, value) {
            return;
        }
        match values::entry(key).filter(|e| e.key == key) {
            Some(e) => {
                let value = Value::from_json(e.ty, &value.to_json()).unwrap_or_else(|| panic!("{key} is a {}, not a {}", e.ty.name(), value.ty().name()));
                self.values.insert(key.to_string(), value);
            }
            None => self.set(key, value.f32()),
        }
    }
}

/// `state["key"]`: a stored value (not a component, nor the camera's: use
/// `get`). Writing through it does not check the type; `set_value` does.
impl std::ops::Index<&str> for ShotState {
    type Output = Value;
    fn index(&self, key: &str) -> &Value {
        self.values.get(key).unwrap_or_else(|| panic!("no stored value `{key}` (a component or the camera's is read with `get`)"))
    }
}

impl std::ops::IndexMut<&str> for ShotState {
    fn index_mut(&mut self, key: &str) -> &mut Value {
        self.values.get_mut(key).unwrap_or_else(|| panic!("no stored value `{key}` (a component or the camera's is set with `set_value`)"))
    }
}

/// The fields a newly selected record highlights in the struct panel.
pub fn default_highlight(record: RecordRef) -> Vec<String> {
    let fields: &[&str] = match record {
        RecordRef::Object { .. } => &["localToWorld", "worldExtent"],
        RecordRef::Voxel { .. } => &["signedDistance", "encoded"],
        RecordRef::Brick { .. } => &["virtualBrick", "physicalBrick"],
        RecordRef::Card { .. } => &["tangentFrame", "pageIndex"],
        RecordRef::Page { .. } => &["physicalPage", "cardUVRect"],
        RecordRef::Ray => &["incidentDir", "normal", "reflectedDir"],
        RecordRef::Probe { .. } => &["position", "framesAccumulated"],
        _ => &[],
    };
    fields.iter().map(|s| s.to_string()).collect()
}

pub use crate::animation::camera_drift;

// ---------------------------------------------------------------------------
// Timeline produced by scripts/build_enhanced.py from the storyboard and
// measured narration. All times are absolute frame indices.

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub total_frames: u64,
    pub title: String,
    pub engine_label: String,
    /// Language of the captions and on-screen text.
    #[serde(default)]
    pub locale: crate::i18n::Locale,
    pub chapters: Vec<TimelineChapter>,
    pub beats: Vec<TimelineBeat>,
    pub keys: Vec<Key>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // endFrame is part of the timeline schema; frames are looked up by start.
pub struct TimelineChapter {
    pub number: u32,
    pub title: String,
    pub start_frame: u64,
    pub end_frame: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BeatKind {
    Scene,
    Mermaid,
    Sources,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineBeat {
    pub id: String,
    pub chapter: u32,
    pub kind: BeatKind,
    pub start_frame: u64,
    pub frames: u64,
    pub headline: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub captions: Vec<Caption>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Caption {
    pub start: u64,
    pub end: u64,
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraKey {
    pub target: Option<[f32; 3]>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    pub distance: Option<f32>,
    pub fov: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Key {
    pub frame: u64,
    #[serde(default)]
    pub blend: u64,
    /// How the key's values move over its blend.
    #[serde(default)]
    pub ease: Ease,
    /// Motions that start at the key (src/animation.rs), each driving its
    /// channels until a later key sets them.
    #[serde(default)]
    pub animate: Vec<Motion>,
    #[serde(default)]
    pub demo: Option<DemoId>,
    #[serde(default)]
    pub camera: Option<CameraKey>,
    #[serde(default)]
    pub record: Option<RecordRef>,
    #[serde(default)]
    pub highlight: Option<Vec<String>>,
    #[serde(default)]
    pub panel: Option<PanelMode>,
    /// The values it sets, as a story writes them (src/values.rs, `resolve`):
    /// a number, `true` or `false`, an option's name, or an array for a vector.
    #[serde(default)]
    pub params: BTreeMap<String, serde_json::Value>,
    /// `params` as the numbers of the channels they set, read on first use
    /// (`targets`).
    #[serde(skip)]
    resolved: std::sync::OnceLock<Result<Vec<(String, f32)>, String>>,
    /// `""` clears the callout.
    #[serde(default)]
    pub callout: Option<String>,
    /// Pseudo-code listing for PanelMode::Code; `""` clears it.
    #[serde(default)]
    pub code: Option<String>,
    /// The formula card: a formula tag (src/rich_text.rs); `""` clears it.
    #[serde(default)]
    pub formula: Option<String>,
    /// The card's lit term (0: none). It fades in over the key's `blend`.
    #[serde(default)]
    pub term: Option<usize>,
    /// An image to show in an inset (packages/rlstd/src/inset.rs): one the
    /// investigation publishes, or a PNG resource (`<package>.<key>`, or its
    /// package's own key); `""` fades it out. It fades in over `blend`.
    #[serde(default)]
    pub image: Option<String>,
    /// Its title: a text key of the investigation's package (the
    /// investigation's own title of it, or none, by default).
    #[serde(default)]
    pub image_title: Option<String>,
    /// Its corner (`top-right` by default).
    #[serde(default)]
    pub image_corner: Option<Corner>,
    /// An action it starts (src/actions.rs), in place of a running one of the
    /// same name.
    #[serde(default)]
    pub run: Option<ActionCall>,
    /// The actions it stops.
    #[serde(default)]
    pub stop: Vec<String>,
    /// Provenance for error messages (storyboard beat id).
    #[serde(default)]
    pub source: Option<String>,
}

impl Key {
    /// The numbers its values set, by channel (a vector by component, a flag
    /// as 1 or 0, an option as its index); why not, for a value the
    /// investigation does not declare or one that does not fit its type.
    pub fn targets(&self) -> Result<&[(String, f32)], String> {
        self.resolved
            .get_or_init(|| {
                let mut out = Vec::new();
                for (name, json) in &self.params {
                    out.extend(values::resolve(name, json)?);
                }
                Ok(out)
            })
            .as_deref()
            .map_err(Clone::clone)
    }

    /// Whether it sets `channel` (by itself, or as a component of a value it sets).
    fn sets(&self, channel: &str) -> bool {
        self.targets().is_ok_and(|t| t.iter().any(|(c, _)| c == channel))
    }
}

/// A channel's value over time: eased from its value at a key to the key's
/// target, or a motion's.
#[derive(Clone, Debug)]
enum Channel {
    Blend(Segment),
    /// The `index`th channel of a motion that started at `from` from `start`.
    Motion { motion: Rc<Motion>, index: usize, from: u64, fps: f32, start: Rc<Vec<f32>> },
}

impl Channel {
    fn hold(from: u64, value: f32) -> Channel {
        Channel::Blend(Segment::hold(from, value))
    }

    fn value(&self, frame: u64) -> f32 {
        match self {
            Channel::Blend(segment) => segment.value(frame),
            Channel::Motion { motion, index, from, fps, start } => motion.sample(frame.saturating_sub(*from) as f32 / fps, start)[*index],
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    from: u64,
    blend: u64,
    ease: Ease,
    start: f32,
    target: f32,
}

impl Segment {
    fn hold(from: u64, value: f32) -> Segment {
        Segment { from, blend: 0, ease: Ease::Smooth, start: value, target: value }
    }

    fn value(&self, frame: u64) -> f32 {
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
const FORMULA_LIT: &str = "formula.lit";
/// The story image's fade (`ShotState::image_alpha`).
const IMAGE_ALPHA: &str = "image.alpha";

/// What a story's `image=` names: an image the investigation publishes, else
/// a PNG resource its package names; why not, for neither.
fn image_source(reference: &str) -> Result<ImageSource, String> {
    let current = investigation::current();
    if current.images().iter().any(|i| i.key == reference) {
        return Ok(ImageSource::Published(reference.to_string()));
    }
    let package = current.package().unwrap_or("rlstd");
    let published: Vec<&str> = current.images().iter().map(|i| i.key).collect();
    let full = crate::resources::resolve(package, reference).map_err(|e| format!("image `{reference}`: {e}"))?;
    if crate::resources::find(&full).is_none() {
        return Err(format!("image `{reference}`: neither an image the investigation publishes ({}) nor a resource (`{full}`)", if published.is_empty() { "none".into() } else { published.join(", ") }));
    }
    Ok(ImageSource::Resource(full))
}

/// The camera's channels, as keys' `camera` and motions name them: its
/// values' (src/camera.rs), by component (`camera.x`, …, `camera.yaw`, …).
fn camera_channels() -> Vec<&'static str> {
    OrbitCamera::schema().iter().flat_map(|e| e.channels()).collect()
}

impl Timeline {
    pub fn load(path: &std::path::Path) -> Result<Timeline> {
        let text = std::fs::read_to_string(path).with_context(|| format!("read timeline {}", path.display()))?;
        let mut timeline: Timeline = serde_json::from_str(&text).with_context(|| format!("parse timeline {}", path.display()))?;
        timeline.keys.sort_by_key(|k| k.frame);
        timeline.validate()?;
        Ok(timeline)
    }

    pub fn validate(&self) -> Result<()> {
        for key in &self.keys {
            if let Err(e) = key.targets() {
                bail!("{e}, in key at frame {} ({:?})", key.frame, key.source);
            }
            for motion in &key.animate {
                if let Some(problem) = motion.problem() {
                    bail!("{problem}, in key at frame {} ({:?})", key.frame, key.source);
                }
                for name in motion.params() {
                    if investigation::current().param_default(name).is_none() && !camera_channels().contains(&name) {
                        bail!("unknown channel `{name}` in a motion of the key at frame {} ({:?})", key.frame, key.source);
                    }
                    if key.sets(name) {
                        bail!("`{name}` is both set and animated by the key at frame {} ({:?})", key.frame, key.source);
                    }
                }
            }
            if let Some(image) = key.image.as_deref().filter(|i| !i.is_empty()) {
                if let Err(e) = image_source(image) {
                    bail!("{e}, in key at frame {} ({:?})", key.frame, key.source);
                }
            }
            if let Some(call) = &key.run {
                if let Err(e) = investigation::current().actions().check(call) {
                    bail!("{e}, in key at frame {} ({:?})", key.frame, key.source);
                }
            }
            if let Some(code) = key.code.as_deref().filter(|c| !c.is_empty()) {
                if investigation::current().listing(code).is_none() {
                    bail!("unknown pseudo-code listing `{code}` in key at frame {} ({:?})", key.frame, key.source);
                }
            }
        }
        let mut frame = 0;
        for beat in &self.beats {
            if beat.start_frame != frame {
                bail!("beat {} starts at {} but the previous beat ended at {frame}", beat.id, beat.start_frame);
            }
            frame += beat.frames;
        }
        if frame != self.total_frames {
            bail!("beats cover {frame} frames, timeline declares {}", self.total_frames);
        }
        Ok(())
    }

    pub fn beat_at(&self, frame: u64) -> (usize, &TimelineBeat) {
        let i = self.beats.partition_point(|b| b.start_frame + b.frames <= frame).min(self.beats.len() - 1);
        (i, &self.beats[i])
    }

    pub fn chapter_at(&self, frame: u64) -> &TimelineChapter {
        self.chapters.iter().rev().find(|c| c.start_frame <= frame).unwrap_or(&self.chapters[0])
    }

    /// Evaluate the shot state at a frame. Pure function of (timeline, frame).
    pub fn state_at(&self, frame: u64) -> ShotState {
        // Before the first key, the investigation's first demonstration.
        let mut state = ShotState::for_demo(investigation::current().demos()[0]);
        let mut segments: BTreeMap<String, Channel> = BTreeMap::new();
        let camera = camera_channels();
        let reset = |segments: &mut BTreeMap<String, Channel>, state: &ShotState, at: u64| {
            for (name, value) in state.channels() {
                segments.insert(name, Channel::hold(at, value));
            }
        };
        reset(&mut segments, &state, 0);
        for key in self.keys.iter().take_while(|k| k.frame <= frame) {
            if let Some(demo) = key.demo {
                let callout = state.callout.clone();
                let (formula, term) = (state.formula.clone(), state.formula_term);
                let camera_now: Vec<f32> = camera.iter().map(|c| segments[*c].value(key.frame)).collect();
                state = ShotState::for_demo(demo);
                state.callout = callout;
                // The formula card belongs to the narration, not the demonstration.
                (state.formula, state.formula_term) = (formula, term);
                reset(&mut segments, &state, key.frame);
                // A demo switch cuts parameters but keeps the camera continuous
                // when the key itself moves the camera.
                if key.camera.is_some() {
                    for (i, c) in camera.iter().enumerate() {
                        segments.insert(c.to_string(), Channel::hold(key.frame, camera_now[i]));
                    }
                }
            }
            if let Some(record) = key.record {
                state.record = record;
            }
            if let Some(highlight) = &key.highlight {
                state.highlight = highlight.clone();
            }
            if let Some(panel) = key.panel {
                state.panel = panel;
            }
            if let Some(callout) = &key.callout {
                state.callout = if callout.is_empty() { None } else { Some(callout.clone()) };
            }
            if let Some(code) = &key.code {
                state.code = if code.is_empty() { None } else { Some(code.clone()) };
            }
            if let Some(formula) = &key.formula {
                state.formula = if formula.is_empty() { None } else { Some(formula.clone()) };
            }
            // A demonstration's switch above stops every action (its state is new).
            state.actions.retain(|a| !key.stop.contains(&a.key));
            if let Some(call) = &key.run {
                state.actions.retain(|a| a.key != call.name);
                state.actions.push(RunningAction { key: call.name.clone(), args: call.args.clone(), started: key.frame, elapsed: 0.0 });
            }
            let mut targets: Vec<(String, f32)> = key.targets().unwrap_or_else(|e| panic!("{e}, in key at frame {}", key.frame)).to_vec();
            match key.image.as_deref() {
                Some("") => targets.push((IMAGE_ALPHA.into(), 0.0)),
                Some(image) => {
                    let source = image_source(image).unwrap_or_else(|e| panic!("{e}, in key at frame {}", key.frame));
                    state.image = Some(StoryImage { source, title: key.image_title.clone(), corner: key.image_corner.unwrap_or(Corner::TopRight) });
                    segments.insert(IMAGE_ALPHA.into(), Channel::hold(key.frame, 0.0));
                    targets.push((IMAGE_ALPHA.into(), 1.0));
                }
                None => {}
            }
            // A newly lit term fades in from dark; unlighting fades the term out.
            if let Some(term) = key.term {
                if term > 0 {
                    state.formula_term = term;
                    segments.insert(FORMULA_LIT.into(), Channel::hold(key.frame, 0.0));
                }
                targets.push((FORMULA_LIT.into(), if term > 0 { 1.0 } else { 0.0 }));
            }
            if let Some(c) = &key.camera {
                if let Some(t) = c.target {
                    let target = OrbitCamera::schema().iter().find(|e| e.key == "camera.target").expect("the camera's target");
                    targets.extend(target.channels().into_iter().zip(t).map(|(n, v)| (n.to_string(), v)));
                }
                for (name, v) in [("camera.yaw", c.yaw), ("camera.pitch", c.pitch), ("camera.distance", c.distance), ("camera.fov", c.fov)] {
                    if let Some(v) = v {
                        targets.push((name.into(), v));
                    }
                }
            }
            for (name, target) in targets {
                let start = segments.get(&name).map(|s| s.value(key.frame)).unwrap_or(target);
                segments.insert(name, Channel::Blend(Segment { from: key.frame, blend: key.blend, ease: key.ease, start, target }));
            }
            for motion in &key.animate {
                let params = motion.params();
                let start = Rc::new(params.iter().map(|p| segments[*p].value(key.frame)).collect::<Vec<f32>>());
                let motion = Rc::new(motion.clone());
                for (index, name) in params.iter().enumerate() {
                    segments.insert(name.to_string(), Channel::Motion { motion: motion.clone(), index, from: key.frame, fps: self.fps as f32, start: start.clone() });
                }
            }
        }
        let value = |name: &str| segments[name].value(frame);
        let mut keyed = state.camera;
        for name in &camera {
            keyed.set(name, Value::F32(value(name)));
        }
        state.camera = camera_drift(keyed, 0.0, frame as f32 / self.fps as f32, segments.get("cameraDrift").map_or(0.0, |s| s.value(frame)));
        for (name, _) in state.value_channels() {
            state.set(&name, value(&name));
        }
        state.formula_lit = segments.get(FORMULA_LIT).map_or(0.0, |s| s.value(frame));
        if state.formula_lit <= 0.0 {
            state.formula_term = 0;
        }
        state.image_alpha = segments.get(IMAGE_ALPHA).map_or(0.0, |s| s.value(frame));
        if state.image_alpha <= 0.0 {
            state.image = None;
        }
        // The running actions set the state last, in the order they started.
        let actions = investigation::current().actions();
        for a in &mut state.actions {
            a.elapsed = (frame - a.started) as f32 / self.fps as f32;
        }
        for action in state.actions.clone() {
            let elapsed = action.elapsed;
            (actions.run)(&action, &mut ActionFrame { state: &mut state, elapsed }).unwrap_or_else(|e| panic!("{e}, at frame {frame}"));
        }
        state
    }
}
