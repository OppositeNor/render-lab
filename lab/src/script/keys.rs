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

/// Attribute names a cue reserves besides its fields' (`fields`): what a formula's cue says (`say`), and the
/// prefix of a value named like a field (`param.`).
const RESERVED: &[&str] = &["say", "param"];

/// Fields render_lab's Lamlet package writes, typed or checked (packages/render_lab/language/render_lab.lam:
/// motions, points, marks, lists of names, records; callouts and text keys, through the lab's natives), so the
/// vocabulary leaves them out (the key list marks them `"typed": true`) and the two never define one name twice.
pub const TYPED: &[&str] = &[
    "animate", "arrowFrom", "arrowTo", "labelAt", "imageMark", "imageHide", "arrowHide", "labelHide", "highlight", "record", "callout", "imageTitle", "imageMarkText",
    "labelText",
];

/// The keys an enum's values are written with (its serde names), in its `ALL` order.
fn names<T: serde::Serialize>(all: &[T]) -> impl Iterator<Item = String> + '_ {
    all.iter().map(|v| serde_json::to_value(v).ok().and_then(|v| v.as_str().map(String::from)).expect("a unit variant"))
}

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

/// The fields of a timeline key a cue may set, less those render_lab types (`TYPED`); `"field": true` in the key
/// list, as are an image kind's parameters and an action's arguments, so tools/video/markup.py tells them from
/// values. Three are named apart from their attribute, where `content` and `present` already have the name
/// (`code`, `image`, `animate`), and `ease` too, whose options would be `present`'s easings.
fn fields(investigation: &dyn Investigation) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    out.push(options("listing", "code", investigation.listings().iter().map(|l| l.name.to_string())));
    out.push(options("panel", "panel", names(&crate::shot::PanelMode::ALL)));
    out.push(options("demo", "demo", investigation.demos().iter().map(|d| d.key().to_string())));
    out.push(options("easing", "ease", names(&crate::animation::Ease::ALL)));
    out.push(options("inset", "image", investigation.images().iter().map(|i| i.key.to_string())));
    out.push(options("imageCorner", "imageCorner", names(&crate::utility::inset::Corner::ALL)));
    let package = investigation.package().unwrap_or("render_lab");
    let kinds = crate::views::kinds(package).map_err(|e| e.to_string())?;
    out.push(options("imageFrom", "imageFrom", kinds.iter().map(|k| k.key.to_string())));
    out.push(key("imageGraph", "imageGraph", "text"));
    out.push(options("imageShow", "imageShow", crate::graph::Show::ALL.map(|s| s.key().to_string())));
    let clicks = investigation.actions().clicks.iter().map(|c| c.key.to_string());
    out.push(options("imageClick", "imageClick", clicks.chain(crate::script::runtime::click_handlers())));
    for name in ["arrow", "arrowColor", "label", "labelColor"] {
        out.push(key(name, name, "text"));
    }
    out.push(key("arrowWidth", "arrowWidth", "float"));
    let sets = investigation.annotations().iter().filter(|a| matches!(a.kind, crate::utility::annotations::AnnotationKind::ArrowSet));
    out.push(options("arrows", "arrows", sets.map(|a| a.key.to_string())));
    out.push(key("blend", "blend", "float"));
    out.push(key("delay", "delay", "float"));
    out.push(key("term", "term", "int"));
    out.push(options("camera", "camera", ["continuous".to_string()]));
    let scripts = crate::script::runtime::actions();
    out.push(options("run", "run", investigation.actions().specs.iter().map(|s| s.key.to_string()).chain(scripts.iter().map(|a| a.name.clone()))));
    out.push(key("stop", "stop", "text"));
    out.push(options("formula", "formula", ["off".to_string()]));
    for name in ["card", "display", "colors"] {
        out.push(key(name, name, "text"));
    }
    // render_lab's, listed for tools/video/markup.py; `render-lab lamlet vocab` leaves them out (`"typed": true`).
    for name in TYPED {
        let mut k = key(name, name, "text");
        k["typed"] = json!(true);
        out.push(k);
    }
    for k in &mut out {
        k["field"] = json!(true);
    }
    Ok(out)
}

