//! Packages: the folders the lab's content comes in. Every package is a
//! directory with a `manifest.toml`. The loader reads the built packages, in
//! `packages/` beside the binary (src/resources.rs); a source checkout's
//! `packages/` holds their sources, which the build makes them from
//! (src/cook.rs). A manifest has:
//!
//! - `name`: the directory's name, lower-case words joined by `_`
//!   (`rlstd`);
//! - `author`, `license`, and, if given, `contact`;
//! - `import`: the packages it names resources of;
//! - `export`: its keys other packages may name;
//! - `library` and `negotiate`, if it has code: the name of its library (a
//!   cdylib, `lib<library>.so`: in a built package's `lib/`, else where the
//!   Cargo build puts it, beside the binary) and the symbol of its
//!   negotiation function (src/plugin.rs), which hands the lab its method
//!   handles (its investigations);
//! - `cooked-from`, in a built package: the source package it was built from;
//! - `credit-text`, if given: a key of its text tables, a short line of the
//!   copyright and license of what it holds, shown after the engine's version
//!   at the top right of its frames (`{year}` in it is the current year);
//! - `[[notice]]` tables, the legal notices of what it holds (an engine's
//!   copyright, license, and trademarks), shown wherever its content is (the
//!   homepage's selection page, the end of its videos, beside a video's file):
//!   each a `notice-text`, a key of its text tables, and, if it has terms of
//!   its own, its `license`, a file of the package that is built with it;
//! - `[[provides]]` tables, each of a `type`:
//!   - `"resource"`: a collection, the file at `path` (relative to the
//!     manifest), of `[[resource]]` and `[[shader]]` tables (the format is in
//!     src/pipeline);
//!   - `"demo"`: a narrated timeline the viewer plays: its `key`, `title`,
//!     `edition` (its name on the homepage's picker), and the resources of its
//!     `timeline` and `audio`.
//!
//! A package's keys are its own (`font/dejavu-sans`); everywhere else a
//! resource is named by its full key, `<package>.<key>`
//! (`rlstd.font/dejavu-sans`). In a package's content, a reference
//! without a `.` is to one of its own keys, and `<package>.<key>` is to a key
//! that an imported package exports. The lab's code names full keys.
//!
//! `scan` reads the manifests and orders the packages so that each comes
//! after the packages it imports; the resource registry (src/resources.rs)
//! reads their collections in that order, and `load` opens a package's
//! library when the lab first needs its investigations
//! (src/investigations.rs).

use crate::plugin::{self, Handles, NegotiateFn};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A legal notice of a package (`[[notice]]`): its text, a key of the
/// package's text tables, and the file of its terms, relative to the package.
#[derive(Clone)]
pub struct Notice {
    pub text: String,
    pub license: Option<String>,
}

/// A narrated timeline a package provides, with its references resolved to full keys.
pub struct Demo {
    pub key: String,
    pub title: String,
    pub edition: String,
    pub timeline: String,
    pub audio: String,
}

pub struct Package {
    pub name: String,
    pub author: String,
    pub contact: Option<String>,
    pub license: String,
    /// Its directory.
    pub dir: PathBuf,
    /// Its manifest, from the lab's root, for messages.
    pub manifest: String,
    pub imports: Vec<String>,
    /// Its own keys, without its name.
    pub exports: Vec<String>,
    /// A built package's source package.
    pub cooked_from: Option<String>,
    /// Its library and the symbol of its negotiation function.
    pub library: Option<(String, String)>,
    /// Its collections, in the order the manifest lists them.
    pub collections: Vec<PathBuf>,
    pub demos: Vec<Demo>,
    /// Its legal notices, in the order the manifest lists them.
    pub notices: Vec<Notice>,
    /// Its copyright and license line (`credit-text`), a text key.
    pub credit: Option<String>,
}

const FIELDS: &[&str] = &["name", "author", "contact", "license", "import", "export", "library", "negotiate", "cooked-from", "provides", "notice", "credit-text"];

/// A package's name: lower-case words joined by `_`.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.split('_').all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

