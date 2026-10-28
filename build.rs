//! Checks the lab's sources before it is built: every WGSL module the
//! packages' manifests declare (`[[shader]]` in each `resources.toml` under
//! src/ and the manifests it includes; src/lab/shaders.rs composes the same
//! modules at run time) parses and validates with naga, and every layer's
//! text tables match the code that names their keys. A failure fails the
//! build; nothing is generated.
//!
//! It also decides whether the investigations of commercial engines are
//! built. They live in src/commercial, which is not published (see
//! .gitignore): when all of `HIDDEN` are present it sets the `commercial`
//! cfg, and when none are, the lab builds with the public investigations
//! alone.

use naga::valid::{Capabilities, ValidationFlags, Validator};
use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Each layer's on-screen text (`text/en.json`, `text/zh-CN.json`) and the
/// code that names its keys (`tr("key")`, `tr_with("key", …)`).
const TEXT_TABLES: &[(&str, &str)] = &[("src/lab/text", "src/lab"), ("src/o3de/lighting/text", "src/o3de/lighting")];
const COMMERCIAL_TEXT_TABLES: &[(&str, &str)] = &[("src/commercial/unreal/lumen/text", "src/commercial/unreal/lumen")];
const LOCALES: &[&str] = &["en", "zh-CN"];
/// The files of src/commercial the build needs, relative to the crate root.
/// They derive from commercial engines' source and stay local (.gitignore).
const HIDDEN: &[&str] = &[
    "src/commercial/mod.rs",
    "src/commercial/lab/fixtures.rs",
    "src/commercial/lab/reflection_rays.rs",
    "src/commercial/lab/surface_cache.rs",
    "src/commercial/lab/surface_cache_gpu.rs",
    "src/commercial/shaders/surface_lookup.wgsl",
    "src/commercial/shaders/surface_cache.wgsl",
];

/// What the manifests declare that the shader check needs: each resource's
/// source file (with the overlay applied), and each shader's parts.
#[derive(Default)]
struct Manifests {
    sources: BTreeMap<String, PathBuf>,
    overridable: Vec<String>,
    shaders: BTreeMap<String, Vec<String>>,
}

/// Every `resources.toml` under `dir`, sorted (the lab's own first, so the
/// overlay applies in the order the lab mounts them).
fn find_packages(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir).map(|e| e.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            find_packages(&path, out);
        } else if path.file_name().is_some_and(|n| n == "resources.toml") {
            out.push(path);
        }
    }
}

fn read_manifest(path: &Path, out: &mut Manifests) {
    println!("cargo:rerun-if-changed={}", path.display());
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let document = toml_edit::Document::parse(text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let dir = path.parent().unwrap();
    for file in document.get("include").and_then(|i| i.as_array()).into_iter().flatten() {
        read_manifest(&dir.join(file.as_str().unwrap_or_default()), out);
    }
    for table in document.get("resource").and_then(|r| r.as_array_of_tables()).into_iter().flatten() {
        let id = table.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let source = dir.join(table.get("source").and_then(|s| s.as_str()).unwrap_or_default());
        let flag = |name: &str| table.get(name).and_then(|f| f.as_bool()).unwrap_or(false);
        if flag("overridable") {
            out.overridable.push(id.clone());
        }
        // The run time reports a clash; this check only needs the winner.
        if !out.sources.contains_key(&id) || (flag("override") && out.overridable.contains(&id)) {
            out.sources.insert(id, source);
        }
    }
    for table in document.get("shader").and_then(|s| s.as_array_of_tables()).into_iter().flatten() {
        let id = table.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let parts = table.get("parts").and_then(|p| p.as_array()).into_iter().flatten().filter_map(|p| p.as_str().map(String::from)).collect();
        out.shaders.insert(id, parts);
    }
}

/// A shader's files, in order.
fn shader_files(manifests: &Manifests, id: &str, out: &mut Vec<PathBuf>) {
    for part in &manifests.shaders[id] {
        if manifests.shaders.contains_key(part) {
            shader_files(manifests, part, out);
        } else {
            out.push(manifests.sources.get(part).unwrap_or_else(|| panic!("shader `{id}`: no resource or shader `{part}`")).clone());
        }
    }
}

/// Every declared module parses and validates with naga.
fn check_shaders(manifest_dir: &Path) {
    let mut packages = Vec::new();
    find_packages(&manifest_dir.join("src/lab"), &mut packages);
    let mut others = Vec::new();
    find_packages(&manifest_dir.join("src"), &mut others);
    packages.extend(others.into_iter().filter(|p| !p.starts_with(manifest_dir.join("src/lab"))));
    // A new manifest anywhere under src/ reruns the check.
    println!("cargo:rerun-if-changed={}", manifest_dir.join("src").display());
    let mut manifests = Manifests::default();
    for package in &packages {
        read_manifest(package, &mut manifests);
    }
    for id in manifests.shaders.keys() {
        let mut files = Vec::new();
        shader_files(&manifests, id, &mut files);
        let source: String = files
            .iter()
            .map(|f| {
                println!("cargo:rerun-if-changed={}", f.display());
                fs::read_to_string(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display()))
            })
            .collect();
        let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|e| panic!("WGSL parse error in shader `{id}`:\n{}", e.emit_to_string(&source)));
        Validator::new(ValidationFlags::all(), Capabilities::default())
            .validate(&module)
            .unwrap_or_else(|e| panic!("WGSL validation error in shader `{id}`:\n{}", e.emit_to_string(&source)));
    }
}

