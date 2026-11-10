//! The checks a package's build runs (its build.rs, and the lab's for its own
//! package, render_lab): every shader the package declares parses and validates
//! with naga, and its text tables match the code and data that name their
//! keys. A failure fails the build; nothing is generated. They read only the
//! package's directory, its code, and the packages beside it it imports.

use fluent_syntax::ast;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The registered locales (render_lab's `locales.toml`, lab/src/i18n.rs `Locale`), beside `module_dir`: each
/// one's code and whether it is complete.
fn locales(module_dir: &Path) -> Vec<(String, bool)> {
    let path = module_dir
        .parent()
        .expect("a package is in a packages directory")
        .join("render_lab/locales.toml");
    println!("cargo:rerun-if-changed={}", path.display());
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let document =
        toml_edit::Document::parse(text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let tables = document
        .get("locale")
        .and_then(|i| i.as_array_of_tables())
        .unwrap_or_else(|| panic!("{}: no [[locale]]", path.display()));
    let field = |t: &toml_edit::Table, k: &str| {
        t.get(k)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| panic!("{}: a locale without `{k}`", path.display()))
    };
    let out: Vec<(String, bool)> = tables
        .iter()
        .map(|t| (field(t, "code"), field(t, "status") == "complete"))
        .collect();
    assert!(
        out.first()
            .is_some_and(|(code, complete)| code == "en" && *complete),
        "{}: the first locale is en, complete",
        path.display()
    );
    out
}

/// The functions a text may call: the rich text's (lab/src/rich_text.rs).
const FUNCTIONS: &[&str] = &["FORMULA", "CODE"];

/// What the collections declare that the shader check needs: each
/// resource's source file (with the overrides applied), and each shader's
/// parts, by full key (`<package>.<key>`).
#[derive(Default)]
struct Manifests {
    sources: BTreeMap<String, PathBuf>,
    /// The `built = true` resources (a tool's or the lab's output, in the
    /// package's `build/`), which may not exist yet.
    built: BTreeMap<String, PathBuf>,
    shaders: BTreeMap<String, Vec<String>>,
}

/// A package: its name, directory, and collections.
struct Package {
    name: String,
    dir: PathBuf,
    collections: Vec<PathBuf>,
}

/// A package and the packages it imports, directly or through another, from
/// the packages directory beside it (a package's directory is its name; the
/// run time checks their manifests in full). Only their manifests rerun the
/// checks: the packages directory itself would rerun them on any change under
/// it, other packages' content and tools' build/ output alike.
fn find_packages(module_dir: &Path) -> Vec<Package> {
    let packages_dir = module_dir
        .parent()
        .expect("a package is a directory of a packages directory");
    let mut packages: Vec<Package> = Vec::new();
    let mut pending = vec![module_dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        // A package is read once, and its imports join the walk, so every package it reaches is found.
        if packages.iter().any(|m| m.dir == dir) {
            continue;
        }
        let path = dir.join("manifest.toml");
        println!("cargo:rerun-if-changed={}", path.display());
        let text =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let document =
            toml_edit::Document::parse(text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let name = document
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or_else(|| panic!("{}: no `name`", path.display()))
            .to_string();
        let imports = document
            .get("import")
            .and_then(|i| i.as_array())
            .into_iter()
            .flatten()
            .filter_map(|i| i.as_str());
        pending.extend(imports.map(|import| packages_dir.join(import)));
        let collections = document
            .get("provides")
            .and_then(|p| p.as_array_of_tables())
            .into_iter()
            .flatten()
            .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("resource"))
            .map(|p| dir.join(p.get("path").and_then(|p| p.as_str()).unwrap_or_default()))
            .collect();
        packages.push(Package {
            name,
            dir,
            collections,
        });
    }
    packages
}

/// A reference's full key: its own package's key, or `<package>.<key>` as it is.
fn full_key(package: &str, reference: &str) -> String {
    if reference.contains('.') {
        reference.to_string()
    } else {
        format!("{package}.{reference}")
    }
}

/// One collection's resources and shaders; overrides go to `overrides`.
fn read_collection(
    package: &str,
    path: &Path,
    out: &mut Manifests,
    overrides: &mut Vec<(String, PathBuf)>,
) {
    println!("cargo:rerun-if-changed={}", path.display());
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let document =
        toml_edit::Document::parse(text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let dir = path.parent().unwrap();
    for table in document
        .get("resource")
        .and_then(|r| r.as_array_of_tables())
        .into_iter()
        .flatten()
    {
        let id = full_key(
            package,
            table.get("id").and_then(|i| i.as_str()).unwrap_or_default(),
        );
        let source = dir.join(
            table
                .get("source")
                .and_then(|s| s.as_str())
                .unwrap_or_default(),
        );
        if table.get("built").and_then(|b| b.as_bool()) == Some(true) {
            out.built.insert(id, source);
            continue;
        }
        if let Some(target) = table.get("override").and_then(|o| o.as_str()) {
            overrides.push((target.to_string(), source.clone()));
        }
        out.sources.insert(id, source);
    }
    for table in document
        .get("shader")
        .and_then(|s| s.as_array_of_tables())
        .into_iter()
        .flatten()
    {
        let id = full_key(
            package,
            table.get("id").and_then(|i| i.as_str()).unwrap_or_default(),
        );
        let parts = table
            .get("parts")
            .and_then(|p| p.as_array())
            .into_iter()
            .flatten()
            .filter_map(|p| p.as_str().map(|p| full_key(package, p)))
            .collect();
        out.shaders.insert(id, parts);
    }
}

/// A shader's files, in order; false when a part is a built resource not
/// written yet (the lab writes the generated ones when it first runs).
fn shader_files(manifests: &Manifests, id: &str, out: &mut Vec<PathBuf>) -> bool {
    for part in &manifests.shaders[id] {
        // A part is another shader (its files, in place), a built resource (once written), or a source file.
        if manifests.shaders.contains_key(part) {
            if !shader_files(manifests, part, out) {
                return false;
            }
        } else if let Some(built) = manifests.built.get(part) {
            if !built.is_file() {
                return false;
            }
            out.push(built.clone());
        } else {
            out.push(
                manifests
                    .sources
                    .get(part)
                    .unwrap_or_else(|| panic!("shader `{id}`: no resource or shader `{part}`"))
                    .clone(),
            );
        }
    }
    true
}

/// Every shader `package` declares parses and validates with naga (once its
/// built parts exist), composed
/// from the packages beside it with its own overrides applied (the run time
/// validates every shader again as it composes it, with every package's).
fn check_shaders(packages: &[Package], package: &str) {
    let mut manifests = Manifests::default();
    let mut overrides = Vec::new();
    for m in packages {
        // Every package's collections fill the manifests; the checked package's own overrides are kept apart.
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
    for id in manifests
        .shaders
        .keys()
        .filter(|id| id.split_once('.').is_some_and(|(m, _)| m == package))
    {
        let mut files = Vec::new();
        // A shader that composes a part not written yet is checked once it is (the run time checks it as it composes it).
        if !shader_files(&manifests, id, &mut files) {
            continue;
        }
        let source: String = files
            .iter()
            .map(|f| {
                println!("cargo:rerun-if-changed={}", f.display());
                fs::read_to_string(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display()))
            })
            .collect();
        // As the lab composes it (lab/src/shaders.rs `hoist_enables`): `enable` directives first.
        let source = {
            let mut enables: Vec<String> = Vec::new();
            let body: String = source
                .split_inclusive('\n')
                .map(|line| {
                    let t = line.trim();
                    if t.starts_with("enable ") && t.ends_with(';') {
                        if !enables.iter().any(|e| e == t) {
                            enables.push(t.to_string());
                        }
                        "\n".to_string()
                    } else {
                        line.to_string()
                    }
                })
                .collect();
            if enables.is_empty() {
                body
            } else {
                format!("{} {body}", enables.join(" "))
            }
        };
        let package = naga::front::wgsl::parse_str(&source).unwrap_or_else(|e| {
            panic!(
                "WGSL parse error in shader `{id}`:\n{}",
                e.emit_to_string(&source)
            )
        });
        // The capabilities the lab validates with (lab/src/shaders.rs `capabilities`).
        let capabilities = Capabilities::default()
            | Capabilities::TEXTURE_ATOMIC
            | Capabilities::TEXTURE_AND_SAMPLER_BINDING_ARRAY
            | Capabilities::TEXTURE_AND_SAMPLER_BINDING_ARRAY_NON_UNIFORM_INDEXING;
        Validator::new(ValidationFlags::all(), capabilities)
            .validate(&package)
            .unwrap_or_else(|e| {
                panic!(
                    "WGSL validation error in shader `{id}`:\n{}",
                    e.emit_to_string(&source)
                )
            });
    }
}

/// What the check reads of a Fluent message: the variables it names, the
/// functions it calls, and the LaTeX of its formulas (`{ FORMULA("…") }`, lab/src/rich_text.rs), in order.
#[derive(Default)]
struct Message {
    variables: Vec<String>,
    functions: Vec<String>,
    formulas: Vec<String>,
}

fn read_pattern(pattern: &ast::Pattern<&str>, out: &mut Message) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            read_expression(expression, out);
        }
    }
}

