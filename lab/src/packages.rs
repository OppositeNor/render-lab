//! Packages: the folders the lab's content comes in. Every package is a
//! directory with a `manifest.toml`. The loader reads the built packages, in
//! `packages/` beside the binary (src/resources.rs); a source checkout's
//! `packages/` holds their sources, which the build makes them from
//! (src/cook.rs). A manifest has:
//!
//! - `name`: the directory's name, lower-case words joined by `_`
//!   (`render_lab`);
//! - `author`, `license`, and, if given, `contact`;
//! - `import`: the packages it names resources of;
//! - `export`: its keys other packages may name;
//! - `library` and `negotiate`, if it has code: the name of its library (a
//!   cdylib, `lib<library>.so`: in a built package's `lib/`, else where the
//!   Cargo build puts it, beside the binary) and the symbol of its
//!   negotiation function (src/plugin.rs), which hands the lab its method
//!   handles (its investigations);
//! - `cooked-from`, in a built package: the source package it was built from;
//! - `requires`, if given: what the lab must offer for its investigations to run (`"ray-tracing"`: hardware
//!   ray queries, src/gpu.rs); where it does not, they are listed as unavailable, with the reason
//!   (src/investigations.rs `unavailable`);
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
//! (`render_lab.font/dejavu-sans`). In a package's content, a reference
//! without a `.` is to one of its own keys, and `<package>.<key>` is to a key
//! that an imported package exports. The lab's code names full keys.
//!
//! `scan` reads the manifests and orders the packages so that each comes
//! after the packages it imports; the resource registry (src/resources.rs)
//! reads their collections in that order, and `load_all` opens the
//! packages' libraries in that order when the lab first needs its
//! investigations (src/investigations.rs): each library gets the handles of
//! its imports' libraries (src/plugin/abi.rs), and one whose import was
//! refused is refused too.

use crate::plugin::abi::{RlPackage, RlStr};
use crate::plugin::{self, Handles, NegotiateFn};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::c_void;
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
    pub timeline: String,
    pub audio: String,
}

/// A series a package provides (`type = "series"`): investigations told one
/// after another as the parts of one story, each part an imported package
/// (its investigation and its narrated timelines), in order.
pub struct Series {
    pub key: String,
    /// Its title and summary by language (`en`, `zh-CN`, ...).
    pub title: BTreeMap<String, String>,
    pub summary: BTreeMap<String, String>,
    /// The packages of its parts, in the order they are told.
    pub parts: Vec<String>,
}

impl Series {
    /// Its title in `language`, else in English.
    pub fn title_in(&self, language: &str) -> &str {
        self.title
            .get(language)
            .or_else(|| self.title.get("en"))
            .map_or("", String::as_str)
    }

    /// Its summary in `language`, else in English.
    pub fn summary_in(&self, language: &str) -> &str {
        self.summary
            .get(language)
            .or_else(|| self.summary.get("en"))
            .map_or("", String::as_str)
    }
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
    pub series: Vec<Series>,
    /// Its legal notices, in the order the manifest lists them.
    pub notices: Vec<Notice>,
    /// Its copyright and license line (`credit-text`), a text key.
    pub credit: Option<String>,
    /// What its investigations need of the lab (`requires`, of `REQUIREMENTS`).
    pub requires: Vec<String>,
}

/// What a package may require: hardware ray tracing (src/gpu.rs `ray_tracing`).
pub const REQUIREMENTS: &[&str] = &["ray-tracing"];

impl Package {
    /// Why its investigations cannot run here, or None when the lab offers all it requires.
    pub fn unmet(&self) -> Option<&'static str> {
        self.requires
            .iter()
            .any(|r| r == "ray-tracing")
            .then_some(())
            .filter(|_| !crate::gpu::ray_tracing_supported())
            .map(|_| "needs hardware ray tracing (Vulkan ray queries), which this GPU or driver does not offer")
    }
}

const FIELDS: &[&str] = &[
    "name",
    "author",
    "contact",
    "license",
    "import",
    "export",
    "library",
    "negotiate",
    "cooked-from",
    "provides",
    "notice",
    "credit-text",
    "requires",
];

/// A package's name: lower-case words joined by `_`.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('_').all(|w| {
            !w.is_empty()
                && w.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// A demonstration's key: letters, digits, `-` and `_` (`video-zh-CN`).
