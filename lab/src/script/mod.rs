//! Lamlet in the lab: the language of the storyboards, from the crates the lab pins (`lamlet`, `lamlet-doc`, and
//! `lamlet-present`, by git tag in lab/Cargo.toml), so every build uses the same Lamlet and nothing uses one
//! installed on the system.
//!
//! The engine has `std`, `content`, `present`, and the lab's own package `render_lab` (the resource
//! `render_lab.script/render-lab`, its package's `language/render_lab.lam`), and any package a command names.
//! `render-lab lamlet ir|deck|vocab` (src/cli.rs) are Lamlet's and lamlet-present's operations on it. The crates
//! are re-exported here, so a package's library uses the lab's build of them.

pub mod keys;

pub use lamlet;
pub use lamlet_doc;
pub use lamlet_present;

use lamlet::embed::{Engine, Exports, MemoryResolver, PackageSource, SourceFile};
use std::path::Path;

/// The resource that holds render_lab's Lamlet package.
pub const RENDER_LAB: &str = "render_lab.script/render-lab";

/// A Lamlet package a document may import: its name, the file its diagnostics name, and its source.
#[derive(Clone, Debug)]
pub struct Package {
    pub name: String,
    pub file: String,
    pub source: String,
}

impl Package {
    /// A package from a file (`--package NAME=PATH`).
    pub fn read(name: &str, path: &Path) -> Result<Package, String> {
        let source = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Package { name: name.to_string(), file: path.display().to_string(), source })
    }

    /// render_lab's package, from its built resource.
    pub fn render_lab() -> Result<Package, String> {
        let resource = crate::resources::find(RENDER_LAB).ok_or_else(|| format!("no resource {RENDER_LAB} (render-lab cook)"))?;
        let source = std::str::from_utf8(resource.bytes()).map_err(|e| format!("{RENDER_LAB}: {e}"))?;
        Ok(Package { name: "render_lab".into(), file: resource.path().display().to_string(), source: source.to_string() })
    }
}

/// render_lab's package, then the given ones (a given package of the same name replaces it).
pub fn packages(given: Vec<Package>) -> Result<Vec<Package>, String> {
    let mut all = Vec::new();
    if !given.iter().any(|p| p.name == "render_lab") {
        all.push(Package::render_lab()?);
    }
    all.extend(given);
    Ok(all)
}

/// A resolver with `content`, `present`, and these packages (`std` is always there).
pub fn resolver(packages: &[Package]) -> MemoryResolver {
    let mut r = lamlet_doc::resolver();
    r.add("present", lamlet_present::PRESENT_SOURCE, Exports::All);
    for p in packages {
        r.add_package(PackageSource { name: p.name.clone(), version: "0".into(), file_name: p.file.clone(), source: p.source.clone(), exports: Exports::All });
    }
    r
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn source(file: &Path) -> Result<SourceFile, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(SourceFile::new(file.display().to_string(), text))
}

/// A document's IR as JSON (lamlet-doc's `ir` backend, what `lamlet build --to ir --prose commonmark` writes): its
/// prose read as CommonMark, every region evaluated and lowered, cues and spans as their anchors and spans, each
/// attribute list in its written order.
pub fn ir(file: &Path, packages: &[Package]) -> Result<String, String> {
    let file = source(file)?;
    let result = (|| -> Result<String, lamlet_doc::Error> {
        let mut engine = Engine::new();
        engine.set_resolver(resolver(packages));
        let doc = engine.check(file)?;
        let mut instance = engine.instance(&doc)?;
        instance.set_time(now());
        let ir = lamlet_doc::evaluate_and_lower(&doc, &mut instance, lamlet_doc::COMMONMARK)?;
        lamlet_doc::backend::json::write(&ir)
    })();
    result.map_err(|e| e.render())
}

/// A presentation's deck as JSON (lamlet-present's `deck`): its sections and sentences, with no timing.
pub fn deck(file: &Path, packages: &[Package]) -> Result<serde_json::Value, String> {
    let file = source(file)?;
    let named: Vec<(String, String)> = packages.iter().map(|p| (p.name.clone(), p.source.clone())).collect();
    let p = lamlet_present::Presentation::compile(file, &named).map_err(|e| e.render())?;
    p.deck.to_json().map_err(|e| e.render())
}

/// The cue vocabulary package `name` from a host's keys (lamlet-present's `vocab`), checked as a presentation
/// imports it with `std`, `content`, `present`, and these packages, so a name that collides shows now.
pub fn vocab(name: &str, keys: &str, packages: &[Package]) -> Result<String, String> {
    let keys = lamlet_present::vocab::keys_from_json(keys)?;
    let package = lamlet_present::vocab::package(&format!("The `{name}` cue vocabulary"), &keys)?;
    let mut all = packages.to_vec();
    all.push(Package { name: name.to_string(), file: format!("<{name} vocabulary>"), source: package.clone() });
    let imports: String = packages.iter().map(|p| format!("$[Import* {}]", p.name)).collect();
    let probe = format!("$[Import* std]$[Import* content]$[Import* present]{imports}$[Import* {name}]\n");
    let mut engine = Engine::new();
    engine.set_resolver(resolver(&all));
    engine.check(SourceFile::new("<vocabulary check>", probe)).map_err(|e| e.render())?;
    Ok(package)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(name: &str, source: &str) -> Package {
        Package { name: name.into(), file: format!("{name}.lam"), source: source.into() }
    }

    fn write(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    fn scratch(test: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("render-lab-script-{test}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn ir_evaluates_regions_with_a_package() {
        let dir = scratch("ir");
        let file = write(&dir, "doc.lam", "$[Import* std]\n$[Import* p]\nThree is $three, $$5.\n");
        let out = ir(&file, &[package("p", "$[# three 3]")]).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
        let text: String = doc["blocks"][0]["inlines"].as_array().unwrap().iter().filter_map(|i| i["text"].as_str()).collect();
        assert_eq!(text, "Three is 3, $5.");
    }

    #[test]
    fn errors_are_diagnostics_with_the_line() {
        let dir = scratch("error");
        let file = write(&dir, "doc.lam", "$[Import* std]\n\n${unknownName 3}\n");
        let e = ir(&file, &[]).unwrap_err();
        assert!(e.contains("unknownName") && e.contains(":3:"), "{e}");
        let file = write(&dir, "bad.lam", "${{+ 1}\n");
        assert!(ir(&file, &[]).is_err());
    }

    #[test]
    fn deck_and_vocab() {
        let dir = scratch("deck");
        let keys = r#"[{"name": "intensity", "kind": "float"}, {"name": "camera.yaw", "kind": "float"}]"#;
        let vocabulary = vocab("probe", keys, &[]).unwrap();
        assert!(vocabulary.contains("cameraYaw"));
        let file = write(&dir, "deck.lam", "$[Import* std]$[Import* content]$[Import* present]$[Import* probe]\n# A deck {#top}\n\n${cue \"k\" {' {intensity 0.5}}}Hello there.\n");
        let deck = deck(&file, &[package("probe", &vocabulary)]).unwrap();
        assert_eq!(deck["sentences"][0]["spoken"], "Hello there.");
        assert_eq!(deck["sentences"][0]["anchors"][0]["attrs"]["intensity"], "0.5");
        // A key whose function std already has collides when the vocabulary is generated.
        assert!(vocab("probe", r#"[{"name": "map", "kind": "float"}]"#, &[]).is_err());
    }
}