fn read_expression(expression: &ast::Expression<&str>, out: &mut Message) {
    match expression {
        ast::Expression::Select { selector, variants } => {
            read_inline(selector, out);
            variants.iter().for_each(|v| read_pattern(&v.value, out));
        }
        ast::Expression::Inline(e) => read_inline(e, out),
    }
}

fn read_inline(e: &ast::InlineExpression<&str>, out: &mut Message) {
    match e {
        ast::InlineExpression::VariableReference { id } => out.variables.push(id.name.to_string()),
        ast::InlineExpression::FunctionReference { id, arguments } => {
            // A call is recorded, a FORMULA's LaTeX kept, and its arguments read in turn.
            out.functions.push(id.name.to_string());
            if id.name == "FORMULA"
                && let Some(ast::InlineExpression::StringLiteral { value }) =
                    arguments.positional.first()
            {
                out.formulas.push(value.to_string());
            }
            arguments
                .positional
                .iter()
                .for_each(|a| read_inline(a, out));
            arguments
                .named
                .iter()
                .for_each(|a| read_inline(&a.value, out));
        }
        ast::InlineExpression::Placeable { expression } => read_expression(expression, out),
        _ => {}
    }
}

/// A Fluent table's messages by key, in order, each read; anything else in the file fails.
fn read_table(path: &Path) -> Vec<(String, Message)> {
    println!("cargo:rerun-if-changed={}", path.display());
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let resource = fluent_syntax::parser::parse(text.as_str()).unwrap_or_else(|(_, errors)| {
        let e = &errors[0];
        let line = text[..e.pos.start].lines().count().max(1);
        panic!("{}:{line}: not Fluent: {:?}", path.display(), e.kind)
    });
    let mut out: Vec<(String, Message)> = Vec::new();
    for entry in resource.body {
        match entry {
            ast::Entry::Message(m) => {
                // A message is one text without attributes, calls only known functions, and appears once.
                let key = m.id.name.to_string();
                assert!(
                    m.attributes.is_empty(),
                    "{}: {key}: a text is one message, without attributes",
                    path.display()
                );
                let mut message = Message::default();
                read_pattern(
                    m.value
                        .as_ref()
                        .unwrap_or_else(|| panic!("{}: {key} has no text", path.display())),
                    &mut message,
                );
                for f in &message.functions {
                    assert!(
                        FUNCTIONS.contains(&f.as_str()),
                        "{}: {key} calls {f}, not one of {FUNCTIONS:?}",
                        path.display()
                    );
                }
                message.variables.sort();
                message.variables.dedup();
                assert!(
                    !out.iter().any(|(k, _)| *k == key),
                    "{}: {key} twice",
                    path.display()
                );
                out.push((key, message));
            }
            ast::Entry::Comment(_)
            | ast::Entry::GroupComment(_)
            | ast::Entry::ResourceComment(_) => {}
            other => panic!(
                "{}: only messages and comments, not {other:?}",
                path.display()
            ),
        }
    }
    out
}

