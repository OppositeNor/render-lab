//! The keys a story's cues may set, as lamlet-present's vocabulary key list (`render-lab story keys`): the
//! investigation's values, and the fields of a timeline key with their options where the lab knows them.
//! `render-lab lamlet vocab` turns the list into the investigation's cue package, so a misspelled key is an
//! unknown name and a wrong value a type error, at the storyboard's line.
//!
//! Every value is written as text, the cue attribute's String, and read by `tools/video/markup.py`
//! (`cue_key`, `param_value`); so the kinds here are what a cue may be written with, not the lab's types: a
//! numeric value is a float, whatever its type, since the timeline blends every value as a number (a range
//! over an integer or a toggle is a fade).

use crate::investigation::Investigation;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Attribute names a cue reserves for the key's fields (as tools/video/markup.py's `cue_key` reads them): a value
/// of one of these names is written `param.<name>`.
pub const RESERVED: &[&str] = &[
    "callout", "code", "panel", "demo", "ease", "image", "imageTitle", "imageCorner", "imageFrom", "imageGraph", "imageShow", "imageClick",
    "imageMarkText", "arrow", "arrowColor", "arrows", "label", "labelText", "labelColor", "formula", "say", "card", "display", "colors",
    "highlight", "record", "blend", "term", "delay", "animate", "camera", "param", "run", "stop", "imageHide", "imageMark",
    "arrowFrom", "arrowTo", "arrowWidth", "arrowHide", "labelAt", "labelHide",
];

/// The right-hand panel's modes (`shot::PanelMode`, as a timeline names them).
pub const PANELS: &[&str] = &["declaration", "values", "memory", "code", "performance", "knowledge", "graph", "dag", "resources", "hidden"];
/// The easings a key blends with (`animation::Ease`).
pub const EASES: &[&str] = &["smooth", "linear", "in", "out", "inOut", "sine"];
/// The corners an image's inset sits in (`inset::Corner`).
pub const CORNERS: &[&str] = &["top-left", "top-right", "bottom-left", "bottom-right"];

fn key(name: &str, key: &str, kind: &str) -> Value {
    if name == key {
        json!({"name": name, "kind": kind})
    } else {
        json!({"name": name, "key": key, "kind": kind})
    }
}

/// A text key with options: only those that make a Lamlet name, once each (the rest are still written as text).
fn options(name: &str, key: &str, options: impl IntoIterator<Item = String>) -> Value {
    let mut names = std::collections::BTreeSet::new();
    let kept: Vec<String> = options.into_iter().filter(|o| lamlet_present::vocab::camel(&[name, o]).is_ok_and(|n| names.insert(n))).collect();
    let mut k = self::key(name, key, "text");
    k["options"] = json!(kept);
    k
}

/// Kinds of the same dotted key from several declarations: one kind, or text when they differ.
fn merge(into: &mut BTreeMap<String, &'static str>, key: String, kind: &'static str) {
    into.entry(key).and_modify(|k| if *k != kind { *k = "text" }).or_insert(kind);
}

