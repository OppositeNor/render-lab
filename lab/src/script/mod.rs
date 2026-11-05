//! Lamlet in the lab: the language of the storyboards, from the crates the lab pins (`lamlet`, `lamlet-doc`, and
//! `lamlet-present`, by git tag in lab/Cargo.toml), so every build uses the same Lamlet and nothing uses one
//! installed on the system.
//!
//! The engine has `std`, `content`, `present`, the lab's natives (`rl`, natives.rs), the lab's own package
//! `render_lab` (the resource `render_lab.script/render-lab`, its package's `language/render_lab.lam`), and any
//! package a command names. `render-lab [--investigation <id>] lamlet ir|deck|vocab` (src/cli.rs) are Lamlet's and
//! lamlet-present's operations on it; the natives check names against the investigation given. The crates are
//! re-exported here, so a package's library uses the lab's build of them.

pub mod data;
pub mod keys;
pub mod natives;
pub mod runtime;

pub use lamlet;
pub use lamlet_doc;
pub use lamlet_present;

use crate::investigation::Investigation;
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

/// A resolver with `content`, `present`, the lab's natives' stubs (`rl`), and these packages (`std` is always there).
pub fn resolver(packages: &[Package]) -> Result<MemoryResolver, String> {
    let mut r = lamlet_doc::resolver();
    r.add("present", lamlet_present::PRESENT_SOURCE, Exports::All);
    r.add(natives::PACKAGE, &natives::declarations()?, Exports::All);
    for p in packages {
        r.add_package(PackageSource { name: p.name.clone(), version: "0".into(), file_name: p.file.clone(), source: p.source.clone(), exports: Exports::All });
    }
    Ok(r)
}

