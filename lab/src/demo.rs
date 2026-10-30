//! Demonstrations as data. An investigation that describes its
//! demonstrations in files (`Investigation::demo_specs`) declares, in its
//! package (`o3de_lighting`, src/packages.rs):
//!
//! - `params` (optional): shot parameters, as `[[param]]` tables of `name`,
//!   `default`, and `doc`. Its parameters are those, the values its shaders
//!   declare (src/shader_data.rs), and the fields of its struct of values
//!   when it has one (`Demos::with_values`, a `#[derive(Values)]` struct,
//!   src/values.rs). A shot holds a declared value in its type
//!   (`ShotState::values`), a vector as one value;
//! - `demos`: `demos = [keys…]`, the order of the viewer's keys;
//! - `demo/<key>`: one demonstration (below);
//! - `demo-preset/<name>`: a camera or parameters several share.
//!
//! A demonstration's file has its `key`; its title (`title-text`, a key of
//! the investigation's text) and tab (`tab-text`, or `tab` for a name that is
//! not translated); `hint-text` for the status line; the `scene` it shows (a
//! scene the lab's package exports, `rlstd.scene/study-room`); the
//! `listing` its Code tab opens; `resets-camera` (Reset parameters also
//! resets the camera); `include`, presets applied in order; a `[camera]`
//! (`target`, `yaw`, `pitch`, `distance`, `fov`); and `[params]`, the values
//! it changes from the defaults, applied after its presets. A parameter may
//! name an object of its scene (`inspectObject = "teapot"`) for its index.
//! What a demonstration does with its state stays the investigation's code.

use crate::camera::OrbitCamera;
use crate::packages::{self, Package};
use crate::resources;
use crate::scene::{self, Scene, SceneId};
use crate::shot::{DemoId, PanelMode, RecordRef, ShotState};
use crate::values::{self, Entry, Reflect, Value, ValueMap};
use anyhow::{anyhow, bail, Context, Result};

/// A shot parameter: its name and default (its file also says what it is,
/// in `doc`).
pub struct Param {
    pub name: &'static str,
    pub default: f32,
}