/// A demonstration's key: letters, digits, `-` and `_` (`video-zh-CN`).
fn valid_demo_key(key: &str) -> bool {
    !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// One table's string field, or an error naming it.
fn string(table: &toml_edit::Table, field: &str, fail: &dyn Fn(String) -> String) -> Result<Option<String>, String> {
    match table.get(field) {
        None => Ok(None),
        Some(item) => item.as_str().filter(|s| !s.is_empty()).map(|s| Some(s.to_string())).ok_or_else(|| fail(format!("`{field}` is a string"))),
    }
}

fn strings(table: &toml_edit::Table, field: &str, fail: &dyn Fn(String) -> String) -> Result<Vec<String>, String> {
    let list = table.get(field).ok_or_else(|| fail(format!("`{field}` is missing (a list, which may be empty)")))?;
    let list = list.as_array().ok_or_else(|| fail(format!("`{field}` is a list of strings")))?;
    list.iter().map(|v| v.as_str().map(String::from).ok_or_else(|| fail(format!("`{field}` is a list of strings")))).collect()
}

/// Reads one manifest. Demo references stay as written until `scan` resolves them.
fn read(root: &Path, dir: &Path) -> Result<(Package, Vec<(String, String)>), String> {
    let path = dir.join("manifest.toml");
    let manifest = relative(root, &path);
    let fail = |message: String| format!("packages: {manifest}: {message}");
    let text = fs::read_to_string(&path).map_err(|e| fail(format!("read: {e}")))?;
    let document = toml_edit::Document::parse(text).map_err(|e| fail(format!("parse: {e}")))?;
    let table = document.as_table();
    if let Some((key, _)) = table.iter().find(|(key, _)| !FIELDS.contains(key)) {
        return Err(fail(format!("unknown key `{key}` (a manifest has {})", FIELDS.join(", "))));
    }
    let required = |field: &str| string(table, field, &fail)?.ok_or_else(|| fail(format!("`{field}` is missing")));
    let name = required("name")?;
    if !valid_name(&name) {
        return Err(fail(format!("`{name}`: a package's name is lower-case words joined by `_`")));
    }
    if dir.file_name().and_then(|n| n.to_str()) != Some(name.as_str()) {
        return Err(fail(format!("the package `{name}` is in a directory of another name")));
    }
    let library = match (string(table, "library", &fail)?, string(table, "negotiate", &fail)?) {
        (Some(library), Some(negotiate)) => Some((library, negotiate)),
        (None, None) => None,
        _ => return Err(fail("a package with code names both its `library` and its `negotiate` symbol".into())),
    };
    let imports = strings(table, "import", &fail)?;
    let exports = strings(table, "export", &fail)?;
    let mut collections = Vec::new();
    let mut demos = Vec::new();
    let mut references = Vec::new();
    let provides = match table.get("provides") {
        None => Vec::new(),
        Some(item) => item.as_array_of_tables().ok_or_else(|| fail("`provides` is a list of `[[provides]]` tables".into()))?.iter().collect(),
    };
    for entry in provides {
        let kind = string(entry, "type", &fail)?.ok_or_else(|| fail("a `[[provides]]` needs `type`, \"resource\" or \"demo\"".into()))?;
        let fields: &[&str] = match kind.as_str() {
            "resource" => &["type", "path"],
            "demo" => &["type", "key", "title", "edition", "timeline", "audio"],
            other => return Err(fail(format!("unknown type `{other}` (a package provides \"resource\" or \"demo\")"))),
        };
        if let Some((key, _)) = entry.iter().find(|(key, _)| !fields.contains(key)) {
            return Err(fail(format!("a {kind} has no field `{key}` (it has {})", fields.join(", "))));
        }
        let field = |name: &str| string(entry, name, &fail)?.ok_or_else(|| fail(format!("a {kind} needs `{name}`, a string")));
        if kind == "resource" {
            let file = field("path")?;
            let target = dir.join(&file);
            if Path::new(&file).is_absolute() || !target.is_file() {
                return Err(fail(format!("resource `{file}`: no such file (a path relative to the manifest)")));
            }
            collections.push(target);
        } else {
            let key = field("key")?;
            if !valid_demo_key(&key) || demos.iter().any(|d: &Demo| d.key == key) {
                return Err(fail(format!("demo `{key}`: a key is letters, digits, `-` and `_`, unique in its package")));
            }
            references.push((field("timeline")?, field("audio")?));
            demos.push(Demo { key, title: field("title")?, edition: field("edition")?, timeline: String::new(), audio: String::new() });
        }
    }
    let mut notices = Vec::new();
    let listed = match table.get("notice") {
        None => Vec::new(),
        Some(item) => item.as_array_of_tables().ok_or_else(|| fail("`notice` is a list of `[[notice]]` tables".into()))?.iter().collect(),
    };
    for entry in listed {
        if let Some((key, _)) = entry.iter().find(|(key, _)| !["notice-text", "license"].contains(key)) {
            return Err(fail(format!("a notice has no field `{key}` (it has notice-text, license)")));
        }
        let text = string(entry, "notice-text", &fail)?.ok_or_else(|| fail("a `[[notice]]` needs `notice-text`, a text key".into()))?;
        let license = string(entry, "license", &fail)?;
        if let Some(file) = &license {
            if Path::new(file).is_absolute() || file.contains("..") || !dir.join(file).is_file() {
                return Err(fail(format!("notice license `{file}`: no such file (a path inside the package)")));
            }
        }
        notices.push(Notice { text, license });
    }
    let credit = string(table, "credit-text", &fail)?;
    let package = Package { author: required("author")?, contact: string(table, "contact", &fail)?, license: required("license")?, cooked_from: string(table, "cooked-from", &fail)?, name, dir: dir.to_path_buf(), manifest, imports, exports, library, collections, demos, notices, credit };
    Ok((package, references))
}

/// Every package in `dir`, each after the packages it imports (and otherwise by name).
pub fn scan(root: &Path, dir: &Path) -> Result<Vec<Package>, String> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir).map_err(|e| format!("packages: read {}: {e}", dir.display()))?.flatten().map(|e| e.path()).filter(|p| p.join("manifest.toml").is_file()).collect();
    entries.sort();
    let mut read_packages = BTreeMap::new();
    for entry in entries {
        let (package, references) = read(root, &entry)?;
        read_packages.insert(package.name.clone(), (package, references));
    }
    for (package, _) in read_packages.values() {
        for import in &package.imports {
            if import == &package.name || !read_packages.contains_key(import) {
                return Err(format!("packages: {}: imports `{import}`, which is not a package of {}", package.manifest, relative(root, dir)));
            }
        }
    }
    // Imports first: take, by name, a package whose imports are all taken.
    let mut ordered: Vec<Package> = Vec::new();
    let mut pending: Vec<(Package, Vec<(String, String)>)> = read_packages.into_values().collect();
    while !pending.is_empty() {
        let Some(i) = pending.iter().position(|(m, _)| m.imports.iter().all(|i| ordered.iter().any(|o| &o.name == i))) else {
            let names: Vec<&str> = pending.iter().map(|(m, _)| m.name.as_str()).collect();
            return Err(format!("packages: the imports of {} form a cycle", names.join(", ")));
        };
        let (mut package, references) = pending.remove(i);
        for (n, (timeline, audio)) in references.iter().enumerate() {
            let fail = |e: String| format!("packages: {}: demo `{}`: {e}", package.manifest, package.demos[n].key);
            let (timeline, audio) = (resolve(&package, &ordered, timeline).map_err(fail)?, resolve(&package, &ordered, audio).map_err(fail)?);
            (package.demos[n].timeline, package.demos[n].audio) = (timeline, audio);
        }
        ordered.push(package);
    }
    Ok(ordered)
}