fn valid_demo_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A field of strings by language (`title = { en = "…", zh-CN = "…" }`), English required.
fn by_language(
    table: &toml_edit::Table,
    field: &str,
    fail: &dyn Fn(String) -> String,
) -> Result<BTreeMap<String, String>, String> {
    let wrong = || {
        fail(format!(
            "`{field}` is a table of strings by language, with `en` (`{field} = {{ en = \"…\", zh-CN = \"…\" }}`)"
        ))
    };
    let item = table.get(field).ok_or_else(wrong)?;
    let entries = item
        .as_inline_table()
        .map(|t| {
            t.iter()
                .map(|(k, v)| (k.to_string(), v.as_str().map(String::from)))
                .collect::<Vec<_>>()
        })
        .or_else(|| {
            item.as_table().map(|t| {
                t.iter()
                    .map(|(k, v)| (k.to_string(), v.as_str().map(String::from)))
                    .collect()
            })
        })
        .ok_or_else(wrong)?;
    let mut out = BTreeMap::new();
    for (language, text) in entries {
        out.insert(language, text.filter(|t| !t.is_empty()).ok_or_else(wrong)?);
    }
    if !out.contains_key("en") {
        return Err(wrong());
    }
    Ok(out)
}

/// One table's string field, or an error naming it.
fn string(
    table: &toml_edit::Table,
    field: &str,
    fail: &dyn Fn(String) -> String,
) -> Result<Option<String>, String> {
    match table.get(field) {
        None => Ok(None),
        Some(item) => item
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| fail(format!("`{field}` is a string"))),
    }
}

fn strings(
    table: &toml_edit::Table,
    field: &str,
    fail: &dyn Fn(String) -> String,
) -> Result<Vec<String>, String> {
    let list = table
        .get(field)
        .ok_or_else(|| fail(format!("`{field}` is missing (a list, which may be empty)")))?;
    let list = list
        .as_array()
        .ok_or_else(|| fail(format!("`{field}` is a list of strings")))?;
    list.iter()
        .map(|v| {
            v.as_str()
                .map(String::from)
                .ok_or_else(|| fail(format!("`{field}` is a list of strings")))
        })
        .collect()
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
        return Err(fail(format!(
            "unknown key `{key}` (a manifest has {})",
            FIELDS.join(", ")
        )));
    }
    let required = |field: &str| {
        string(table, field, &fail)?.ok_or_else(|| fail(format!("`{field}` is missing")))
    };
    let name = required("name")?;
    if !valid_name(&name) {
        return Err(fail(format!(
            "`{name}`: a package's name is lower-case words joined by `_`"
        )));
    }
    if dir.file_name().and_then(|n| n.to_str()) != Some(name.as_str()) {
        return Err(fail(format!(
            "the package `{name}` is in a directory of another name"
        )));
    }
    let library = match (
        string(table, "library", &fail)?,
        string(table, "negotiate", &fail)?,
    ) {
        (Some(library), Some(negotiate)) => Some((library, negotiate)),
        (None, None) => None,
        _ => {
            return Err(fail(
                "a package with code names both its `library` and its `negotiate` symbol".into(),
            ));
        }
    };
    let imports = strings(table, "import", &fail)?;
    let exports = strings(table, "export", &fail)?;
    let Provided {
        collections,
        demos,
        series,
        references,
    } = read_provides(table, dir, &imports, &fail)?;
    let notices = read_notices(table, dir, &fail)?;
    let credit = string(table, "credit-text", &fail)?;
    // What it requires: optional, each one the lab knows.
    let requires = if table.contains_key("requires") {
        strings(table, "requires", &fail)?
    } else {
        Vec::new()
    };
    if let Some(r) = requires
        .iter()
        .find(|r| !REQUIREMENTS.contains(&r.as_str()))
    {
        return Err(fail(format!(
            "`requires` names `{r}`; the lab knows {}",
            REQUIREMENTS.join(", ")
        )));
    }
    let package = Package {
        author: required("author")?,
        contact: string(table, "contact", &fail)?,
        license: required("license")?,
        cooked_from: string(table, "cooked-from", &fail)?,
        name,
        dir: dir.to_path_buf(),
        manifest,
        imports,
        exports,
        library,
        collections,
        demos,
        series,
        notices,
        credit,
        requires,
    };
    Ok((package, references))
}

