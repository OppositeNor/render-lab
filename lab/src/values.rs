//! Values by name: what a demonstration lets the user, a timeline, and a
//! story change, each with its key, type, default, range, unit, labels,
//! name, colour, and text, whatever declares it. The declarations are the
//! schema; a shot stores the values ([`ValueMap`]).
//!
//! - A Rust struct declares its fields with `#[derive(Values)]`
//!   (lab/derive): the camera's (`camera.yaw`, src/camera.rs), which the
//!   shot keeps as a typed struct that [`Reflect`] reaches by key, and an
//!   investigation's (`Demos::with_values`, src/demo.rs), whose values the
//!   shot stores by key and converts to the struct on request
//!   (`ShotState::view`, `ShotState::store`, through serde).
//! - A shader declares the members of a uniform struct with tags in `///`
//!   comments (src/shader_data.rs): the package's parameters then include
//!   them (src/demo.rs), and the lab packs them into the shader's buffer.
//!
//! [`list`] and [`labelled`] name every value the current investigation
//! has; `state["key"]`, [`ShotState::p`](crate::shot::ShotState::p),
//! [`ShotState::get`](crate::shot::ShotState::get), and
//! [`ShotState::set_value`](crate::shot::ShotState::set_value) read and
//! change one by key, a vector's components by their own keys. A timeline
//! blends every value by number, a vector by component (`camera.x`, …); a
//! flag or a whole number snaps when it is set.

use crate::investigation;
use std::collections::BTreeMap;

/// How a value is shown: its suffix and decimals in the dock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Metres,
    Degrees,
    Factor,
    Plain,
    Index,
}

impl Unit {
    /// The unit a tag names: `m`, `deg`, `factor`, `plain`, `index`.
    pub fn parse(name: &str) -> Option<Unit> {
        Some(match name {
            "m" => Unit::Metres,
            "deg" => Unit::Degrees,
            "factor" => Unit::Factor,
            "plain" => Unit::Plain,
            "index" => Unit::Index,
            _ => return None,
        })
    }
}

/// A value's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    F32,
    I32,
    U32,
    /// On or off; a timeline holds it as 0 or 1.
    Bool,
    Vec2,
    Vec3,
    Vec4,
}

impl Type {
    /// How many numbers a timeline keeps of it.
    pub fn components(self) -> usize {
        match self {
            Type::Vec2 => 2,
            Type::Vec3 => 3,
            Type::Vec4 => 4,
            _ => 1,
        }
    }

    /// Whole numbers and flags snap to the nearest value instead of blending.
    pub fn snaps(self) -> bool {
        matches!(self, Type::I32 | Type::U32 | Type::Bool)
    }

    pub fn name(self) -> &'static str {
        match self {
            Type::F32 => "f32",
            Type::I32 => "i32",
            Type::U32 => "u32",
            Type::Bool => "bool",
            Type::Vec2 => "vec2<f32>",
            Type::Vec3 => "vec3<f32>",
            Type::Vec4 => "vec4<f32>",
        }
    }
}

/// A value of one of the [`Type`]s (serialized as a number, a flag, or an array).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum Value {
    F32(f32),
    I32(i32),
    U32(u32),
    Bool(bool),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
}

impl Value {
    pub fn ty(self) -> Type {
        match self {
            Value::F32(_) => Type::F32,
            Value::I32(_) => Type::I32,
            Value::U32(_) => Type::U32,
            Value::Bool(_) => Type::Bool,
            Value::Vec2(_) => Type::Vec2,
            Value::Vec3(_) => Type::Vec3,
            Value::Vec4(_) => Type::Vec4,
        }
    }

    /// Its numbers, as a timeline keeps them (a flag is 0 or 1).
    pub fn components(self) -> Vec<f32> {
        match self {
            Value::F32(x) => vec![x],
            Value::I32(x) => vec![x as f32],
            Value::U32(x) => vec![x as f32],
            Value::Bool(b) => vec![if b { 1.0 } else { 0.0 }],
            Value::Vec2(v) => v.to_vec(),
            Value::Vec3(v) => v.to_vec(),
            Value::Vec4(v) => v.to_vec(),
        }
    }

