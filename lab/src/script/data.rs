//! Content written in Lamlet, read by the lab's TOML readers: a `.lam` resource's static top-level binding,
//! written out as the TOML document the reader takes today, so the rest of the lab is unchanged. Lamlet computes
//! what the TOML would repeat (a room's walls from its size, a second scene from the first), and the reader keeps
//! checking what it loads.
//!
//! | Lamlet | TOML |
//! |---|---|
//! | the binding, a record | the document |
//! | a record field of the document | a table (`[lighting]`) |
//! | a list of records in the document | an array of tables (`[[object]]`) |
//! | a record anywhere deeper | an inline table |
//! | `{#x 90.0}`, an alternative of a variant | an inline table of one entry, `{ x = 90.0 }` |
//! | `none`, `{some v}` (std's options) | the field left out, or `v` |
//! | Int, Float, String, Bool, a list | an integer, a float, a string, a boolean, an array |
//!
//! A label in camel case is a key in kebab case (`emissiveStrength` is `emissive-strength`).

use lamlet::embed::{Data, SourceFile};
use std::collections::BTreeMap;

/// `camelCase` as `kebab-case`.
fn kebab(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// An option's payload (`some`), none for `none`, or the value itself.
fn present(d: &Data) -> Option<&Data> {
    match d {
        Data::Variant(tag, payload) if tag == "none" && matches!(**payload, Data::Unit) => None,
        Data::Variant(tag, payload) if tag == "some" => Some(payload),
        d => Some(d),
    }
}

fn key(label: &str) -> String {
    let k = kebab(label);
    if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        k
    } else {
        string(&k)
    }
}

fn string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A value as TOML writes it in a key's place; records and variants inline.
fn value(d: &Data, at: &str) -> Result<String, String> {
    Ok(match d {
        Data::Bool(b) => b.to_string(),
        Data::Int(n) => n.to_string(),
        // Rust's shortest round-trip form, which TOML reads back as the same number (`0.2`, `90.0`, `1e20`).
        Data::Float(x) => format!("{x:?}"),
        Data::String(s) => string(s),
        Data::List(items) => {
            let parts: Vec<String> = items.iter().enumerate().map(|(i, x)| value(x, &format!("{at}[{i}]"))).collect::<Result<_, _>>()?;
            format!("[{}]", parts.join(", "))
        }
        Data::Record(fields) => {
            let mut parts = Vec::new();
            for (k, v) in fields {
                if let Some(v) = present(v) {
                    parts.push(format!("{} = {}", key(k), value(v, &format!("{at}.{k}"))?));
                }
            }
            format!("{{ {} }}", parts.join(", "))
        }
        Data::Variant(tag, payload) => format!("{{ {} = {} }}", key(tag), value(payload, &format!("{at}.{tag}"))?),
        Data::Unit | Data::Type(_) => return Err(format!("{at}: TOML has no value for {d:?}")),
    })
}

/// A table's `key = value` lines.
fn lines(fields: &BTreeMap<String, Data>, at: &str, out: &mut String) -> Result<(), String> {
    for (k, v) in fields {
        if let Some(v) = present(v) {
            out.push_str(&format!("{} = {}\n", key(k), value(v, &format!("{at}.{k}"))?));
        }
    }
    Ok(())
}

/// The TOML document of a record: its record fields as tables, its lists of records as arrays of tables, the
/// rest as values.
pub fn document(d: &Data, name: &str) -> Result<String, String> {
    let Data::Record(fields) = d else { return Err(format!("`{name}` is a record")) };
    let is_tables = |v: &Data| matches!(v, Data::List(items) if !items.is_empty() && items.iter().all(|x| matches!(x, Data::Record(_))));
    let (mut plain, mut tables, mut arrays) = (BTreeMap::new(), Vec::new(), Vec::new());
    for (k, v) in fields {
        match present(v) {
            None => {}
            Some(Data::Record(inner)) => tables.push((k, inner)),
            Some(v) if is_tables(v) => arrays.push((k, v)),
            Some(v) => {
                plain.insert(k.clone(), v.clone());
            }
        }
    }
    let mut out = String::new();
    lines(&plain, name, &mut out)?;
    for (k, inner) in tables {
        out.push_str(&format!("\n[{}]\n", key(k)));
        lines(inner, &format!("{name}.{k}"), &mut out)?;
    }
    for (k, v) in arrays {
        let Data::List(items) = v else { unreachable!() };
        for (i, x) in items.iter().enumerate() {
            let Data::Record(inner) = x else { unreachable!() };
            out.push_str(&format!("\n[[{}]]\n", key(k)));
            lines(inner, &format!("{name}.{k}[{i}]"), &mut out)?;
        }
    }
    Ok(out)
}

/// The static top-level binding `name` of the Lamlet document `text` (its diagnostics naming `file`), as TOML.
pub fn toml(file: &str, text: &str, name: &str) -> Result<String, String> {
    let engine = super::engine(&[], None)?;
    let doc = engine.check(SourceFile::new(file, text)).map_err(|e| e.render())?;
    let binding = doc.binding(name).map_err(|e| format!("{file}: {}", e.render()))?;
    let data = Data::from_value(&binding.value).map_err(|e| format!("{file}: `{name}`: {e}"))?;
    document(&data, name).map_err(|e| format!("{file}: {e}"))
}

/// A resource's text as TOML: a `.lam` resource's binding named after the resource's last segment in camel case
/// (`render_lab.scene/study-room` is `studyRoom`), else the resource's own text.
pub fn resource_toml(resource: &str) -> Result<String, String> {
    let r = crate::resources::get(resource);
    if r.path().extension().is_some_and(|e| e == "lam") {
        let name = lamlet_present::vocab::camel(&[resource.rsplit('/').next().unwrap_or(resource)])?;
        toml(&r.path().display().to_string(), r.text(), &name)
    } else {
        Ok(r.text().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_become_tables_and_options_fields() {
        let text = "$[Import* std]\n$[# rot [% {Variant {' {Field \"x\" Float} {Field \"aim\" {List Float}}}}]]\n\
            $[# scene [% {. titleText \"scene.room\" cameraClear {: {#roomMargin 0.2} {Variant {' {Field \"roomMargin\" Float} {Field \"above\" Float}}}}\n\
              lighting {. ggxSamples 16 exposure 1.0}\n\
              object {' {. name \"floor\" rotation {: none {Option rot}}} {. name \"wall\" rotation {some {: {#x 90.0} rot}}}}}]]\n";
        let toml = toml("scene.lam", text, "scene").unwrap();
        let doc = toml_edit::Document::parse(toml.clone()).unwrap_or_else(|e| panic!("{e}: {toml}"));
        let get = |k: &str| doc.get(k).unwrap_or_else(|| panic!("{k} in {toml}"));
        assert_eq!(get("title-text").as_str(), Some("scene.room"));
        assert_eq!(get("camera-clear").as_inline_table().and_then(|t| t.get("room-margin")).and_then(|v| v.as_float()), Some(0.2));
        assert_eq!(get("lighting").as_table().and_then(|t| t.get("ggx-samples")).and_then(|i| i.as_integer()), Some(16));
        let objects = get("object").as_array_of_tables().unwrap();
        assert!(objects.get(0).unwrap().get("rotation").is_none());
        let rotation = objects.get(1).unwrap().get("rotation").and_then(|i| i.as_inline_table()).and_then(|t| t.get("x")).and_then(|v| v.as_float());
        assert_eq!(rotation, Some(90.0));
    }
}
