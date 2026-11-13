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
use crate::camera::{FreeCamera, OrbitCamera};
use crate::investigation;
use crate::utility::annotations::{Anchor, AnnotationKind, NoteKind, StoryNote};
use crate::utility::inset::{Corner, ImageSource, StoryImage};
use crate::values::{self, Reflect, Value, ValueMap};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::rc::Rc;

mod check;
mod eval;
mod timeline;
pub(crate) use check::*;
use eval::*;
pub use timeline::*;

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
        investigation::current()
            .demos()
            .iter()
            .copied()
            .find(|d| d.key() == key)
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
        DemoId::find(&key).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "unknown demonstration `{key}` for {}",
                investigation::current().title()
            ))
        })
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
    /// The frame's cost: GPU time per pass, measured as it renders (src/timing.rs);
    /// `highlight` names the passes to call out.
    Performance,
    /// The knowledge graph and the investigation's audience (src/knowledge.rs; the Prerequisites tab).
    Knowledge,
    /// The frame's render graph (src/graph/): its passes in order, the
    /// resources each reads and writes, and their GPU times; a click on a
    /// pass's resource inspects it. `highlight` names passes to call out.
    Graph,
    /// The same graph as a DAG in the wider panel: each pass a card holding
    /// its result as it stood right after it (the inspector, src/graph/
    /// `FrameGraph::result_of`), and arrows for what flows between them.
    Dag,
    /// Every resource of the frame as a card holding its state at the end
    /// of the frame, with its writers and readers (the wider panel).
    Resources,
    Hidden,
}

impl PanelMode {
    /// Every mode, in declaration order (the story vocabulary's options, lab/src/script/keys.rs).
    pub const ALL: [PanelMode; 10] = [
        PanelMode::Declaration,
        PanelMode::Values,
        PanelMode::Memory,
        PanelMode::Code,
        PanelMode::Performance,
        PanelMode::Knowledge,
        PanelMode::Graph,
        PanelMode::Dag,
        PanelMode::Resources,
        PanelMode::Hidden,
    ];

    /// It takes the wider panel (the code panel's width, `ViewerLayout::code_panel_width`): the pseudo-code,
    /// the knowledge graph, the DAG, the resources' cards.
    pub fn wide(self) -> bool {
        matches!(
            self,
            PanelMode::Code | PanelMode::Knowledge | PanelMode::Dag | PanelMode::Resources
        )
    }

    /// It shows the frame graph (src/graph/): its passes, the DAG, or the resources' cards.
    pub fn shows_graph(self) -> bool {
        matches!(
            self,
            PanelMode::Graph | PanelMode::Dag | PanelMode::Resources
        )
    }

    /// It shows the frame's passes with their GPU times: the frame is timed while it shows (src/timing.rs),
    /// and `highlight` names passes.
    pub fn shows_passes(self) -> bool {
        self == PanelMode::Performance || self.shows_graph()
    }