/// What a manifest provides: its resource collections, demos, and series, and each demo's timeline and audio as
/// written.
struct Provided {
    collections: Vec<PathBuf>,
    demos: Vec<Demo>,
    series: Vec<Series>,
    references: Vec<(String, String)>,
}

/// A manifest's `[[provides]]`: its resource collections, demos, and series, and each demo's timeline and audio
/// as written (resolved once every package is read).
fn read_provides(
    table: &toml_edit::Table,
    dir: &Path,
    imports: &[String],
    fail: &dyn Fn(String) -> String,
) -> Result<Provided, String> {
    let mut collections = Vec::new();
    let mut demos = Vec::new();
    let mut series = Vec::new();
    let mut references = Vec::new();
    let provides = match table.get("provides") {
        None => Vec::new(),
        Some(item) => item
            .as_array_of_tables()
            .ok_or_else(|| fail("`provides` is a list of `[[provides]]` tables".into()))?
            .iter()
            .collect(),
    };
    for entry in provides {
        let kind = string(entry, "type", fail)?.ok_or_else(|| {
            fail("a `[[provides]]` needs `type`, \"resource\", \"demo\", or \"series\"".into())
        })?;
        let fields: &[&str] = match kind.as_str() {
            "resource" => &["type", "path"],
            "demo" => &["type", "key", "title", "edition", "timeline", "audio"],
            "series" => &["type", "key", "title", "summary", "parts"],
            other => {
                return Err(fail(format!(
                    "unknown type `{other}` (a package provides \"resource\", \"demo\", or \"series\")"
                )));
            }
        };
        if let Some((key, _)) = entry.iter().find(|(key, _)| !fields.contains(key)) {
            return Err(fail(format!(
                "a {kind} has no field `{key}` (it has {})",
                fields.join(", ")
            )));
        }
        let field = |name: &str| {
            string(entry, name, fail)?
                .ok_or_else(|| fail(format!("a {kind} needs `{name}`, a string")))
        };
        if kind == "resource" {
            let file = field("path")?;
            let target = dir.join(&file);
            if Path::new(&file).is_absolute() || !target.is_file() {
                return Err(fail(format!(
                    "resource `{file}`: no such file (a path relative to the manifest)"
                )));
            }
            collections.push(target);
        } else if kind == "series" {
            series.push(read_series(entry, (imports, &series), fail)?);
        } else {
            let key = field("key")?;
            if !valid_demo_key(&key) || demos.iter().any(|d: &Demo| d.key == key) {
                return Err(fail(format!(
                    "demo `{key}`: a key is letters, digits, `-` and `_`, unique in its package"
                )));
            }
            references.push((field("timeline")?, field("audio")?));
            demos.push(Demo {
                key,
                title: field("title")?,
                timeline: String::new(),
                audio: String::new(),
            });
        }
    }
    Ok(Provided {
        collections,
        demos,
        series,
        references,
    })
}

/// A `[[provides]]` series: its key (unique among `series`), its title and summary by language, and its parts,
/// each a package `imports` lists, once.
fn read_series(
    entry: &toml_edit::Table,
    (imports, series): (&[String], &[Series]),
    fail: &dyn Fn(String) -> String,
) -> Result<Series, String> {
    let field = |name: &str| {
        string(entry, name, fail)?.ok_or_else(|| fail(format!("a series needs `{name}`, a string")))
    };
    let key = field("key")?;
    if !valid_demo_key(&key) || series.iter().any(|s| s.key == key) {
        return Err(fail(format!(
            "series `{key}`: a key is letters, digits, `-` and `_`, unique in its package"
        )));
    }
    let parts = strings(entry, "parts", fail)?;
    if parts.is_empty() {
        return Err(fail(format!(
            "series `{key}`: `parts` lists its parts' packages, in order"
        )));
    }
    for (i, part) in parts.iter().enumerate() {
        if !imports.contains(part) || parts[..i].contains(part) {
            return Err(fail(format!(
                "series `{key}`: part `{part}` is a package `import` lists, once"
            )));
        }
    }
    Ok(Series {
        key,
        title: by_language(entry, "title", fail)?,
        summary: by_language(entry, "summary", fail)?,
        parts,
    })
}

