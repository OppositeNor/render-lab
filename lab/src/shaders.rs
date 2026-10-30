//! WGSL shader modules, composed at run time from the packages' `[[shader]]`
//! tables (packages/rlstd/shaders.toml, and each investigation's package): a module is its
//! parts in order, each a WGSL resource or another module. The lab's pass
//! modules start with `shader/prelude` (types, bindings, and common math). naga, the
//! compiler wgpu uses, validates a module before the lab creates it, so a
//! broken edit is an error that names the file and the line; it also computes
//! the struct layouts of `wgsl/types` that the struct panel shows and
//! layout.rs checks.

use crate::resources;
use anyhow::{anyhow, bail, Result};
use naga::valid::{Capabilities, ValidationFlags, Validator};
use naga::{Scalar, ScalarKind, TypeInner, VectorSize};
use serde_json::{json, Value};

/// The lab's pass modules.
pub const SCENE: &str = "rlstd.shader/scene";
pub const OVERLAY: &str = "rlstd.shader/overlay";
pub const VERIFY: &str = "rlstd.shader/verify";
pub const FROST: &str = "rlstd.shader/frost";
/// The records the struct panel displays.
const TYPES: &str = "rlstd.wgsl/types";

/// The files a module is made of, in order (resource ids).
pub fn files(id: &str) -> Result<Vec<&'static str>> {
    fn expand(id: &str, chain: &mut Vec<String>, out: &mut Vec<&'static str>) -> Result<()> {
        if chain.iter().any(|c| c == id) {
            bail!("shader `{id}` includes itself: {}", chain.join(" -> "));
        }
        let shader = resources::shader(id).ok_or_else(|| anyhow!("no shader `{id}`"))?;
        chain.push(id.to_string());
        for part in &shader.parts {
            match resources::shader(part) {
                Some(_) => expand(part, chain, out)?,
                None => out.push(resources::get(part).id()),
            }
        }
        chain.pop();
        Ok(())
    }
    let mut out = Vec::new();
    expand(id, &mut Vec::new(), &mut out)?;
    Ok(out)
}

/// The module's WGSL: its files joined, unvalidated.
pub fn compose(id: &str) -> Result<String> {
    Ok(files(id)?.iter().map(|f| resources::get(f).text()).collect())
}

/// Checks a module's WGSL with naga; the error names the file and line.
pub fn validate(id: &str, source: &str) -> Result<naga::Module> {
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|e| anyhow!("WGSL parse error in {}{}:\n{}", describe(id), locate(id, e.location(source).map(|l| l.line_number)), e.emit_to_string(source)))?;
    Validator::new(ValidationFlags::all(), Capabilities::default())
        .validate(&module)
        .map_err(|e| anyhow!("WGSL validation error in {}{}:\n{}", describe(id), locate(id, e.location(source).map(|l| l.line_number)), e.emit_to_string(source)))?;
    Ok(module)
}

/// ", at file:line" for a line of the composed module (each file ends with
/// a newline, so the composed lines are the files' lines in order).
pub(crate) fn locate(id: &str, line: Option<u32>) -> String {
    let Some(mut line) = line else { return String::new() };
    for file in files(id).unwrap_or_default() {
        let resource = resources::get(file);
        let lines = resource.text().lines().count() as u32;
        if line <= lines {
            return format!(", at {}:{line}", resource.source());
        }
        line -= lines;
    }
    String::new()
}

/// A module as naga reads it with its `///` comments, which carry the tags of
/// its values (src/shader_data.rs).
pub struct Reflected {
    pub id: &'static str,
    /// Its composed WGSL.
    pub source: String,
    pub module: naga::Module,
}

impl Reflected {
    /// ", at file:line" for the first line of the composed module that holds `text`.
    pub fn locate(&self, text: &str) -> String {
        let line = self.source.lines().position(|l| l.contains(text)).map(|i| i as u32 + 1);
        locate(self.id, line)
    }
}

/// The module `id`, composed, validated, and read with its doc comments;
/// once per run (the packages do not change while the lab runs).
pub fn reflect(id: &str) -> Result<&'static Reflected> {
    static CACHE: std::sync::Mutex<Vec<&'static Reflected>> = std::sync::Mutex::new(Vec::new());
    if let Some(r) = CACHE.lock().unwrap().iter().find(|r| r.id == id) {
        return Ok(r);
    }
    let source = module(id)?;
    let mut options = naga::front::wgsl::Options::new();
    options.parse_doc_comments = true;
    let module = naga::front::wgsl::Frontend::new_with_options(options)
        .parse(&source)
        .map_err(|e| anyhow!("WGSL parse error (with doc comments) in {}{}:\n{}", describe(id), locate(id, e.location(&source).map(|l| l.line_number)), e.emit_to_string(&source)))?;
    let reflected: &'static Reflected = Box::leak(Box::new(Reflected { id: Box::leak(id.to_string().into_boxed_str()), source, module }));
    let mut cache = CACHE.lock().unwrap();
    // Another thread may have read it meanwhile; keep one.
    if let Some(r) = cache.iter().find(|r| r.id == id) {
        return Ok(r);
    }
    cache.push(reflected);
    Ok(reflected)
}