/// The keys `tr("…")` and `tr_with("…"` name in the `.rs` files under `dir`,
/// with the `text = "…"` and `help = "…"` of their `#[value(…)]` attributes (the lab's
/// values, lab/src/values.rs), the `text: Some("…")` and `help: Some("…")` of the entries written out
/// (lab/src/entity.rs) and the `text: "…"` of an `EntitySpec`, the `text=` and `help=` of the `{@ …}` tags in its
/// `.wgsl` files' `///` comments (lab/src/shader_data.rs), the `imageTitle`, `imageMarkText`, and
/// `labelText` attributes of the cues in its storyboards (`.lam`), the `titleText` of its Lamlet data files,
/// the `{textKey "…"}` its scripts name (lab/src/script/runtime.rs), the
/// `# title-text:` and `# dispatch-text:` of its pseudo-code listings (`.py`),
/// and the `-text` fields of its
/// `.toml` files (not under links, nor in what a build writes: `build/`,
/// `target/`).
fn text_keys_used(dir: &Path, keys: &mut Vec<(String, String)>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        // Links and build outputs are skipped; a directory is walked, and each kind of file has its own scanner.
        if path.is_symlink()
            || path
                .file_name()
                .is_some_and(|n| n == "build" || n == "target")
        {
            continue;
        }
        if path.is_dir() {
            text_keys_used(&path, keys);
            continue;
        }
        // Data files name keys in fields ending in `-text` (`title-text = "scene-terrain"`).
        if path.extension().is_some_and(|e| e == "toml") {
            toml_keys(&path, keys);
            continue;
        }
        // A storyboard's image title, mark label, and label text: render_lab's `{imageTitle "key"}`,
        // `{imageMarkText "key"}` (empty: none), and `{labelText "key"}`, or `{attr "imageTitle" "key"}`; and
        // a Lamlet data file's `titleText "key"` (a scene's `title-text`, lab/src/script/data.rs).
        if path.extension().is_some_and(|e| e == "lam") {
            lam_keys(&path, keys);
            continue;
        }
        // A pseudo-code listing's title and dispatch line (lab/src/pseudocode.rs): `# title-text: key`.
        if path.extension().is_some_and(|e| e == "py") {
            py_keys(&path, keys);
            continue;
        }
        // A shader value's name on screen: `/// {@ … text=key …}`.
        if path.extension().is_some_and(|e| e == "wgsl") {
            wgsl_keys(&path, keys);
            continue;
        }
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        rust_keys(&path, keys);
    }
}

