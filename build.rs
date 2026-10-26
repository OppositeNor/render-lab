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
//! - `shader-manifest.json`: naga version, module hashes, and entry points.
//!
//! It also fails when an investigation's formula image no longer matches
//! its LaTeX (tools/video/formulas.py renders them).

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
/// Investigations' readout formulas: formulas.json, the PNGs made from it,
/// and manifest.json with their hashes.
const FORMULAS: &[&str] = &["src/o3de/lighting/formulas"];
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

/// Every formula's LaTeX hashes to what its manifest recorded, and every
/// PNG to its recorded hash, so a formula edited without rendering it again
/// fails the build.
fn check_formulas(manifest_dir: &std::path::Path) {
    for dir in FORMULAS {
        let investigation = dir.trim_start_matches("src/").trim_end_matches("/formulas");
        let fix = format!("run python3 tools/video/formulas.py --investigation {investigation}");
        let read = |name: &str| {
            let path = manifest_dir.join(dir).join(name);
            println!("cargo:rerun-if-changed={}", path.display());
            fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}; {fix}", path.display()))
        };
        let sources: serde_json::Map<String, Value> = serde_json::from_slice(&read("formulas.json")).unwrap_or_else(|e| panic!("{dir}/formulas.json: {e}"));
        let manifest: Value = serde_json::from_slice(&read("manifest.json")).unwrap_or_else(|e| panic!("{dir}/manifest.json: {e}"));
        let recorded = manifest["formulas"].as_object().unwrap_or_else(|| panic!("{dir}/manifest.json lists no formulas; {fix}"));
        let mut names: Vec<&String> = sources.keys().collect();
        let mut made: Vec<&String> = recorded.keys().collect();
        names.sort();
        made.sort();
        assert!(names == made, "{dir}: formulas.json has {names:?}, the images were made for {made:?}; {fix}");
        for (key, latex) in &sources {
            let entry = &recorded[key];
            assert!(entry["latexSha256"].as_str() == Some(hex(latex.as_str().unwrap_or_default().as_bytes()).as_str()), "{dir}: formula {key} changed since its images were made; {fix}");
            for (variant, image) in entry["variants"].as_object().into_iter().flatten() {
                let png = read(&format!("{variant}.png"));
                assert!(image["pngSha256"].as_str() == Some(hex(&png).as_str()), "{dir}/{variant}.png is not the image its manifest records; {fix}");
            }
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
    check_formulas(&manifest_dir);
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