/// The module `id`, composed and validated: what the lab hands wgpu.
pub fn module(id: &str) -> Result<String> {
    let source = compose(id)?;
    validate(id, &source)?;
    Ok(source)
}

/// A module and the source files it is made of, for messages.
fn describe(id: &str) -> String {
    let files: Vec<&str> = files(id).unwrap_or_default().iter().map(|f| resources::get(f).source()).collect();
    format!("shader `{id}` ({})", files.join(" + "))
}

/// The type declarations the struct panel displays.
pub fn types_source() -> &'static str {
    resources::get(TYPES).text()
}

fn scalar(s: Scalar) -> String {
    let kind = match s.kind {
        ScalarKind::Float => "f",
        ScalarKind::Uint => "u",
        ScalarKind::Sint => "i",
        ScalarKind::Bool => return "bool".into(),
        _ => "?",
    };
    format!("{kind}{}", s.width as u32 * 8)
}

fn size(v: VectorSize) -> u32 {
    v as u32
}

fn type_name(module: &naga::Module, ty: naga::Handle<naga::Type>) -> String {
    let t = &module.types[ty];
    match &t.inner {
        TypeInner::Scalar(s) => scalar(*s),
        TypeInner::Vector { size: n, scalar: s } => format!("vec{}<{}>", size(*n), scalar(*s)),
        TypeInner::Matrix { columns, rows, scalar: s } => format!("mat{}x{}<{}>", size(*columns), size(*rows), scalar(*s)),
        TypeInner::Struct { .. } => t.name.clone().unwrap_or_else(|| "struct".into()),
        _ => "?".into(),
    }
}

/// Size, alignment, and member offsets of every struct in `wgsl/types`, as
/// naga lays them out (layout.rs parses it).
pub fn layout_json() -> Result<String> {
    let source = types_source();
    let types = naga::front::wgsl::parse_str(source).map_err(|e| anyhow!("WGSL parse error in {TYPES}:\n{}", e.emit_to_string(source)))?;
    let mut layouter = naga::proc::Layouter::default();
    layouter.update(types.to_ctx()).map_err(|e| anyhow!("layout of {TYPES}: {e}"))?;
    let mut structs = serde_json::Map::new();
    for (handle, ty) in types.types.iter() {
        if let TypeInner::Struct { members, span } = &ty.inner {
            let name = ty.name.clone().unwrap_or_default();
            let members: Vec<Value> = members
                .iter()
                .map(|m| {
                    let matrix = match &types.types[m.ty].inner {
                        TypeInner::Matrix { columns, rows, .. } => json!([size(*columns), size(*rows)]),
                        _ => Value::Null,
                    };
                    json!({
                        "name": m.name.clone().unwrap_or_default(),
                        "offset": m.offset,
                        "size": layouter[m.ty].size,
                        "alignment": layouter[m.ty].alignment.round_up(1),
                        "type": type_name(&types, m.ty),
                        "matrix": matrix,
                    })
                })
                .collect();
            structs.insert(name, json!({"size": span, "alignment": layouter[handle].alignment.round_up(1), "members": members}));
        }
    }
    Ok(serde_json::to_string_pretty(&json!({"source": resources::get(TYPES).source(), "generator": "naga proc::Layouter", "structs": structs}))?)
}

/// Every shader module the packages declare, validated by naga: its files, size,
/// hash, whether it validated (and why not), and its entry points.
pub fn manifest() -> Value {
    use sha2::{Digest, Sha256};
    let modules: Vec<Value> = resources::shaders()
        .iter()
        .map(|shader| {
            let id = shader.id.as_str();
            let files: Vec<&str> = files(id).unwrap_or_default().iter().map(|f| resources::get(f).source()).collect();
            let source = compose(id);
            let checked = source.as_ref().map_err(|e| anyhow!("{e:#}")).and_then(|s| validate(id, s));
            let entry_points: Vec<Value> = checked.as_ref().map(|m| m.entry_points.iter().map(|e| json!({"name": e.name, "stage": format!("{:?}", e.stage)})).collect()).unwrap_or_default();
            let source = source.unwrap_or_default();
            json!({
                "module": id,
                "files": files,
                "bytes": source.len(),
                "sha256": Sha256::digest(source.as_bytes()).iter().map(|b| format!("{b:02x}")).collect::<String>(),
                "validated": checked.is_ok(),
                "error": checked.err().map(|e| format!("{e:#}")),
                "entryPoints": entry_points,
            })
        })
        .collect();
    json!({"shaderLanguage": "WGSL", "validator": "naga (wgsl-in, ValidationFlags::all)", "modules": modules})
}
