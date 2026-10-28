//! The resource pipeline: it reads the packages' collections
//! (src/packages.rs), in a source package's form or a built package's
//! (`Form`), and makes a resource's output for its built package from its
//! source (`Declaration::produce`, run by the build, src/cook.rs).
//!
//! A collection is a list of `[[resource]]` and `[[shader]]` tables. Every
//! resource has
//! - `id`: its key, unique in its package; lower-case words joined by `-`, in
//!   groups joined by `/` (`font/dejavu-sans`). Its full key is
//!   `<package>.<id>`;
//! - `kind`: its processor, one of `KINDS`;
//! - `source`: the file it is made from, relative to its collection and
//!   inside its package;
//! - `license`: the source's license, as THIRD_PARTY.md records it;
//!
//! and every parameter its kind takes (there are no defaults). A resource
//! may be `overridable = true`: a package that imports its own may then
//! declare one with `override = "<package>.<id>"`, which replaces it for
//! everyone. A resource `built = true` is a file one of the lab's tools
//! writes into its package (a timeline, a narration track, under the package's
//! `build/`): it may be missing until it is built.
//!
//! A `[[shader]]` is a WGSL module the lab composes at run time
//! (src/shaders.rs): its `id`, and its `parts`, in order, each a WGSL
//! resource or another shader. References (`parts`, `override`) name a key of
//! the package or `<package>.<key>`, a key an imported package exports.
//!
//! A built package's collection (src/cook.rs writes it) names each
//! resource's output instead, `file` (relative to it), and its `stamp`: a
//! hash of its kind, its processor's code, its parameters, and its source's
//! size and time, so a build that changes none of them keeps the output.

mod mesh;
mod radiance;

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A parameter's type.
#[derive(Clone, Copy, PartialEq)]
pub enum Ty {
    Float,
    Int,
}

/// A resource's parameters, checked against its kind's.
#[derive(Clone)]
pub struct Params(BTreeMap<String, f64>);

impl Params {
    pub fn float(&self, name: &str) -> f32 {
        self.0[name] as f32
    }
    pub fn int(&self, name: &str) -> usize {
        self.0[name] as usize
    }
}

/// A processor: its parameters, its code (part of the cache key), and the
/// function that makes the output from the source's bytes.
pub struct Kind {
    pub name: &'static str,
    pub params: &'static [(&'static str, Ty)],
    pub code: &'static [&'static str],
    pub process: fn(&[u8], &Params) -> Result<Vec<u8>, String>,
}

pub const KINDS: &[Kind] = &[
    // The source's bytes as they are.
    Kind { name: "file", params: &[], code: &[], process: |bytes, _| Ok(bytes.to_vec()) },
    // A directory of files read where they are (the images a timeline names
    // beside it), linked into the built package file by file.
    Kind { name: "directory", params: &[], code: &[], process: |_, _| Err("a directory is linked file by file, not processed".into()) },
    radiance::KIND,
    mesh::KIND,
];

/// The fields every resource has, and the overlay's flags.
const FIELDS: &[&str] = &["id", "kind", "source", "license", "overridable", "override", "built"];
/// A built collection's fields (src/cook.rs writes them).
const BUILT_FIELDS: &[&str] = &["id", "kind", "file", "source", "license", "stamp", "overridable", "override", "built"];

/// Which form of a collection to read: a source package's (`source` and the
/// kind's parameters), or a built package's (`file`, the processed output).
#[derive(Clone, Copy, PartialEq)]
pub enum Form {
    Source,
    Built,
}

/// A resource as its collection declares it.
#[derive(Clone)]
pub struct Declaration {
    pub id: String,
    pub kind: &'static Kind,
    /// Paths from the lab's root, for messages and listings.
    pub manifest: String,
    pub source: String,
    pub license: String,
    pub params: Params,
    /// The file to read: a source package's source, or a built package's output.
    pub file: PathBuf,
    /// A source's path in its package (what its output is named after).
    pub local: String,
    /// A built resource's stamp, as the build recorded it.
    pub stamp: Option<String>,
    /// A package that imports its own may replace it.
    pub overridable: bool,
    /// The full key of the overridable resource it replaces.
    pub overrides: Option<String>,
    /// A tool's output (maybe not built yet).
    pub built: bool,
}