    /// The value of type `ty` that a timeline's numbers hold: a flag is on
    /// above 0.5; a whole number is the nearest one (a negative one is 0 for
    /// `u32`).
    pub fn from_components(ty: Type, numbers: &[f32]) -> Value {
        assert_eq!(numbers.len(), ty.components(), "a {} has {} numbers", ty.name(), ty.components());
        let n = numbers[0];
        match ty {
            Type::F32 => Value::F32(n),
            Type::I32 => Value::I32(n.round() as i32),
            Type::U32 => Value::U32(n.round().max(0.0) as u32),
            Type::Bool => Value::Bool(n > 0.5),
            Type::Vec2 => Value::Vec2([numbers[0], numbers[1]]),
            Type::Vec3 => Value::Vec3([numbers[0], numbers[1], numbers[2]]),
            Type::Vec4 => Value::Vec4([numbers[0], numbers[1], numbers[2], numbers[3]]),
        }
    }

    /// As one number: a scalar's value (a flag is 0 or 1). Panics for a vector.
    pub fn f32(self) -> f32 {
        match self {
            Value::Vec2(_) | Value::Vec3(_) | Value::Vec4(_) => panic!("a {} is not one number", self.ty().name()),
            _ => self.components()[0],
        }
    }

    /// As a flag: a number is on above 0.5.
    pub fn bool(self) -> bool {
        self.f32() > 0.5
    }

    /// As a whole number: the nearest one, 0 or more.
    pub fn u32(self) -> u32 {
        match self {
            Value::U32(x) => x,
            _ => self.f32().round().max(0.0) as u32,
        }
    }

    /// As a whole number: the nearest one.
    pub fn i32(self) -> i32 {
        match self {
            Value::I32(x) => x,
            _ => self.f32().round() as i32,
        }
    }

    pub fn vec3(self) -> [f32; 3] {
        match self {
            Value::Vec3(v) => v,
            _ => panic!("a {} is not a vec3<f32>", self.ty().name()),
        }
    }
}

impl From<f32> for Value {
    fn from(x: f32) -> Value {
        Value::F32(x)
    }
}

impl From<i32> for Value {
    fn from(x: i32) -> Value {
        Value::I32(x)
    }
}

impl From<u32> for Value {
    fn from(x: u32) -> Value {
        Value::U32(x)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}

impl From<[f32; 2]> for Value {
    fn from(v: [f32; 2]) -> Value {
        Value::Vec2(v)
    }
}

impl From<[f32; 3]> for Value {
    fn from(v: [f32; 3]) -> Value {
        Value::Vec3(v)
    }
}

impl From<[f32; 4]> for Value {
    fn from(v: [f32; 4]) -> Value {
        Value::Vec4(v)
    }
}

/// One value as its declaration describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Its key, with its declaration's scope (`camera.yaw`).
    pub key: &'static str,
    pub ty: Type,
    /// None when a demonstration always gives it (the camera).
    pub default: Option<Value>,
    /// The dock's range, which an index also clamps to.
    pub range: Option<(f32, f32)>,
    /// A vector's range per component, when they differ (else `range`).
    pub component_ranges: &'static [(f32, f32)],
    pub step: Option<f32>,
    pub unit: Unit,
    /// An angle the dock shows wrapped into its range (a yaw of 400° as 40°).
    pub wrap: bool,
    /// What it is about, for `labelled` (the dock's groups).
    pub labels: &'static [&'static str],
    /// Its name on screen: a key of its package's text (`package`), else
    /// `name`, else its key without the scope.
    pub text: Option<&'static str>,
    /// Its name on screen in every language (`x`, `IBL`), when it has no `text`.
    pub name: Option<&'static str>,
    /// The colour of its name in the dock (the text colour by default).
    pub color: Option<egui::Color32>,
    /// A number used as on (above 0.5) or off: the dock shows a checkbox.
    pub toggle: bool,
    /// For a choice, its options (`#[derive(Choice)]`): the value is an
    /// option's index, a story names it, and the dock offers the names.
    pub choices: Option<&'static Choices>,
    /// The package whose text `text` names.
    pub package: &'static str,
    /// What it is (its declaration's doc comment).
    pub doc: &'static str,
    /// A vector's components' keys, in order (`camera.x`, …); empty for a scalar.
    pub components: &'static [&'static str],
}

impl Entry {
    pub fn has_label(&self, label: &str) -> bool {
        self.labels.contains(&label)
    }

    /// The range of component `i` (0 for a scalar).
    pub fn range_of(&self, i: usize) -> Option<(f32, f32)> {
        self.component_ranges.get(i).copied().or(self.range)
    }

    /// The keys a timeline stores it under: its components', or its own.
    pub fn channels(&self) -> Vec<&'static str> {
        if self.components.is_empty() { vec![self.key] } else { self.components.to_vec() }
    }

    /// Its name on screen: its text, else its name, else its key without the scope (`yaw`).
    pub fn label_text(&self) -> &'static str {
        match (self.text, self.name) {
            (Some(key), _) => crate::i18n::package_text(self.package, key),
            (None, Some(name)) => name,
            (None, None) => self.key.rsplit('.').next().unwrap_or(self.key),
        }
    }
}