/// The length of the tag at the start of `tail` (`{@ …}`, src/lab/rich_text.rs):
/// up to its closing brace, outside quoted values (where `\"` is a quote).
fn tag_len(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let (mut i, mut quoted) = (2, false);
    while let Some(&b) = bytes.get(i) {
        match b {
            b'\\' if quoted && bytes.get(i + 1) == Some(&b'"') => i += 1,
            b'"' => quoted = !quoted,
            b'}' if !quoted => return Some(i + 1),
            _ => {}
        }
        i += 1;
    }
    None
}

/// The LaTeX of every formula tag in a text, in order.
fn formulas(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("{@") {
        let len = tag_len(&rest[i..]).unwrap_or_else(|| panic!("unclosed tag in text {text:?}"));
        let tag = &rest[i..i + len];
        if let Some(start) = tag.find("formula=\"") {
            let body = &tag[start + 9..];
            let end = body.char_indices().find(|&(j, c)| c == '"' && !body[..j].ends_with('\\')).map(|(j, _)| j).unwrap_or(body.len());
            out.push(&body[..end]);
        }
        rest = &rest[i + len..];
    }
    out
}

/// A text's placeholder names (`{seconds}`; `{{` and `}}` are literal braces;
/// tags, `{@ …}`, are skipped).
fn placeholders(text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(['{', '}']) {
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            rest = &tail[2..];
            continue;
        }
        // A tag (a formula's LaTeX has braces of its own) is not a placeholder.
        if tail.starts_with("{@") {
            rest = &tail[tag_len(tail).unwrap_or_else(|| panic!("unclosed tag in text {text:?}"))..];
            continue;
        }
        let end = tail.find('}').filter(|_| tail.starts_with('{')).unwrap_or_else(|| panic!("unbalanced brace in text {text:?}"));
        names.push(&tail[1..end]);
        rest = &tail[end + 1..];
    }
    names.sort();
    names.dedup();
    names
}

