//! The build of the packages: every source package (packages/ in a source
//! checkout) is built into the folder the loader reads, `packages/` beside
//! the binary (src/resources.rs, `packages_dir`). A built package is a
//! package like its source, self-contained:
//!
//! - `manifest.toml`: the source's, with one collection, and `cooked-from`,
//!   the source package it was built from;
//! - `resources.toml`: every resource with its output (`file`), its source
//!   (for messages), and its `stamp` (src/pipeline: what the output is made
//!   from), and the shaders;
//! - the outputs: a `file` resource at its path in the package (linked to its
//!   source, or copied), a processed one at `cooked/<key>.bin`;
//! - `lib/`: its library, from where the Cargo build put it (beside the binary);
//! - its notices' license files, at their paths in the package.
//!
//! The build is incremental: an output whose stamp has not changed is kept,
//! so running it on every launch from a checkout costs a look at each
//! source's size and time. A built package that was not built from this
//! checkout (one shared with the lab, with no `cooked-from` of this
//! checkout's) is never touched.

use crate::packages::{self, Package};
use crate::pipeline::{self, Declaration, Form};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// What one build did.
#[derive(Default)]
pub struct Report {
    /// Outputs made, by full key.
    pub produced: Vec<String>,
    /// Outputs kept.
    pub kept: usize,
    /// Libraries linked into their packages.
    pub libraries: Vec<String>,
    /// Files and packages removed (no source names them any more).
    pub removed: Vec<PathBuf>,
}

/// A TOML basic string.
fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => write!(out, "\\u{:04X}", c as u32).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn list(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| quote(s)).collect::<Vec<_>>().join(", "))
}

/// Writes `text` to `path` so that a reader never sees half of it.
fn write(path: &Path, text: &str) -> Result<(), String> {
    let partial = path.with_extension(format!("partial.{}", std::process::id()));
    fs::write(&partial, text).and_then(|_| fs::rename(&partial, path)).map_err(|e| format!("packages: write {}: {e}", path.display()))
}

/// The package a built package's manifest says it was built from.
fn cooked_from(dir: &Path) -> Option<String> {
    let text = fs::read_to_string(dir.join("manifest.toml")).ok()?;
    let document = toml_edit::Document::parse(text).ok()?;
    document.get("cooked-from")?.as_str().map(String::from)
}

/// The stamps and outputs a built collection recorded, by key.
fn previous(dir: &Path) -> BTreeMap<String, (String, String)> {
    let mut out = BTreeMap::new();
    let Ok(text) = fs::read_to_string(dir.join("resources.toml")) else { return out };
    let Ok(document) = toml_edit::Document::parse(text) else { return out };
    for table in document.get("resource").and_then(|r| r.as_array_of_tables()).into_iter().flatten() {
        let field = |name: &str| table.get(name).and_then(|v| v.as_str()).map(String::from);
        if let (Some(id), Some(file), Some(stamp)) = (field("id"), field("file"), field("stamp")) {
            out.insert(id, (stamp, file));
        }
    }
    out
}

/// Holds the build's lock while it lives, so two launches do not build at once.
struct Lock(#[allow(dead_code)] fs::File);

fn lock(dir: &Path) -> Result<Lock, String> {
    fs::create_dir_all(dir).map_err(|e| format!("packages: create {}: {e}", dir.display()))?;
    let file = fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(".build.lock")).map_err(|e| format!("packages: lock {}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: a valid descriptor; the lock is released when the file closes.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
            return Err(format!("packages: lock {}: {}", dir.display(), std::io::Error::last_os_error()));
        }
    }
    Ok(Lock(file))
}

/// Whether `a` and `b` are the same file, or files of the same size and time.
pub fn same_file(a: &Path, b: &Path) -> bool {
    let (Ok(a), Ok(b)) = (fs::metadata(a), fs::metadata(b)) else { return false };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a.dev() == b.dev() && a.ino() == b.ino() {
            return true;
        }
    }
    a.len() == b.len() && a.modified().ok() == b.modified().ok()
}

