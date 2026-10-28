//! Validates the WGSL shader modules with naga (the shader compiler wgpu
//! uses) and exports the struct layouts naga computes for shaders/wgsl/
//! types.wgsl. A module that fails to parse or validate fails the build.
//!
//! It also decides whether the investigations of commercial engines are
//! built. They live in src/commercial, which is not published (see
//! .gitignore): when all of `HIDDEN` are present it sets the `commercial`
//! cfg, and when none are, the lab builds with the public investigations
//! alone and its shaders compose `FALLBACK` in place of `LOOKUP`.
//!
//! Outputs in OUT_DIR:
//! - `wgsl-layout.json`: size, alignment, and member offsets of every struct,
//!   used by the struct panel and checked against the Rust records;
//! - `shader-manifest.json`: naga version, module hashes, and entry points;
//! - `resources.rs` and `resources/`: the resources the `resources.toml`
//!   manifests declare, processed by the resource pipeline (pipeline/mod.rs).
//!

#[path = "pipeline/mod.rs"]
mod pipeline;

use naga::valid::{Capabilities, ValidationFlags, Validator};
use naga::{Scalar, ScalarKind, TypeInner, VectorSize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::PathBuf;

/// Each shader module is the shared prelude followed by one pass file. Shader
/// files are named by their path from the crate root, without `.wgsl`.
const PRELUDE: &[&str] = &["shaders/wgsl/types", "shaders/wgsl/bindings", "shaders/wgsl/common"];
/// The prelude's last file: src/commercial's Surface Cache lookup, or the
/// public fallback without it.
const LOOKUP: &str = "src/commercial/shaders/surface_lookup";
const FALLBACK: &str = "shaders/wgsl/surface_fallback";
const MODULES: &[&str] = &["shaders/wgsl/scene", "shaders/wgsl/overlay", "shaders/wgsl/verify"];
/// Modules that need src/commercial.
const COMMERCIAL_MODULES: &[&str] = &["src/commercial/shaders/surface_cache"];
/// Investigations' own scene shading (`Investigation::scene_shading`), each
/// composed after the scene module.
const SCENE_SHADINGS: &[&str] = &["src/o3de/lighting/shaders/standard_lighting"];
/// Each layer's on-screen text (`text/en.json`, `text/zh-CN.json`) and the
/// code that names its keys (`tr("key")`, `tr_with("key", …)`).
const TEXT_TABLES: &[(&str, &str)] = &[("src/lab/text", "src/lab"), ("src/o3de/lighting/text", "src/o3de/lighting")];
const COMMERCIAL_TEXT_TABLES: &[(&str, &str)] = &[("src/commercial/unreal/lumen/text", "src/commercial/unreal/lumen")];
const LOCALES: &[&str] = &["en", "zh-CN"];
/// Modules without the prelude.
const STANDALONE: &[&str] = &["shaders/wgsl/frost"];
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

/// A shader's short name, for messages and the manifest.
fn stem(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
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

fn naga_version() -> String {
    let lock = fs::read_to_string(PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("Cargo.lock")).unwrap_or_default();
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line == "name = \"naga\"" {
            if let Some(v) = lines.next().and_then(|l| l.strip_prefix("version = \"")) {
                return v.trim_end_matches('"').to_string();
            }
        }
    }
    "unknown".into()
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
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

/// The keys `tr("…")` and `tr_with("…"` name in the `.rs` files under `dir`.
fn text_keys_used(dir: &std::path::Path, keys: &mut Vec<(String, String)>) {
    let mut entries: Vec<_> = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())).map(|e| e.unwrap().path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            text_keys_used(&path, keys);
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
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

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
    pipeline::build(&manifest_dir, &out_dir);
    check_text(&manifest_dir, TEXT_TABLES);
    if commercial {
        check_text(&manifest_dir, COMMERCIAL_TEXT_TABLES);
    }
    let read = |name: &str| {
        let path = manifest_dir.join(format!("{name}.wgsl"));
        println!("cargo:rerun-if-changed={}", path.display());
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    };
    let mut modules = Vec::new();
    let prelude_files: Vec<&str> = PRELUDE.iter().copied().chain([if commercial { LOOKUP } else { FALLBACK }]).collect();
    let prelude: String = prelude_files.iter().map(|name| read(name)).collect();
    // Each module as its pass files after the prelude.
    let mut compositions: Vec<Vec<&str>> = MODULES.iter().map(|m| vec![*m]).collect();
    if commercial {
        compositions.extend(COMMERCIAL_MODULES.iter().map(|m| vec![*m]));
    }
    compositions.extend(SCENE_SHADINGS.iter().map(|s| vec!["shaders/wgsl/scene", *s]));
    for passes in &compositions {
        let path = passes.last().unwrap();
        let name = stem(path);
        let source = format!("{prelude}{}", passes.iter().map(|p| read(p)).collect::<String>());
        let module = naga::front::wgsl::parse_str(&source)
            .unwrap_or_else(|e| panic!("WGSL parse error in {name} module:\n{}", e.emit_to_string(&source)));
        Validator::new(ValidationFlags::all(), Capabilities::default())
            .validate(&module)
            .unwrap_or_else(|e| panic!("WGSL validation error in {name} module:\n{}", e.emit_to_string(&source)));
        let entry_points: Vec<Value> = module
            .entry_points
            .iter()
            .map(|e| json!({"name": e.name, "stage": format!("{:?}", e.stage)}))
            .collect();
        modules.push(json!({
            "module": name,
            "files": prelude_files.iter().chain(passes).map(|f| format!("{f}.wgsl")).collect::<Vec<_>>(),
            "bytes": source.len(),
            "sha256": hex(source.as_bytes()),
            "validated": true,
            "entryPoints": entry_points,
        }));
    }

    for path in STANDALONE {
        let name = stem(path);
        let source = read(path);
        let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|e| panic!("WGSL parse error in {name} module:\n{}", e.emit_to_string(&source)));
        Validator::new(ValidationFlags::all(), Capabilities::default())
            .validate(&module)
            .unwrap_or_else(|e| panic!("WGSL validation error in {name} module:\n{}", e.emit_to_string(&source)));
        let entry_points: Vec<Value> = module.entry_points.iter().map(|e| json!({"name": e.name, "stage": format!("{:?}", e.stage)})).collect();
        modules.push(json!({
            "module": name,
            "files": [format!("{path}.wgsl")],
            "bytes": source.len(),
            "sha256": hex(source.as_bytes()),
            "validated": true,
            "entryPoints": entry_points,
        }));
    }

    // Struct layouts of the type declarations alone.
    let types_source = read("shaders/wgsl/types");
    let types = naga::front::wgsl::parse_str(&types_source)
        .unwrap_or_else(|e| panic!("WGSL parse error in types.wgsl:\n{}", e.emit_to_string(&types_source)));
    let mut layouter = naga::proc::Layouter::default();
    layouter.update(types.to_ctx()).expect("layout of types.wgsl");
    let mut structs = serde_json::Map::new();
    for (handle, ty) in types.types.iter() {
        if let TypeInner::Struct { members, span } = &ty.inner {
            let name = ty.name.clone().unwrap_or_default();
            let layout = &layouter[handle];
            let members: Vec<Value> = members
                .iter()
                .map(|m| {
                    let inner = &types.types[m.ty].inner;
                    let matrix = match inner {
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
            structs.insert(name, json!({"size": span, "alignment": layout.alignment.round_up(1), "members": members}));
        }
    }
    let layout_json = json!({
        "source": "shaders/wgsl/types.wgsl",
        "generator": format!("naga {} proc::Layouter", naga_version()),
        "structs": structs,
    });
    fs::write(out_dir.join("wgsl-layout.json"), serde_json::to_string_pretty(&layout_json).unwrap()).unwrap();

    let manifest = json!({
        "shaderLanguage": "WGSL",
        "validator": format!("naga {} (wgsl-in, ValidationFlags::all)", naga_version()),
        "typesSha256": hex(types_source.as_bytes()),
        "modules": modules,
        "rustc": env::var("RUSTC").ok().and_then(|rustc| std::process::Command::new(rustc).arg("--version").output().ok())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default(),
        "profile": env::var("PROFILE").unwrap_or_default(),
    });
    fs::write(out_dir.join("shader-manifest.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
}
