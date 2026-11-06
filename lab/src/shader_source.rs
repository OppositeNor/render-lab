//! A pass's WGSL as the lab compiles it: the module its pipeline is made from
//! (src/shaders.rs: the package's files composed, as they are now), cut to the
//! pass's entry points and everything they use (the functions they call, the
//! structs, constants, overrides, aliases, and bindings those name), in the
//! module's order after the entry points, each piece with the file and line it comes from. The frame
//! graph's full-screen view shows it beside the graph (src/ui/).

use crate::resources;
use crate::shaders;
use std::collections::BTreeSet;

/// A top-level declaration of a module: its name, what it is, its text (with
/// the doc comments and attributes above it), and where it comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub name: String,
    pub kind: &'static str,
    pub text: String,
    /// The file (from the lab's root) and its first line there.
    pub file: String,
    pub line: u32,
}

/// The declarations of WGSL `source`, in order, each with its first line in
/// `source` (1-based). Comments and whitespace between them are dropped;
/// `///` doc comments and attributes stay with what they precede.
pub fn items(source: &str) -> Vec<(Item, u32)> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let line_of = |at: usize| source[..at].bytes().filter(|&b| b == b'\n').count() as u32 + 1;
    while i < bytes.len() {
        // Skip whitespace and plain comments; keep doc comments as the start of an item.
        let start = i;
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if source[i..].starts_with("//") && !source[i..].starts_with("///") {
            i = source[i..].find('\n').map_or(bytes.len(), |n| i + n + 1);
            continue;
        }
        if source[i..].starts_with("/*") {
            i = source[i..].find("*/").map_or(bytes.len(), |n| i + n + 2);
            continue;
        }
        // An item: up to its end at depth 0 (a `}` that closes its body, or a `;`).
        let mut depth = 0i32;
        let mut j = i;
        let mut opened = false;
        while j < bytes.len() {
            if source[j..].starts_with("//") {
                j = source[j..].find('\n').map_or(bytes.len(), |n| j + n + 1);
                continue;
            }
            if source[j..].starts_with("/*") {
                j = source[j..].find("*/").map_or(bytes.len(), |n| j + n + 2);
                continue;
            }
            match bytes[j] {
                b'{' => {
                    depth += 1;
                    opened = true;
                }
                b'}' => {
                    depth -= 1;
                    if depth == 0 && opened {
                        j += 1;
                        // `struct S { … };` takes its semicolon.
                        let rest = source[j..].trim_start_matches([' ', '\t']);
                        if rest.starts_with(';') {
                            j = source.len() - rest.len() + 1;
                        }
                        break;
                    }
                }
                b';' if depth == 0 => {
                    j += 1;
                    break;
                }
                _ => {}
            }
            j += 1;
        }
        let text = source[start..j.min(bytes.len())].trim_end().to_string();
        if let Some((kind, name)) = declared(&text) {
            out.push((Item { name, kind, text, file: String::new(), line: 0 }, line_of(start)));
        }
        i = j.max(start + 1);
    }
    out
}

/// What a declaration declares, past its doc comments and attributes.
fn declared(text: &str) -> Option<(&'static str, String)> {
    let code: String = text.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join(" ");
    let mut rest = code.trim_start();
    // Attributes: `@name` or `@name(…)`.
    while let Some(after) = rest.strip_prefix('@') {
        let name_end = after.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(after.len());
        let mut tail = &after[name_end..];
        if tail.trim_start().starts_with('(') {
            let open = tail.find('(')?;
            let close = tail[open..].find(')')? + open;
            tail = &tail[close + 1..];
        }
        rest = tail.trim_start();
    }
    for (keyword, kind) in [("fn", "fn"), ("struct", "struct"), ("const", "const"), ("override", "override"), ("alias", "alias"), ("var", "var")] {
        if let Some(after) = rest.strip_prefix(keyword).filter(|a| a.starts_with([' ', '<', '\t'])) {
            let mut after = after.trim_start();
            // `var<storage, read> name`.
            if keyword == "var" && after.starts_with('<') {
                after = after[after.find('>')? + 1..].trim_start();
            }
            let name: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            return (!name.is_empty()).then_some((kind, name));
        }
    }
    None
}