/// A manifest's `[[notice]]`s: each its text key and its license's file (inside the package).
fn read_notices(
    table: &toml_edit::Table,
    dir: &Path,
    fail: &dyn Fn(String) -> String,
) -> Result<Vec<Notice>, String> {
    let mut notices = Vec::new();
    let listed = match table.get("notice") {
        None => Vec::new(),
        Some(item) => item
            .as_array_of_tables()
            .ok_or_else(|| fail("`notice` is a list of `[[notice]]` tables".into()))?
            .iter()
            .collect(),
    };
    for entry in listed {
        if let Some((key, _)) = entry
            .iter()
            .find(|(key, _)| !["notice-text", "license"].contains(key))
        {
            return Err(fail(format!(
                "a notice has no field `{key}` (it has notice-text, license)"
            )));
        }
        let text = string(entry, "notice-text", fail)?
            .ok_or_else(|| fail("a `[[notice]]` needs `notice-text`, a text key".into()))?;
        let license = string(entry, "license", fail)?;
        if let Some(file) = &license
            && (Path::new(file).is_absolute() || file.contains("..") || !dir.join(file).is_file())
        {
            return Err(fail(format!(
                "notice license `{file}`: no such file (a path inside the package)"
            )));
        }
        notices.push(Notice { text, license });
    }
    Ok(notices)
}

/// Every package in `dir`, each after the packages it imports (and otherwise by name).
pub fn scan(root: &Path, dir: &Path) -> Result<Vec<Package>, String> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("packages: read {}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("manifest.toml").is_file())
        .collect();
    entries.sort();
    let mut read_packages = BTreeMap::new();
    for entry in entries {
        let (package, references) = read(root, &entry)?;
        read_packages.insert(package.name.clone(), (package, references));
    }
    for (package, _) in read_packages.values() {
        for import in &package.imports {
            if import == &package.name || !read_packages.contains_key(import) {
                return Err(format!(
                    "packages: {}: imports `{import}`, which is not a package of {}",
                    package.manifest,
                    relative(root, dir)
                ));
            }
        }
    }
    // Imports first: take, by name, a package whose imports are all taken.
    let mut ordered: Vec<Package> = Vec::new();
    let mut pending: Vec<(Package, Vec<(String, String)>)> = read_packages.into_values().collect();
    while !pending.is_empty() {
        let Some(i) = pending.iter().position(|(m, _)| {
            m.imports
                .iter()
                .all(|i| ordered.iter().any(|o| &o.name == i))
        }) else {
            return Err(format!(
                "packages: their imports form a cycle: {}",
                cycle(pending.iter().map(|(m, _)| m)).join(" -> ")
            ));
        };
        let (mut package, references) = pending.remove(i);
        // A demo's timeline and audio resolve against the packages taken before it.
        for (n, (timeline, audio)) in references.iter().enumerate() {
            let fail = |e: String| {
                format!(
                    "packages: {}: demo `{}`: {e}",
                    package.manifest, package.demos[n].key
                )
            };
            let (timeline, audio) = (
                resolve(&package, &ordered, timeline).map_err(fail)?,
                resolve(&package, &ordered, audio).map_err(fail)?,
            );
            (package.demos[n].timeline, package.demos[n].audio) = (timeline, audio);
        }
        ordered.push(package);
    }
    Ok(ordered)
}

/// A cycle among `pending` packages, none of which has all its imports
/// taken: its names in order, the first again at the end.
fn cycle<'a>(pending: impl Iterator<Item = &'a Package> + Clone) -> Vec<&'a str> {
    let find = |name: &str| pending.clone().find(|p| p.name == name);
    let mut path: Vec<&Package> = pending.clone().take(1).collect();
    loop {
        // Each pending package imports a pending one; follow it until a package comes round again.
        let last = path[path.len() - 1];
        let Some(next) = last.imports.iter().find_map(|i| find(i)) else {
            return path.iter().map(|p| p.name.as_str()).collect();
        };
        if let Some(start) = path.iter().position(|p| p.name == next.name) {
            let mut names: Vec<&str> = path[start..].iter().map(|p| p.name.as_str()).collect();
            names.push(&next.name);
            return names;
        }
        path.push(next);
    }
}

/// The directory of the running binary (for a test, of the binary it tests),
/// where the Cargo build puts the packages' libraries.
pub fn bin_dir() -> PathBuf {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    binaries_dir(&exe)
}