/// A WGSL module as its collection declares it.
pub struct ShaderDeclaration {
    pub id: String,
    pub manifest: String,
    /// Resource or shader full keys, in order.
    pub parts: Vec<String>,
    /// The parts as the collection wrote them (a built collection keeps them).
    pub written: Vec<String>,
}

/// Everything a collection declares.
#[derive(Default)]
pub struct Collection {
    pub resources: Vec<Declaration>,
    pub shaders: Vec<ShaderDeclaration>,
}

pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.split('/').all(|group| !group.is_empty() && group.split('-').all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())))
}

/// Reads one collection of the package `package` (in `package_dir`), in
/// `form`: its ids become full keys, and `resolve` turns its references into
/// full keys. A source must be inside its package.
pub fn read_collection(root: &Path, package_dir: &Path, path: &Path, package: &str, form: Form, resolve: &dyn Fn(&str) -> Result<String, String>) -> Result<Collection, String> {
    let mut out = Collection::default();
    let name = relative(root, path);
    let fail = |message: String| format!("resources: {name}: {message}");
    let text = fs::read_to_string(path).map_err(|e| fail(format!("read: {e}")))?;
    let document = toml_edit::Document::parse(text).map_err(|e| fail(format!("parse: {e}")))?;
    let dir = path.parent().unwrap();
    for (key, _) in document.as_table() {
        if key != "resource" && key != "shader" {
            return Err(fail(format!("unknown key `{key}` (a collection has `[[resource]]` and `[[shader]]` tables)")));
        }
    }
    if let Some(shaders) = document.get("shader") {
        let tables = shaders.as_array_of_tables().ok_or_else(|| fail("`shader` is a list of `[[shader]]` tables".into()))?;
        for table in tables {
            let id = table.get("id").and_then(|i| i.as_str()).filter(|s| valid_id(s)).ok_or_else(|| fail("a shader needs `id`, lower-case words joined by `-`, in groups joined by `/`".into()))?.to_string();
            let fail = |message: String| fail(format!("shader `{id}`: {message}"));
            if let Some((key, _)) = table.iter().find(|(key, _)| *key != "id" && *key != "parts") {
                return Err(fail(format!("unknown field `{key}` (a shader has `id` and `parts`)")));
            }
            let parts: Option<Vec<String>> = table.get("parts").and_then(|p| p.as_array()).and_then(|a| a.iter().map(|v| v.as_str().map(String::from)).collect());
            let parts = parts.filter(|p| !p.is_empty()).ok_or_else(|| fail("`parts` is a list of resource and shader ids".into()))?;
            let written = parts.clone();
            let parts = parts.iter().map(|p| resolve(p).map_err(|e| fail(format!("part {e}")))).collect::<Result<_, _>>()?;
            out.shaders.push(ShaderDeclaration { id: format!("{package}.{id}"), manifest: name.clone(), parts, written });
        }
    }
    let Some(resources) = document.get("resource") else { return Ok(out) };
    let tables = resources.as_array_of_tables().ok_or_else(|| fail("`resource` is a list of `[[resource]]` tables".into()))?;
    for table in tables {
        let string = |field: &str| -> Result<String, String> {
            match table.get(field).and_then(|item| item.as_str()) {
                Some(s) if !s.is_empty() => Ok(s.to_string()),
                _ => Err(fail(format!("a resource needs `{field}`, a string"))),
            }
        };
        let id = string("id")?;
        let fail = |message: String| fail(format!("resource `{id}`: {message}"));
        if !valid_id(&id) {
            return Err(fail("ids are lower-case words joined by `-`, in groups joined by `/`".into()));
        }
        let kind_name = string("kind")?;
        let kind = KINDS
            .iter()
            .find(|k| k.name == kind_name)
            .ok_or_else(|| fail(format!("unknown kind `{kind_name}` (kinds: {})", KINDS.iter().map(|k| k.name).collect::<Vec<_>>().join(", "))))?;
        let flag = |field: &str| -> Result<bool, String> {
            match table.get(field) {
                None => Ok(false),
                Some(item) => item.as_bool().ok_or_else(|| fail(format!("`{field}` is true or false"))),
            }
        };
        let (overridable, built) = (flag("overridable")?, flag("built")?);
        let overrides = match table.get("override") {
            None => None,
            Some(item) => {
                let target = item.as_str().ok_or_else(|| fail("`override` names the resource it replaces, `<package>.<id>`".into()))?;
                Some(resolve(target).map_err(|e| fail(format!("override {e}")))?)
            }
        };
        if form == Form::Built {
            if let Some((key, _)) = table.iter().find(|(key, _)| !BUILT_FIELDS.contains(key)) {
                return Err(fail(format!("unknown field `{key}` in a built collection")));
            }
            let file = string("file")?;
            let path = dir.join(&file);
            if !built && !path.exists() {
                return Err(fail(format!("file `{file}`: no such file (rebuild the package)")));
            }
            let stamp = table.get("stamp").and_then(|s| s.as_str()).map(String::from);
            let source = table.get("source").and_then(|s| s.as_str()).unwrap_or(&file).to_string();
            out.resources.push(Declaration { license: string("license")?, id: format!("{package}.{id}"), kind, manifest: name.clone(), source, params: Params(BTreeMap::new()), file: path, local: file, stamp, overridable, overrides, built });
            continue;
        }
        let mut params = BTreeMap::new();
        for (key, item) in table {
            if FIELDS.contains(&key) {
                continue;
            }
            let Some(&(_, ty)) = kind.params.iter().find(|(name, _)| *name == key) else {
                return Err(fail(format!("`{key}` is not a parameter of kind `{}`", kind.name)));
            };
            let value = match (ty, item.as_value()) {
                (Ty::Float, Some(toml_edit::Value::Float(f))) => *f.value(),
                (Ty::Float | Ty::Int, Some(toml_edit::Value::Integer(i))) if *i.value() >= 0 || ty == Ty::Float => *i.value() as f64,
                _ => return Err(fail(format!("`{key}` is {}", if ty == Ty::Float { "a number" } else { "a whole number, at least 0" }))),
            };
            params.insert(key.to_string(), value);
        }
        for (name, _) in kind.params {
            if !params.contains_key(*name) {
                return Err(fail(format!("kind `{}` needs `{name}`", kind.name)));
            }
        }
        let source_name = string("source")?;
        if Path::new(&source_name).is_absolute() {
            return Err(fail("`source` is relative to the collection".into()));
        }
        let file = if built {
            // It may not be built yet.
            fs::canonicalize(dir).map_err(|e| fail(format!("{e}")))?.join(&source_name)
        } else {
            let found = if kind.name == "directory" { dir.join(&source_name).is_dir() } else { dir.join(&source_name).is_file() };
            if !found {
                return Err(fail(format!("source `{source_name}`: no such {}", if kind.name == "directory" { "directory" } else { "file" })));
            }
            fs::canonicalize(dir.join(&source_name)).map_err(|e| fail(format!("source `{source_name}`: {e}")))?
        };
        let package_dir = fs::canonicalize(package_dir).map_err(|e| fail(format!("{e}")))?;
        if !file.starts_with(&package_dir) {
            return Err(fail(format!("source `{source_name}` is outside its package (a package is self-contained)")));
        }
        let local = relative(&package_dir, &file);
        out.resources.push(Declaration { license: string("license")?, id: format!("{package}.{id}"), kind, manifest: name.clone(), source: relative(root, &file), params: Params(params), file, local, stamp: None, overridable, overrides, built });
    }
    Ok(out)
}

