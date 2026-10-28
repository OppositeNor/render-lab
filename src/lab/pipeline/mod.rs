//! The resource pipeline, run by the lab (src/lab/resources.rs) when it loads
//! a resource. It reads the packages' manifests (a layer keeps its
//! `resources.toml` beside its code), runs each resource through its kind's
//! processor, and keeps the result in a cache under the lab's root.
//!
//! A manifest is a list of `[[resource]]` and `[[shader]]` tables, and may
//! `include` other manifests (paths relative to it; never another
//! `resources.toml`, which is a package of its own). Every resource has
//! - `id`: unique across all packages; lower-case words joined by `-`, in
//!   groups joined by `/` (`font/dejavu-sans`);
//! - `kind`: its processor, one of `KINDS`;
//! - `source`: the file it is made from, relative to its manifest;
//! - `license`: the source's license, as THIRD_PARTY.md records it;
//!
//! and every parameter its kind takes (there are no defaults). A resource
//! may be `overridable = true`: a package mounted after its own may then
//! declare the same id with `override = true`, which replaces it.
//!
//! A `[[shader]]` is a WGSL module the lab composes at run time
//! (src/lab/shaders.rs): its `id`, and its `parts`, in order, each a WGSL
//! resource or another shader.
//!
//! A processed resource is cached in build/resources under a hash of its kind,
//! its processor's code, its parameters, and its source's bytes, so a run that
//! changes none of them reuses it.

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
    radiance::KIND,
    mesh::KIND,
];

/// The fields every resource has, and the overlay's flags.
const FIELDS: &[&str] = &["id", "kind", "source", "license", "overridable", "override"];

/// A resource as its manifest declares it.
pub struct Declaration {
    pub id: String,
    pub kind: &'static Kind,
    /// Paths from the lab's root, for messages and listings.
    pub manifest: String,
    pub source: String,
    pub license: String,
    pub params: Params,
    /// A later package may replace it.
    pub overridable: bool,
    /// It replaces an overridable resource of an earlier package.
    pub overrides: bool,
}

/// A WGSL module as its manifest declares it.
pub struct ShaderDeclaration {
    pub id: String,
    pub manifest: String,
    /// Resource or shader ids, in order.
    pub parts: Vec<String>,
}

/// Everything a package's manifests declare.
#[derive(Default)]
pub struct Package {
    pub resources: Vec<Declaration>,
    pub shaders: Vec<ShaderDeclaration>,
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.split('/').all(|group| !group.is_empty() && group.split('-').all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())))
}

/// Reads a package's manifest and those it includes.
pub fn read_package(root: &Path, manifest: &Path) -> Result<Package, String> {
    let mut out = Package::default();
    read_manifest(root, manifest, &mut Vec::new(), &mut out)?;
    Ok(out)
}