/// The binaries' directory (`target/<profile>/`) for a binary in `exe`: a
/// test runs from `deps/` below it before Cargo 1.100, and from
/// `build/<package>/<hash>/out/` since; any other binary from it.
fn binaries_dir(exe: &Path) -> PathBuf {
    let named =
        |dir: Option<&Path>, name: &str| dir.and_then(Path::file_name).is_some_and(|n| n == name);
    let mut up = exe.ancestors();
    if named(Some(exe), "deps") {
        return exe.parent().unwrap_or(exe).to_path_buf();
    }
    if named(Some(exe), "out") && named(up.nth(3), "build") {
        return up.next().unwrap_or(exe).to_path_buf();
    }
    exe.to_path_buf()
}

/// Where `package`'s library `name` is: in its `lib/` (a built package),
/// else beside the binary (where the Cargo build puts it).
pub fn library_path(package: &Package, name: &str) -> PathBuf {
    let file = libloading::library_filename(name);
    let own = package.dir.join("lib").join(&file);
    if own.is_file() {
        own
    } else {
        bin_dir().join(file)
    }
}

/// Opens the libraries of `packages` (in their order, imports first) with
/// `open`, each given the opened libraries of its imports (those that have
/// one). A package whose imported library was refused is refused too, with
/// the reason down the chain.
pub fn load_all<'a, T>(
    packages: &'a [Package],
    mut open: impl FnMut(&'a Package, &[(&'a str, &T)]) -> Result<T, String>,
) -> Vec<(&'a Package, Result<T, String>)> {
    let mut loaded: Vec<(&'a Package, Result<T, String>)> = Vec::new();
    for package in packages.iter().filter(|p| p.library.is_some()) {
        let result = {
            let mut imports: Vec<(&'a str, &T)> = Vec::new();
            let mut refused = None;
            for name in &package.imports {
                // An import without a library has no handle to give.
                let Some((import, result)) = loaded.iter().find(|(p, _)| &p.name == name) else {
                    continue;
                };
                match result {
                    Ok(opened) => imports.push((import.name.as_str(), opened)),
                    Err(e) => {
                        refused = Some(format!(
                            "packages: {}: imports `{name}`, whose library was not loaded: {e}",
                            package.manifest
                        ));
                        break;
                    }
                }
            }
            match refused {
                Some(e) => Err(e),
                None => open(package, &imports),
            }
        };
        loaded.push((package, result));
    }
    loaded
}

/// What an importer's handle of another package's library points to: that
/// library, and the symbols the importer found in it.
struct Lookup<'a> {
    package: &'a str,
    library: &'static libloading::Library,
    found: RefCell<Vec<String>>,
}

/// `RlPackage::symbol`: `name`'s address in a `Lookup`'s library, or null.
unsafe extern "C" fn lookup(library: *const c_void, name: RlStr) -> *const c_void {
    // SAFETY: `load` passes its `Lookup`s, alive for the negotiation, and the library's UTF-8 name.
    let (lookup, name) = unsafe { (&*(library as *const Lookup), name.as_str()) };
    let Some(name) = name else {
        return std::ptr::null();
    };
    // SAFETY: an address only; the importer checks its signature before it uses it.
    match unsafe { lookup.library.get::<*const c_void>(name.as_bytes()) } {
        Ok(symbol) => {
            lookup.found.borrow_mut().push(name.to_string());
            *symbol
        }
        Err(_) => std::ptr::null(),
    }
}

/// Opens `package`'s library and negotiates with it, handing it the
/// libraries of its `imports`: the handles it registers, or why it cannot be
/// used. The library stays loaded for the rest of the run (its handles are
/// `'static`).
pub fn load(package: &Package, imports: &[(&str, &Handles)]) -> Result<Handles, String> {
    let Some((name, symbol)) = &package.library else {
        return Ok(Handles::default());
    };
    let path = library_path(package, name);
    let fail = |message: String| {
        format!(
            "packages: {}: library {}: {message}",
            package.manifest,
            path.display()
        )
    };
    if !path.is_file() {
        return Err(fail("not built (`cargo build --release` builds the repository's packages, and the next launch puts them in their built packages; a local package's Cargo.toml says how to build it)".into()));
    }
    // SAFETY: a package's library is Render Lab code that links this lab; its
    // initialisers do nothing else.
    let library = unsafe { libloading::Library::new(&path) }
        .map_err(|e| fail(format!("{e} (rebuild it against this lab)")))?;
    // SAFETY: the manifest names the function's symbol; its type is the negotiation's.
    let negotiate: NegotiateFn = *unsafe { library.get::<NegotiateFn>(symbol.as_bytes()) }
        .map_err(|e| fail(format!("no negotiation function `{symbol}`: {e}")))?;
    let lookups: Vec<Lookup> = imports
        .iter()
        .filter_map(|(package, handles)| {
            Some(Lookup {
                package,
                library: handles.library?,
                found: RefCell::default(),
            })
        })
        .collect();
    let handed: Vec<RlPackage> = lookups
        .iter()
        .map(|l| RlPackage {
            name: RlStr::borrowed(l.package),
            library: l as *const Lookup as *const c_void,
            symbol: lookup,
        })
        .collect();
    // SAFETY: as above; the handles point to `lookups`, alive for the call.
    let mut handles = unsafe { plugin::negotiate(negotiate, &handed) }
        .map_err(|e| fail(format!("negotiation `{symbol}`: {e}")))?;
    handles.library = Some(Box::leak(Box::new(library)));
    handles.resolved = lookups
        .into_iter()
        .flat_map(|l| {
            let package = l.package.to_string();
            l.found
                .take()
                .into_iter()
                .map(move |s| (package.clone(), s))
        })
        .collect();
    Ok(handles)
}

/// The full key a reference in `package`'s content names: its own key, or a
/// key an imported package (one of `imported`) exports.
pub fn resolve(package: &Package, imported: &[Package], reference: &str) -> Result<String, String> {
    let Some((name, key)) = reference.split_once('.') else {
        return Ok(format!("{}.{reference}", package.name));
    };
    if name == package.name {
        return Ok(reference.to_string());
    }
    if !package.imports.iter().any(|i| i == name) {
        return Err(format!(
            "`{reference}` names the package `{name}`, which `import` does not list"
        ));
    }
    let other = imported
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| format!("`{reference}`: no package `{name}`"))?;
    if !other.exports.iter().any(|e| e == key) {
        return Err(format!(
            "`{reference}`: the package `{name}` does not export `{key}` ({})",
            other.manifest
        ));
    }
    Ok(reference.to_string())
}

