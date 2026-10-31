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
use crate::utility::annotations::{Anchor, AnnotationKind, NoteKind, StoryNote};
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
    Camera,
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
    /// The images the story shows (`Key::image`), in the order it showed them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<StoryImage>,
    /// The arrows, labels, and arrow sets the story shows (`Key::arrow`, `label`, `arrows`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<StoryNote>,
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
    pub fn shows_arrows(&self, key: &str) -> bool {
        self.notes.iter().any(|n| n.kind == NoteKind::Arrows && n.name == key)
    }

    /// Whether the story shows the image the investigation publishes as `key`
    /// (so its frame publishes it, in `FrameContent::images`).
    pub fn shows_image(&self, key: &str) -> bool {
        self.images.iter().any(|i| matches!(&i.source, ImageSource::Published(k) if k == key))
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

/// The type of a beat's scene: the lab's 3D frame, or a 2D page in its
/// place, the beat's image (a diagram) or, without one, its sources card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum SceneType {
    #[serde(rename = "3d")]
    ThreeD,
    #[serde(rename = "2d")]
    TwoD,
}

impl TimelineBeat {
    /// A 2D scene that shows an image (a diagram), not its sources card.
    pub fn shows_image(&self) -> bool {
        self.kind == SceneType::TwoD && self.image.is_some()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineBeat {
    pub id: String,
    pub chapter: u32,
    pub kind: SceneType,
    pub start_frame: u64,
    pub frames: u64,
    pub headline: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub captions: Vec<Caption>,
    /// A 2D scene that shows the investigation's package's legal notices
    /// (`[[notice]]`; the video builder ends a video with one).
    #[serde(default)]
    pub notices: bool,
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
    /// An image to show in an inset (packages/rlstd/src/inset.rs), by name:
    /// one the investigation publishes, a PNG resource (`<package>.<key>`, or
    /// its package's own key), or, with `image_from`, an instance of an image
    /// kind (src/views.rs) of that name. It fades in over `blend`; `""` fades
    /// out every image.
    #[serde(default)]
    pub image: Option<String>,
    /// The image kind of the package that `image` is an instance of.
    #[serde(default)]
    pub image_from: Option<String>,
    /// The instance's parameters it sets (`image.<param>=…`), blended over `blend`.
    #[serde(default)]
    pub image_params: BTreeMap<String, serde_json::Value>,
    /// The images it fades out.
    #[serde(default)]
    pub image_hide: Vec<String>,
    /// The click handler a click on `image`'s picture runs (src/actions.rs, `#[click]`).
    #[serde(default)]
    pub image_click: Option<String>,
    /// An arrow to show, by name (packages/rlstd/src/annotations.rs); `""`
    /// fades out every arrow and set. A new one names both ends.
    #[serde(default)]
    pub arrow: Option<String>,
    /// Its ends: three numbers (blended over `blend`) or a point the
    /// investigation publishes, by key.
    #[serde(default)]
    pub arrow_from: Option<serde_json::Value>,
    #[serde(default)]
    pub arrow_to: Option<serde_json::Value>,
    /// Its colour (`#rrggbb`; a set's arrows keep their own without one).
    #[serde(default)]
    pub arrow_color: Option<String>,
    /// Its width, in reference pixels (3 by default).
    #[serde(default)]
    pub arrow_width: Option<f32>,
    /// An arrow set the investigation publishes, to show.
    #[serde(default)]
    pub arrows: Option<String>,
    /// The arrows and sets it fades out.
    #[serde(default)]
    pub arrow_hide: Vec<String>,
    /// A label to show, by name; `""` fades out every label. A new one names
    /// its point and its text.
    #[serde(default)]
    pub label: Option<String>,
    /// Its point: three numbers or a published point's key.
    #[serde(default)]
    pub label_at: Option<serde_json::Value>,
    /// Its text: a key of the investigation's package's text.
    #[serde(default)]
    pub label_text: Option<String>,
    #[serde(default)]
    pub label_color: Option<String>,
    /// The labels it fades out.
    #[serde(default)]
    pub label_hide: Vec<String>,
    /// Its title: a text key of the investigation's package (the
    /// investigation's own title of it, or none, by default).
    #[serde(default)]
    pub image_title: Option<String>,
    /// Its corner (`top-right` by default).
    #[serde(default)]
    pub image_corner: Option<Corner>,
    /// A region of it to point out (`imageMark=u0,v0,u1,v1`, fractions of the
    /// picture from its top left): outlined, the rest dimmed, faded in over
    /// `blend` and moved there from a region already shown; empty: faded out.
    #[serde(default)]
    pub image_mark: Option<Vec<f32>>,
    /// The mark's label: a text key of the investigation's package (empty: none).
    #[serde(default)]
    pub image_mark_text: Option<String>,
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
/// The channel of the fade of the story's image `name`.
fn image_alpha(name: &str) -> String {
    format!("image.{name}.alpha")
}

/// The channel of component `k` of parameter `param` of the story's image instance `name`.
fn image_mark(name: &str, k: usize) -> String {
    format!("image.{name}.mark#{k}")
}

fn image_mark_alpha(name: &str) -> String {
    format!("image.{name}.markAlpha")
}

fn image_param(name: &str, param: &str, k: usize) -> String {
    format!("image.{name}.param.{param}#{k}")
}

/// A parameter's value as a key holds it: a number, a flag, or numbers.
fn components(value: &serde_json::Value) -> Result<Vec<f32>, String> {
    match value {
        serde_json::Value::Bool(b) => Ok(vec![if *b { 1.0 } else { 0.0 }]),
        serde_json::Value::Array(a) => a.iter().map(|v| v.as_f64().map(|n| n as f32).ok_or_else(|| format!("{value} is not numbers"))).collect(),
        v => v.as_f64().map(|n| vec![n as f32]).ok_or_else(|| format!("{value} is not a number or numbers")),
    }
}

/// Why `key`'s image cannot show: an image or a kind the investigation does
/// not have, or parameters its kind does not take. `instances` are the
/// instances made by the keys before it, by name.
/// The channel of the fade of the story's note `name`.
fn note_alpha(name: &str) -> String {
    format!("note.{name}.alpha")
}

/// The channel of component `k` of a fixed point `end` (`from`, `to`, `at`) of note `name`.
fn note_point(name: &str, end: &str, k: usize) -> String {
    format!("note.{name}.{end}#{k}")
}

/// A point as a key holds it: three numbers, or a published point's key.
enum PointValue {
    At([f32; 3]),
    Named(String),
}

fn point_value(value: &serde_json::Value) -> Result<PointValue, String> {
    if let Some(key) = value.as_str() {
        let declared = investigation::current().annotations().iter().any(|a| a.key == key && a.kind == AnnotationKind::Point);
        if !declared {
            let known: Vec<&str> = investigation::current().annotations().iter().filter(|a| a.kind == AnnotationKind::Point).map(|a| a.key).collect();
            return Err(format!("no point `{key}` (the investigation's: {})", if known.is_empty() { "none".into() } else { known.join(", ") }));
        }
        return Ok(PointValue::Named(key.to_string()));
    }
    let n = components(value)?;
    let [x, y, z] = n[..] else { return Err(format!("{value} is not a point (three numbers, or a point's key)")) };
    Ok(PointValue::At([x, y, z]))
}

/// Why `key`'s arrows and labels cannot show: a point, a set, or a colour
/// the investigation does not have, or a new one without its ends, point, or
/// text. `made` are the notes the keys before it made.
fn check_notes(key: &Key, made: &mut BTreeMap<String, &'static str>) -> Result<(), String> {
    for (field, value) in [("arrowFrom", &key.arrow_from), ("arrowTo", &key.arrow_to), ("labelAt", &key.label_at)] {
        if let Some(v) = value {
            point_value(v).map_err(|e| format!("{field}: {e}"))?;
        }
    }
    for color in [&key.arrow_color, &key.label_color].into_iter().flatten() {
        crate::utility::annotations::parse_color(color)?;
    }
    if let Some(name) = key.arrow.as_deref().filter(|n| !n.is_empty()) {
        if !made.contains_key(name) && (key.arrow_from.is_none() || key.arrow_to.is_none()) {
            return Err(format!("the new arrow `{name}` names both its ends (arrowFrom, arrowTo)"));
        }
        made.insert(name.to_string(), "arrow");
    } else if key.arrow_from.is_some() || key.arrow_to.is_some() || key.arrow_width.is_some() || (key.arrow_color.is_some() && key.arrows.is_none()) {
        return Err("arrowFrom, arrowTo, arrowColor, and arrowWidth go with the arrow they set (`arrow=<name>`)".into());
    }
    if let Some(set) = &key.arrows {
        let declared = investigation::current().annotations().iter().any(|a| a.key == set && a.kind == AnnotationKind::ArrowSet);
        if !declared {
            let known: Vec<&str> = investigation::current().annotations().iter().filter(|a| a.kind == AnnotationKind::ArrowSet).map(|a| a.key).collect();
            return Err(format!("no arrow set `{set}` (the investigation's: {})", if known.is_empty() { "none".into() } else { known.join(", ") }));
        }
    }
    if let Some(name) = key.label.as_deref().filter(|n| !n.is_empty()) {
        if !made.contains_key(name) && (key.label_at.is_none() || key.label_text.is_none()) {
            return Err(format!("the new label `{name}` names its point and its text (labelAt, labelText)"));
        }
        made.insert(name.to_string(), "label");
    } else if key.label_at.is_some() || key.label_text.is_some() || key.label_color.is_some() {
        return Err("labelAt, labelText, and labelColor go with the label they set (`label=<name>`)".into());
    }
    Ok(())
}

/// Applies `key`'s arrows and labels to `state`: shown ones fade in over the
/// key's blend, hidden ones out, fixed points blend to their new place.
fn apply_notes(key: &Key, state: &mut ShotState, segments: &mut BTreeMap<String, Channel>, targets: &mut Vec<(String, f32)>) {
    let hide = |state: &ShotState, targets: &mut Vec<(String, f32)>, which: &dyn Fn(&StoryNote) -> bool| {
        targets.extend(state.notes.iter().filter(|n| which(n)).map(|n| (note_alpha(&n.name), 0.0)));
    };
    if key.arrow.as_deref() == Some("") {
        hide(state, targets, &|n| !matches!(n.kind, NoteKind::Label { .. }));
    }
    if key.label.as_deref() == Some("") {
        hide(state, targets, &|n| matches!(n.kind, NoteKind::Label { .. }));
    }
    targets.extend(key.arrow_hide.iter().chain(&key.label_hide).map(|name| (note_alpha(name), 0.0)));
    let color = |c: &Option<String>| c.as_deref().map(|c| crate::utility::annotations::parse_color(c).unwrap_or_else(|e| panic!("{e}, in key at frame {}", key.frame)));
    // A point's anchor: published, or fixed and blended over its channels.
    let set_point = |name: &str, end: &str, value: &serde_json::Value, segments: &mut BTreeMap<String, Channel>, targets: &mut Vec<(String, f32)>| -> Anchor {
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
    };
    let show = |name: &str, kind: NoteKind, color: Option<[u8; 3]>, state: &mut ShotState, segments: &mut BTreeMap<String, Channel>, targets: &mut Vec<(String, f32)>| {
        let shown = segments.get(&note_alpha(name)).is_some_and(|s| s.value(key.frame) > 0.0);
        match state.notes.iter_mut().find(|n| n.name == name) {
            Some(n) => {
                n.kind = kind;
                if color.is_some() {
                    n.color = color;
                }
            }
            None => state.notes.push(StoryNote { name: name.to_string(), kind, color, alpha: 0.0 }),
        }
        if !shown {
            segments.insert(note_alpha(name), Channel::hold(key.frame, 0.0));
        }
        targets.push((note_alpha(name), 1.0));
    };
    if let Some(name) = key.arrow.as_deref().filter(|n| !n.is_empty()) {
        let old = state.notes.iter().find(|n| n.name == name).map(|n| n.kind.clone());
        let (old_from, old_to, old_width) = match old {
            Some(NoteKind::Arrow { from, to, width }) => (Some(from), Some(to), width),
            _ => (None, None, 3.0),
        };
        let from = key.arrow_from.as_ref().map(|v| set_point(name, "from", v, segments, targets)).or(old_from).expect("a new arrow names its ends (checked)");
        let to = key.arrow_to.as_ref().map(|v| set_point(name, "to", v, segments, targets)).or(old_to).expect("a new arrow names its ends (checked)");
        show(name, NoteKind::Arrow { from, to, width: key.arrow_width.unwrap_or(old_width) }, color(&key.arrow_color), state, segments, targets);
    }
    if let Some(set) = &key.arrows {
        show(set, NoteKind::Arrows, color(&key.arrow_color), state, segments, targets);
    }
    if let Some(name) = key.label.as_deref().filter(|n| !n.is_empty()) {
        let old = state.notes.iter().find(|n| n.name == name).map(|n| n.kind.clone());
        let (old_at, old_text) = match old {
            Some(NoteKind::Label { at, text }) => (Some(at), Some(text)),
            _ => (None, None),
        };
        let at = key.label_at.as_ref().map(|v| set_point(name, "at", v, segments, targets)).or(old_at).expect("a new label names its point (checked)");
        let text = key.label_text.clone().or(old_text).expect("a new label names its text (checked)");
        show(name, NoteKind::Label { at, text }, color(&key.label_color), state, segments, targets);
    }
}

/// The notes at `frame`: each one's fade and its fixed points from their
/// channels; the faded-out ones are gone.
fn finish_notes(state: &mut ShotState, segments: &BTreeMap<String, Channel>, frame: u64) {
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
        note.alpha = segments.get(&note_alpha(&note.name)).map_or(0.0, |s| s.value(frame));
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

fn check_image(key: &Key, instances: &mut BTreeMap<String, &'static crate::views::ViewKind>) -> Result<(), String> {
    let Some(name) = key.image.as_deref().filter(|n| !n.is_empty()) else {
        return if key.image_from.is_some() || !key.image_params.is_empty() || key.image_click.is_some() { Err("imageFrom, imageClick, and image.<param> go with the image they make or set (`image=<name>`)".into()) } else { Ok(()) };
    };
    if let Some(click) = &key.image_click {
        investigation::current().actions().check_click(click)?;
    }
    let package = investigation::current().package().unwrap_or("rlstd");
    if let Some(from) = &key.image_from {
        instances.insert(name.to_string(), crate::views::kind(package, from)?);
    }
    match instances.get(name) {
        Some(kind) => key.image_params.iter().try_for_each(|(param, value)| kind.check(param, &components(value)?)),
        None if !key.image_params.is_empty() => Err(format!("image.<param> sets the parameters of an image kind's instance; `{name}` is not one (make it with imageFrom=<kind>)")),
        None => image_source(name).map(|_| ()),
    }
}

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
        let mut instances = BTreeMap::new();
        let mut notes = BTreeMap::new();
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
            if let Err(e) = check_notes(key, &mut notes) {
                bail!("{e}, in key at frame {} ({:?})", key.frame, key.source);
            }
            if let Err(e) = check_image(key, &mut instances) {
                bail!("{e}, in key at frame {} ({:?})", key.frame, key.source);
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
                let images = std::mem::take(&mut state.images);
                let notes = std::mem::take(&mut state.notes);
                let camera_now: Vec<f32> = camera.iter().map(|c| segments[*c].value(key.frame)).collect();
                state = ShotState::for_demo(demo);
                state.callout = callout;
                // The images and the formula card belong to the narration, not the demonstration.
                state.images = images;
                state.notes = notes;
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
            if key.image.as_deref() == Some("") {
                targets.extend(state.images.iter().map(|i| (image_alpha(&i.name), 0.0)));
            }
            targets.extend(key.image_hide.iter().map(|name| (image_alpha(name), 0.0)));
            if let Some(name) = key.image.as_deref().filter(|n| !n.is_empty()) {
                let made = key.image_from.as_ref().map(|kind| ImageSource::View { kind: kind.clone(), params: BTreeMap::new() });
                let shown = segments.get(&image_alpha(name)).is_some_and(|s| s.value(key.frame) > 0.0);
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
                        let source = made.unwrap_or_else(|| image_source(name).unwrap_or_else(|e| panic!("{e}, in key at frame {}", key.frame)));
                        state.images.push(StoryImage { name: name.to_string(), source, title: None, corner: Corner::TopRight, alpha: 0.0, click: None, mark: [0.0; 4], mark_alpha: 0.0, mark_text: None });
                    }
                }
                let image = state.images.iter_mut().find(|i| i.name == name).expect("the image just shown");
                if let Some(title) = &key.image_title {
                    image.title = Some(title.clone());
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
                match key.image_mark.as_deref() {
                    Some([]) => targets.push((image_mark_alpha(name), 0.0)),
                    Some(&[u0, v0, u1, v1]) => {
                        // A mark not yet shown appears where it points; a shown one moves there.
                        let marked = segments.get(&image_mark_alpha(name)).is_some_and(|s| s.value(key.frame) > 0.0);
                        for (k, v) in [u0, v0, u1, v1].into_iter().enumerate() {
                            if !marked {
                                segments.insert(image_mark(name, k), Channel::hold(key.frame, v));
                            }
                            targets.push((image_mark(name, k), v));
                        }
                        targets.push((image_mark_alpha(name), 1.0));
                    }
                    Some(other) => panic!("imageMark takes four numbers (u0,v0,u1,v1) or none, not {other:?}, in key at frame {}", key.frame),
                    None => {}
                }
                targets.push((image_alpha(name), 1.0));
                for (param, value) in &key.image_params {
                    let value = components(value).unwrap_or_else(|e| panic!("{e}, in key at frame {}", key.frame));
                    targets.extend(value.into_iter().enumerate().map(|(k, v)| (image_param(name, param, k), v)));
                }
            }
            // A newly lit term fades in from dark; unlighting fades the term out.
            apply_notes(key, &mut state, &mut segments, &mut targets);
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
        // Each image's fade and an instance's parameters; the faded-out ones are gone.
        for image in &mut state.images {
            image.alpha = segments.get(&image_alpha(&image.name)).map_or(0.0, |s| s.value(frame));
            image.mark_alpha = segments.get(&image_mark_alpha(&image.name)).map_or(0.0, |s| s.value(frame));
            for k in 0..4 {
                image.mark[k] = segments.get(&image_mark(&image.name, k)).map_or(0.0, |s| s.value(frame));
            }
            if let ImageSource::View { params, .. } = &mut image.source {
                let prefix = format!("image.{}.param.", image.name);
                for (channel, segment) in segments.range(prefix.clone()..).take_while(|(c, _)| c.starts_with(&prefix)) {
                    let (param, k) = channel[prefix.len()..].rsplit_once('#').expect("a parameter's channel");
                    let k: usize = k.parse().expect("a component's index");
                    let value = params.entry(param.to_string()).or_default();
                    value.resize(value.len().max(k + 1), 0.0);
                    value[k] = segment.value(frame);
                }
            }
        }
        state.images.retain(|i| i.alpha > 0.0);
        finish_notes(&mut state, &segments, frame);
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
