//! Experiments on an investigation's algorithm: one file format for a preset
//! saved from the viewer's Experiments tab, the A/B pair it compares, and a
//! research run the command line renders headless (src/research.rs).
//!
//! An experiment file is TOML:
//!
//! ```toml
//! name = "probe-rays"
//! description = "How the probe ray count trades quality for time."
//! investigation = "godot/sdfgi"     # optional: checked against --investigation
//! demo = "room"                     # the starting demonstration (default: the first)
//! resolution = [1280, 720]          # the frame a research run renders
//! frames = 16                       # frames rendered before measuring (history converges)
//! warmup = 8                        # frames rendered, untimed, before the timed ones
//! repeats = 3                       # timed frames (and images) per run
//! convergence = false               # also measure the image after every frame
//! metrics = ["gpu_ms", "psnr", "ssim", "flip"]   # default: all (METRICS)
//!
//! [camera]                          # optional: the demonstration's camera otherwise
//! target = [0.0, 1.0, 0.0]
//! yaw = 30.0
//!
//! [base.values]                     # every run's values (and the reference's)
//! sdfgiEnergy = 1.0
//!
//! [reference]                       # the ground truth runs are measured against
//! frames = 64
//! [reference.values]
//! probeRayCount = "128"             # a choice by its option's name
//!
//! [[sweep]]                         # each combination of its lists (`zip = true`: in step)
//! values = { probeRayCount = ["4", "8", "16", "32", "64"] }
//!
//! [[variant]]                       # named ablations
//! name = "no-bounce"
//! values = { bounceFeedback = 0.0 }
//!
//! [inspect]                         # optional: measure a resource of the frame's graph
//! of = "sdfgiForwardFS.hdr"         # <node>.<resource> (render-lab graph), right after the node
//! show = "color"                    # how the inspector draws it (src/graph/ Show)
//! values = { exposure = 0.0 }       # the inspector's parameters (INSPECT_PARAMS)
//! ```
//!
//! With `inspect`, the images measured (and written) are the inspector's
//! drawing of that resource (display colours, 8 bits a channel), not the
//! frame: a run measures what one pass produces against the reference's same
//! resource; relMSE is then over those display values.
//!
//! Values are set by key as a story sets them (`values::resolve`): a number,
//! `true`/`false`, an option's name, an array for a vector. The runs are the
//! base itself, then each sweep's combinations, then each variant.

use crate::investigation::{self, Investigation};
use crate::shot::ShotState;
use crate::values::{self, Reflect, Value};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{Map, Value as Json};
use std::path::{Path, PathBuf};

/// The metrics a research run can take (src/metrics.rs, src/timing.rs), in report order.
pub const METRICS: &[&str] = &["gpu_ms", "cpu_ms", "mse", "rmse", "psnr", "ssim", "flip", "relmse", "luminance"];

/// The fields an experiment file may have.
const FIELDS: &[&str] = &["name", "description", "investigation", "demo", "resolution", "frames", "warmup", "repeats", "convergence", "metrics", "camera", "base", "reference", "sweep", "variant", "inspect"];

#[derive(Clone, Debug, PartialEq)]
pub struct Experiment {
    pub name: String,
    pub description: String,
    /// The investigation it is for (`engine/feature`), when it says.
    pub investigation: Option<String>,
    /// The starting demonstration's key.
    pub demo: Option<String>,
    pub resolution: [u32; 2],
    pub frames: u32,
    pub warmup: u32,
    pub repeats: u32,
    pub convergence: bool,
    pub metrics: Vec<String>,
    /// The camera's values by key (`camera.yaw`), set before the base.
    pub camera: Map<String, Json>,
    pub base: Map<String, Json>,
    pub reference: Option<Reference>,
    pub sweeps: Vec<Sweep>,
    pub variants: Vec<Variant>,
    /// The resource of the frame's graph measured instead of the frame.
    pub inspect: Option<Inspect>,
}

/// What an experiment measures in place of the frame: a resource of the
/// frame's graph right after a node (src/graph/), as the inspector draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct Inspect {
    /// `<node>.<resource>`.
    pub of: String,
    pub show: Option<String>,
    pub values: std::collections::BTreeMap<String, Vec<f32>>,
}

/// The ground truth: values over the base, and how many frames it renders.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Reference {
    pub values: Map<String, Json>,
    pub frames: Option<u32>,
}

/// Values to sweep: every combination of the lists, or (`zip`) the lists in step.
#[derive(Clone, Debug, PartialEq)]
pub struct Sweep {
    pub values: Vec<(String, Vec<Json>)>,
    pub zip: bool,
}

