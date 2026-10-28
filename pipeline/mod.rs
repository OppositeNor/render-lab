//! The resource pipeline, run by build.rs. It finds every `resources.toml`
//! under src/ (a layer keeps its manifest beside its code), reads the resources
//! they declare, runs each through its kind's processor, and writes the
//! results to OUT_DIR with `resources.rs`, the index the lab embeds
//! (src/lab/resources.rs).
//!
//! A manifest is a list of `[[resource]]` tables, and may `include` other
//! manifests (paths relative to it; never another `resources.toml`, which is a
//! root of its own). Every resource has
//! - `id`: unique across all manifests; lower-case words joined by `-`, in
//!   groups joined by `/` (`font/dejavu-sans`); the lab names it by the
//!   static `FONT_DEJAVU_SANS`;
//! - `kind`: its processor, one of `KINDS`;
//! - `source`: the file it is made from, relative to its manifest;
//! - `license`: the source's license, as THIRD_PARTY.md records it;
//!
//! and every parameter its kind takes (there are no defaults).
//!
//! A processed resource is cached in OUT_DIR/resources under a hash of its
//! kind, its processor's code, its parameters, and its source's bytes, so a
//! build that changes none of them reuses it.

mod mesh;
mod radiance;
#[path = "../src/lab/rgb9e5.rs"]
mod rgb9e5;

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
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

/// The fields every resource has.
const FIELDS: &[&str] = &["id", "kind", "source", "license"];

struct Resource {
    id: String,
    kind: &'static Kind,
    /// Paths from the crate root, for messages and the index.
    manifest: String,
    source: String,
    license: String,
    params: Params,
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

/// Every `resources.toml` under `dir`, sorted.
fn find_roots(dir: &Path, roots: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            find_roots(&path, roots);
        } else if path.file_name().is_some_and(|n| n == "resources.toml") {
            roots.push(path);
        }
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.split('/').all(|group| !group.is_empty() && group.split('-').all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())))
}

/// The static the lab names a resource by.
fn static_name(id: &str) -> String {
    id.to_ascii_uppercase().replace(['-', '/'], "_")
}

/// Reads a manifest and those it includes; `chain` holds the manifests that
/// include this one, to catch a cycle.
fn read_manifest(root: &Path, path: &Path, chain: &mut Vec<PathBuf>, out: &mut Vec<Resource>) {
    let name = relative(root, path);
    let fail = |message: String| -> ! { panic!("resources: {name}: {message}") };
    if chain.contains(&path.to_path_buf()) {
        fail(format!("included in a cycle: {}", chain.iter().map(|p| relative(root, p)).collect::<Vec<_>>().join(" -> ")));
    }
    println!("cargo:rerun-if-changed={}", path.display());
    let text = fs::read_to_string(path).unwrap_or_else(|e| fail(format!("read: {e}")));
    let document = toml_edit::Document::parse(text).unwrap_or_else(|e| fail(format!("parse: {e}")));
    let dir = path.parent().unwrap();
    for (key, _) in document.as_table() {
        if key != "include" && key != "resource" {
            fail(format!("unknown key `{key}` (a manifest has `include` and `[[resource]]` tables)"));
        }
    }
    chain.push(path.to_path_buf());
    if let Some(include) = document.get("include") {
        let list = include.as_array().unwrap_or_else(|| fail("`include` is a list of paths".into()));
        for entry in list {
            let file = entry.as_str().unwrap_or_else(|| fail("`include` is a list of paths".into()));
            if Path::new(file).is_absolute() || !file.ends_with(".toml") || file.ends_with("resources.toml") {
                fail(format!("include `{file}`: a relative path to a .toml manifest other than a resources.toml"));
            }
            let target = dir.join(file);
            if !target.is_file() {
                fail(format!("include `{file}`: no such file"));
            }
            read_manifest(root, &target, chain, out);
        }
    }
    chain.pop();
    let Some(resources) = document.get("resource") else { return };
    let tables = resources.as_array_of_tables().unwrap_or_else(|| fail("`resource` is a list of `[[resource]]` tables".into()));
    for table in tables {
        let string = |field: &str| -> String {
            match table.get(field).and_then(|item| item.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => fail(format!("a resource needs `{field}`, a string")),
            }
        };
        let id = string("id");
        let fail = |message: String| -> ! { fail(format!("resource `{id}`: {message}")) };
        if !valid_id(&id) {
            fail("ids are lower-case words joined by `-`, in groups joined by `/`".into());
        }
        let kind_name = string("kind");
        let kind = KINDS.iter().find(|k| k.name == kind_name).unwrap_or_else(|| {
            fail(format!("unknown kind `{kind_name}` (kinds: {})", KINDS.iter().map(|k| k.name).collect::<Vec<_>>().join(", ")))
        });
        let mut params = BTreeMap::new();
        for (key, item) in table {
            if FIELDS.contains(&key) {
                continue;
            }
            let Some(&(_, ty)) = kind.params.iter().find(|(name, _)| *name == key) else {
                fail(format!("`{key}` is not a parameter of kind `{}`", kind.name));
            };
            let value = match (ty, item.as_value()) {
                (Ty::Float, Some(toml_edit::Value::Float(f))) => *f.value(),
                (Ty::Float | Ty::Int, Some(toml_edit::Value::Integer(i))) if *i.value() >= 0 || ty == Ty::Float => *i.value() as f64,
                _ => fail(format!("`{key}` is {}", if ty == Ty::Float { "a number" } else { "a whole number, at least 0" })),
            };
            params.insert(key.to_string(), value);
        }
        for (name, _) in kind.params {
            if !params.contains_key(*name) {
                fail(format!("kind `{}` needs `{name}`", kind.name));
            }
        }
        let source_name = string("source");
        if Path::new(&source_name).is_absolute() {
            fail("`source` is relative to the manifest".into());
        }
        let source = dir.join(&source_name);
        if !source.is_file() {
            fail(format!("source `{source_name}`: no such file"));
        }
        let source = fs::canonicalize(&source).unwrap();
        out.push(Resource { license: string("license"), id, kind, manifest: name.clone(), source: relative(root, &source), params: Params(params) });
    }
}