/// A demonstration's name on the viewer's tab row.
#[derive(Clone, Copy)]
pub enum Tab {
    /// A key of the investigation's text.
    Text(&'static str),
    /// A name the same in every language (`IBL`).
    Plain(&'static str),
}

/// One demonstration as its file describes it.
pub struct DemoSpec {
    pub id: DemoId,
    /// A key of the investigation's text.
    pub title: &'static str,
    pub tab: Tab,
    pub hint: Option<&'static str>,
    pub scene: SceneId,
    pub listing: Option<&'static str>,
    pub resets_camera: bool,
    pub camera: OrbitCamera,
    /// The parameters it changes, in the order they apply.
    pub params: Vec<(&'static str, f32)>,
}

/// An investigation's demonstrations and parameters.
pub struct Demos {
    pub params: Vec<Param>,
    /// The parameters whose declarations give their metadata (src/values.rs).
    pub entries: Vec<&'static crate::values::Entry>,
    /// Each entry by its key and by its components' keys.
    index: std::collections::HashMap<&'static str, &'static Entry>,
    pub specs: Vec<DemoSpec>,
    ids: Vec<DemoId>,
}

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

fn document(id: &str) -> Result<toml_edit::Document<String>> {
    let resource = resources::find(id).ok_or_else(|| anyhow!("no resource `{id}`"))?;
    toml_edit::Document::parse(resource.text().to_string()).with_context(|| format!("{id} ({})", resource.source()))
}

fn string<'a>(table: &'a toml_edit::Table, key: &str) -> Result<Option<&'a str>> {
    match table.get(key) {
        None => Ok(None),
        Some(item) => item.as_str().map(Some).ok_or_else(|| anyhow!("`{key}` is a string")),
    }
}

/// The camera a `[camera]` table describes.
fn camera(table: &toml_edit::Table) -> Result<OrbitCamera> {
    let value = |key: &str| table.get(key).and_then(|i| i.as_value()).with_context(|| format!("`[camera] {key}` is missing"));
    let target = value("target")?.as_array().filter(|a| a.len() == 3).context("`target` is [x, y, z]")?;
    let t: Vec<f32> = target.iter().map(scene::number).collect::<Result<_>>()?;
    Ok(OrbitCamera { target: [t[0], t[1], t[2]], yaw: scene::number(value("yaw")?)?, pitch: scene::number(value("pitch")?)?, distance: scene::number(value("distance")?)?, fov: scene::number(value("fov")?)? })
}

/// A file's `include`d presets, then its own camera and parameters, into
/// `camera` and `params` (later values replace earlier ones).
fn apply(prefix: &str, table: &toml_edit::Table, scene: &Scene, defaults: &[Param], camera_out: &mut Option<OrbitCamera>, params: &mut Vec<(&'static str, f32)>) -> Result<()> {
    if let Some(include) = table.get("include") {
        let list = include.as_array().context("`include` is a list of preset names")?;
        for name in list {
            let name = name.as_str().context("`include` is a list of preset names")?;
            let id = format!("{prefix}.demo-preset/{name}");
            let preset = document(&id)?;
            apply(prefix, preset.as_table(), scene, defaults, camera_out, params).with_context(|| format!("preset `{name}`"))?;
        }
    }
    if let Some(c) = table.get("camera") {
        *camera_out = Some(camera(c.as_table().context("`camera` is a table")?)?);
    }
    if let Some(p) = table.get("params") {
        for (name, item) in p.as_table().context("`params` is a table")? {
            let param = defaults.iter().find(|d| d.name == name).ok_or_else(|| anyhow!("no parameter `{name}` ({prefix}.params declares them)"))?;
            let value = item.as_value().with_context(|| format!("`{name}` is a value"))?;
            let value = match value.as_str() {
                // An object of the scene, by name: its index.
                Some(object) => scene.objects.iter().position(|o| o.name == object).ok_or_else(|| anyhow!("`{name}`: the scene has no object `{object}`"))? as f32,
                None => scene::number(value).with_context(|| format!("`{name}`"))?,
            };
            params.retain(|(n, _)| *n != param.name);
            params.push((param.name, value));
        }
    }
    Ok(())
}

/// A declared value as a parameter by each of its components, with its default.
fn declare(entry: &'static Entry, params: &mut Vec<Param>, entries: &mut Vec<&'static Entry>) -> Result<()> {
    let default = entry.default.ok_or_else(|| anyhow!("value `{}` needs a default", entry.key))?.components();
    for (channel, value) in entry.channels().into_iter().zip(default) {
        if params.iter().any(|p: &Param| p.name == channel) {
            bail!("parameter `{channel}` is declared twice");
        }
        params.push(Param { name: channel, default: value });
    }
    entries.push(entry);
    Ok(())
}

/// Each entry by its key and by its components' keys.
fn index_of(entries: &[&'static Entry]) -> std::collections::HashMap<&'static str, &'static Entry> {
    let mut index = std::collections::HashMap::new();
    for entry in entries {
        index.insert(entry.key, *entry);
        for c in entry.components {
            index.insert(*c, *entry);
        }
    }
    index
}

impl Demos {
    /// Reads the demonstrations the package `prefix` declares.
    pub fn load(prefix: &str) -> Result<Demos> {
        Demos::load_with(prefix, &[])
    }

    /// Reads them, with the values `T`'s fields declare (`#[derive(Values)]`,
    /// every field with a `default`).
    pub fn with_values<T: Reflect>(prefix: &str) -> Result<Demos> {
        Demos::load_with(prefix, T::schema())
    }

    fn load_with(prefix: &str, declared: &'static [Entry]) -> Result<Demos> {
        let package: &Package = resources::packages().iter().find(|m| m.name == prefix).ok_or_else(|| anyhow!("no package `{prefix}`"))?;
        let mut params = Vec::new();
        let mut entries: Vec<&'static Entry> = Vec::new();
        for entry in declared {
            declare(entry, &mut params, &mut entries)?;
        }
        let params_doc = match resources::find(&format!("{prefix}.params")) {
            Some(_) => Some(document(&format!("{prefix}.params"))?),
            None => None,
        };
        let tables = match &params_doc {
            Some(doc) => doc.get("param").and_then(|p| p.as_array_of_tables()).context("`params` has `[[param]]` tables")?.iter().collect(),
            None => Vec::new(),
        };
        for table in tables {
            let name = string(table, "name")?.context("a parameter needs `name`")?;
            let default = scene::number(table.get("default").and_then(|i| i.as_value()).with_context(|| format!("parameter `{name}` needs `default`"))?)?;
            if params.iter().any(|p: &Param| p.name == name) {
                bail!("parameter `{name}` is declared twice");
            }
            string(table, "doc")?.with_context(|| format!("parameter `{name}` needs `doc`, what it is"))?;
            params.push(Param { name: leak(name), default });
        }
        // The values its shaders declare (src/shader_data.rs) are parameters too, by component.
        for shader in resources::shaders().iter().filter(|s| s.id.strip_prefix(prefix).is_some_and(|rest| rest.starts_with('.'))) {
            let Some(schema) = crate::shader_data::schema(&shader.id)? else { continue };
            for entry in schema.entries() {
                if let Some(same) = entries.iter().find(|e| e.key == entry.key) {
                    if *same != entry {
                        bail!("`{}` is declared differently by two shaders ({})", entry.key, shader.id);
                    }
                    continue;
                }
                declare(entry, &mut params, &mut entries).with_context(|| format!("the shader {}", shader.id))?;
            }
        }
        let by_key = index_of(&entries);
        let index = document(&format!("{prefix}.demos"))?;
        let keys = index.get("demos").and_then(|d| d.as_array()).context("`demos` is a list of keys")?;
        let mut specs = Vec::new();
        for key in keys {
            let key = key.as_str().context("`demos` is a list of keys")?;
            let id = format!("{prefix}.demo/{key}");
            let spec = (|| -> Result<DemoSpec> {
                let doc = document(&id)?;
                let table = doc.as_table();
                if string(table, "key")? != Some(key) {
                    bail!("its `key` is not `{key}`");
                }
                let reference = string(table, "scene")?.context("`scene` is missing")?;
                let scene_id = packages::resolve(package, resources::packages(), reference).map_err(|e| anyhow!("scene {e}"))?;
                let scene = SceneId::from_resource(&scene_id).ok_or_else(|| anyhow!("`{reference}` is not a scene"))?;
                let tab = match (string(table, "tab-text")?, string(table, "tab")?) {
                    (Some(k), None) => Tab::Text(leak(k)),
                    (None, Some(name)) => Tab::Plain(leak(name)),
                    _ => bail!("a demonstration has `tab-text` or `tab`"),
                };
                let mut camera = None;
                let mut overrides = Vec::new();
                apply(prefix, table, &Scene::build(scene), &params, &mut camera, &mut overrides)?;
                Ok(DemoSpec {
                    id: DemoId::new(leak(key)),
                    title: leak(string(table, "title-text")?.context("`title-text` is missing")?),
                    tab,
                    hint: string(table, "hint-text")?.map(leak),
                    scene,
                    listing: string(table, "listing")?.map(leak),
                    resets_camera: table.get("resets-camera").map(|r| r.as_bool().context("`resets-camera` is true or false")).transpose()?.unwrap_or(false),
                    camera: camera.context("no `[camera]`, in the file or a preset")?,
                    params: overrides,
                })
            })()
            .with_context(|| format!("demonstration `{key}` ({id})"))?;
            specs.push(spec);
        }
        let ids = specs.iter().map(|s| s.id).collect();
        Ok(Demos { params, entries, index: by_key, specs, ids })
    }

    /// The demonstrations in order.
    pub fn ids(&self) -> &[DemoId] {
        &self.ids
    }

    pub fn spec(&self, demo: DemoId) -> &DemoSpec {
        self.specs.iter().find(|s| s.id == demo).unwrap_or_else(|| panic!("no demonstration `{}`", demo.key()))
    }

    pub fn param_default(&self, name: &str) -> Option<f32> {
        self.params.iter().find(|p| p.name == name).map(|p| p.default)
    }

    /// Every value at its default: each declared one in its type, each other
    /// parameter as a number.
    pub fn defaults(&self) -> ValueMap {
        let mut values = ValueMap::default();
        for entry in &self.entries {
            values.insert(entry.key.to_string(), entry.default.expect("a declared value has a default"));
        }
        for p in self.params.iter().filter(|p| self.entry(p.name).is_none()) {
            values.insert(p.name.to_string(), Value::F32(p.default));
        }
        values
    }

    /// Demonstrations of nothing, with these declared values and other
    /// parameters (the lab's tests).
    #[cfg(test)]
    pub(crate) fn for_tests(declared: &'static [Entry], others: &[(&'static str, f32)]) -> Demos {
        let mut params: Vec<Param> = others.iter().map(|(name, default)| Param { name, default: *default }).collect();
        let mut entries = Vec::new();
        for entry in declared {
            declare(entry, &mut params, &mut entries).unwrap();
        }
        let index = index_of(&entries);
        Demos { params, entries, index, specs: Vec::new(), ids: Vec::new() }
    }

    /// The declared value whose key, or one of whose components' keys, is `key`.
    pub fn entry(&self, key: &str) -> Option<&'static Entry> {
        self.index.get(key).copied()
    }