/// A named set of values over the base: an ablation.
#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    pub name: String,
    pub values: Map<String, Json>,
}

/// One run of an experiment: its name, the values it varies from the base, and every value it sets.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub name: String,
    pub varied: Vec<(String, Json)>,
    pub values: Map<String, Json>,
}

impl Default for Experiment {
    fn default() -> Self {
        Experiment {
            name: "experiment".into(),
            description: String::new(),
            investigation: None,
            demo: None,
            resolution: [1280, 720],
            frames: 16,
            warmup: 8,
            repeats: 3,
            convergence: false,
            metrics: METRICS.iter().map(|m| m.to_string()).collect(),
            camera: Map::new(),
            base: Map::new(),
            reference: None,
            sweeps: Vec::new(),
            variants: Vec::new(),
            inspect: None,
        }
    }
}

/// A TOML item as JSON (a datetime as its text).
fn json_of(item: &toml_edit::Item) -> Json {
    match item {
        toml_edit::Item::None => Json::Null,
        toml_edit::Item::Value(v) => json_of_value(v),
        toml_edit::Item::Table(t) => Json::Object(t.iter().map(|(k, v)| (k.to_string(), json_of(v))).collect()),
        toml_edit::Item::ArrayOfTables(a) => Json::Array(a.iter().map(|t| Json::Object(t.iter().map(|(k, v)| (k.to_string(), json_of(v))).collect())).collect()),
    }
}

fn json_of_value(value: &toml_edit::Value) -> Json {
    match value {
        toml_edit::Value::String(s) => Json::String(s.value().clone()),
        toml_edit::Value::Integer(i) => Json::from(*i.value()),
        toml_edit::Value::Float(f) => serde_json::Number::from_f64(*f.value()).map_or(Json::Null, Json::Number),
        toml_edit::Value::Boolean(b) => Json::Bool(*b.value()),
        toml_edit::Value::Datetime(d) => Json::String(d.value().to_string()),
        toml_edit::Value::Array(a) => Json::Array(a.iter().map(json_of_value).collect()),
        toml_edit::Value::InlineTable(t) => Json::Object(t.iter().map(|(k, v)| (k.to_string(), json_of_value(v))).collect()),
    }
}

/// A JSON value as TOML: a number, a flag, a string, or an array of them.
fn toml_of(json: &Json) -> String {
    match json {
        Json::Null => "\"\"".into(),
        Json::Bool(b) => b.to_string(),
        Json::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.to_string(),
            (None, Some(f)) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{f:.1}"),
            (None, Some(f)) => format!("{f}"),
            _ => n.to_string(),
        },
        Json::String(s) => quote(s),
        Json::Array(a) => format!("[{}]", a.iter().map(toml_of).collect::<Vec<_>>().join(", ")),
        Json::Object(o) => format!("{{ {} }}", o.iter().map(|(k, v)| format!("{} = {}", key(k), toml_of(v))).collect::<Vec<_>>().join(", ")),
    }
}

/// A TOML basic string.
fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A TOML key: bare when it can be, else quoted (`"camera.yaw"`).
fn key(k: &str) -> String {
    if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') { k.to_string() } else { quote(k) }
}

/// A table of values (`values = {…}` or `[x.values]`) as keys and JSON. A key in kebab case is the value in camel
/// case (`view-mode` is `viewMode`), as an experiment written in Lamlet writes it (lab/src/script/data.rs); no value's
/// key has a hyphen.
fn values_of(json: Option<&Json>, at: &str) -> Result<Map<String, Json>> {
    let camel = |k: &str| {
        let mut parts = k.split('-');
        let first = parts.next().unwrap_or_default().to_string();
        parts.fold(first, |out, p| {
            let mut c = p.chars();
            out + &c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
        })
    };
    match json {
        None => Ok(Map::new()),
        Some(Json::Object(o)) => Ok(o.iter().map(|(k, v)| (camel(k), v.clone())).collect()),
        Some(other) => bail!("`{at}` is a table of values by key, not {other}"),
    }
}

fn count(json: Option<&Json>, at: &str, default: u32) -> Result<u32> {
    match json {
        None => Ok(default),
        Some(j) => j.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| anyhow!("`{at}` is a whole number, not {j}")),
    }
}

fn text(json: Option<&Json>, at: &str) -> Result<Option<String>> {
    match json {
        None => Ok(None),
        Some(Json::String(s)) => Ok(Some(s.clone())),
        Some(other) => bail!("`{at}` is a string, not {other}"),
    }
}

