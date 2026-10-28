//! Demonstrations as data. An investigation that describes its
//! demonstrations in files (`Investigation::demo_specs`) declares, in its
//! package, under its prefix (`o3de/lighting`):
//!
//! - `<prefix>/params`: every shot parameter, as `[[param]]` tables of
//!   `name`, `default`, and `doc`;
//! - `<prefix>/demos`: `demos = [keys…]`, the order of the viewer's keys;
//! - `<prefix>/demo/<key>`: one demonstration (below);
//! - `<prefix>/demo-preset/<name>`: a camera or parameters several share.
//!
//! A demonstration's file has its `key`; its title (`title-text`, a key of
//! the investigation's text) and tab (`tab-text`, or `tab` for a name that is
//! not translated); `hint-text` for the status line; the `scene` it shows;
//! the `listing` its Code tab opens; `resets-camera` (Reset parameters also
//! resets the camera); `include`, presets applied in order; a `[camera]`
//! (`target`, `yaw`, `pitch`, `distance`, `fov`); and `[params]`, the values
//! it changes from the defaults, applied after its presets. A parameter may
//! name an object of its scene (`inspectObject = "teapot"`) for its index.
//! What a demonstration does with its state stays the investigation's code.

use crate::lab::camera::OrbitCamera;
use crate::lab::resources;
use crate::lab::scene::{self, Scene, SceneId};
use crate::lab::shot::{DemoId, PanelMode, RecordRef, ShotState};
use anyhow::{anyhow, bail, Context, Result};
use std::collections::BTreeMap;

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
            let id = format!("{prefix}/demo-preset/{name}");
            let preset = document(&id)?;
            apply(prefix, preset.as_table(), scene, defaults, camera_out, params).with_context(|| format!("preset `{name}`"))?;
        }
    }
    if let Some(c) = table.get("camera") {
        *camera_out = Some(camera(c.as_table().context("`camera` is a table")?)?);
    }
    if let Some(p) = table.get("params") {
        for (name, item) in p.as_table().context("`params` is a table")? {
            let param = defaults.iter().find(|d| d.name == name).ok_or_else(|| anyhow!("no parameter `{name}` ({prefix}/params declares them)"))?;
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

impl Demos {
    /// Reads the demonstrations an investigation's package declares under `prefix`.
    pub fn load(prefix: &str) -> Result<Demos> {
        let params_doc = document(&format!("{prefix}/params"))?;
        let mut params = Vec::new();
        for table in params_doc.get("param").and_then(|p| p.as_array_of_tables()).context("`params` has `[[param]]` tables")? {
            let name = string(table, "name")?.context("a parameter needs `name`")?;
            let default = scene::number(table.get("default").and_then(|i| i.as_value()).with_context(|| format!("parameter `{name}` needs `default`"))?)?;
            if params.iter().any(|p: &Param| p.name == name) {
                bail!("parameter `{name}` is declared twice");
            }
            string(table, "doc")?.with_context(|| format!("parameter `{name}` needs `doc`, what it is"))?;
            params.push(Param { name: leak(name), default });
        }
        let index = document(&format!("{prefix}/demos"))?;
        let keys = index.get("demos").and_then(|d| d.as_array()).context("`demos` is a list of keys")?;
        let mut specs = Vec::new();
        for key in keys {
            let key = key.as_str().context("`demos` is a list of keys")?;
            let id = format!("{prefix}/demo/{key}");
            let spec = (|| -> Result<DemoSpec> {
                let doc = document(&id)?;
                let table = doc.as_table();
                if string(table, "key")? != Some(key) {
                    bail!("its `key` is not `{key}`");
                }
                let scene_key = string(table, "scene")?.context("`scene` is missing")?;
                let scene = SceneId::find(scene_key).ok_or_else(|| anyhow!("no scene `{scene_key}`"))?;
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
        Ok(Demos { params, specs, ids })
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

    /// A demonstration's starting state: every default, its own values, its camera.
    pub fn shot_state(&self, demo: DemoId) -> ShotState {
        let spec = self.spec(demo);
        let mut params: BTreeMap<String, f32> = self.params.iter().map(|p| (p.name.to_string(), p.default)).collect();
        for (name, value) in &spec.params {
            params.insert(name.to_string(), *value);
        }
        ShotState {
            demo,
            camera: spec.camera,
            record: RecordRef::None,
            highlight: Vec::new(),
            panel: PanelMode::Values,
            params,
            callout: None,
            code: None,
            formula: None,
            formula_term: 0,
            formula_lit: 0.0,
        }
    }
}