    /// Sets parameter `name` (a value's key or a component's) to the number
    /// `value` in `values`, in its declared type (a flag on above 0.5, an
    /// index the nearest whole number); false for a name it does not declare.
    pub fn set(&self, values: &mut ValueMap, name: &str, value: f32) -> bool {
        match self.entry(name) {
            Some(entry) => {
                let mut numbers = values.get(entry.key).copied().or(entry.default).expect("a declared value has a default").components();
                assert!(entry.ty.components() == 1 || entry.key != name, "{name} is a {}: set it by component, or with set_value", entry.ty.name());
                numbers[values::component(entry, name)] = value;
                values.insert(entry.key.to_string(), Value::from_components(entry.ty, &numbers));
                true
            }
            None if self.param_default(name).is_some() => {
                values.insert(name.to_string(), Value::F32(value));
                true
            }
            None => false,
        }
    }

    /// A demonstration's starting state: every default, its own values, its camera.
    pub fn shot_state(&self, demo: DemoId) -> ShotState {
        let spec = self.spec(demo);
        let mut values = self.defaults();
        for (name, value) in &spec.params {
            self.set(&mut values, name, *value);
        }
        ShotState {
            demo,
            camera: spec.camera,
            record: RecordRef::None,
            highlight: Vec::new(),
            panel: PanelMode::Values,
            values,
            callout: None,
            code: None,
            formula: None,
            formula_term: 0,
            formula_lit: 0.0,
            actions: Vec::new(),
            images: Vec::new(),
            notes: Vec::new(),
        }
    }
}