impl Experiment {
    /// Parse an experiment file.
    pub fn parse(source: &str) -> Result<Experiment> {
        let document = toml_edit::Document::parse(source).context("parse the experiment (TOML)")?;
        let Json::Object(top) = json_of(document.as_item()) else { bail!("an experiment is a table") };
        if let Some(unknown) = top.keys().find(|k| !FIELDS.contains(&k.as_str())) {
            bail!("unknown field `{unknown}` (an experiment has {})", FIELDS.join(", "));
        }
        let defaults = Experiment::default();
        let resolution = match top.get("resolution") {
            None => defaults.resolution,
            Some(Json::Array(a)) if a.len() == 2 => {
                let n = |j: &Json| j.as_u64().filter(|n| (16..=8192).contains(n)).map(|n| n as u32).ok_or_else(|| anyhow!("`resolution` is two whole numbers from 16 to 8192"));
                [n(&a[0])?, n(&a[1])?]
            }
            Some(other) => bail!("`resolution` is [width, height], not {other}"),
        };
        let metrics = match top.get("metrics") {
            None => defaults.metrics,
            Some(Json::Array(a)) => {
                let names = a.iter().map(|m| m.as_str().map(str::to_string).ok_or_else(|| anyhow!("`metrics` is a list of names"))).collect::<Result<Vec<_>>>()?;
                if let Some(unknown) = names.iter().find(|m| !METRICS.contains(&m.as_str())) {
                    bail!("unknown metric `{unknown}` (the metrics: {})", METRICS.join(", "));
                }
                names
            }
            Some(other) => bail!("`metrics` is a list of names, not {other}"),
        };
        let mut camera = Map::new();
        for (k, v) in values_of(top.get("camera"), "camera")? {
            camera.insert(if k.starts_with("camera.") { k } else { format!("camera.{k}") }, v);
        }
        let base = match top.get("base") {
            None => Map::new(),
            Some(Json::Object(b)) => {
                if let Some(unknown) = b.keys().find(|k| *k != "values") {
                    bail!("unknown field `base.{unknown}` (base has values)");
                }
                values_of(b.get("values"), "base.values")?
            }
            Some(other) => bail!("`base` is a table, not {other}"),
        };
        let reference = match top.get("reference") {
            None => None,
            Some(Json::Object(r)) => {
                if let Some(unknown) = r.keys().find(|k| *k != "values" && *k != "frames") {
                    bail!("unknown field `reference.{unknown}` (reference has values and frames)");
                }
                Some(Reference { values: values_of(r.get("values"), "reference.values")?, frames: r.get("frames").map(|f| count(Some(f), "reference.frames", 0)).transpose()? })
            }
            Some(other) => bail!("`reference` is a table, not {other}"),
        };
        let tables = |name: &str| -> Result<Vec<Map<String, Json>>> {
            match top.get(name) {
                None => Ok(Vec::new()),
                Some(Json::Array(a)) => a.iter().map(|t| t.as_object().cloned().ok_or_else(|| anyhow!("`[[{name}]]` entries are tables"))).collect(),
                Some(other) => bail!("`{name}` is a list of tables (`[[{name}]]`), not {other}"),
            }
        };
        let mut sweeps = Vec::new();
        for (i, t) in tables("sweep")?.into_iter().enumerate() {
            if let Some(unknown) = t.keys().find(|k| *k != "values" && *k != "zip") {
                bail!("unknown field `{unknown}` in sweep {} (a sweep has values and zip)", i + 1);
            }
            let zip = match t.get("zip") {
                None => false,
                Some(Json::Bool(b)) => *b,
                Some(other) => bail!("sweep {}: `zip` is true or false, not {other}", i + 1),
            };
            let mut axes = Vec::new();
            for (k, v) in values_of(t.get("values"), "sweep.values")? {
                match v {
                    Json::Array(list) if !list.is_empty() => axes.push((k, list)),
                    other => bail!("sweep {}: `{k}` is a non-empty list of values, not {other}", i + 1),
                }
            }
            if axes.is_empty() {
                bail!("sweep {} sweeps nothing", i + 1);
            }
            if zip && axes.iter().any(|a| a.1.len() != axes[0].1.len()) {
                bail!("sweep {}: a zipped sweep's lists have one length", i + 1);
            }
            sweeps.push(Sweep { values: axes, zip });
        }
        let mut variants: Vec<Variant> = Vec::new();
        for (i, t) in tables("variant")?.into_iter().enumerate() {
            if let Some(unknown) = t.keys().find(|k| *k != "values" && *k != "name") {
                bail!("unknown field `{unknown}` in variant {} (a variant has name and values)", i + 1);
            }
            let name = text(t.get("name"), "variant.name")?.with_context(|| format!("variant {} needs a name", i + 1))?;
            if name == "base" || name == "reference" || variants.iter().any(|v| v.name == name) {
                bail!("variant `{name}`: a variant's name is its own (not base or reference)");
            }
            variants.push(Variant { name, values: values_of(t.get("values"), "variant.values")? });
        }
        let inspect = match top.get("inspect") {
            None => None,
            Some(Json::Object(t)) => {
                if let Some(unknown) = t.keys().find(|k| !["of", "show", "values"].contains(&k.as_str())) {
                    bail!("unknown field `inspect.{unknown}` (inspect has of, show, and values)");
                }
                let of = text(t.get("of"), "inspect.of")?.context("inspect needs `of`, a <node>.<resource> (render-lab graph)")?;
                if of.rsplit_once('.').is_none() {
                    bail!("inspect.of `{of}`: expected <node>.<resource>");
                }
                let show = text(t.get("show"), "inspect.show")?;
                if let Some(show) = &show {
                    crate::graph::Show::from_key(show).map_err(|e| anyhow!(e))?;
                }
                let mut parsed = std::collections::BTreeMap::new();
                for (k, v) in values_of(t.get("values"), "inspect.values")? {
                    let numbers: Vec<f32> = match &v {
                        Json::Number(n) => vec![n.as_f64().unwrap_or(0.0) as f32],
                        Json::Array(a) => a.iter().map(|x| x.as_f64().map(|f| f as f32).ok_or_else(|| anyhow!("inspect.values.{k}: numbers"))).collect::<Result<_>>()?,
                        other => bail!("inspect.values.{k}: a number or numbers, not {other}"),
                    };
                    crate::graph::check_inspect_param(&k, &numbers).map_err(|e| anyhow!(e))?;
                    parsed.insert(k, numbers);
                }
                Some(Inspect { of, show, values: parsed })
            }
            Some(other) => bail!("`inspect` is a table (of, show, values), not {other}"),
        };
        Ok(Experiment {
            name: text(top.get("name"), "name")?.unwrap_or(defaults.name),
            description: text(top.get("description"), "description")?.unwrap_or_default(),
            investigation: text(top.get("investigation"), "investigation")?,
            demo: text(top.get("demo"), "demo")?,
            resolution,
            frames: count(top.get("frames"), "frames", defaults.frames)?.max(1),
            warmup: count(top.get("warmup"), "warmup", defaults.warmup)?,
            repeats: count(top.get("repeats"), "repeats", defaults.repeats)?.max(1),
            convergence: match top.get("convergence") {
                None => false,
                Some(Json::Bool(b)) => *b,
                Some(other) => bail!("`convergence` is true or false, not {other}"),
            },
            metrics,
            camera,
            base,
            reference,
            sweeps,
            variants,
            inspect,
        })
    }