/// The vocabulary key list for `investigation`.
pub fn keys(investigation: &dyn Investigation) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    let mut components = Vec::new();
    for e in crate::values::list() {
        let (name, attr) = if RESERVED.contains(&e.key) { (format!("param.{}", e.key), format!("param.{}", e.key)) } else { (e.key.to_string(), e.key.to_string()) };
        out.push(match e.choices {
            Some(c) => options(&name, &attr, c.options.iter().map(|o| o.name.to_string())),
            None if e.ty.components() > 1 => key(&name, &attr, "vector"),
            None => key(&name, &attr, "float"),
        });
        // A vector's components are keys of their own (`lightX` of `lightPosition`), after every value.
        for c in e.components {
            let attr = if RESERVED.contains(c) { format!("param.{c}") } else { c.to_string() };
            components.push(key(&attr, &attr, "float"));
        }
    }
    out.extend(components);
    // The key's fields. Three are named apart from their attribute, where `content` and `present` already have the
    // name (`code`, `image`, `animate`), and `ease` too, whose options would be `present`'s easings.
    out.push(key("callout", "callout", "text"));
    out.push(options("listing", "code", investigation.listings().iter().map(|l| l.name.to_string())));
    out.push(options("panel", "panel", PANELS.iter().map(|s| s.to_string())));
    out.push(options("demo", "demo", investigation.demos().iter().map(|d| d.key().to_string())));
    out.push(options("easing", "ease", EASES.iter().map(|s| s.to_string())));
    out.push(options("inset", "image", investigation.images().iter().map(|i| i.key.to_string())));
    out.push(key("imageTitle", "imageTitle", "text"));
    out.push(options("imageCorner", "imageCorner", CORNERS.iter().map(|s| s.to_string())));
    let package = investigation.package().unwrap_or("render_lab");
    let kinds = crate::views::kinds(package).map_err(|e| e.to_string())?;
    out.push(options("imageFrom", "imageFrom", kinds.iter().map(|k| k.key.to_string())));
    out.push(key("imageGraph", "imageGraph", "text"));
    out.push(options("imageShow", "imageShow", crate::graph::Show::ALL.map(|s| s.key().to_string())));
    out.push(options("imageClick", "imageClick", investigation.actions().clicks.iter().map(|c| c.key.to_string())));
    for name in ["imageMark", "imageMarkText", "imageHide", "arrow", "arrowColor", "arrowFrom", "arrowTo", "arrowHide", "label", "labelText", "labelColor", "labelAt", "labelHide"] {
        out.push(key(name, name, "text"));
    }
    out.push(key("arrowWidth", "arrowWidth", "float"));
    let sets = investigation.annotations().iter().filter(|a| matches!(a.kind, crate::utility::annotations::AnnotationKind::ArrowSet));
    out.push(options("arrows", "arrows", sets.map(|a| a.key.to_string())));
    out.push(key("highlight", "highlight", "text"));
    out.push(key("record", "record", "text"));
    out.push(key("blend", "blend", "float"));
    out.push(key("delay", "delay", "float"));
    out.push(key("term", "term", "int"));
    out.push(key("motion", "animate", "text"));
    out.push(options("camera", "camera", ["continuous".to_string()]));
    out.push(options("run", "run", investigation.actions().specs.iter().map(|s| s.key.to_string())));
    out.push(key("stop", "stop", "text"));
    out.push(options("formula", "formula", ["off".to_string()]));
    for name in ["card", "display", "colors"] {
        out.push(key(name, name, "text"));
    }
    // Dotted keys: an image kind's parameters, and an action's arguments.
    let mut dotted = BTreeMap::new();
    for kind in kinds {
        for p in &kind.params {
            merge(&mut dotted, format!("image.{}", p.key), if p.components > 1 { "vector" } else { "float" });
        }
    }
    for spec in investigation.actions().specs {
        for a in spec.args {
            use crate::actions::ArgKind;
            let kind = match a.kind {
                ArgKind::Number | ArgKind::Index | ArgKind::Integer => "float",
                ArgKind::Toggle => "bool",
                ArgKind::Vector(_) => "vector",
                ArgKind::Name => "text",
            };
            merge(&mut dotted, format!("run.{}", a.key), kind);
        }
    }
    // A dotted key is named after it (`image.mip` is `imageMip`), or `imageParam…`/`runArg…` where that name is taken.
    let taken: std::collections::BTreeSet<String> = out.iter().filter_map(|k| lamlet_present::vocab::camel(&[k["name"].as_str()?]).ok()).collect();
    for (k, kind) in dotted {
        let named = lamlet_present::vocab::camel(&[&k]).map_err(|e| format!("{k}: {e}"))?;
        let name = if taken.contains(&named) { k.replacen("image.", "imageParam.", 1).replacen("run.", "runArg.", 1) } else { k.clone() };
        out.push(key(&name, &k, kind));
    }
    // One function per name: a component whose name another key has (`camera.x` beside the value `cameraX`)
    // is left out; it can still be written with `attr`.
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|k| seen.insert(k["name"].as_str().and_then(|n| lamlet_present::vocab::camel(&[n]).ok()).unwrap_or_default()));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_option_lists_are_the_labs() {
        for p in PANELS {
            serde_json::from_value::<crate::shot::PanelMode>(json!(p)).unwrap_or_else(|e| panic!("panel {p}: {e}"));
        }
        for e in EASES {
            serde_json::from_value::<crate::animation::Ease>(json!(e)).unwrap_or_else(|x| panic!("ease {e}: {x}"));
        }
        for c in CORNERS {
            serde_json::from_value::<crate::utility::inset::Corner>(json!(c)).unwrap_or_else(|e| panic!("corner {c}: {e}"));
        }
    }
}