/// The identifiers `text` names (outside comments).
fn names(text: &str) -> BTreeSet<String> {
    let code: String = text.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
    code.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| w.starts_with(|c: char| c.is_alphabetic() || c == '_')).map(str::to_string).collect()
}

/// `entries` of module `id` and everything they use, as the module stands now.
pub fn excerpt(id: &str, entries: &[&str]) -> Result<Vec<Item>, String> {
    let source = shaders::compose(id).map_err(|e| format!("{e:#}"))?;
    let all = items(&source);
    let mut wanted: Vec<String> = Vec::new();
    for e in entries {
        if !all.iter().any(|(it, _)| it.kind == "fn" && it.name == *e) {
            return Err(format!("{id}: no entry point `{e}`"));
        }
        wanted.push(e.to_string());
    }
    // Everything the wanted items name, until nothing new is named.
    let mut k = 0;
    while k < wanted.len() {
        let name = wanted[k].clone();
        if let Some((it, _)) = all.iter().find(|(it, _)| it.name == name) {
            for n in names(&it.text) {
                if n != name && !wanted.contains(&n) && all.iter().any(|(o, _)| o.name == n) {
                    wanted.push(n);
                }
            }
        }
        k += 1;
    }
    // Each kept item's file and line: the module's files are its lines in order.
    let files: Vec<(&str, u32)> = shaders::files(id).map_err(|e| format!("{e:#}"))?.iter().map(|f| {
        let r = resources::get(f);
        (r.source(), r.text().lines().count() as u32)
    }).collect();
    let locate = |mut line: u32| -> (String, u32) {
        for (file, count) in &files {
            if line <= *count {
                return (file.to_string(), line);
            }
            line -= count;
        }
        (id.to_string(), line)
    };
    // The entry points first, then what they use, in the module's order.
    let mut kept: Vec<Item> = all
        .into_iter()
        .filter(|(it, _)| wanted.contains(&it.name))
        .map(|(mut it, line)| {
            (it.file, it.line) = locate(line);
            it
        })
        .collect();
    kept.sort_by_key(|it| !entries.contains(&it.name.as_str()));
    Ok(kept)
}

/// `excerpt`, kept per module, entries, and generation of the resources (a
/// live reload makes them again).
pub fn cached(id: &str, entries: &[&str]) -> Result<std::sync::Arc<Vec<Item>>, String> {
    type Key = (String, Vec<String>, u64);
    type Cache = Vec<(Key, Result<std::sync::Arc<Vec<Item>>, String>)>;
    static CACHE: std::sync::Mutex<Cache> = std::sync::Mutex::new(Vec::new());
    let key: Key = (id.to_string(), entries.iter().map(|e| e.to_string()).collect(), resources::generation());
    let mut cache = CACHE.lock().unwrap();
    if let Some((_, v)) = cache.iter().find(|(k, _)| *k == key) {
        return v.clone();
    }
    let made = excerpt(id, entries).map(std::sync::Arc::new);
    cache.retain(|(k, _)| k.2 == key.2);
    cache.push((key, made.clone()));
    made
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_a_module_into_its_declarations() {
        let source = "// a plain comment\nstruct A {\n    x: f32,\n};\n\n/// The data.\n@group(0) @binding(1) var<storage, read> gData: array<A>;\nconst K: f32 = 2.0;\nfn helper(a: A) -> f32 {\n    return a.x * K; // {braces} in a comment\n}\n@compute @workgroup_size(8, 8)\nfn mainCS(@builtin(global_invocation_id) id: vec3<u32>) {\n    let v = helper(gData[id.x]);\n}\nfn unused() {}\n";
        let found: Vec<(String, &str, u32)> = items(source).into_iter().map(|(it, line)| (it.name, it.kind, line)).collect();
        assert_eq!(found, [("A".to_string(), "struct", 2), ("gData".into(), "var", 6), ("K".into(), "const", 8), ("helper".into(), "fn", 9), ("mainCS".into(), "fn", 12), ("unused".into(), "fn", 16)]);
        // The doc comment and the attributes stay with the binding.
        assert!(items(source)[1].0.text.starts_with("/// The data.\n@group(0)"));
    }
}