    /// The experiment as a file that `parse` reads back.
    pub fn to_toml(&self) -> String {
        let mut out = String::new();
        let line = |out: &mut String, k: &str, v: String| out.push_str(&format!("{} = {v}\n", key(k)));
        line(&mut out, "name", quote(&self.name));
        if !self.description.is_empty() {
            line(&mut out, "description", quote(&self.description));
        }
        if let Some(i) = &self.investigation {
            line(&mut out, "investigation", quote(i));
        }
        if let Some(d) = &self.demo {
            line(&mut out, "demo", quote(d));
        }
        line(&mut out, "resolution", format!("[{}, {}]", self.resolution[0], self.resolution[1]));
        line(&mut out, "frames", self.frames.to_string());
        line(&mut out, "warmup", self.warmup.to_string());
        line(&mut out, "repeats", self.repeats.to_string());
        if self.convergence {
            line(&mut out, "convergence", "true".into());
        }
        line(&mut out, "metrics", toml_of(&Json::Array(self.metrics.iter().map(|m| Json::String(m.clone())).collect())));
        let table = |out: &mut String, header: &str, values: &Map<String, Json>| {
            out.push_str(&format!("\n[{header}]\n"));
            for (k, v) in values {
                out.push_str(&format!("{} = {}\n", key(k), toml_of(v)));
            }
        };
        if !self.camera.is_empty() {
            let camera: Map<String, Json> = self.camera.iter().map(|(k, v)| (k.strip_prefix("camera.").unwrap_or(k).to_string(), v.clone())).collect();
            table(&mut out, "camera", &camera);
        }
        if !self.base.is_empty() {
            table(&mut out, "base.values", &self.base);
        }
        if let Some(r) = &self.reference {
            out.push_str("\n[reference]\n");
            if let Some(f) = r.frames {
                out.push_str(&format!("frames = {f}\n"));
            }
            if !r.values.is_empty() {
                table(&mut out, "reference.values", &r.values);
            }
        }
        for s in &self.sweeps {
            out.push_str("\n[[sweep]]\n");
            if s.zip {
                out.push_str("zip = true\n");
            }
            let values: Map<String, Json> = s.values.iter().map(|(k, v)| (k.clone(), Json::Array(v.clone()))).collect();
            out.push_str(&format!("values = {}\n", toml_of(&Json::Object(values))));
        }
        for v in &self.variants {
            out.push_str(&format!("\n[[variant]]\nname = {}\nvalues = {}\n", quote(&v.name), toml_of(&Json::Object(v.values.clone()))));
        }
        if let Some(i) = &self.inspect {
            out.push_str(&format!("\n[inspect]\nof = {}\n", quote(&i.of)));
            if let Some(show) = &i.show {
                out.push_str(&format!("show = {}\n", quote(show)));
            }
            if !i.values.is_empty() {
                let values: Map<String, Json> = i.values.iter().map(|(k, v)| (k.clone(), if v.len() == 1 { Json::from(v[0] as f64) } else { Json::Array(v.iter().map(|x| Json::from(*x as f64)).collect()) })).collect();
                out.push_str(&format!("values = {}\n", toml_of(&Json::Object(values))));
            }
        }
        out
    }