/// The directory of the running binary (for a test, of the binary it tests),
/// where the Cargo build puts the packages' libraries.
pub fn bin_dir() -> PathBuf {
    let exe = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)).unwrap_or_default();
    if exe.file_name().is_some_and(|n| n == "deps") { exe.parent().unwrap().to_path_buf() } else { exe }
}

/// Where `package`'s library `name` is: in its `lib/` (a built package),
/// else beside the binary (where the Cargo build puts it).
pub fn library_path(package: &Package, name: &str) -> PathBuf {
    let file = libloading::library_filename(name);
    let own = package.dir.join("lib").join(&file);
    if own.is_file() { own } else { bin_dir().join(file) }
}

/// Opens `package`'s library and negotiates with it: the handles it
/// registers, or why it cannot be used. The library stays loaded for the
/// rest of the run (its handles are `'static`).
pub fn load(package: &Package) -> Result<Handles, String> {
    let Some((name, symbol)) = &package.library else { return Ok(Handles::default()) };
    let path = library_path(package, name);
    let fail = |message: String| format!("packages: {}: library {}: {message}", package.manifest, path.display());
    if !path.is_file() {
        return Err(fail("not built (`cargo build --release` builds the repository's packages, and the next launch puts them in their built packages; a local package's Cargo.toml says how to build it)".into()));
    }
    // SAFETY: a package's library is Render Lab code that links this lab; its
    // initialisers do nothing else.
    let library = unsafe { libloading::Library::new(&path) }.map_err(|e| fail(format!("{e} (rebuild it against this lab)")))?;
    // SAFETY: the manifest names the function's symbol; its type is the negotiation's.
    let negotiate: NegotiateFn = *unsafe { library.get::<NegotiateFn>(symbol.as_bytes()) }.map_err(|e| fail(format!("no negotiation function `{symbol}`: {e}")))?;
    // SAFETY: as above.
    let handles = unsafe { plugin::negotiate(negotiate) }.map_err(|e| fail(format!("negotiation `{symbol}`: {e}")))?;
    std::mem::forget(library);
    Ok(handles)
}