/// A type whose values reach their fields by key: what `#[derive(Values)]`
/// writes (lab/derive). A key is an entry's, or one of a vector's components'.
pub trait Reflect {
    /// Its entries (the same for every value of the type).
    fn schema() -> &'static [Entry]
    where
        Self: Sized;
    /// The value of `key` (a component's is an `F32`), or None for a key it does not have.
    fn get(&self, key: &str) -> Option<Value>;
    /// Sets `key` (a number for a component); false for a key it does not have.
    fn set(&mut self, key: &str, value: Value) -> bool;

    /// The value of `key`, its full key (`camera.yaw`) or its key in the
    /// struct's scope (`yaw`). Panics for a key it does not have.
    fn get_field(&self, key: &str) -> Value
    where
        Self: Sized,
    {
        let (full, _) = field::<Self>(key);
        self.get(full).expect("a key of its own")
    }

    /// Sets `key` (full, or in the struct's scope) to `value`, converted to
    /// the field's type (a number for a component; a flag on above 0.5).
    /// Panics for a key it does not have, or a value that does not fit.
    fn set_field(&mut self, key: &str, value: impl Into<Value>)
    where
        Self: Sized,
    {
        let value = value.into();
        let (full, entry) = field::<Self>(key);
        let value = if full == entry.key { Value::from_json(entry.ty, &value.to_json()).unwrap_or_else(|| panic!("{full} is a {}, not a {}", entry.ty.name(), value.ty().name())) } else { Value::F32(value.f32()) };
        assert!(self.set(full, value));
    }
}

/// The full key of `key` in `T` (itself, or `scope.key`), and its entry.
fn field<T: Reflect>(key: &str) -> (&'static str, &'static Entry) {
    let suffix = format!(".{key}");
    for entry in T::schema() {
        for full in std::iter::once(entry.key).chain(entry.components.iter().copied()) {
            if full == key || full.ends_with(&suffix) {
                return (full, entry);
            }
        }
    }
    let keys: Vec<&str> = T::schema().iter().map(|e| e.key).collect();
    panic!("{} has no value `{key}` (its values: {})", std::any::type_name::<T>(), keys.join(", "))
}

/// One option of a choice.
#[derive(Debug, PartialEq)]
pub struct ChoiceOption {
    /// Its name in stories and timelines (`ggxD`).
    pub name: &'static str,
    /// Its name on screen: a key of the package's text, else `label`, else `name`.
    pub text: Option<&'static str>,
    pub label: Option<&'static str>,
    /// Its WGSL constant's name after the choice's prefix (`D`, for `VIEW_D`).
    pub wgsl: &'static str,
}

/// A choice's options in index order: what `#[derive(Choice)]` declares
/// for an enum (lab/derive), and what a value's `choices` names.
#[derive(Debug, PartialEq)]
pub struct Choices {
    /// The enum's name, as a shader's tag names it (`choice=ViewMode`).
    pub name: &'static str,
    /// The package whose text the options name, and whose shaders compose
    /// the constants (`build/generated/<name>.wgsl`).
    pub package: &'static str,
    /// The WGSL constants' prefix (`VIEW`), when a shader uses them.
    pub wgsl: Option<&'static str>,
    pub options: &'static [ChoiceOption],
}

impl Choices {
    /// The index of the option named `name`.
    pub fn index(&self, name: &str) -> Option<u32> {
        self.options.iter().position(|o| o.name == name).map(|i| i as u32)
    }

    /// The names of the options, for messages.
    pub fn names(&self) -> String {
        self.options.iter().map(|o| o.name).collect::<Vec<_>>().join(", ")
    }