    /// The runs: the base, each sweep's combinations, then each variant.
    pub fn runs(&self) -> Vec<Run> {
        let with = |varied: Vec<(String, Json)>| {
            let mut values = self.base.clone();
            for (k, v) in &varied {
                values.insert(k.clone(), v.clone());
            }
            values
        };
        let mut runs = vec![Run { name: "base".into(), varied: Vec::new(), values: self.base.clone() }];
        for sweep in &self.sweeps {
            let points: Vec<Vec<(String, Json)>> = if sweep.zip {
                (0..sweep.values[0].1.len()).map(|i| sweep.values.iter().map(|(k, list)| (k.clone(), list[i].clone())).collect()).collect()
            } else {
                let mut points: Vec<Vec<(String, Json)>> = vec![Vec::new()];
                for (k, list) in &sweep.values {
                    points = points.into_iter().flat_map(|p| list.iter().map(move |v| [p.clone(), vec![(k.clone(), v.clone())]].concat())).collect();
                }
                points
            };
            for varied in points {
                let name = varied.iter().map(|(k, v)| format!("{k}={}", short(v))).collect::<Vec<_>>().join(",");
                if runs.iter().any(|r| r.name == name) {
                    continue;
                }
                runs.push(Run { name, values: with(varied.clone()), varied });
            }
        }
        for v in &self.variants {
            let varied: Vec<(String, Json)> = v.values.iter().map(|(k, j)| (k.clone(), j.clone())).collect();
            runs.push(Run { name: v.name.clone(), values: with(varied.clone()), varied });
        }
        runs
    }

    /// The reference's values: the base with the reference's over it.
    pub fn reference_values(&self) -> Option<Map<String, Json>> {
        self.reference.as_ref().map(|r| {
            let mut values = self.base.clone();
            values.extend(r.values.clone());
            values
        })
    }

    /// Every key the experiment sets resolves for the current investigation;
    /// an error names the keys that do not and the values there are.
    pub fn check(&self) -> Result<()> {
        if let Some(path) = &self.investigation {
            let current = investigation::current().path();
            if *path != current {
                bail!("the experiment is for {path}, not {current} (give --investigation {path})");
            }
        }
        if let Some(i) = &self.inspect {
            crate::graph::FrameGraph::current(&[]).inspected(&i.of).map_err(|e| anyhow!("inspect.of: {e}"))?;
        }
        if let Some(demo) = &self.demo {
            demo_named(demo)?;
        }
        let mut problems = Vec::new();
        let mut check = |k: &str, v: &Json| {
            if let Err(e) = values::resolve(k, v) {
                problems.push(e);
            }
        };
        for (k, v) in self.camera.iter().chain(&self.base) {
            check(k, v);
        }
        if let Some(r) = &self.reference {
            for (k, v) in &r.values {
                check(k, v);
            }
        }
        for s in &self.sweeps {
            for (k, list) in &s.values {
                for v in list {
                    check(k, v);
                }
            }
        }
        for v in &self.variants {
            for (k, j) in &v.values {
                check(k, j);
            }
        }
        if problems.is_empty() {
            return Ok(());
        }
        problems.dedup();
        let keys: Vec<&str> = values::list().iter().map(|e| e.key).collect();
        bail!("{}\n(the values of {}: {}; `render-lab values` describes them)", problems.join("\n"), investigation::current().path(), keys.join(", "))
    }