/// Links (or copies) `from` to `to`.
fn link(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("packages: create {}: {e}", dir.display()))?;
    }
    let partial = to.with_extension(format!("partial.{}", std::process::id()));
    if fs::hard_link(from, &partial).is_err() {
        fs::copy(from, &partial).map_err(|e| format!("packages: copy {}: {e}", from.display()))?;
    }
    fs::rename(&partial, to).map_err(|e| format!("packages: write {}: {e}", to.display()))
}

/// Every file under `dir`, relative to it.
fn files(dir: &Path, base: &Path, out: &mut Vec<String>) {
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files(&path, base, out);
        } else {
            out.push(pipeline::relative(base, &path));
        }
    }
}

/// Builds `package` (the source in `package.dir`, after the packages it imports, `imported`) into `out`.
fn build(root: &Path, package: &Package, imported: &[Package], out: &Path, report: &mut Report) -> Result<(), String> {
    let source = fs::canonicalize(&package.dir).map_err(|e| format!("packages: {}: {e}", package.dir.display()))?.to_string_lossy().into_owned();
    if out.join("manifest.toml").is_file() && cooked_from(out).as_deref() != Some(source.as_str()) {
        return Err(format!("packages: {} was not built from {source} (it came from elsewhere): remove it or rename the source package", out.display()));
    }
    let resolve = |reference: &str| packages::resolve(package, imported, reference);
    let mut resources: Vec<Declaration> = Vec::new();
    let mut shaders = Vec::new();
    for collection in &package.collections {
        let read = pipeline::read_collection(root, &package.dir, collection, &package.name, Form::Source, &resolve).map_err(|e| e.to_string())?;
        for decl in read.resources {
            if let Some(other) = resources.iter().find(|r| r.id == decl.id) {
                return Err(format!("resources: `{}` is declared by {} and {}", decl.id, other.manifest, decl.manifest));
            }
            resources.push(decl);
        }
        shaders.extend(read.shaders);
    }
    let previous = previous(out);
    let mut kept_files: BTreeSet<String> = ["manifest.toml", "resources.toml"].into_iter().map(String::from).collect();
    let mut collection = String::from("# Built from the package's source collections; rebuilt when they change. Do not edit.\n");
    for decl in &resources {
        let output = decl.output();
        let stamp = decl.stamp();
        match &stamp {
            // A tool's output that is not built yet.
            None => {}
            Some(stamp) if previous.get(decl.key()).is_some_and(|(s, f)| s == stamp && *f == output) && out.join(&output).exists() => report.kept += 1,
            Some(_) => {
                decl.produce(&out.join(&output))?;
                report.produced.push(decl.id.clone());
            }
        }
        if decl.kind.name == "directory" {
            kept_files.extend(decl.files().into_iter().map(|f| format!("{output}/{f}")));
        } else {
            kept_files.insert(output.clone());
        }
        writeln!(collection, "\n[[resource]]\nid = {}\nkind = {}\nfile = {}\nsource = {}\nlicense = {}", quote(decl.key()), quote(decl.kind.name), quote(&output), quote(&decl.source), quote(&decl.license)).unwrap();
        if let Some(stamp) = &stamp {
            writeln!(collection, "stamp = {}", quote(stamp)).unwrap();
        }
        if decl.built {
            collection.push_str("built = true\n");
        }
        if decl.overridable {
            collection.push_str("overridable = true\n");
        }
        if let Some(target) = &decl.overrides {
            writeln!(collection, "override = {}", quote(target)).unwrap();
        }
    }
    for shader in &shaders {
        let key = shader.id.split_once('.').map(|(_, k)| k).unwrap_or(&shader.id);
        writeln!(collection, "\n[[shader]]\nid = {}\nparts = {}", quote(key), list(&shader.written)).unwrap();
    }
    // Its library, from the Cargo build.
    if let Some((name, _)) = &package.library {
        let file = libloading::library_filename(name).to_string_lossy().into_owned();
        let built = packages::bin_dir().join(&file);
        let target = out.join("lib").join(&file);
        if built.is_file() && !same_file(&built, &target) {
            link(&built, &target)?;
            report.libraries.push(package.name.clone());
        }
        kept_files.insert(format!("lib/{file}"));
    }
    // Its notices' terms travel with it.
    for file in package.notices.iter().filter_map(|n| n.license.as_ref()) {
        let (from, to) = (package.dir.join(file), out.join(file));
        if !same_file(&from, &to) {
            link(&from, &to)?;
        }
        kept_files.insert(file.clone());
    }
    write(&out.join("resources.toml"), &collection)?;
    let mut manifest = format!("# Built from {source}; rebuilt when it changes. Do not edit.\n");
    writeln!(manifest, "name = {}\nauthor = {}", quote(&package.name), quote(&package.author)).unwrap();
    if let Some(contact) = &package.contact {
        writeln!(manifest, "contact = {}", quote(contact)).unwrap();
    }
    writeln!(manifest, "license = {}\nimport = {}\nexport = {}", quote(&package.license), list(&package.imports), list(&package.exports)).unwrap();
    if let Some((library, negotiate)) = &package.library {
        writeln!(manifest, "library = {}\nnegotiate = {}", quote(library), quote(negotiate)).unwrap();
    }
    // Top-level keys before the first table.
    if let Some(credit) = &package.credit {
        writeln!(manifest, "credit-text = {}", quote(credit)).unwrap();
    }
    writeln!(manifest, "cooked-from = {}\n\n[[provides]]\ntype = \"resource\"\npath = \"resources.toml\"", quote(&source)).unwrap();
    for demo in &package.demos {
        writeln!(manifest, "\n[[provides]]\ntype = \"demo\"\nkey = {}\ntitle = {}\nedition = {}\ntimeline = {}\naudio = {}", quote(&demo.key), quote(&demo.title), quote(&demo.edition), quote(&demo.timeline), quote(&demo.audio)).unwrap();
    }
    for notice in &package.notices {
        writeln!(manifest, "\n[[notice]]\nnotice-text = {}", quote(&notice.text)).unwrap();
        if let Some(file) = &notice.license {
            writeln!(manifest, "license = {}", quote(file)).unwrap();
        }
    }
    write(&out.join("manifest.toml"), &manifest)?;
    // What no source names any more, and what an interrupted build left
    // (no other build runs: this one holds the lock).
    let mut present = Vec::new();
    files(out, out, &mut present);
    for file in present {
        if !kept_files.contains(&file) {
            let path = out.join(&file);
            fs::remove_file(&path).map_err(|e| format!("packages: remove {}: {e}", path.display()))?;
            report.removed.push(path);
        }
    }
    Ok(())
}

/// Builds every package of `source_dir` into `out_dir`, and removes the
/// packages there that were built from it but no longer exist.
pub fn cook(root: &Path, source_dir: &Path, out_dir: &Path) -> Result<Report, String> {
    let _lock = lock(out_dir)?;
    let sources = packages::scan(root, source_dir)?;
    let mut report = Report::default();
    for (i, package) in sources.iter().enumerate() {
        build(root, package, &sources[..i], &out_dir.join(&package.name), &mut report)?;
    }
    let source_dir = fs::canonicalize(source_dir).map_err(|e| format!("packages: {}: {e}", source_dir.display()))?;
    for entry in fs::read_dir(out_dir).into_iter().flatten().flatten() {
        let dir = entry.path();
        let built_here = cooked_from(&dir).is_some_and(|from| Path::new(&from).starts_with(&source_dir));
        if built_here && !sources.iter().any(|p| dir.file_name() == Some(p.name.as_ref())) {
            fs::remove_dir_all(&dir).map_err(|e| format!("packages: remove {}: {e}", dir.display()))?;
            report.removed.push(dir);
        }
    }
    Ok(report)
}