#[cfg(all(test, unix))]
mod library_tests;

#[cfg(test)]
mod tests {
    use super::*;

    /// A packages directory of `(name, manifest)` under the system's temporary directory.
    fn directory(test: &str, packages: &[(&str, &str)]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("render-lab-packages-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for (name, manifest) in packages {
            fs::create_dir_all(dir.join(name)).unwrap();
            fs::write(dir.join(name).join("manifest.toml"), manifest).unwrap();
        }
        dir
    }

    fn manifest(name: &str, import: &str, export: &str) -> String {
        format!(
            "name = \"{name}\"\nauthor = \"a\"\nlicense = \"MIT\"\nimport = [{import}]\nexport = [{export}]\n"
        )
    }

    #[test]
    fn imports_come_first() {
        let dir = directory(
            "order",
            &[
                ("a", &manifest("a", "\"b\"", "")),
                ("b", &manifest("b", "", "\"x/y\"")),
            ],
        );
        let packages = scan(&dir, &dir).unwrap();
        assert_eq!(
            packages.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            ["b", "a"]
        );
        assert_eq!(resolve(&packages[1], &packages, "b.x/y").unwrap(), "b.x/y");
        assert_eq!(
            resolve(&packages[1], &packages, "own/key").unwrap(),
            "a.own/key"
        );
        assert!(
            resolve(&packages[1], &packages, "b.hidden")
                .unwrap_err()
                .contains("does not export")
        );
        assert!(
            resolve(&packages[0], &packages, "a.key")
                .unwrap_err()
                .contains("does not list")
        );
    }

