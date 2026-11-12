//! The keys a story's cues may set, as lamlet-present's vocabulary key list (`render-lab story keys`): the
//! investigation's values, and the fields of a timeline key with their options where the lab knows them.
//! `render-lab lamlet vocab` turns the list into the investigation's cue package, so a misspelled key is an
//! unknown name and a wrong value a type error, at the storyboard's line.
//!
//! The vocabulary writes each value as typed data (lamlet-present 0.3: a Float, an Int, a Bool, a `List Float`,
//! a range `{. from a to b}`), read by `tools/video/markup.py` (`cue_key`, `param_value`); the kinds here are
//! what a cue may be written with, not the lab's types: a numeric value is a float, whatever its type, since
//! the timeline blends every value as a number (a range over an integer or a toggle is a fade).

use crate::investigation::Investigation;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Attribute names a cue reserves besides its fields' (`fields`): what a formula's cue says (`say`), and the
/// prefix of a value named like a field (`param.`).
const RESERVED: &[&str] = &["say", "param"];

/// Fields render_lab's Lamlet package writes, typed or checked (packages/render_lab/language/render_lab.lam:
/// motions, points, marks, lists of names, records; callouts and text keys, through the lab's natives), so the
/// vocabulary leaves them out (the key list marks them `"typed": true`) and the two never define one name twice.
pub const TYPED: &[&str] = &[
    "animate",
    "arrowFrom",
    "arrowTo",
    "labelAt",
    "imageMark",
    "imageHide",
    "arrowHide",
    "labelHide",
    "record",
    "callout",
    "imageTitle",
    "imageMarkText",
    "labelText",
    "highlightPasses",
    "highlightCode",
    "highlightFields",
];

/// The keys an enum's values are written with (its serde names), in its `ALL` order.
fn names<T: serde::Serialize>(all: &[T]) -> impl Iterator<Item = String> + '_ {
    all.iter().map(|v| {
        serde_json::to_value(v)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .expect("a unit variant")
    })
}

fn key(name: &str, key: &str, kind: &str) -> Value {
    if name == key {
        json!({"name": name, "kind": kind})
    } else {
        json!({"name": name, "key": key, "kind": kind})
    }
}