/// Runs the pipeline: reads the manifests under `root`/src, processes their
/// resources into `out_dir`/resources, and writes `out_dir`/resources.rs.
pub fn build(root: &Path, out_dir: &Path) {
    // A new manifest anywhere under src/ reruns the build script.
    println!("cargo:rerun-if-changed={}", root.join("src").display());
    for file in ["mod.rs", "radiance.rs", "mesh.rs"] {
        println!("cargo:rerun-if-changed={}", root.join("pipeline").join(file).display());
    }
    let mut roots = Vec::new();
    find_roots(&root.join("src"), &mut roots);
    let mut resources = Vec::new();
    for manifest in &roots {
        read_manifest(root, manifest, &mut Vec::new(), &mut resources);
    }
    let mut constants = BTreeMap::new();
    for r in &resources {
        if let Some(other) = constants.insert(static_name(&r.id), r) {
            panic!("resources: `{}` ({}) and `{}` ({}) are the same id, or name the same static", other.id, other.manifest, r.id, r.manifest);
        }
    }

    let dir = out_dir.join("resources");
    fs::create_dir_all(&dir).unwrap();
    let mut outputs = BTreeSet::new();
    let mut index = String::from("// Generated by build.rs (pipeline/mod.rs) from the resource manifests.\n\n");
    for (name, r) in &constants {
        let source = root.join(&r.source);
        println!("cargo:rerun-if-changed={}", source.display());
        let bytes = fs::read(&source).unwrap_or_else(|e| panic!("resources: {}: read {}: {e}", r.id, r.source));
        let mut hash = Sha256::new();
        hash.update(r.kind.name);
        for code in r.kind.code {
            hash.update(code);
        }
        for (key, value) in &r.params.0 {
            hash.update(format!("{key}={value:?};"));
        }
        hash.update(&bytes);
        let key: String = hash.finalize().iter().take(12).map(|b| format!("{b:02x}")).collect();
        let file = format!("{}-{key}.bin", r.id.replace('/', "-"));
        let output = dir.join(&file);
        if !output.is_file() {
            let processed = (r.kind.process)(&bytes, &r.params).unwrap_or_else(|e| panic!("resources: {} ({}): {e}", r.id, r.manifest));
            let partial = dir.join(format!("{file}.partial"));
            fs::write(&partial, processed).unwrap();
            fs::rename(&partial, &output).unwrap();
        }
        outputs.insert(file);
        writeln!(
            index,
            "pub static {name}: Resource = Resource {{ id: {:?}, kind: {:?}, manifest: {:?}, source: {:?}, license: {:?}, bytes: include_bytes!({:?}) }};",
            r.id, r.kind.name, r.manifest, r.source, r.license, output.display().to_string()
        )
        .unwrap();
    }
    writeln!(index, "\n/// Every resource, by id.\npub static ALL: &[&Resource] = &[{}];", constants.keys().map(|name| format!("&{name}")).collect::<Vec<_>>().join(", ")).unwrap();
    // Outputs of earlier builds that no resource names any more.
    for entry in fs::read_dir(&dir).unwrap().flatten() {
        if !outputs.contains(entry.file_name().to_string_lossy().as_ref()) {
            fs::remove_file(entry.path()).unwrap();
        }
    }
    fs::write(out_dir.join("resources.rs"), index).unwrap();
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