/// An engine with these packages and the lab's natives, which check against `investigation`, and that
/// investigation's own natives (`Investigation::script_natives`), the package `<package>_natives`.
pub fn engine(packages: &[Package], investigation: Option<&'static dyn Investigation>) -> Result<Engine, String> {
    let mut engine = Engine::new();
    // A `Native` declaration with no registered implementation fails the check, not the first call.
    engine.options_mut().require_linked = true;
    natives::register(&mut engine, investigation).map_err(|e| e.render())?;
    let mut all = packages.to_vec();
    if let Some(i) = investigation {
        let package = i.package().unwrap_or("render_lab");
        i.script_natives(engine.natives_mut()).map_err(|e| format!("{}'s natives: {}", i.path(), e.render()))?;
        let stubs = engine.natives_mut().declarations(package).map_err(|e| e.render())?;
        all.push(Package { name: format!("{package}_natives"), file: format!("<{package} natives>"), source: stubs });
    }
    engine.set_resolver(resolver(&all)?);
    Ok(engine)
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
pub fn ir(file: &Path, packages: &[Package], investigation: Option<&'static dyn Investigation>) -> Result<String, String> {
    lamlet_doc::backend::json::write(&lowered(file, packages, investigation)?).map_err(|e| e.render())
}

/// A document checked, evaluated, and lowered to lamlet-doc's IR.
fn lowered(file: &Path, packages: &[Package], investigation: Option<&'static dyn Investigation>) -> Result<lamlet_doc::Doc, String> {
    let file = source(file)?;
    let engine = engine(packages, investigation)?;
    let result = (|| -> Result<lamlet_doc::Doc, lamlet_doc::Error> {
        let doc = engine.check(file)?;
        let mut instance = engine.instance(&doc)?;
        instance.set_time(now());
        lamlet_doc::evaluate_and_lower(&doc, &mut instance, lamlet_doc::COMMONMARK)
    })();
    result.map_err(|e| e.render())
}

/// A presentation's deck as JSON (lamlet-present's `deck`): its sections and sentences, with no timing.
pub fn deck(file: &Path, packages: &[Package], investigation: Option<&'static dyn Investigation>) -> Result<serde_json::Value, String> {
    let ir = lowered(file, packages, investigation)?;
    lamlet_present::Deck::from_doc(&ir).and_then(|d| d.to_json()).map_err(|e| e.render())
}

/// The cue vocabulary package `name` from a host's keys (lamlet-present's `vocab`; but those marked `"typed": true`,
/// which render_lab writes), checked as a presentation
/// imports it with `std`, `content`, `present`, and these packages, so a name that collides shows now.
pub fn vocab(name: &str, keys: &str, packages: &[Package]) -> Result<String, String> {
    // Keys render_lab's package writes itself (`"typed": true`, keys::TYPED) have no function here.
    let mut list: serde_json::Value = serde_json::from_str(keys).map_err(|e| format!("the key list is not JSON: {e}"))?;
    if let Some(items) = list.as_array_mut() {
        items.retain(|k| k.get("typed") != Some(&serde_json::Value::Bool(true)));
    }
    let keys = lamlet_present::vocab::keys_from_json(&list.to_string())?;
    let package = lamlet_present::vocab::package(&format!("The `{name}` cue vocabulary"), &keys)?;
    let mut all = packages.to_vec();
    all.push(Package { name: name.to_string(), file: format!("<{name} vocabulary>"), source: package.clone() });
    let imports: String = packages.iter().map(|p| format!("$[Import* {}]", p.name)).collect();
    let probe = format!("$[Import* std]$[Import* content]$[Import* present]{imports}$[Import* {name}]\n");
    engine(&all, None)?.check(SourceFile::new("<vocabulary check>", probe)).map_err(|e| e.render())?;
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
        let out = ir(&file, &[package("p", "$[# three 3]")], None).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
        let text: String = doc["blocks"][0]["inlines"].as_array().unwrap().iter().filter_map(|i| i["text"].as_str()).collect();
        assert_eq!(text, "Three is 3, $5.");
    }

    #[test]
    fn errors_are_diagnostics_with_the_line() {
        let dir = scratch("error");
        let file = write(&dir, "doc.lam", "$[Import* std]\n\n${unknownName 3}\n");
        let e = ir(&file, &[], None).unwrap_err();
        assert!(e.contains("unknownName") && e.contains(":3:"), "{e}");
        let file = write(&dir, "bad.lam", "${{+ 1}\n");
        assert!(ir(&file, &[], None).is_err());
        // A native declared with no implementation fails the check, though nothing calls it.
        let file = write(&dir, "unlinked.lam", "$[Import* std]$[Native ping [% {Fn {'} Int}] \"rl/ping\" Pure]\nText.\n");
        assert!(ir(&file, &[], None).is_err());
    }

    #[test]
    fn deck_and_vocab() {
        let dir = scratch("deck");
        let keys = r#"[{"name": "intensity", "kind": "float"}, {"name": "camera.yaw", "kind": "float"}]"#;
        let vocabulary = vocab("probe", keys, &[]).unwrap();
        assert!(vocabulary.contains("cameraYaw"));
        let file = write(&dir, "deck.lam", "$[Import* std]$[Import* content]$[Import* present]$[Import* probe]\n# A deck {#top}\n\n${cue \"k\" {' {intensity 0.5}}}Hello there.\n");
        let deck = deck(&file, &[package("probe", &vocabulary)], None).unwrap();
        assert_eq!(deck["sentences"][0]["spoken"], "Hello there.");
        assert_eq!(deck["sentences"][0]["anchors"][0]["attrs"]["intensity"], "0.5");
        // A key whose function std already has collides when the vocabulary is generated.
        assert!(vocab("probe", r#"[{"name": "map", "kind": "float"}]"#, &[]).is_err());
    }

    /// render_lab's package from its source (the built resource is not there in a unit test).
    fn render_lab_source() -> Package {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packages/render_lab/language/render_lab.lam");
        Package::read("render_lab", &path).unwrap()
    }

    /// The attributes of a document's point cues, in order.
    fn cue_attrs(file: &Path, packages: &[Package]) -> Result<Vec<(String, String)>, String> {
        let doc: serde_json::Value = serde_json::from_str(&ir(file, packages, None)?).unwrap();
        let mut out = Vec::new();
        for block in doc["blocks"].as_array().unwrap() {
            for inline in block["inlines"].as_array().into_iter().flatten().filter(|i| i["type"] == "anchor") {
                for pair in inline["attrs"].as_array().unwrap() {
                    out.push((pair[0].as_str().unwrap().to_string(), pair[1].as_str().unwrap().to_string()));
                }
            }
        }
        Ok(out)
    }

    #[test]
    fn render_lab_writes_what_the_lab_reads() {
        let dir = scratch("typed");
        let channels = r#"[{"name": "camera.yaw", "kind": "float"}, {"name": "camera.pitch", "kind": "float"}, {"name": "lightX", "kind": "float"},
            {"name": "lightY", "kind": "float"}, {"name": "lightZ", "kind": "float"}, {"name": "easing", "key": "ease", "kind": "text", "options": ["sine", "inOut"]}]"#;
        let packages = [render_lab_source(), package("probe", &vocab("probe", channels, &[render_lab_source()]).unwrap())];
        let head = "$[Import* std]$[Import* content]$[Import* present]$[Import* render_lab]$[Import* probe]\n# A deck\n\n";
        let doc = |body: &str| cue_attrs(&write(&dir, "doc.lam", &format!("{head}{body}\n")), &packages);
        let motions = [
            "{path {' cameraYaw cameraPitch} {' {' 18.0 16.0} {' -8.0 22.5}} beat}",
            "{closed {looping {eased easingInOut {path {' cameraYaw} {' {' 1.0}} {seconds 4.0}}}}}",
            "{pingPonging {eased easingSine {orbit {' lightX lightY} {' -1.05 1.24} 70.0 sentence}}}",
            "{from {' 1.0 2.0 3.0} {around {' 0.0 0.0 1.0} {orbit {' lightX lightY lightZ} {' 0.0 1.0 0.0} 90.0 beat}}}",
            "{lasting sentence {sway cameraYaw 6.0 24.0}}",
            "{fading 0.5 {phased 90.0 {centered 2.0 {lasting {seconds 3.0} {sway lightX 0.5 2.0}}}}}",
        ];
        let attrs = doc(&format!("${{cue \"key\" {{' {{motion {{' {}}}}}}}}}", motions.join(" "))).unwrap();
        assert_eq!(attrs[0].0, "animate");
        // A beat's or a sentence's length is `seconds` the video builder fills in; here, a number.
        let json = attrs[0].1.replace("\"beat\"", "1.5").replace("\"sentence\"", "2.5");
        let parsed: Vec<crate::animation::Motion> = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}: {json}"));
        assert_eq!(parsed.len(), motions.len());
        let value: serde_json::Value = serde_json::from_str(&attrs[0].1).unwrap();
        assert_eq!(value[0]["path"]["through"], serde_json::json!([[18.0, 16.0], [-8.0, 22.5]]));
        assert_eq!(value[0]["path"]["seconds"], "beat");
        assert_eq!(value[2]["orbit"]["repeat"], "pingPong");
        assert_eq!(value[4]["wave"]["param"], "camera.yaw");
        // Points, marks, names, and records, as tools/video/markup.py reads them.
        let attrs = doc("${cue \"key\" {' {labelAt {at 1.8 2.4 -2.95}} {arrowTo {at 0.0 1.0 2.0}} {mark 0.28 0.42 0.62 1.0} unmark {highlight {' \"scene pass\" \"x\"}} {imageHide {'}} recordCamera {recordObject 3} {callout \"\"}}}").unwrap();
        let expected = [("labelAt", "1.8,2.4,-2.95"), ("arrowTo", "0.0,1.0,2.0"), ("imageMark", "0.28,0.42,0.62,1.0"), ("imageMark", ""), ("highlight", "scene pass,x"), ("imageHide", ""), ("record", "camera"), ("record", "object:3"), ("callout", "")];
        assert_eq!(attrs, expected.map(|(k, v)| (k.to_string(), v.to_string())));
        // A wrong shape is an error at the storyboard, before any speech: a channel per waypoint value, a
        // length that is a Length, a part only some kinds have, a name without a comma.
        for bad in [
            "{motion {' {path {' cameraYaw cameraPitch} {' {' 18.0}} beat}}}",
            "{motion {' {path {' cameraYaw} {' {' 18.0}} \"beat\"}}}",
            "{motion {' {closed {sway cameraYaw 1.0 2.0}}}}",
            "{motion {' {eased {cameraYaw 1.0} {path {' cameraYaw} {' {' 18.0}} beat}}}}",
            "{highlight {' \"a,b\"}}",
            "{labelAt {at 1 2 3}}",
        ] {
            assert!(doc(&format!("${{cue \"key\" {{' {bad}}}}}")).is_err(), "{bad}");
        }
        // A name the lab checks (a callout, a text key, a published point, a formula) needs the investigation.
        for checked in ["{callout \"frame-order\"}", "{imageTitle \"views.depth\"}", "{arrowTo {point \"probe.top\"}}"] {
            let e = doc(&format!("${{cue \"key\" {{' {checked}}}}}")).unwrap_err();
            assert!(e.contains("--investigation") && e.contains("doc.lam:4:"), "{e}");
        }
    }
}