/// The text keys a data file names, in fields ending in `-text` (`title-text = "scene-terrain"`).
fn toml_keys(path: &Path, keys: &mut Vec<(String, String)>) {
    println!("cargo:rerun-if-changed={}", path.display());
    for (n, line) in fs::read_to_string(path).unwrap().lines().enumerate() {
        // A `<name>-text = "key"` line names a key; comments and other fields are skipped.
        let Some((field, value)) = line.split_once('=') else {
            continue;
        };
        if !field.trim().ends_with("-text") || line.trim_start().starts_with('#') {
            continue;
        }
        let at = format!("{}:{}", path.display(), n + 1);
        let key = value
            .trim()
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or_else(|| panic!("{at}: a text key is a quoted string"));
        keys.push((key.to_string(), at));
    }
}

/// The text keys a Lamlet file names: a storyboard's image title, mark label, and label text, a script's
/// `{textKey "…"}`, and a data file's `titleText "…"`.
fn lam_keys(path: &Path, keys: &mut Vec<(String, String)>) {
    println!("cargo:rerun-if-changed={}", path.display());
    for (n, line) in fs::read_to_string(path).unwrap().lines().enumerate() {
        // A data file's `titleText "key"`, then each cue that takes a key, called directly or through `attr`.
        for (i, found) in line.match_indices("titleText \"") {
            let rest = &line[i + found.len()..];
            let key = &rest[..rest.find('"').unwrap_or(rest.len())];
            if !key.is_empty() {
                keys.push((key.to_string(), format!("{}:{}", path.display(), n + 1)));
            }
        }
        for name in ["imageTitle", "imageMarkText", "labelText", "textKey"] {
            let (call, attr) = (format!("{{{name} \""), format!("{{attr \"{name}\" \""));
            for (i, found) in line.match_indices(&call).chain(line.match_indices(&attr)) {
                let rest = &line[i + found.len()..];
                let key = &rest[..rest.find('"').unwrap_or(rest.len())];
                if !key.is_empty() {
                    keys.push((key.to_string(), format!("{}:{}", path.display(), n + 1)));
                }
            }
        }
    }
}

