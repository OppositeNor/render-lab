//! The checks a package's build runs (its build.rs, and the lab's for its own
//! package, render_lab): every shader the package declares parses and validates
//! with naga, and its text tables match the code and data that name their
//! keys. A failure fails the build; nothing is generated. They read only the
//! package's directory, its code, and the packages beside it it imports.

use naga::valid::{Capabilities, ValidationFlags, Validator};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const LOCALES: &[&str] = &["en", "zh-CN"];

/// What the collections declare that the shader check needs: each
/// resource's source file (with the overrides applied), and each shader's
/// parts, by full key (`<package>.<key>`).
#[derive(Default)]
struct Manifests {
    sources: BTreeMap<String, PathBuf>,
    shaders: BTreeMap<String, Vec<String>>,
}

/// A package: its name, directory, and collections.
struct Package {
    name: String,
    dir: PathBuf,
    collections: Vec<PathBuf>,
}

/// Every package of a packages directory, by name (the run time checks their manifests in full).
fn find_packages(dir: &Path) -> Vec<Package> {
    // A new package reruns the checks.
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut entries: Vec<PathBuf> = fs::read_dir(dir).map(|e| e.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    entries.sort();
    let mut packages = Vec::new();
    for dir in entries {
        let path = dir.join("manifest.toml");
        if !path.is_file() {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let document = toml_edit::Document::parse(text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let name = document.get("name").and_then(|n| n.as_str()).unwrap_or_else(|| panic!("{}: no `name`", path.display())).to_string();
        let collections = document
            .get("provides")
            .and_then(|p| p.as_array_of_tables())
            .into_iter()
            .flatten()
            .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("resource"))
            .map(|p| dir.join(p.get("path").and_then(|p| p.as_str()).unwrap_or_default()))
            .collect();
        packages.push(Package { name, dir, collections });
    }
    packages
}

/// A reference's full key: its own package's key, or `<package>.<key>` as it is.
fn full_key(package: &str, reference: &str) -> String {
    if reference.contains('.') { reference.to_string() } else { format!("{package}.{reference}") }
}

/// One collection's resources and shaders; overrides go to `overrides`.
fn read_collection(package: &str, path: &Path, out: &mut Manifests, overrides: &mut Vec<(String, PathBuf)>) {
    println!("cargo:rerun-if-changed={}", path.display());
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let document = toml_edit::Document::parse(text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let dir = path.parent().unwrap();
    for table in document.get("resource").and_then(|r| r.as_array_of_tables()).into_iter().flatten() {
        if table.get("built").and_then(|b| b.as_bool()) == Some(true) {
            continue;
        }
        let id = full_key(package, table.get("id").and_then(|i| i.as_str()).unwrap_or_default());
        let source = dir.join(table.get("source").and_then(|s| s.as_str()).unwrap_or_default());
        if let Some(target) = table.get("override").and_then(|o| o.as_str()) {
            overrides.push((target.to_string(), source.clone()));
        }
        out.sources.insert(id, source);
    }
    for table in document.get("shader").and_then(|s| s.as_array_of_tables()).into_iter().flatten() {
        let id = full_key(package, table.get("id").and_then(|i| i.as_str()).unwrap_or_default());
        let parts = table.get("parts").and_then(|p| p.as_array()).into_iter().flatten().filter_map(|p| p.as_str().map(|p| full_key(package, p))).collect();
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

/// Every shader `package` declares parses and validates with naga, composed
/// from the packages beside it with its own overrides applied (the run time
/// validates every shader again as it composes it, with every package's).
fn check_shaders(packages: &[Package], package: &str) {
    let mut manifests = Manifests::default();
    let mut overrides = Vec::new();
    for m in packages {
        let mut own = Vec::new();
        for collection in &m.collections {
            read_collection(&m.name, collection, &mut manifests, &mut own);
        }
        if m.name == package {
            overrides = own;
        }
    }
    for (target, source) in overrides {
        manifests.sources.insert(target, source);
    }
    for id in manifests.shaders.keys().filter(|id| id.split_once('.').is_some_and(|(m, _)| m == package)) {
        let mut files = Vec::new();
        shader_files(&manifests, id, &mut files);
        let source: String = files
            .iter()
            .map(|f| {
                println!("cargo:rerun-if-changed={}", f.display());
                fs::read_to_string(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display()))
            })
            .collect();
        let package = naga::front::wgsl::parse_str(&source).unwrap_or_else(|e| panic!("WGSL parse error in shader `{id}`:\n{}", e.emit_to_string(&source)));
        Validator::new(ValidationFlags::all(), Capabilities::default())
            .validate(&package)
            .unwrap_or_else(|e| panic!("WGSL validation error in shader `{id}`:\n{}", e.emit_to_string(&source)));
    }
}

/// The length of the tag at the start of `tail` (`{@ …}`, src/rich_text.rs):
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
/// and the `-text` fields of its `.toml` files (not under links, nor in what
/// a build writes: `build/`, `target/`).
fn text_keys_used(dir: &Path, keys: &mut Vec<(String, String)>) {
    let mut entries: Vec<_> = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())).map(|e| e.unwrap().path()).collect();
    entries.sort();
    for path in entries {
        if path.is_symlink() || path.file_name().is_some_and(|n| n == "build" || n == "target") {
            continue;
        }
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
/// package's code and data name exists, and every key is named somewhere.
fn check_text(tables: &[(PathBuf, Vec<PathBuf>)]) {
    for (dir, code) in tables {
        let dir = dir.display();
        let tables: Vec<(&str, serde_json::Map<String, Value>)> = LOCALES
            .iter()
            .map(|locale| {
                let path = PathBuf::from(dir.to_string()).join(format!("{locale}.json"));
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
        for code in code {
            text_keys_used(code, &mut used);
        }
        for (key, at) in &used {
            assert!(reference.contains_key(key), "{at}: no text {key:?} in {dir}");
        }
        for key in reference.keys() {
            assert!(used.iter().any(|(k, _)| k == key), "{dir}: no code uses the text {key:?}");
        }
    }
}

/// Checks the package in `module_dir` (a directory of a packages directory):
/// its shaders, and, if it has text tables, them against the keys its code
/// (the `.rs` files under `code`) and its data (`-text` fields of the TOML
/// files in its directory) name.
pub fn check_package(module_dir: &Path, code: &Path) {
    let packages = find_packages(module_dir.parent().expect("a package is a directory of a packages directory"));
    let name = packages.iter().find(|m| m.dir == module_dir).unwrap_or_else(|| panic!("{}: no manifest.toml", module_dir.display())).name.clone();
    if module_dir.join("text/en.json").is_file() {
        check_text(&[(module_dir.join("text"), vec![code.to_path_buf(), module_dir.to_path_buf()])]);
    }
    check_shaders(&packages, &name);
}