/// A key's attribute (its `key`, else its name).
fn attribute(k: &Value) -> &str {
    k.get("key").or_else(|| k.get("name")).and_then(Value::as_str).unwrap_or_default()
}

/// The vocabulary key list for `investigation`.
pub fn keys(investigation: &dyn Investigation) -> Result<Vec<Value>, String> {
    let fields = fields(investigation)?;
    // A value named like a field's attribute is written `param.<name>`.
    let reserved = |name: &str| RESERVED.contains(&name) || TYPED.contains(&name) || fields.iter().any(|f| attribute(f) == name);
    let mut out = Vec::new();
    let mut components = Vec::new();
    for e in crate::values::list() {
        let (name, attr) = if reserved(e.key) { (format!("param.{}", e.key), format!("param.{}", e.key)) } else { (e.key.to_string(), e.key.to_string()) };
        out.push(match e.choices {
            Some(c) => options(&name, &attr, c.options.iter().map(|o| o.name.to_string())),
            None if e.ty.components() > 1 => key(&name, &attr, "vector"),
            None => key(&name, &attr, "float"),
        });
        // A vector's components are keys of their own (`lightX` of `lightPosition`), after every value.
        for c in e.components {
            let attr = if reserved(c) { format!("param.{c}") } else { c.to_string() };
            components.push(key(&attr, &attr, "float"));
        }
    }
    out.extend(components);
    out.extend(fields);
    // Dotted keys: an image kind's parameters, and an action's arguments.
    let mut dotted = BTreeMap::new();
    let package = investigation.package().unwrap_or("render_lab");
    for kind in crate::views::kinds(package).map_err(|e| e.to_string())? {
        for p in &kind.params {
            merge(&mut dotted, format!("image.{}", p.key), if p.components > 1 { "vector" } else { "float" });
        }
    }
    let script_args: Vec<(String, crate::actions::ArgKind)> = crate::script::runtime::actions().into_iter().flat_map(|a| a.args).map(|a| (a.key, a.kind)).collect();
    let code_args = investigation.actions().specs.iter().flat_map(|s| s.args).map(|a| (a.key.to_string(), a.kind));
    for (key, kind) in code_args.chain(script_args) {
        use crate::actions::ArgKind;
        let kind = match kind {
            ArgKind::Number | ArgKind::Index | ArgKind::Integer => "float",
            ArgKind::Toggle => "bool",
            ArgKind::Vector(_) => "vector",
            ArgKind::Name => "text",
        };
        merge(&mut dotted, format!("run.{key}"), kind);
    }
    // A dotted key is named after it (`image.mip` is `imageMip`), or `imageParam…`/`runArg…` where that name is taken.
    let taken: std::collections::BTreeSet<String> = out.iter().filter_map(|k| lamlet_present::vocab::camel(&[k["name"].as_str()?]).ok()).collect();
    for (k, kind) in dotted {
        let named = lamlet_present::vocab::camel(&[&k]).map_err(|e| format!("{k}: {e}"))?;
        let name = if taken.contains(&named) { k.replacen("image.", "imageParam.", 1).replacen("run.", "runArg.", 1) } else { k.clone() };
        let mut field = key(&name, &k, kind);
        field["field"] = json!(true);
        out.push(field);
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
        for p in names(&crate::shot::PanelMode::ALL) {
            serde_json::from_value::<crate::shot::PanelMode>(json!(p)).unwrap_or_else(|e| panic!("panel {p}: {e}"));
        }
        for e in names(&crate::animation::Ease::ALL) {
            serde_json::from_value::<crate::animation::Ease>(json!(e)).unwrap_or_else(|x| panic!("ease {e}: {x}"));
        }
        assert!(names(&crate::animation::Ease::ALL).any(|e| e == "sine"));
        for c in names(&crate::utility::inset::Corner::ALL) {
            serde_json::from_value::<crate::utility::inset::Corner>(json!(c)).unwrap_or_else(|e| panic!("corner {c}: {e}"));
        }
    }
}