    /// Option `i`'s name on screen.
    pub fn option_text(&self, i: usize) -> &'static str {
        let o = &self.options[i];
        match (o.text, o.label) {
            (Some(key), _) => crate::i18n::package_text(self.package, key),
            (None, Some(label)) => label,
            (None, None) => o.name,
        }
    }

    /// The generated file's name in the package (`build/generated/view_mode.wgsl`),
    /// a `built = true` resource its shaders compose.
    pub fn wgsl_file(&self) -> String {
        let mut snake = String::new();
        for (i, c) in self.name.chars().enumerate() {
            if c.is_uppercase() && i > 0 {
                snake.push('_');
            }
            snake.extend(c.to_lowercase());
        }
        format!("build/generated/{snake}.wgsl")
    }

    /// The WGSL its shaders compose (`const VIEW_D: u32 = 3u;`, …, and
    /// `VIEW_COUNT`), when it has a prefix: what the lab writes to
    /// `wgsl_file` when it loads the investigation (`shader_data::write_choices`).
    pub fn wgsl_source(&self) -> Option<String> {
        let prefix = self.wgsl?;
        let mut out = format!("// Generated by render-lab from {}'s {} when it loads the investigation; do not edit.\n", self.package, self.name);
        for (i, o) in self.options.iter().enumerate() {
            out.push_str(&format!("const {prefix}_{}: u32 = {i}u;\n", o.wgsl));
        }
        out.push_str(&format!("const {prefix}_COUNT: u32 = {}u;\n", self.options.len()));
        Some(out)
    }
}

/// An enum whose variants are a choice's options (`#[derive(Choice)]`).
pub trait Choice: Sized + Copy {
    fn choices() -> &'static Choices;
    /// Its option's index.
    fn index(self) -> u32;
    fn from_index(index: u32) -> Option<Self>;
}

/// A value as a timeline or a story writes it (a number; `true` or
/// `false`; an option's name; an array of numbers for a vector), as the
/// numbers of the channels a timeline blends: one per component of a
/// declared value, in its type (a flag 1 or 0, an option its index), or the
/// number of any other parameter.
pub fn resolve(name: &str, json: &serde_json::Value) -> Result<Vec<(String, f32)>, String> {
    let entry = entry(name);
    match entry {
        Some(e) if e.key == name => {
            let value = match (json, e.choices) {
                (serde_json::Value::String(option), Some(c)) => Value::U32(c.index(option).ok_or_else(|| format!("`{name}` has no option `{option}` (its options: {})", c.names()))?),
                _ => Value::from_json(e.ty, json).ok_or_else(|| format!("`{name}` is a {}{}, not {json}", e.ty.name(), e.choices.map(|c| format!(" (or one of {})", c.names())).unwrap_or_default()))?,
            };
            Ok(e.channels().into_iter().map(|c| c.to_string()).zip(value.components()).collect())
        }
        _ if entry.is_some() || investigation::current().param_default(name).is_some() => {
            let n = json.as_f64().ok_or_else(|| format!("`{name}` is a number, not {json}"))?;
            Ok(vec![(name.to_string(), n as f32)])
        }
        _ => Err(format!("unknown shot parameter `{name}`")),
    }
}

/// A shot's values by key (`ShotState::values`): each declared value in its
/// own type (a vector as one value), and each other parameter as an `F32`.
/// Sorted by key, so iterating is deterministic. Read and write it through
/// the shot (`state["key"]`, `p`, `set`, `set_value`), which checks keys and
/// types and reaches a vector's components.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct ValueMap(pub BTreeMap<String, Value>);

impl std::ops::Deref for ValueMap {
    type Target = BTreeMap<String, Value>;
    fn deref(&self) -> &BTreeMap<String, Value> {
        &self.0
    }
}

impl std::ops::DerefMut for ValueMap {
    fn deref_mut(&mut self) -> &mut BTreeMap<String, Value> {
        &mut self.0
    }
}

/// Parameters as plain numbers (an investigation that declares no types).
impl From<BTreeMap<String, f32>> for ValueMap {
    fn from(params: BTreeMap<String, f32>) -> ValueMap {
        ValueMap(params.into_iter().map(|(k, v)| (k, Value::F32(v))).collect())
    }
}

impl Value {
    /// As JSON, for a typed view (`ShotState::view`): a number, a flag, or an array.
    pub fn to_json(self) -> serde_json::Value {
        match self {
            Value::F32(x) => serde_json::json!(x),
            Value::I32(x) => serde_json::json!(x),
            Value::U32(x) => serde_json::json!(x),
            Value::Bool(b) => serde_json::json!(b),
            v => serde_json::json!(v.components()),
        }
    }

    /// From JSON (`ShotState::store`): a number, a flag, or an array of numbers, as type `ty`.
    pub fn from_json(ty: Type, json: &serde_json::Value) -> Option<Value> {
        let numbers: Vec<f32> = match json {
            serde_json::Value::Bool(b) => vec![if *b { 1.0 } else { 0.0 }],
            serde_json::Value::Number(n) => vec![n.as_f64()? as f32],
            serde_json::Value::Array(a) => a.iter().map(|n| n.as_f64().map(|n| n as f32)).collect::<Option<_>>()?,
            _ => return None,
        };
        (numbers.len() == ty.components()).then(|| Value::from_components(ty, &numbers))
    }
}