/// An option `options` left out: its attribute, the option, and why.
type Dropped = (String, String, &'static str);

thread_local! {
    /// The options `options` left out, while `schema` collects them.
    static DROPPED: std::cell::RefCell<Option<Vec<Dropped>>> = const { std::cell::RefCell::new(None) };
}

/// A text key with options: only those that make a Lamlet name, once each (the rest are still written as text).
fn options(name: &str, key: &str, options: impl IntoIterator<Item = String>) -> Value {
    open_or_closed(name, key, options, false)
}

/// A text key whose options are the lab's own (panels, demos, listings, eases, …): closed, so the vocabulary has a
/// constant per option and no function that takes any String, and the key check (`apply_keys`) refuses any other
/// value, also one written with `attr`; open when it has no options or the vocabulary left one out.
fn closed(name: &str, key: &str, options: impl IntoIterator<Item = String>) -> Value {
    open_or_closed(name, key, options, true)
}

fn open_or_closed(
    name: &str,
    key: &str,
    options: impl IntoIterator<Item = String>,
    closed: bool,
) -> Value {
    let options: Vec<String> = options.into_iter().collect();
    let all = options.len();
    let mut names = std::collections::BTreeSet::new();
    let kept: Vec<String> = options
        .into_iter()
        .filter(|o| {
            let why = match lamlet_present::vocab::camel(&[name, o]) {
                Err(_) => Some("not a Lamlet name"),
                Ok(n) => (!names.insert(n)).then_some("another option makes the same name"),
            };
            if let Some(why) = why {
                DROPPED.with(|d| {
                    d.borrow_mut()
                        .as_mut()
                        .map(|d| d.push((key.to_string(), o.clone(), why)))
                });
            }
            why.is_none()
        })
        .collect();
    let mut k = self::key(name, key, "text");
    if closed && !kept.is_empty() && kept.len() == all {
        k["closed"] = json!(true);
    }
    k["options"] = json!(kept);
    k
}

/// Kinds of the same dotted key from several declarations: one kind, or text when they differ.
fn merge(into: &mut BTreeMap<String, &'static str>, key: String, kind: &'static str) {
    into.entry(key)
        .and_modify(|k| {
            if *k != kind {
                *k = "text"
            }
        })
        .or_insert(kind);
}

/// The fields of a timeline key a cue may set, less those render_lab types (`TYPED`); `"field": true` in the key
/// list, as are an image kind's parameters and an action's arguments, so tools/video/markup.py tells them from
/// values. Three are named apart from their attribute, where `content` and `present` already have the name
/// (`code`, `image`, `animate`), and `ease` too, whose options would be `present`'s easings.
fn fields(investigation: &dyn Investigation) -> Result<Vec<Value>, String> {
    let mut out = vec![
        closed(
            "listing",
            "code",
            investigation.listings().iter().map(|l| l.name.to_string()),
        ),
        closed("panel", "panel", names(&crate::shot::PanelMode::ALL)),
        closed(
            "demo",
            "demo",
            investigation.demos().iter().map(|d| d.key().to_string()),
        ),
        closed("easing", "ease", names(&crate::animation::Ease::ALL)),
        // Open: a story names the instances of image kinds it makes, and `""` hides every image.
        options(
            "inset",
            "image",
            investigation.images().iter().map(|i| i.key.to_string()),
        ),
        closed(
            "imageCorner",
            "imageCorner",
            names(&crate::utility::inset::Corner::ALL),
        ),
    ];
    let package = investigation.package().unwrap_or("render_lab");
    let kinds = crate::views::kinds(package).map_err(|e| e.to_string())?;
    out.push(closed(
        "imageFrom",
        "imageFrom",
        kinds.iter().map(|k| k.key.to_string()),
    ));
    out.push(key("imageGraph", "imageGraph", "text"));
    out.push(closed(
        "imageShow",
        "imageShow",
        crate::graph::Show::ALL.map(|s| s.key().to_string()),
    ));
    let clicks = investigation
        .actions()
        .clicks
        .iter()
        .map(|c| c.key.to_string());
    out.push(closed(
        "imageClick",
        "imageClick",
        clicks.chain(crate::script::runtime::click_handlers()),
    ));
    for name in ["arrow", "arrowColor", "label", "labelColor"] {
        out.push(key(name, name, "text"));
    }
    out.push(key("arrowWidth", "arrowWidth", "float"));
    let sets = investigation.annotations().iter().filter(|a| {
        matches!(
            a.kind,
            crate::utility::annotations::AnnotationKind::ArrowSet
        )
    });
    out.push(closed("arrows", "arrows", sets.map(|a| a.key.to_string())));
    out.push(key("blend", "blend", "float"));
    out.push(key("delay", "delay", "float"));
    out.push(key("term", "term", "int"));
    out.push(closed("camera", "camera", ["continuous".to_string()]));
    let scripts = crate::script::runtime::actions();
    let actions: Vec<String> = investigation
        .actions()
        .specs
        .iter()
        .map(|s| s.key.to_string())
        .chain(scripts.iter().map(|a| a.name.clone()))
        .collect();
    out.push(closed("run", "run", actions.clone()));
    // The actions a cue stops: a list of their names, closed to them (an investigation without actions has none).
    let mut stop = key("stop", "stop", "names");
    if !actions.is_empty() {
        stop["options"] = json!(actions);
        stop["closed"] = json!(true);
    }
    out.push(stop);
    // Open: besides `off`, a formula's cue carries its LaTeX (render_lab's `formulaSaid`).
    out.push(options("formula", "formula", ["off".to_string()]));
    for name in ["card", "display"] {
        out.push(closed(name, name, ["yes", "no"].map(str::to_string)));
    }
    out.push(key("colors", "colors", "text"));
    // render_lab's (`"typed": true`, for tools/video/markup.py): keys the vocabulary has no function for (`any`),
    // so the key check knows them.
    for name in TYPED {
        let mut k = key(name, name, "any");
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
    k.get("key")
        .or_else(|| k.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// The vocabulary key list for `investigation`.
pub fn keys(investigation: &dyn Investigation) -> Result<Vec<Value>, String> {
    let fields = fields(investigation)?;
    // A value named like a field's attribute is written `param.<name>`.
    let reserved = |name: &str| {
        RESERVED.contains(&name)
            || TYPED.contains(&name)
            || fields.iter().any(|f| attribute(f) == name)
    };
    let mut out = Vec::new();
    let mut components = Vec::new();
    for e in crate::values::list() {
        let (name, attr) = if reserved(e.key) {
            (format!("param.{}", e.key), format!("param.{}", e.key))
        } else {
            (e.key.to_string(), e.key.to_string())
        };
        out.push(match e.choices {
            Some(c) => closed(&name, &attr, c.options.iter().map(|o| o.name.to_string())),
            None if e.ty.components() > 1 => key(&name, &attr, "vector"),
            None => key(&name, &attr, "float"),
        });
        // A vector's components are keys of their own (`lightX` of `lightPosition`), after every value.
        for c in e.components {
            let attr = if reserved(c) {
                format!("param.{c}")
            } else {
                c.to_string()
            };
            components.push(key(&attr, &attr, "float"));
        }
    }
    out.extend(components);
    out.extend(fields);
    // Dotted keys: an image kind's parameters, and an action's arguments.
    let dotted = dotted_keys(investigation)?;
    // A dotted key is named after it (`image.mip` is `imageMip`), or `imageParam…`/`runArg…` where that name is taken.
    let taken: std::collections::BTreeSet<String> = out
        .iter()
        .filter_map(|k| lamlet_present::vocab::camel(&[k["name"].as_str()?]).ok())
        .collect();
    for (k, kind) in dotted {
        let named = lamlet_present::vocab::camel(&[&k]).map_err(|e| format!("{k}: {e}"))?;
        let name = if taken.contains(&named) {
            k.replacen("image.", "imageParam.", 1)
                .replacen("run.", "runArg.", 1)
        } else {
            k.clone()
        };
        let mut field = key(&name, &k, kind);
        field["field"] = json!(true);
        out.push(field);
    }
    // One function per name: a component whose name another key has (`camera.x` beside the value `cameraX`)
    // is left out; it can still be written with `attr`.
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|k| {
        let kept = seen.insert(
            k["name"]
                .as_str()
                .and_then(|n| lamlet_present::vocab::camel(&[n]).ok())
                .unwrap_or_default(),
        );
        if !kept {
            DROPPED.with(|d| {
                d.borrow_mut().as_mut().map(|d| {
                    d.push((
                        attribute(k).to_string(),
                        String::new(),
                        "another key has its function name",
                    ))
                })
            });
        }
        kept
    });
    // Families the lab checks when it loads the timeline: an image's parameters besides those of the kinds the
    // package defines (`image.<param>`, after every other key, since the key check takes the first that matches), and another view's values (render_lab's `view`
    // writes `view.<name>.<key>`). No functions; `{attr "image.level" 1.0}` writes the first.
    for (name, family) in [("imageParam", "image"), ("view", "view")] {
        let mut k = key(name, family, "any");
        k["prefix"] = json!(true);
        out.push(k);
    }
    out.extend(heading_keys(false));
    Ok(out)
}

/// Dotted keys and their types: an image kind's parameters (`image.<param>`), and an action's arguments
/// (`run.<arg>`), the code's and the scripts'.
fn dotted_keys(
    investigation: &dyn Investigation,
) -> Result<BTreeMap<String, &'static str>, String> {
    let mut dotted = BTreeMap::new();
    let package = investigation.package().unwrap_or("render_lab");
    for kind in crate::views::kinds(package).map_err(|e| e.to_string())? {
        for p in &kind.params {
            merge(
                &mut dotted,
                format!("image.{}", p.key),
                if p.components > 1 {
                    "vector"
                } else {
                    "float"
                },
            );
        }
    }
    let script_args: Vec<(String, crate::actions::ArgKind)> = crate::script::runtime::actions()
        .into_iter()
        .flat_map(|a| a.args)
        .map(|a| (a.key, a.kind))
        .collect();
    let code_args = investigation
        .actions()
        .specs
        .iter()
        .flat_map(|s| s.args)
        .map(|a| (a.key.to_string(), a.kind));
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
    Ok(dotted)
}

/// A heading key of `levels` (lamlet-present's: no function; `apply_keys` reads the heading's attribute by its
/// kind, or adds `default` when the heading leaves it out).
fn heading(name: &str, kind: &str, levels: &[u8], default: Option<Value>) -> Value {
    let mut k = key(name, name, kind);
    k["on"] = json!("heading");
    if !levels.is_empty() {
        k["levels"] = json!(levels);
    }
    if let Some(d) = default {
        k["default"] = d;
    }
    k
}

/// The attributes a storyboard's headings take (`# video`, `## chapter`, `### beat`), or its translation's: each
/// read by its kind (`fps=30` an Int, `sources="a, b"` a list of names), the pacing and caption width with their
/// defaults (tools/video/README.md), and the archival metadata on any heading (`engineVersion`, `commit`, `date`,
/// `meta.<name>`), kept as written. What a kind cannot say (a required key, a number's limits) tools/video's
/// headings.py checks.
pub fn heading_keys(translation: bool) -> Vec<Value> {
    let locales: Vec<String> = crate::i18n::Locale::all()
        .into_iter()
        .map(|l| l.code().to_string())
        .collect();
    let mut locale = heading("locale", "text", &[1], None);
    locale["options"] = json!(locales);
    locale["closed"] = json!(true);
    let mut out = vec![
        heading("title", "text", &[1], None),
        heading("subtitle", "text", &[1], None),
        locale,
    ];
    if translation {
        out.push(heading("glossary", "names", &[1], None));
        // The zh-CN edition's display units per caption line (tools/video/zh.py's `CAPTION_UNITS` when left out).
        out.push(heading("captionWidth", "int", &[1], None));
        out.push(heading("number", "int", &[2], None));
        out.push(heading("summary", "text", &[2], None));
    } else {
        for name in ["fps", "width", "height"] {
            out.push(heading(name, "int", &[1], None));
        }
        // Characters per caption line, and the pacing (seconds; `taggedWeight` a ratio).
        out.push(heading("captionWidth", "int", &[1], Some(json!(58))));
        for (name, default) in [
            ("lead", 0.40),
            ("gap", 0.32),
            ("tail", 0.70),
            ("maxExtraPause", 2.2),
            ("taggedWeight", 2.0),
            ("termFade", 0.4),
            ("captionBridge", 0.6),
        ] {
            out.push(heading(name, "float", &[1], Some(json!(default))));
        }
        out.push(heading("number", "int", &[2], None));
        out.push(heading("target", "float", &[2], None));
        out.push(heading("summary", "text", &[2], None));
        out.push(heading("sources", "names", &[2], None));
        let mut kind = heading("kind", "text", &[3], None);
        kind["options"] = json!(["3d", "2d"]);
        kind["closed"] = json!(true);
        out.push(kind);
        for name in ["diagram", "graph"] {
            out.push(heading(name, "text", &[3], None));
        }
        for name in ["focus", "sources"] {
            out.push(heading(name, "names", &[3], None));
        }
        // A 2d beat that shows the works the story cites, the source index's references.
        let mut references = heading("references", "text", &[3], None);
        references["options"] = json!(["true", "false"]);
        references["closed"] = json!(true);
        out.push(references);
    }
    for name in ["engineVersion", "commit", "date"] {
        out.push(heading(name, "any", &[], None));
    }
    let mut meta = heading("meta", "any", &[], None);
    meta["prefix"] = json!(true);
    out.push(meta);
    out
}

/// The key list with what a reader needs to write each key (`render-lab story schema`): a value's type, unit,
/// range, default, and what it is; the attribute a function writes; and what the vocabulary leaves out (options
/// that make no Lamlet name, keys whose function name another has), each with the `attr` that still writes it.
pub fn schema(investigation: &dyn Investigation) -> Result<Value, String> {
    DROPPED.with(|d| *d.borrow_mut() = Some(Vec::new()));
    let keys = keys(investigation);
    let dropped = DROPPED.with(|d| d.borrow_mut().take()).unwrap_or_default();
    let mut keys = keys?;
    let values: BTreeMap<&str, &crate::values::Entry> = crate::values::list()
        .into_iter()
        .map(|e| (e.key, e))
        .collect();
    for k in &mut keys {
        let attr = attribute(k).to_string();
        let name = k["name"].as_str().unwrap_or_default().to_string();
        k["attribute"] = json!(attr);
        k["function"] = json!(lamlet_present::vocab::camel(&[&name]).ok());
        if let Some(e) = values.get(attr.trim_start_matches("param.")) {
            k["value"] = json!({
                "type": format!("{:?}", e.ty),
                "unit": format!("{:?}", e.unit),
                "range": e.range.map(|(a, b)| [a, b]),
                "default": e.default.map(|v| v.to_json()),
                "components": e.components,
                "doc": e.doc,
            });
        }
    }
    let omitted: Vec<Value> = dropped
        .into_iter()
        .map(|(key, option, why)| {
            let write = if option.is_empty() {
                format!("{{attr \"{key}\" \"…\"}}")
            } else {
                format!("{{attr \"{key}\" \"{option}\"}}")
            };
            json!({"attribute": key, "option": (!option.is_empty()).then_some(option), "why": why, "write": write})
        })
        .collect();
    Ok(json!({"renderLabStorySchema": 1, "keys": keys, "omitted": omitted}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_option_lists_are_the_labs() {
        for p in names(&crate::shot::PanelMode::ALL) {
            serde_json::from_value::<crate::shot::PanelMode>(json!(p))
                .unwrap_or_else(|e| panic!("panel {p}: {e}"));
        }
        for e in names(&crate::animation::Ease::ALL) {
            serde_json::from_value::<crate::animation::Ease>(json!(e))
                .unwrap_or_else(|x| panic!("ease {e}: {x}"));
        }
        assert!(names(&crate::animation::Ease::ALL).any(|e| e == "sine"));
        for c in names(&crate::utility::inset::Corner::ALL) {
            serde_json::from_value::<crate::utility::inset::Corner>(json!(c))
                .unwrap_or_else(|e| panic!("corner {c}: {e}"));
        }
    }
}