/// The text keys a pseudo-code listing names: its title and dispatch line (`# title-text: key`, lab/src/pseudocode.rs).
fn py_keys(path: &Path, keys: &mut Vec<(String, String)>) {
    println!("cargo:rerun-if-changed={}", path.display());
    for (n, line) in fs::read_to_string(path).unwrap().lines().enumerate() {
        // A `# title-text: key` or `# dispatch-text: key` line names a key.
        for field in ["# title-text: ", "# dispatch-text: "] {
            if let Some(key) = line.strip_prefix(field) {
                keys.push((
                    key.trim().to_string(),
                    format!("{}:{}", path.display(), n + 1),
                ));
            }
        }
    }
}

/// The text keys a shader names: a value's name on screen, `/// {@ … text=key …}` (and `help=`).
fn wgsl_keys(path: &Path, keys: &mut Vec<(String, String)>) {
    println!("cargo:rerun-if-changed={}", path.display());
    for (n, line) in fs::read_to_string(path).unwrap().lines().enumerate() {
        // Only `///` comments holding a `{@ … }` tag name keys, in their `text=` and `help=`.
        let Some(comment) = line
            .trim_start()
            .strip_prefix("///")
            .filter(|c| c.contains("{@"))
        else {
            continue;
        };
        for (i, _) in comment
            .match_indices("text=")
            .chain(comment.match_indices("help="))
        {
            if comment[..i]
                .chars()
                .next_back()
                .is_some_and(|c| !c.is_whitespace())
            {
                continue;
            }
            let rest = comment[i..]
                .split_once('=')
                .map_or("", |(_, r)| r)
                .trim_start_matches('"');
            let key = &rest[..rest
                .find(|c: char| c == '"' || c == '}' || c.is_whitespace())
                .unwrap_or(rest.len())];
            keys.push((key.to_string(), format!("{}:{}", path.display(), n + 1)));
        }
    }
}

/// The text keys a Rust file names: `tr("…")` and `tr_with("…", …)`, the `text` and `help` of a `#[value(…)]` or
/// `#[choice(…)]`, a value entry's `text: Some("…")` and `help: Some("…")`, and an `EntitySpec`'s `text: "…"`.
fn rust_keys(path: &Path, keys: &mut Vec<(String, String)>) {
    println!("cargo:rerun-if-changed={}", path.display());
    let source = fs::read_to_string(path).unwrap();
    // The whole file, not line by line: rustfmt may put a key on a line after its call or field. Comment
    // lines are blanked, their newlines kept, so an offset's line stays the source's.
    let code: String = source
        .split_inclusive('\n')
        .map(|line| {
            if line.trim_start().starts_with("//") {
                line.chars()
                    .map(|c| {
                        if c == '\n' {
                            c
                        } else {
                            ' '
                        }
                    })
                    .collect()
            } else {
                line.to_string()
            }
        })
        .collect();
    let at = |i: usize| format!("{}:{}", path.display(), code[..i].matches('\n').count() + 1);
    // A derived value's or a choice option's name on screen: `#[value(… text = "key" …)]`, `#[choice(text = "key")]`.
    for attribute in ["#[value(", "#[choice("] {
        for (i, _) in code.match_indices(attribute) {
            // The attribute, up to its closing bracket, is searched for `text = "…"` and `help = "…"`.
            let span = &code[i..closing(&code, i + 1)];
            for (j, _) in span
                .match_indices("text = \"")
                .chain(span.match_indices("help = \""))
            {
                let place = at(i + j);
                let key = string_literal(&span[j + 7..], &place)
                    .unwrap_or_else(|| panic!("{place}: a text key must be a string literal"));
                keys.push((key, place));
            }
        }
    }
    // A value entry's name and help written out (`text: Some("key")`, `help: Some("key")`).
    for field in ["text: Some(", "help: Some("] {
        for (i, _) in code.match_indices(field) {
            let place = at(i);
            if let Some(key) = string_literal(&code[i + field.len()..], &place) {
                keys.push((key, place));
            }
        }
    }
    // An entity's name (`EntitySpec { text: "key", … }`, src/entity.rs).
    for (i, _) in code.match_indices("EntitySpec {") {
        let open = i + "EntitySpec ".len();
        let span = &code[open..closing(&code, open)];
        for (j, _) in span.match_indices("text:") {
            let place = at(open + j);
            if let Some(key) = string_literal(&span[j + "text:".len()..], &place) {
                keys.push((key, place));
            }
        }
    }
    for call in ["tr(", "tr_with("] {
        for (i, _) in code.match_indices(call) {
            // A call of `tr` or `tr_with` (not a longer name ending in it, nor its definition) names a key.
            let before = code[..i].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_')
                || code[..i].trim_end().ends_with("fn")
            {
                continue;
            }
            let place = at(i);
            let key = string_literal(&code[i + call.len()..], &place)
                .unwrap_or_else(|| panic!("{place}: a text key must be a string literal"));
            keys.push((key, place));
        }
    }
}