/// The full key a reference in `package`'s content names: its own key, or a
/// key an imported package (one of `imported`) exports.
pub fn resolve(package: &Package, imported: &[Package], reference: &str) -> Result<String, String> {
    let Some((name, key)) = reference.split_once('.') else { return Ok(format!("{}.{reference}", package.name)) };
    if name == package.name {
        return Ok(reference.to_string());
    }
    if !package.imports.iter().any(|i| i == name) {
        return Err(format!("`{reference}` names the package `{name}`, which `import` does not list"));
    }
    let other = imported.iter().find(|m| m.name == name).ok_or_else(|| format!("`{reference}`: no package `{name}`"))?;
    if !other.exports.iter().any(|e| e == key) {
        return Err(format!("`{reference}`: the package `{name}` does not export `{key}` ({})", other.manifest));
    }
    Ok(reference.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A packages directory of `(name, manifest)` under the system's temporary directory.
    fn directory(test: &str, packages: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("render-lab-packages-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for (name, manifest) in packages {
            fs::create_dir_all(dir.join(name)).unwrap();
            fs::write(dir.join(name).join("manifest.toml"), manifest).unwrap();
        }
        dir
    }

    fn manifest(name: &str, import: &str, export: &str) -> String {
        format!("name = \"{name}\"\nauthor = \"a\"\nlicense = \"MIT\"\nimport = [{import}]\nexport = [{export}]\n")
    }

    #[test]
    fn imports_come_first() {
        let dir = directory("order", &[("a", &manifest("a", "\"b\"", "")), ("b", &manifest("b", "", "\"x/y\""))]);
        let packages = scan(&dir, &dir).unwrap();
        assert_eq!(packages.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        assert_eq!(resolve(&packages[1], &packages, "b.x/y").unwrap(), "b.x/y");
        assert_eq!(resolve(&packages[1], &packages, "own/key").unwrap(), "a.own/key");
        assert!(resolve(&packages[1], &packages, "b.hidden").unwrap_err().contains("does not export"));
        assert!(resolve(&packages[0], &packages, "a.key").unwrap_err().contains("does not list"));
    }

    #[test]
    fn missing_import_and_cycle() {
        let dir = directory("missing", &[("a", &manifest("a", "\"c\"", ""))]);
        assert!(scan(&dir, &dir).err().unwrap().contains("imports `c`"));
        let dir = directory("cycle", &[("a", &manifest("a", "\"b\"", "")), ("b", &manifest("b", "\"a\"", ""))]);
        assert!(scan(&dir, &dir).err().unwrap().contains("cycle"));
    }

    #[test]
    fn name_matches_directory() {
        let dir = directory("name", &[("a", &manifest("b", "", ""))]);
        assert!(scan(&dir, &dir).err().unwrap().contains("directory of another name"));
    }
}