    #[test]
    fn missing_import_and_cycle() {
        let dir = directory("missing", &[("a", &manifest("a", "\"c\"", ""))]);
        assert!(scan(&dir, &dir).err().unwrap().contains("imports `c`"));
        let dir = directory(
            "cycle",
            &[
                ("a", &manifest("a", "\"b\"", "")),
                ("b", &manifest("b", "\"a\"", "")),
            ],
        );
        assert!(
            scan(&dir, &dir)
                .err()
                .unwrap()
                .contains("cycle: a -> b -> a")
        );
        // A package importing the cycle is not part of it.
        let dir = directory(
            "cycle-path",
            &[
                ("a", &manifest("a", "\"b\"", "")),
                ("b", &manifest("b", "\"c\"", "")),
                ("c", &manifest("c", "\"b\"", "")),
            ],
        );
        assert!(
            scan(&dir, &dir)
                .err()
                .unwrap()
                .contains("cycle: b -> c -> b")
        );
    }

    #[test]
    fn a_refused_import_refuses_its_importers() {
        let library = "library = \"x\"\nnegotiate = \"x_negotiate\"\n";
        let dir = directory(
            "load-order",
            &[
                ("a", &(manifest("a", "\"b\", \"c\"", "") + library)),
                ("b", &(manifest("b", "\"d\"", "") + library)),
                ("c", &manifest("c", "", "")),
                ("d", &(manifest("d", "", "") + library)),
                ("e", &(manifest("e", "\"c\"", "") + library)),
            ],
        );
        let packages = scan(&dir, &dir).unwrap();
        // `d` fails to open; every library is opened after its imports', with theirs.
        let mut opened = Vec::new();
        let loaded = load_all(&packages, |p, imports: &[(&str, &String)]| {
            opened.push(p.name.clone());
            if p.name == "d" {
                return Err("d: broken".into());
            }
            let names: Vec<&str> = imports.iter().map(|(n, _)| *n).collect();
            Ok(format!("{}({})", p.name, names.join(",")))
        });
        assert_eq!(opened, ["d", "e"]);
        let result = |name: &str| {
            loaded
                .iter()
                .find(|(p, _)| p.name == name)
                .map(|(_, r)| r.clone())
                .unwrap()
        };
        assert_eq!(result("e").unwrap(), "e()");
        let b = result("b").unwrap_err();
        assert!(b.contains("imports `d`, whose library was not loaded: d: broken"));
        let a = result("a").unwrap_err();
        assert!(a.contains("imports `b`, whose library was not loaded") && a.contains("d: broken"));
        assert!(!loaded.iter().any(|(p, _)| p.name == "c"));
    }

    #[test]
    fn series_lists_imported_parts() {
        let series = "[[provides]]\ntype = \"series\"\nkey = \"s\"\ntitle = { en = \"S\", zh-CN = \"甲\" }\nsummary = { en = \"Two parts.\" }\nparts = [\"b\", \"c\"]\n";
        let a = manifest("a", "\"b\", \"c\"", "") + series;
        let dir = directory(
            "series",
            &[
                ("a", &a),
                ("b", &manifest("b", "", "")),
                ("c", &manifest("c", "", "")),
            ],
        );
        let packages = scan(&dir, &dir).unwrap();
        let s = &packages.iter().find(|p| p.name == "a").unwrap().series[0];
        assert_eq!(s.parts, ["b", "c"]);
        assert_eq!(
            (s.title_in("zh-CN"), s.title_in("fr"), s.summary_in("zh-CN")),
            ("甲", "S", "Two parts.")
        );
        let unimported = manifest("a", "\"b\"", "") + series;
        let dir = directory(
            "series-unimported",
            &[
                ("a", &unimported),
                ("b", &manifest("b", "", "")),
                ("c", &manifest("c", "", "")),
            ],
        );
        assert!(scan(&dir, &dir).err().unwrap().contains("part `c`"));
    }

    #[test]
    fn binaries_dir_in_both_build_layouts() {
        let release = Path::new("/w/target/release");
        assert_eq!(binaries_dir(release), release);
        assert_eq!(binaries_dir(&release.join("deps")), release);
        let out = release.join("build/render-lab-core/bf11dd145b9d9a73/out");
        assert_eq!(binaries_dir(&out), release);
        // An `out` folder elsewhere is a binary's own.
        assert_eq!(binaries_dir(Path::new("/w/out")), Path::new("/w/out"));
    }

    #[test]
    fn name_matches_directory() {
        let dir = directory("name", &[("a", &manifest("b", "", ""))]);
        assert!(
            scan(&dir, &dir)
                .err()
                .unwrap()
                .contains("directory of another name")
        );
    }
}