/// The string literal `rest` starts with (after whitespace), or None when it starts with something else.
fn string_literal(rest: &str, at: &str) -> Option<String> {
    let rest = rest.trim_start().strip_prefix('"')?;
    let end = rest
        .find('"')
        .unwrap_or_else(|| panic!("{at}: unterminated text key"));
    Some(rest[..end].to_string())
}

/// The offset just past the bracket that closes the one at `open` in `code`, string literals skipped (or the
/// end of `code` when it is not closed).
fn closing(code: &str, open: usize) -> usize {
    let mut depth = 0;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in code[open..].char_indices() {
        if in_string {
            // Inside a string literal, only its closing quote matters (an escaped one does not).
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                // The closing bracket that brings the depth back to 0 closes the one at `open`.
                depth -= 1;
                if depth == 0 {
                    return open + i + 1;
                }
            }
            _ => {}
        }
    }
    code.len()
}

/// Every complete locale of a text table has every key of English's, and a
/// draft one only English's keys; each key names the same variables and the
/// same formulas in every locale (formulas stay English: the same LaTeX in
/// every language); every key the package's code and data name exists, and
/// every key is named somewhere.
fn check_text(locales: &[(String, bool)], tables: &[(PathBuf, Vec<PathBuf>)]) {
    for (dir, code) in tables {
        /// A table's messages by key.
        type Messages = Vec<(String, Message)>;
        let tables: Vec<(&str, bool, Messages)> = locales
            .iter()
            .filter_map(|(locale, complete)| {
                let path = dir.join(format!("{locale}.ftl"));
                assert!(
                    path.is_file() || !complete,
                    "{}: no table for {locale}, a complete locale",
                    dir.display()
                );
                path.is_file()
                    .then(|| (locale.as_str(), *complete, read_table(&path)))
            })
            .collect();
        let dir = dir.display();
        let (first, _, reference) = &tables[0];
        for (locale, complete, table) in &tables {
            // Every locale's keys must be English's, naming the same variables and formulas.
            for (key, message) in table {
                let Some((_, other)) = reference.iter().find(|(k, _)| k == key) else {
                    panic!("{dir}/{locale}.ftl has {key}, {first}.ftl does not")
                };
                assert!(
                    message.variables == other.variables,
                    "{dir}/{locale}.ftl: {key} names {:?}, {first}.ftl {:?}",
                    message.variables,
                    other.variables
                );
                assert!(
                    message.formulas == other.formulas,
                    "{dir}/{locale}.ftl: {key}'s formulas differ from {first}.ftl's"
                );
            }
            if *complete {
                // A complete locale must also have every English key.
                for (key, _) in reference {
                    assert!(
                        table.iter().any(|(k, _)| k == key),
                        "{dir}/{locale}.ftl has no {key}"
                    );
                }
            }
        }
        let mut used = Vec::new();
        for code in code {
            text_keys_used(code, &mut used);
        }
        for (key, at) in &used {
            assert!(
                reference.iter().any(|(k, _)| k == key),
                "{at}: no text {key:?} in {dir}"
            );
        }
        for (key, _) in reference {
            assert!(
                used.iter().any(|(k, _)| k == key),
                "{dir}: no code uses the text {key:?}"
            );
        }
    }
}