impl Declaration {
    /// Its key in its package (its full key without the package).
    pub fn key(&self) -> &str {
        self.id.split_once('.').map(|(_, k)| k).unwrap_or(&self.id)
    }

    /// Its output's path in its built package: a `file` resource keeps its
    /// path in the package; a processed one is `cooked/<key>.bin`.
    pub fn output(&self) -> String {
        if self.kind.name == "file" || self.kind.name == "directory" { self.local.clone() } else { format!("cooked/{}.bin", self.key().replace('/', "-")) }
    }

    /// A directory resource's files, relative to it, sorted.
    pub fn files(&self) -> Vec<String> {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
            for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() { walk(&path, base, out) } else { out.push(relative(base, &path)) }
            }
        }
        let mut out = Vec::new();
        walk(&self.file, &self.file, &mut out);
        out.sort();
        out
    }

    /// What its output is made from: its kind, its processor's code, its
    /// parameters, and its source's size and time (a source's). None while a
    /// built source is missing.
    pub fn stamp(&self) -> Option<String> {
        let size_and_time = |path: &Path| -> Option<String> {
            let meta = fs::metadata(path).ok()?;
            Some(format!("{}:{}", meta.len(), meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos()))
        };
        let mut hash = Sha256::new();
        hash.update(self.kind.name);
        for code in self.kind.code {
            hash.update(code);
        }
        for (key, value) in &self.params.0 {
            hash.update(format!("{key}={value:?};"));
        }
        if self.kind.name == "directory" {
            if !self.file.is_dir() {
                return None;
            }
            for file in self.files() {
                hash.update(format!("{file}={};", size_and_time(&self.file.join(&file))?));
            }
        } else {
            hash.update(size_and_time(&self.file)?);
        }
        Some(hash.finalize().iter().take(12).map(|b| format!("{b:02x}")).collect())
    }

    /// Makes its output at `out` from its source: a `file` resource is
    /// linked (or copied), a `directory` file by file, a processed one is
    /// processed and written.
    pub fn produce(&self, out: &Path) -> Result<(), String> {
        if self.kind.name == "directory" {
            for file in self.files() {
                let one = Declaration { kind: &KINDS[0], file: self.file.join(&file), ..self.clone() };
                one.produce(&out.join(&file))?;
            }
            return Ok(());
        }
        if let Some(dir) = out.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("resources: create {}: {e}", dir.display()))?;
        }
        // Written beside it, then moved, so a reader never sees half a file.
        let partial = out.with_extension(format!("partial.{}", std::process::id()));
        if self.kind.name == "file" {
            // Already a link to its source (a rename onto a link to the same file does nothing).
            if crate::cook::same_file(&self.file, out) {
                return Ok(());
            }
            if fs::hard_link(&self.file, &partial).is_err() {
                fs::copy(&self.file, &partial).map_err(|e| format!("resources: {}: copy {}: {e}", self.id, self.source))?;
            }
        } else {
            let bytes = fs::read(&self.file).map_err(|e| format!("resources: {}: read {}: {e}", self.id, self.source))?;
            let processed = (self.kind.process)(&bytes, &self.params).map_err(|e| format!("resources: {} ({}): {e}", self.id, self.manifest))?;
            fs::write(&partial, &processed).map_err(|e| format!("resources: write {}: {e}", partial.display()))?;
        }
        fs::rename(&partial, out).map_err(|e| format!("resources: write {}: {e}", out.display()))
    }
}

/// The little-endian writer processors use: `u32` words, then raw arrays.
pub fn put_u32(out: &mut Vec<u8>, words: &[u32]) {
    for w in words {
        out.extend_from_slice(&w.to_le_bytes());
    }
}

pub fn put_f32(out: &mut Vec<u8>, values: &[f32]) {
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
}