    /// It analyses the frame or the story (the performance, the knowledge graph, the frame graph): a feature's
    /// panel that yields (`FeatureLayout::yields`) gives way to it.
    pub fn analysis(self) -> bool {
        self == PanelMode::Knowledge || self.shows_passes()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ShotState {
    pub demo: DemoId,
    /// The main camera: what the investigation's frame follows.
    pub camera: OrbitCamera,
    /// The camera the viewport shows the scene from while it is on
    /// (src/camera.rs `FreeCamera`); the main camera otherwise.
    #[serde(skip_serializing_if = "FreeCamera::is_off")]
    pub free_camera: FreeCamera,
    /// The scene's own size, when it does not follow the viewport's (`FrameSize`).
    #[serde(skip_serializing_if = "FrameSize::follows_viewport")]
    pub frame: FrameSize,
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
    /// The formula card in the viewport: a formula, `{ FORMULA(…) }` (src/rich_text.rs).
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

/// The scene's own size in pixels (`frame.width`, `frame.height`). At 0, the default, the scene renders at the
/// viewport's size; at a size, it renders at that size and is shown fitted into the viewport (src/ui/mod.rs
/// `Layout::with_frame`), so what depends on its pixels (a pass at half its size, a depth pyramid's cells) is the
/// same in every window and beside every panel. A demonstration may name one (`frame = [w, h]`, src/demo.rs); a
/// story sets it at a cut, as a blend would render every size between.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, crate::Values)]
#[values(scope = "frame", package = "render_lab")]
pub struct FrameSize {
    /// Its width in pixels; 0, the viewport's.
    #[value(range = 0.0..=4096.0, unit = plain, text = "values-frame-width", help = "help-frame-width")]
    pub width: f32,
    /// Its height in pixels; 0, the viewport's.
    #[value(range = 0.0..=4096.0, unit = plain, text = "values-frame-height", help = "help-frame-height")]
    pub height: f32,
}

impl FrameSize {
    /// The size, when both sides name one.
    pub fn size(&self) -> Option<[u32; 2]> {
        let (w, h) = (
            self.width.round().min(4096.0),
            self.height.round().min(4096.0),
        );
        (w >= 1.0 && h >= 1.0).then_some([w as u32, h as u32])
    }

    /// Whether the scene follows the viewport's size (a state then serializes without it).
    pub fn follows_viewport(&self) -> bool {
        self.size().is_none()
    }
}

/// A value of view `name` (src/view.rs): `view.<name>.<key>`, its view and key.
pub fn view_key(name: &str) -> Option<(&str, &str)> {
    name.strip_prefix("view.")?.split_once('.')
}

impl ShotState {
    /// The state view `name` sees: this one with that view's values
    /// (`view.<name>.<key>`, set by a story's `{view …}` cue or the dock) in
    /// place of the main view's, its camera theirs too.
    pub fn view_state(&self, name: &str) -> ShotState {
        let mut out = self.clone();
        let prefix = format!("view.{name}.");
        let own: Vec<(String, f32)> = self
            .values
            .iter()
            .filter_map(|(k, v)| {
                k.strip_prefix(&prefix)
                    .map(|inner| (inner.to_string(), v.f32()))
            })
            .collect();
        for (key, value) in own {
            out.set(&key, value);
        }
        out
    }

    /// View `name`'s own camera: `rest`, with the camera values the view has of its own.
    pub fn own_camera(&self, name: &str, rest: OrbitCamera) -> OrbitCamera {
        let mut camera = rest;
        for c in camera_channels() {
            if let Some(v) = self.values.get(&format!("view.{name}.{c}")) {
                camera.set(c, *v);
            }
        }
        camera
    }

    /// Sets view `name`'s own camera (its camera values, `view.<name>.camera…`).
    pub fn set_own_camera(&mut self, name: &str, camera: &OrbitCamera) {
        for c in camera_channels() {
            let v = camera.get(c).expect("a camera channel").f32();
            self.values
                .insert(format!("view.{name}.{c}"), Value::F32(v));
        }
    }

    /// The current investigation's starting state for a demonstration.
    pub fn for_demo(demo: impl Into<DemoId>) -> ShotState {
        investigation::current().shot_state(demo.into())
    }

    /// Whether the story shows the image the investigation publishes as `key`
    /// (so its frame publishes it, in `FrameContent::images`).
    pub fn shows_arrows(&self, key: &str) -> bool {
        self.notes
            .iter()
            .any(|n| n.kind == NoteKind::Arrows && n.name == key)
    }

    /// Whether the story shows the image the investigation publishes as `key`
    /// (so its frame publishes it, in `FrameContent::images`).
    pub fn shows_image(&self, key: &str) -> bool {
        self.images
            .iter()
            .any(|i| matches!(&i.source, ImageSource::Published(k) if k == key))
    }

    /// A number by name: a value's (a flag is 0 or 1), a component's, a
    /// parameter's, or the camera's. Panics for a name the investigation lacks.
    pub fn p(&self, name: &str) -> f32 {
        if let Some(v) = self.values.get(name) {
            return v.f32();
        }
        self.try_get(name)
            .unwrap_or_else(|| panic!("unknown shot parameter {name}"))
            .f32()
    }

    /// Sets a number by name: a value (in its type: a flag on above 0.5, an
    /// index the nearest whole number), a component, a parameter, or the camera's.
    pub fn set(&mut self, name: &str, value: f32) {
        if self.camera.set(name, Value::F32(value))
            || self.free_camera.set(name, Value::F32(value))
            || self.frame.set(name, Value::F32(value))
        {
            return;
        }
        // A value of another view (src/view.rs): kept by its full name, a known key.
        if let Some((_, key)) = view_key(name) {
            assert!(self.try_get(key).is_some(), "unknown shot parameter {name}");
            self.values.insert(name.to_string(), Value::F32(value));
            return;
        }
        // A movable object's value (src/entity.rs).
        if crate::entity::is_key(name) {
            self.values.insert(name.to_string(), Value::F32(value));
            return;
        }
        let investigation = investigation::current();
        if let Some(demos) = investigation.demo_specs() {
            assert!(
                demos.set(&mut self.values, name, value),
                "unknown shot parameter {name}"
            );
            return;
        }
        assert!(
            investigation.param_default(name).is_some(),
            "unknown shot parameter {name}"
        );
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
        let json: serde_json::Map<String, serde_json::Value> = self
            .values
            .iter()
            .map(|(k, v)| (k.clone(), json_of(k, v)))
            .collect();
        serde_json::from_value(serde_json::Value::Object(json))
            .unwrap_or_else(|e| panic!("the shot's values as {}: {e}", std::any::type_name::<T>()))
    }

    /// Sets every value a typed struct holds (`#[derive(serde::Serialize)]`),
    /// by its fields' names (a choice's enum by its option's name).
    pub fn store<T: serde::Serialize>(&mut self, typed: &T) {
        let serde_json::Value::Object(fields) =
            serde_json::to_value(typed).expect("a struct of values")
        else {
            panic!("{} is not a struct", std::any::type_name::<T>())
        };
        for (key, json) in fields {
            for (channel, number) in values::resolve(&key, &json).unwrap_or_else(|e| panic!("{e}"))
            {
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
                Some(e) => out.extend(
                    e.components
                        .iter()
                        .zip(value.components())
                        .map(|(c, n)| (c.to_string(), n)),
                ),
                None => out.push((key.clone(), value.f32())),
            }
        }
        out
    }

    #[cfg(test)]
    pub(crate) fn channels_for_tests(&self) -> Vec<(String, f32)> {
        self.channels()
    }

    /// Every channel a timeline blends: the cameras', then the values'.
    fn channels(&self) -> Vec<(String, f32)> {
        let camera = camera_channels().into_iter().map(|c| {
            (
                c.to_string(),
                self.camera.get(c).expect("a camera channel").f32(),
            )
        });
        let free = free_camera_channels().into_iter().map(|c| {
            (
                c.to_string(),
                self.free_camera
                    .get(c)
                    .expect("a free camera channel")
                    .f32(),
            )
        });
        let frame = frame_channels().into_iter().map(|c| {
            (
                c.to_string(),
                self.frame.get(c).expect("a frame channel").f32(),
            )
        });
        camera
            .chain(free)
            .chain(frame)
            .chain(self.value_channels())
            .collect()
    }

    /// The camera the viewport shows the scene from: the free camera while
    /// it is on, else the main camera.
    pub fn view_camera(&self) -> OrbitCamera {
        if self.free_camera.on {
            self.free_camera.orbit()
        } else {
            self.camera
        }
    }

    /// The value of `key` (src/values.rs): the camera's, a stored one, a
    /// vector's component (an `F32`), or None for no such key.
    pub fn try_get(&self, key: &str) -> Option<Value> {
        if let Some(v) = self
            .camera
            .get(key)
            .or_else(|| self.free_camera.get(key))
            .or_else(|| self.frame.get(key))
        {
            return Some(v);
        }
        if let Some(v) = self.values.get(key) {
            return Some(*v);
        }
        // Another view's value not set: the main view's.
        if let Some((_, inner)) = view_key(key) {
            return self.try_get(inner);
        }
        let entry = values::entry(key)?;
        // A value no state has set yet (a movable object's, src/entity.rs): its default.
        let stored = match self.values.get(entry.key) {
            Some(v) => *v,
            None if entry.key.starts_with("object.") => entry.default?,
            None => return None,
        };
        Some(Value::F32(
            stored.components()[values::component(entry, key)],
        ))
    }

    /// The value of `key`; panics for a key the investigation does not have.
    pub fn get(&self, key: &str) -> Value {
        self.try_get(key)
            .unwrap_or_else(|| panic!("unknown value {key}"))
    }

    /// Sets `key` to `value`, in the key's declared type: a camera value, a
    /// stored value (a vector whole), a component, or a parameter.
    pub fn set_value(&mut self, key: &str, value: impl Into<Value>) {
        let value = value.into();
        if self.camera.set(key, value)
            || self.free_camera.set(key, value)
            || self.frame.set(key, value)
        {
            return;
        }
        match values::entry(key).filter(|e| e.key == key) {
            Some(e) => {
                let value = Value::from_json(e.ty, &value.to_json()).unwrap_or_else(|| {
                    panic!("{key} is a {}, not a {}", e.ty.name(), value.ty().name())
                });
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
        self.values.get(key).unwrap_or_else(|| {
            panic!("no stored value `{key}` (a component or the camera's is read with `get`)")
        })
    }
}

impl std::ops::IndexMut<&str> for ShotState {
    fn index_mut(&mut self, key: &str) -> &mut Value {
        self.values.get_mut(key).unwrap_or_else(|| {
            panic!("no stored value `{key}` (a component or the camera's is set with `set_value`)")
        })
    }
}

pub use crate::animation::camera_drift;