/// Reads a manifest and those it includes; `chain` holds the manifests that
/// include this one, to catch a cycle.
fn read_manifest(root: &Path, path: &Path, chain: &mut Vec<PathBuf>, out: &mut Package) -> Result<(), String> {
    let name = relative(root, path);
    let fail = |message: String| format!("resources: {name}: {message}");
    if chain.contains(&path.to_path_buf()) {
        return Err(fail(format!("included in a cycle: {}", chain.iter().map(|p| relative(root, p)).collect::<Vec<_>>().join(" -> "))));
    }
    let text = fs::read_to_string(path).map_err(|e| fail(format!("read: {e}")))?;
    let document = toml_edit::Document::parse(text).map_err(|e| fail(format!("parse: {e}")))?;
    let dir = path.parent().unwrap();
    for (key, _) in document.as_table() {
        if key != "include" && key != "resource" && key != "shader" {
            return Err(fail(format!("unknown key `{key}` (a manifest has `include`, `[[resource]]`, and `[[shader]]` tables)")));
        }
    }
    chain.push(path.to_path_buf());
    if let Some(include) = document.get("include") {
        let list = include.as_array().ok_or_else(|| fail("`include` is a list of paths".into()))?;
        for entry in list {
            let file = entry.as_str().ok_or_else(|| fail("`include` is a list of paths".into()))?;
            if Path::new(file).is_absolute() || !file.ends_with(".toml") || file.ends_with("resources.toml") {
                return Err(fail(format!("include `{file}`: a relative path to a .toml manifest other than a resources.toml")));
            }
            let target = dir.join(file);
            if !target.is_file() {
                return Err(fail(format!("include `{file}`: no such file")));
            }
            read_manifest(root, &target, chain, out)?;
        }
    }
    chain.pop();
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
            out.shaders.push(ShaderDeclaration { id, manifest: name.clone(), parts });
        }
    }
    let Some(resources) = document.get("resource") else { return Ok(()) };
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
        let (overridable, overrides) = (flag("overridable")?, flag("override")?);
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
            return Err(fail("`source` is relative to the manifest".into()));
        }
        let source = dir.join(&source_name);
        if !source.is_file() {
            return Err(fail(format!("source `{source_name}`: no such file")));
        }
        let source = fs::canonicalize(&source).map_err(|e| fail(format!("source `{source_name}`: {e}")))?;
        out.resources.push(Declaration { license: string("license")?, id, kind, manifest: name.clone(), source: relative(root, &source), params: Params(params), overridable, overrides });
    }
    Ok(())
}

impl Declaration {
    /// The file its processed bytes are cached in, under `cache`, for these
    /// source bytes.
    fn cache_file(&self, cache: &Path, bytes: &[u8]) -> PathBuf {
        let mut hash = Sha256::new();
        hash.update(self.kind.name);
        for code in self.kind.code {
            hash.update(code);
        }
        for (key, value) in &self.params.0 {
            hash.update(format!("{key}={value:?};"));
        }
        hash.update(bytes);
        let key: String = hash.finalize().iter().take(12).map(|b| format!("{b:02x}")).collect();
        cache.join(format!("{}-{key}.bin", self.id.replace('/', "-")))
    }

    /// Processes it into the cache if it is not there yet.
    pub fn prepare(&self, root: &Path, cache: &Path) -> Result<(), String> {
        let bytes = fs::read(root.join(&self.source)).map_err(|e| format!("resources: {}: read {}: {e}", self.id, self.source))?;
        if self.cache_file(cache, &bytes).is_file() {
            return Ok(());
        }
        self.process(&bytes, cache).map(|_| ())
    }

    /// The processed bytes: from the cache, or processed now and cached.
    /// Returns them with the cache file and whether it was already there.
    pub fn cook(&self, root: &Path, cache: &Path) -> Result<(Vec<u8>, PathBuf, bool), String> {
        let bytes = fs::read(root.join(&self.source)).map_err(|e| format!("resources: {}: read {}: {e}", self.id, self.source))?;
        let output = self.cache_file(cache, &bytes);
        if let Ok(processed) = fs::read(&output) {
            return Ok((processed, output, true));
        }
        let (processed, output) = self.process(&bytes, cache)?;
        Ok((processed, output, false))
    }

    /// Processes the source's `bytes` and caches the result.
    fn process(&self, bytes: &[u8], cache: &Path) -> Result<(Vec<u8>, PathBuf), String> {
        let output = self.cache_file(cache, bytes);
        let processed = (self.kind.process)(bytes, &self.params).map_err(|e| format!("resources: {} ({}): {e}", self.id, self.manifest))?;
        fs::create_dir_all(cache).map_err(|e| format!("resources: create {}: {e}", cache.display()))?;
        // Another run may cook the same resource at the same time.
        let partial = output.with_extension(format!("bin.{}.partial", std::process::id()));
        fs::write(&partial, &processed).and_then(|_| fs::rename(&partial, &output)).map_err(|e| format!("resources: write {}: {e}", output.display()))?;
        Ok((processed, output))
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