    /// The starting demonstration.
    pub fn start_demo(&self) -> Result<crate::shot::DemoId> {
        match &self.demo {
            Some(d) => demo_named(d),
            None => Ok(investigation::current().demos()[0]),
        }
    }

    /// Set the camera and then `values` on `state`.
    pub fn apply(&self, state: &mut ShotState, values: &Map<String, Json>) -> Result<()> {
        apply(state, &self.camera)?;
        apply(state, values)
    }

    /// An experiment of the viewer's states: the live state's changes from
    /// the demonstration's defaults as the base, its camera, and the
    /// reference's changes (if one is set) as the reference.
    pub fn capture(name: &str, live: &ShotState, defaults: &ShotState, reference: Option<&ShotState>) -> Experiment {
        let camera = crate::camera::OrbitCamera::schema()
            .iter()
            .filter_map(|e| live.camera.get(e.key).map(|v| (e.key.to_string(), json_value(e.key, v))))
            .collect();
        let base = changes(live, defaults);
        // The reference holds what it sets over the base: where it differs from the live state.
        let reference = reference.map(|r| Reference { values: changes(r, live), frames: None });
        Experiment {
            name: name.to_string(),
            investigation: Some(investigation::current().path()),
            demo: Some(live.demo.key().to_string()),
            camera,
            base,
            reference,
            ..Experiment::default()
        }
    }
}

/// A value as JSON for a file: a choice by its option's name.
pub fn json_value(key: &str, value: Value) -> Json {
    match values::entry(key).and_then(|e| e.choices) {
        Some(c) => Json::String(c.options.get(value.u32() as usize).map_or_else(String::new, |o| o.name.to_string())),
        None => value.to_json(),
    }
}

/// The values of `state` that differ from `defaults`, as JSON by key.
pub fn changes(state: &ShotState, defaults: &ShotState) -> Map<String, Json> {
    state.values.iter().filter(|(k, v)| defaults.values.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), json_value(k, *v))).collect()
}

/// Set each value of `values` on `state` (`values::resolve`).
pub fn apply(state: &mut ShotState, values: &Map<String, Json>) -> Result<()> {
    for (k, v) in values {
        for (channel, n) in values::resolve(k, v).map_err(|e| anyhow!(e))? {
            state.set(&channel, n);
        }
    }
    Ok(())
}

fn demo_named(key: &str) -> Result<crate::shot::DemoId> {
    let demos = investigation::current().demos();
    demos.iter().copied().find(|d| d.key() == key).ok_or_else(|| anyhow!("no demonstration `{key}` (the demonstrations: {})", demos.iter().map(|d| d.key()).collect::<Vec<_>>().join(", ")))
}

/// A value in a run's name.
fn short(v: &Json) -> String {
    match v {
        Json::String(s) => s.clone(),
        Json::Number(n) => n.to_string(),
        other => other.to_string().replace(' ', ""),
    }
}

/// A name usable as a file name.
pub fn file_name(name: &str) -> String {
    let cleaned: String = name.trim().chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' }).collect();
    let cleaned = cleaned.trim_matches(['-', '.']).to_string();
    if cleaned.is_empty() { "experiment".into() } else { cleaned }
}

/// Where an experiment file comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// A package's `experiment/<name>` resource.
    Shipped(String),
    /// A file of the user's.
    User(PathBuf),
}

/// An experiment the Experiments tab offers.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    pub source: Source,
}

impl Preset {
    pub fn load(&self) -> Result<Experiment> {
        let (text, at) = match &self.source {
            Source::Shipped(id) => (crate::script::data::resource_toml(id).map_err(anyhow::Error::msg)?, id.clone()),
            Source::User(path) => (file_toml(path)?, path.display().to_string()),
        };
        Experiment::parse(&text).with_context(|| at)
    }
}