/// The keys `tr("…")` and `tr_with("…"` name in the `.rs` files under `dir`,
/// and the `-text` fields of its `.toml` files.
fn text_keys_used(dir: &std::path::Path, keys: &mut Vec<(String, String)>) {
    let mut entries: Vec<_> = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())).map(|e| e.unwrap().path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            text_keys_used(&path, keys);
            continue;
        }
        // Data files name keys in fields ending in `-text` (`title-text = "scene.terrain"`).
        if path.extension().is_some_and(|e| e == "toml") {
            println!("cargo:rerun-if-changed={}", path.display());
            for (n, line) in fs::read_to_string(&path).unwrap().lines().enumerate() {
                let Some((field, value)) = line.split_once('=') else { continue };
                if !field.trim().ends_with("-text") || line.trim_start().starts_with('#') {
                    continue;
                }
                let at = format!("{}:{}", path.display(), n + 1);
                let key = value.trim().strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or_else(|| panic!("{at}: a text key is a quoted string"));
                keys.push((key.to_string(), at));
            }
            continue;
        }
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());
        let source = fs::read_to_string(&path).unwrap();
        for (n, line) in source.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for call in ["tr(", "tr_with("] {
                for (i, _) in line.match_indices(call) {
                    let before = line[..i].chars().next_back();
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') || line[..i].trim_end().ends_with("fn") {
                        continue;
                    }
                    let at = format!("{}:{}", path.display(), n + 1);
                    let rest = line[i + call.len()..].strip_prefix('"').unwrap_or_else(|| panic!("{at}: a text key must be a string literal"));
                    let key = &rest[..rest.find('"').unwrap_or_else(|| panic!("{at}: unterminated text key"))];
                    keys.push((key.to_string(), at));
                }
            }
        }
    }
}

/// Every locale of a text table has the same keys; English names every
/// placeholder of a key and the other locales a subset (English may need a
/// value another language does not, such as a plural's "s"); every key the
/// layer's code names exists, and every key is named somewhere.
fn check_text(manifest_dir: &std::path::Path, tables: &[(&str, &str)]) {
    for (dir, code) in tables {
        let tables: Vec<(&str, serde_json::Map<String, Value>)> = LOCALES
            .iter()
            .map(|locale| {
                let path = manifest_dir.join(dir).join(format!("{locale}.json"));
                println!("cargo:rerun-if-changed={}", path.display());
                let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
                (*locale, serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
            })
            .collect();
        let (first, reference) = &tables[0];
        for (locale, table) in &tables {
            for (key, text) in table {
                let text = text.as_str().unwrap_or_else(|| panic!("{dir}/{locale}.json: {key} is not a string"));
                let Some(other) = reference.get(key) else { panic!("{dir}/{locale}.json has {key}, {first}.json does not") };
                let english = placeholders(other.as_str().unwrap_or_default());
                assert!(placeholders(text).iter().all(|p| english.contains(p)), "{dir}/{locale}.json: {key} has a placeholder {first}.json does not");
                // Formulas stay English: the same LaTeX in every language.
                assert!(formulas(text) == formulas(other.as_str().unwrap_or_default()), "{dir}/{locale}.json: {key}'s formulas differ from {first}.json's");
            }
            for key in reference.keys() {
                assert!(table.contains_key(key), "{dir}/{locale}.json has no {key}");
            }
        }
        let mut used = Vec::new();
        text_keys_used(&manifest_dir.join(code), &mut used);
        for (key, at) in &used {
            assert!(reference.contains_key(key), "{at}: no text {key:?} in {dir}");
        }
        for key in reference.keys() {
            assert!(used.iter().any(|(k, _)| k == key), "{dir}: no code uses the text {key:?}");
        }
    }
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    println!("cargo::rustc-check-cfg=cfg(commercial)");
    for path in HIDDEN {
        println!("cargo:rerun-if-changed={}", manifest_dir.join(path).display());
    }
    let present: Vec<&str> = HIDDEN.iter().copied().filter(|p| manifest_dir.join(p).is_file()).collect();
    let commercial = present.len() == HIDDEN.len();
    if commercial {
        println!("cargo::rustc-cfg=commercial");
    } else if !present.is_empty() {
        let missing: Vec<&str> = HIDDEN.iter().copied().filter(|p| !present.contains(p)).collect();
        panic!("src/commercial is only partly present; missing: {}", missing.join(", "));
    }
    check_text(&manifest_dir, TEXT_TABLES);
    if commercial {
        check_text(&manifest_dir, COMMERCIAL_TEXT_TABLES);
    }
    check_shaders(&manifest_dir);
}