/// Every value of the current investigation: the camera's, then its own
/// parameters that declare their metadata (its shaders' tagged values).
pub fn list() -> Vec<&'static Entry> {
    let mut entries: Vec<&'static Entry> = crate::camera::OrbitCamera::schema().iter().collect();
    if let Some(demos) = investigation::current().demo_specs() {
        entries.extend(demos.entries.iter().copied());
    }
    entries
}

/// The values with `label`.
pub fn labelled(label: &str) -> Vec<&'static Entry> {
    list().into_iter().filter(|e| e.has_label(label)).collect()
}

/// The entry whose key, or one of whose components' keys, is `key`: the
/// camera's, or the investigation's (`Demos::entry`, an index).
pub fn entry(key: &str) -> Option<&'static Entry> {
    crate::camera::OrbitCamera::schema()
        .iter()
        .find(|e| e.key == key || e.components.contains(&key))
        .or_else(|| investigation::current().demo_specs().and_then(|d| d.entry(key)))
}

/// Which component of `entry` the key `key` is (0 for its own key).
pub fn component(entry: &Entry, key: &str) -> usize {
    entry.components.iter().position(|c| *c == key).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A struct as a package would declare it.
    #[derive(Clone, Debug, PartialEq, lab::Values)]
    #[values(scope = "probe", package = "rlstd")]
    struct Probe {
        /// How far it reaches.
        #[value(default = 2.5, range = 0.0..=10.0, step = 0.1, unit = m, label(light, size))]
        radius: f32,
        #[value(default = true, label(light))]
        on: bool,
        #[value(default = 3, range = 0.0..=8.0, unit = index)]
        count: u32,
        #[value(key = "where", components(px, py, pz), default = [0.0, 1.0, 0.0])]
        position: [f32; 3],
    }

    #[test]
    fn derived_schema() {
        let schema = Probe::schema();
        let keys: Vec<&str> = schema.iter().map(|e| e.key).collect();
        assert_eq!(keys, ["probe.radius", "probe.on", "probe.count", "probe.where"]);
        let radius = &schema[0];
        assert_eq!((radius.ty, radius.default, radius.range, radius.step, radius.unit), (Type::F32, Some(Value::F32(2.5)), Some((0.0, 10.0)), Some(0.1), Unit::Metres));
        assert_eq!(radius.labels, ["light", "size"]);
        assert_eq!(radius.doc, "How far it reaches.");
        assert_eq!(schema[1].default, Some(Value::Bool(true)));
        assert_eq!(schema[2].default, Some(Value::U32(3)));
        assert_eq!(schema[3].components, ["probe.px", "probe.py", "probe.pz"]);
        assert_eq!(schema[3].channels(), ["probe.px", "probe.py", "probe.pz"]);
        assert_eq!(schema[0].channels(), ["probe.radius"]);
    }

    #[test]
    fn get_and_set_by_key() {
        let mut p = Probe { radius: 1.0, on: false, count: 2, position: [1.0, 2.0, 3.0] };
        assert_eq!(p.get("probe.radius"), Some(Value::F32(1.0)));
        assert_eq!(p.get("probe.where"), Some(Value::Vec3([1.0, 2.0, 3.0])));
        assert_eq!(p.get("probe.py"), Some(Value::F32(2.0)));
        assert_eq!(p.get("probe.nothing"), None);
        // A timeline's numbers: a flag above 0.5 is on; a count is the nearest whole number.
        assert!(p.set("probe.on", Value::F32(0.7)));
        assert!(p.set("probe.count", Value::F32(4.6)));
        assert!(p.set("probe.pz", Value::F32(9.0)));
        assert!(p.set("probe.radius", 4.0.into()));
        assert!(!p.set("probe.nothing", 1.0.into()));
        assert_eq!(p, Probe { radius: 4.0, on: true, count: 5, position: [1.0, 2.0, 9.0] });
    }

    /// A struct with camelCase keys and every default declared.
    #[derive(Clone, Debug, PartialEq, lab::Values)]
    #[values(rename_all = "camelCase")]
    struct Lights {
        #[value(default = 6, name = "cd", toggle)]
        light_intensity: f32,
        #[value(default = [1.0, 0.5])]
        uv_scale: [f32; 2],
    }

    #[test]
    fn camel_case_and_defaults() {
        let keys: Vec<&str> = Lights::schema().iter().map(|e| e.key).collect();
        assert_eq!(keys, ["lightIntensity", "uvScale"]);
        assert_eq!(Lights::defaults(), Lights { light_intensity: 6.0, uv_scale: [1.0, 0.5] });
        let e = &Lights::schema()[0];
        assert_eq!((e.label_text(), e.toggle), ("cd", true));
        assert_eq!(Lights::schema()[1].label_text(), "uvScale");
    }

    /// An investigation whose demonstrations declare `Probe`'s values and one
    /// plain parameter (`exposure`); the shot tests run with it current.
    struct Test;

    /// A choice, as an investigation declares one.
    #[derive(Clone, Copy, Debug, PartialEq, lab::Choice, serde::Serialize, serde::Deserialize)]
    #[choice(wgsl = "MODE")]
    #[serde(rename_all = "camelCase")]
    enum Mode {
        First,
        #[choice(label = "Second one")]
        SecondOne,
        // A renamed option is renamed for serde too, so a typed view reads it.
        #[choice(name = "last", wgsl = "END")]
        #[serde(rename = "last")]
        Third,
    }

    fn demos() -> &'static crate::demo::Demos {
        static DEMOS: std::sync::OnceLock<crate::demo::Demos> = std::sync::OnceLock::new();
        DEMOS.get_or_init(|| {
            // Probe's values, and a choice of Mode as a shader's tag declares one.
            let mut entries = Probe::schema().to_vec();
            entries.push(Entry { key: "mode", ty: Type::U32, default: Some(Value::U32(0)), range: Some((0.0, 2.0)), unit: Unit::Index, choices: Some(Mode::choices()), ..Probe::schema()[0].clone() });
            crate::demo::Demos::for_tests(Box::leak(entries.into_boxed_slice()), &[("exposure", 1.0)])
        })
    }

    impl crate::investigation::Investigation for Test {
        fn engine(&self) -> &'static crate::investigation::Engine {
            unimplemented!()
        }
        fn id(&self) -> &'static str {
            "test"
        }
        fn title(&self) -> &'static str {
            "test"
        }
        fn summary(&self) -> &'static str {
            ""
        }
        fn wording(&self) -> &'static crate::investigation::Wording {
            unimplemented!()
        }
        fn footer(&self) -> &'static str {
            ""
        }
        fn sources(&self) -> &'static crate::sources::SourceIndex {
            unimplemented!()
        }
        fn listings(&self) -> &'static [crate::pseudocode::Listing] {
            &[]
        }
        fn build_frame(&self, _: &ShotState, _: &crate::frame::Statics, _: [f32; 2], _: f32, _: f32) -> crate::frame::FrameContent {
            unimplemented!()
        }
        fn dock(&self, _: &mut crate::controls::Dock, _: &mut egui::Ui) {}
        fn click(&self, _: &ShotState, _: &mut ShotState, _: &crate::frame::FrameContent, _: &crate::frame::Statics, _: &crate::camera::Projector, _: glam::Vec2) -> bool {
            false
        }
        fn selftest(&self, _: &crate::investigation::SelftestOptions) -> anyhow::Result<bool> {
            Ok(true)
        }
        fn demo_specs(&self) -> Option<&'static crate::demo::Demos> {
            Some(demos())
        }
    }

    /// A shot of `Test` at its defaults.
    fn shot() -> ShotState {
        ShotState {
            demo: crate::shot::DemoId::new("test"),
            camera: crate::camera::OrbitCamera::default(),
            record: crate::shot::RecordRef::None,
            highlight: Vec::new(),
            panel: crate::shot::PanelMode::Values,
            values: demos().defaults(),
            callout: None,
            code: None,
            formula: None,
            formula_term: 0,
            formula_lit: 0.0,
        }
    }

    /// Runs `f` with `Test` current on this thread.
    fn with_test<R>(f: impl FnOnce() -> R) -> R {
        crate::investigation::scoped(&Test, f)
    }

    use crate::shot::ShotState;

    #[test]
    fn a_shot_stores_declared_values_in_their_types() {
        with_test(|| {
            let mut s = shot();
            // Defaults, each in its type; a vector is one value; a plain parameter a number.
            assert_eq!(s["probe.radius"], Value::F32(2.5));
            assert_eq!(s["probe.on"], Value::Bool(true));
            assert_eq!(s["probe.count"], Value::U32(3));
            assert_eq!(s["probe.where"], Value::Vec3([0.0, 1.0, 0.0]));
            assert_eq!(s["exposure"], Value::F32(1.0));
            // A timeline's numbers snap where the type does: a flag at 0.5, a count to the nearest whole number.
            s.set("probe.on", 0.4);
            s.set("probe.count", 4.6);
            assert_eq!((s["probe.on"], s["probe.count"]), (Value::Bool(false), Value::U32(5)));
            assert_eq!((s.p("probe.on"), s.p("probe.count")), (0.0, 5.0));
            // A component by its own key.
            s.set("probe.py", 7.0);
            assert_eq!(s["probe.where"], Value::Vec3([0.0, 7.0, 0.0]));
            assert_eq!((s.p("probe.py"), s.get("probe.py")), (7.0, Value::F32(7.0)));
            // The camera's, by key too.
            s.set("camera.yaw", 33.0);
            assert_eq!((s.camera.yaw, s.p("camera.yaw")), (33.0, 33.0));
        });
    }

    #[test]
    fn index_set_value_and_their_checks() {
        with_test(|| {
            let mut s = shot();
            s["probe.radius"] = 4.0.into();
            assert_eq!(s.p("probe.radius"), 4.0);
            // set_value converts to the declared type.
            s.set_value("probe.count", 2.0);
            s.set_value("probe.where", [1.0, 2.0, 3.0]);
            s.set_value("probe.on", false);
            assert_eq!((s["probe.count"], s["probe.where"], s["probe.on"]), (Value::U32(2), Value::Vec3([1.0, 2.0, 3.0]), Value::Bool(false)));
            assert_eq!(s.try_get("probe.nothing"), None);
            let panics = |f: fn(&mut ShotState)| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| with_test(|| f(&mut shot())))).is_err();
            assert!(panics(|s| s.set("probe.nothing", 1.0)), "an unknown key");
            assert!(panics(|s| s.set_value("probe.where", 1.0)), "a vector is not one number");
            assert!(panics(|s| s.set("probe.where", 1.0)), "a vector is set by component");
            assert!(panics(|s| {
                let _ = s["probe.px"];
            }), "a component is not stored");
        });
    }

    #[test]
    fn a_timeline_blends_every_value_by_channel() {
        with_test(|| {
            let channels: Vec<String> = shot().channels_for_tests().into_iter().map(|c| c.0).collect();
            for c in ["camera.x", "camera.yaw", "probe.radius", "probe.on", "probe.px", "probe.py", "probe.pz", "exposure"] {
                assert!(channels.iter().any(|k| k == c), "{c} in {channels:?}");
            }
            assert!(!channels.iter().any(|k| k == "probe.where"), "a vector blends by component");
        });
    }

    /// The typed view of `Probe`'s values.
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct ProbeView {
        #[serde(rename = "probe.radius")]
        radius: f32,
        #[serde(rename = "probe.on")]
        on: bool,
        #[serde(rename = "probe.count")]
        count: u32,
        #[serde(rename = "probe.where")]
        position: [f32; 3],
    }

    #[test]
    fn typed_views() {
        with_test(|| {
            let mut s = shot();
            assert_eq!(s.view::<ProbeView>(), ProbeView { radius: 2.5, on: true, count: 3, position: [0.0, 1.0, 0.0] });
            s.store(&ProbeView { radius: 9.0, on: false, count: 1, position: [3.0, 2.0, 1.0] });
            assert_eq!((s["probe.radius"], s["probe.on"], s["probe.count"], s.p("probe.pz")), (Value::F32(9.0), Value::Bool(false), Value::U32(1), 1.0));
        });
    }

    #[test]
    fn choices() {
        let c = Mode::choices();
        assert_eq!((c.name, c.names()), ("Mode", "first, secondOne, last".to_string()));
        assert_eq!((c.index("secondOne"), c.index("last"), c.index("nope")), (Some(1), Some(2), None));
        assert_eq!((Mode::SecondOne.index(), Mode::from_index(2), Mode::from_index(3)), (1, Some(Mode::Third), None));
        assert_eq!((c.option_text(0), c.option_text(1)), ("first", "Second one"));
        assert_eq!(c.wgsl_file(), "build/generated/mode.wgsl");
        let wgsl = c.wgsl_source().unwrap();
        assert!(wgsl.contains("const MODE_FIRST: u32 = 0u;\nconst MODE_SECOND_ONE: u32 = 1u;\nconst MODE_END: u32 = 2u;\nconst MODE_COUNT: u32 = 3u;\n"), "{wgsl}");
        // The constants parse as WGSL.
        naga::front::wgsl::parse_str(&wgsl).unwrap();
    }

    #[test]
    fn fields_by_short_or_full_key() {
        let mut p = Probe { radius: 1.0, on: false, count: 2, position: [1.0, 2.0, 3.0] };
        p.set_field("radius", 3.0);
        p.set_field("probe.on", 0.7);
        p.set_field("px", 9.0);
        p.set_field("where", [4.0, 5.0, 6.0]);
        assert_eq!((p.radius, p.on, p.position), (3.0, true, [4.0, 5.0, 6.0]));
        assert_eq!((p.get_field("count"), p.get_field("probe.py")), (Value::U32(2), Value::F32(5.0)));
        assert!(std::panic::catch_unwind(|| Probe { radius: 1.0, on: false, count: 2, position: [0.0; 3] }.set_field("nothing", 1.0)).is_err());
        assert!(std::panic::catch_unwind(|| Probe { radius: 1.0, on: false, count: 2, position: [0.0; 3] }.set_field("where", 1.0)).is_err());
    }

    #[test]
    fn a_story_value_resolves_against_its_declaration() {
        with_test(|| {
            let r = |name: &str, json: serde_json::Value| resolve(name, &json);
            let channels = |pairs: &[(&str, f32)]| Ok(pairs.iter().map(|(c, n)| (c.to_string(), *n)).collect::<Vec<_>>());
            assert_eq!(r("probe.where", serde_json::json!([1.0, 2.0, 3.0])), channels(&[("probe.px", 1.0), ("probe.py", 2.0), ("probe.pz", 3.0)]));
            assert_eq!(r("probe.on", serde_json::json!(true)), channels(&[("probe.on", 1.0)]));
            assert_eq!(r("probe.count", serde_json::json!(2.6)), channels(&[("probe.count", 3.0)]));
            assert_eq!(r("mode", serde_json::json!("last")), channels(&[("mode", 2.0)]));
            assert_eq!(r("mode", serde_json::json!(1)), channels(&[("mode", 1.0)]));
            assert_eq!(r("probe.py", serde_json::json!(7)), channels(&[("probe.py", 7.0)]));
            assert_eq!(r("exposure", serde_json::json!(1.5)), channels(&[("exposure", 1.5)]));
            assert_eq!(r("camera.yaw", serde_json::json!(40)), channels(&[("camera.yaw", 40.0)]));
            for (name, json, error) in [("mode", serde_json::json!("nope"), "first, secondOne, last"), ("exposure", serde_json::json!("high"), "is a number"), ("probe.where", serde_json::json!(1.0), "vec3"), ("nothing", serde_json::json!(1), "unknown")] {
                let e = r(name, json).unwrap_err();
                assert!(e.contains(error), "{name}: {e}");
            }
        });
    }

    #[test]
    fn a_timeline_key_sets_channels() {
        with_test(|| {
            let key: crate::shot::Key = serde_json::from_value(serde_json::json!({"frame": 0, "params": {"mode": "secondOne", "probe.where": [1.0, 2.0, 3.0], "exposure": 2}})).unwrap();
            let mut targets = key.targets().unwrap().to_vec();
            targets.sort_by(|a, b| a.0.cmp(&b.0));
            assert_eq!(targets, [("exposure".to_string(), 2.0), ("mode".to_string(), 1.0), ("probe.px".to_string(), 1.0), ("probe.py".to_string(), 2.0), ("probe.pz".to_string(), 3.0)]);
            let bad: crate::shot::Key = serde_json::from_value(serde_json::json!({"frame": 0, "params": {"mode": "nope"}})).unwrap();
            assert!(bad.targets().is_err());
            // A typed view holds a choice as its enum.
            #[derive(serde::Deserialize)]
            struct View {
                mode: Mode,
            }
            let mut s = shot();
            s.set("mode", 2.0);
            assert_eq!(s.view::<View>().mode, Mode::Third);
        });
    }

    #[test]
    fn json() {
        for v in [Value::F32(0.25), Value::Bool(true), Value::U32(7), Value::Vec3([1.0, 2.0, 3.0])] {
            assert_eq!(Value::from_json(v.ty(), &v.to_json()), Some(v));
        }
        assert_eq!(Value::from_json(Type::Vec2, &serde_json::json!([1.0])), None);
        assert_eq!(serde_json::to_string(&ValueMap::from(BTreeMap::from([("a".to_string(), 0.5f32)]))).unwrap(), r#"{"a":0.5}"#);
    }

    #[test]
    fn components() {
        assert_eq!(Value::from_components(Type::Bool, &[0.4]), Value::Bool(false));
        assert_eq!(Value::from_components(Type::U32, &[-2.0]), Value::U32(0));
        assert_eq!(Value::from_components(Type::Vec2, &[1.0, 2.0]).components(), [1.0, 2.0]);
        assert_eq!(Value::Bool(true).f32(), 1.0);
    }
}