/// The folder of the user's experiments for `investigation`:
/// `$XDG_CONFIG_HOME/render-lab/experiments/<engine>/<feature>`.
pub fn user_dir(investigation: &'static dyn Investigation) -> Option<PathBuf> {
    let config = crate::config::Config::default_path()?;
    Some(config.parent()?.join("experiments").join(investigation.path()))
}

/// The experiments for the current investigation: its package's
/// (`experiment/<name>` resources), then the user's.
pub fn presets() -> Vec<Preset> {
    let current = investigation::current();
    let mut out = Vec::new();
    if let Some(package) = current.package() {
        let prefix = format!("{package}.experiment/");
        for resource in crate::resources::all() {
            if let Some(name) = resource.id().strip_prefix(&prefix) {
                out.push(Preset { name: name.to_string(), source: Source::Shipped(resource.id().to_string()) });
            }
        }
    }
    if let Some(dir) = user_dir(current) {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "toml")).collect()).unwrap_or_default();
        files.sort();
        for path in files {
            let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            out.push(Preset { name, source: Source::User(path) });
        }
    }
    out
}

/// Save `experiment` among the user's experiments; the path written.
pub fn save_user(experiment: &Experiment) -> Result<PathBuf> {
    let dir = user_dir(investigation::current()).context("no settings folder (XDG_CONFIG_HOME or HOME)")?;
    write(&dir.join(format!("{}.toml", file_name(&experiment.name))), experiment)
}

/// Write `experiment` to `path` (its folder made), through a temporary file.
pub fn write(path: &Path, experiment: &Experiment) -> Result<PathBuf> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, experiment.to_toml()).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("rename to {}", path.display()))?;
    Ok(path.to_path_buf())
}

/// Read an experiment: a file, or a package's resource by full key
/// (`godot_voxelgi.experiment/bias`).
pub fn load(at: &str) -> Result<Experiment> {
    Experiment::parse(&source_toml(at)?).with_context(|| at.to_string())
}

/// An experiment's TOML: a file's (a `.lam` file's binding), or a package's resource's by full key.
pub fn source_toml(at: &str) -> Result<String> {
    let path = Path::new(at);
    if path.exists() {
        return file_toml(path);
    }
    match crate::resources::find(at) {
        Some(_) => crate::script::data::resource_toml(at).map_err(anyhow::Error::msg),
        None => bail!("no experiment file or resource `{at}`"),
    }
}

/// An experiment file's text as TOML: a `.lam` file's binding named after the file (`ssao.lam` is `ssao`, as a
/// resource's is after its key, lab/src/script/data.rs), else the file's own text.
fn file_toml(path: &Path) -> Result<String> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    if path.extension().is_some_and(|e| e == "lam") {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        let name = lamlet_present::vocab::camel(&[stem]).map_err(anyhow::Error::msg)?;
        return crate::script::data::toml(&path.display().to_string(), &text, &name).map_err(anyhow::Error::msg);
    }
    Ok(text)
}

/// How the viewer compares the reference (A) with the live state (B).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Compare {
    /// The live state alone.
    #[default]
    Off,
    /// The reference left of a line, the live state right of it.
    Wipe,
    /// The two side by side, each in half the scene area.
    SideBySide,
    /// Their difference as a heat map (`compositeDiffFS`).
    Difference,
    /// The reference alone (a flicker test: toggle it).
    Reference,
}

impl Compare {
    pub const ALL: [Compare; 5] = [Compare::Off, Compare::Wipe, Compare::SideBySide, Compare::Difference, Compare::Reference];
}

/// The viewer's experiment: the reference it compares against, how, and what it measured.
#[derive(Clone, Debug)]
pub struct Session {
    /// The reference's values (its camera follows the live one).
    pub reference: Option<ShotState>,
    pub compare: Compare,
    /// The wipe's line, as a share of the scene area's width.
    pub split: f32,
    /// The difference view's gain.
    pub gain: f32,
    pub measurement: Option<crate::metrics::Measurement>,
    /// The last message (saved, loaded, an error).
    pub message: Option<String>,
    /// The experiments offered, read when the tab opens and after a save.
    pub presets: Vec<Preset>,
}

impl Default for Session {
    fn default() -> Self {
        Session { reference: None, compare: Compare::Off, split: 0.5, gain: 4.0, measurement: None, message: None, presets: Vec::new() }
    }
}