/// A mapping's lab name as a key: what is not a letter, a digit, `_`, or `-`
/// becomes `-`, once per run, none at the ends (`PointLight / PointLightSample`
/// is `PointLight-PointLightSample`). tools/video/config.py writes it the same way.
fn source_key(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        // A character outside letters, digits, `_`, and `-` becomes `-`, and a run of them collapses to one.
        let c = if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            c
        } else {
            '-'
        };
        if !(c == '-' && out.ends_with('-')) {
            out.push(c);
        }
    }
    out.trim_matches('-').to_string()
}

/// What a source index's other languages translate (lab/src/sources.rs): each
/// entry's `inspect` and details, each chapter's title, and each mapping's
/// engine name, exact layout, and lab layout, as their keys.
fn source_keys(index: &serde_json::Value) -> Vec<String> {
    let mut keys = Vec::new();
    for (key, entry) in index["entries"].as_object().into_iter().flatten() {
        keys.push(format!("entry-{key}-inspect"));
        for n in 1..=entry["details"].as_array().map_or(0, Vec::len) {
            keys.push(format!("entry-{key}-detail-{n}"));
        }
    }
    for chapter in index["chapters"].as_array().into_iter().flatten() {
        keys.push(format!("chapter-{}-title", chapter["number"]));
    }
    for mapping in index["mappings"].as_array().into_iter().flatten() {
        let lab = source_key(mapping["lab"].as_str().unwrap_or_default());
        keys.extend(["engine-name", "exact", "lab-layout"].map(|f| format!("mapping-{lab}-{f}")));
    }
    keys
}

/// Every source table (`text/sources/<locale>.ftl`) translates only what the
/// source index has, without variables, and a complete locale's all of it.
fn check_source_tables(locales: &[(String, bool)], module_dir: &Path) {
    let index_path = module_dir.join("source-index.json");
    // Watched only when it exists: cargo counts a watched file that is missing as changed, so a package without a
    // source index (render_lab) would rebuild on every build.
    let Ok(text) = fs::read_to_string(&index_path) else {
        return;
    };
    println!("cargo:rerun-if-changed={}", index_path.display());
    let index: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", index_path.display()));
    let expected = source_keys(&index);
    for (locale, complete) in locales.iter().skip(1) {
        // Each other locale's source table translates only the index's texts, and a complete one all of them.
        let path = module_dir.join(format!("text/sources/{locale}.ftl"));
        if !path.is_file() {
            // A missing table is allowed only for a draft locale.
            assert!(
                !complete,
                "{}: no source table for {locale}, a complete locale",
                path.display()
            );
            continue;
        }
        let table = read_table(&path);
        for (key, message) in &table {
            assert!(
                expected.contains(key),
                "{}: {key} translates nothing in source-index.json",
                path.display()
            );
            assert!(
                message.variables.is_empty(),
                "{}: {key} names variables; a source text has none",
                path.display()
            );
        }
        if *complete {
            for key in &expected {
                assert!(
                    table.iter().any(|(k, _)| k == key),
                    "{}: no {key}",
                    path.display()
                );
            }
        }
    }
}

/// Checks the package in `module_dir` (a directory of a packages directory):
/// its shaders, and, if it has text tables, them against the keys its code
/// (the `.rs` files under `code`) and its data (`-text` fields of the TOML
/// files in its directory) name.
pub fn check_package(module_dir: &Path, code: &Path) {
    let packages = find_packages(module_dir);
    let name = packages[0].name.clone();
    if module_dir.join("text/en.ftl").is_file() {
        let locales = locales(module_dir);
        check_text(
            &locales,
            &[(
                module_dir.join("text"),
                vec![code.to_path_buf(), module_dir.to_path_buf()],
            )],
        );
        check_source_tables(&locales, module_dir);
    }
    check_shaders(&packages, &name);
}