impl Session {
    /// The reference as it is rendered: its values, with the live state's camera and demonstration.
    pub fn reference_state(&self, live: &ShotState) -> Option<ShotState> {
        let reference = self.reference.as_ref()?;
        Some(ShotState { camera: live.camera, values: reference.values.clone(), ..live.clone() })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_experiment_in_lamlet_reads_as_its_toml() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../packages/o3de_lighting/experiments");
        let lam = super::file_toml(&dir.join("ssao.lam")).unwrap();
        let toml = r#"name = "ssao"
description = "O3DE's SSAO sampling radius swept, with and without its blur, against no SSAO."
investigation = "o3de/lighting"
demo = "ssao"
resolution = [1280, 720]
frames = 2
warmup = 30
repeats = 30
metrics = ["gpu_ms", "mse", "psnr", "ssim", "flip", "luminance"]
[base.values]
viewMode = "final"
[reference.values]
ssao = false
[[sweep]]
values = { ssaoRadius = [0.02, 0.05, 0.1, 0.2] }
[[variant]]
name = "no-blur"
values = { ssaoBlur = false }
"#;
        assert_eq!(Experiment::parse(&lam).unwrap(), Experiment::parse(toml).unwrap());
    }

    use super::*;

    const FILE: &str = r#"
name = "rays"
demo = "room"
resolution = [320, 180]
frames = 4
metrics = ["psnr", "gpu_ms"]
[camera]
yaw = 30.0
[base.values]
energy = 1.0
[reference]
frames = 32
[reference.values]
rays = "128"
[[sweep]]
values = { rays = ["4", "8"], bias = [0.5, 1.0] }
[[sweep]]
zip = true
values = { a = [1, 2], b = [3, 4] }
[[variant]]
name = "off"
values = { energy = 0.0 }
"#;

    #[test]
    fn parse_and_expand() {
        let e = Experiment::parse(FILE).unwrap();
        assert_eq!((e.name.as_str(), e.resolution, e.frames, e.repeats), ("rays", [320, 180], 4, 3));
        assert_eq!(e.metrics, ["psnr", "gpu_ms"]);
        assert_eq!(e.camera.get("camera.yaw"), Some(&serde_json::json!(30.0)));
        assert_eq!(e.reference.as_ref().unwrap().frames, Some(32));
        let runs = e.runs();
        let names: Vec<&str> = runs.iter().map(|r| r.name.as_str()).collect();
        // base, the 2 × 2 grid (keys in order), the zipped pair, the variant.
        assert_eq!(names, ["base", "bias=0.5,rays=4", "bias=0.5,rays=8", "bias=1.0,rays=4", "bias=1.0,rays=8", "a=1,b=3", "a=2,b=4", "off"]);
        assert_eq!(runs[1].values.get("energy"), Some(&serde_json::json!(1.0)));
        assert_eq!(runs[7].values.get("energy"), Some(&serde_json::json!(0.0)));
        let reference = e.reference_values().unwrap();
        assert_eq!((reference.get("rays"), reference.get("energy")), (Some(&serde_json::json!("128")), Some(&serde_json::json!(1.0))));
    }

    #[test]
    fn round_trip() {
        let e = Experiment::parse(FILE).unwrap();
        let again = Experiment::parse(&e.to_toml()).unwrap();
        assert_eq!(e, again);
    }

    #[test]
    fn errors() {
        assert!(Experiment::parse("nme = 1").unwrap_err().to_string().contains("unknown field `nme`"));
        assert!(Experiment::parse("metrics = [\"psnrr\"]").unwrap_err().to_string().contains("unknown metric"));
        // A resource of the frame's graph in place of the frame, read back as written.
        let e = Experiment::parse("[inspect]\nof = \"blurCS.blur\"\nshow = \"ramp\"\nvalues = { range = [0.0, 4.0], channel = 2 }\n").unwrap();
        let i = e.inspect.as_ref().unwrap();
        assert_eq!((i.of.as_str(), i.show.as_deref(), i.values["range"].clone()), ("blurCS.blur", Some("ramp"), vec![0.0, 4.0]));
        assert_eq!(Experiment::parse(&e.to_toml()).unwrap(), e);
        assert!(Experiment::parse("[inspect]\nof = \"a.b\"\nshow = \"rainbow\"\n").is_err());
        assert!(Experiment::parse("[inspect]\nof = \"a.b\"\nvalues = { gain = 1 }\n").is_err());
        assert!(Experiment::parse("[[sweep]]\nvalues = { a = [] }").is_err());
        assert!(Experiment::parse("[[sweep]]\nzip = true\nvalues = { a = [1], b = [1, 2] }").is_err());
        assert!(Experiment::parse("[[variant]]\nname = \"base\"").is_err());
        assert_eq!(file_name(" my test/1 "), "my-test-1");
    }
}
