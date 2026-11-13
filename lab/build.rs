//! Checks the lab's own package, render_lab, before the lab is built: its
//! shaders validate with naga and its text tables match the lab's code
//! (render-lab-check; every other package's build checks its own). A failure
//! fails the build.
//!
//! It also tells the lab what it is built from: the repository's root
//! (`LAB_ROOT`, the default for where the packages are), and the build's
//! fingerprint, which a package's library must share to be loaded
//! (`LAB_RUSTC`, `LAB_SOURCE_HASH`; src/plugin.rs): the compiler, the lab's
//! sources (its own, its derive macros', and render_lab's code, compiled
//! into it as `lab::utility`), and the versions its dependencies resolved to.

use std::env;
use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Hashes every file under `dir` (sorted, by path and bytes) into `hash`.
fn hash_dir(dir: &Path, root: &Path, hash: &mut DefaultHasher) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map(|e| e.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            hash_dir(&path, root, hash);
        } else if let Ok(bytes) = fs::read(&path) {
            hash.write(
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .as_bytes(),
            );
            hash.write(&bytes);
        }
    }
}

/// The `[[package]]` entries of a Cargo.lock that `root` depends on, itself
/// included, each as its text, sorted: the versions the lab's dependencies
/// resolved to.
fn lock_closure(lock: &str, root: &str) -> Vec<String> {
    let entries: Vec<&str> = lock.split("[[package]]").skip(1).collect();
    let field = |entry: &str, name: &str| {
        entry.lines().find_map(|l| {
            let value = l.strip_prefix(name)?.trim_start().strip_prefix('=')?;
            Some(value.trim().trim_matches('"').to_string())
        })
    };
    // A dependency is `name`, `name version`, or `name version (source)`.
    let find = |dependency: &str| {
        let mut words = dependency.split(' ');
        let (name, version) = (words.next(), words.next());
        entries.iter().position(|e| {
            field(e, "name").as_deref() == name
                && version.is_none_or(|v| field(e, "version").as_deref() == Some(v))
        })
    };
    let mut taken = vec![false; entries.len()];
    let mut pending: Vec<usize> = find(root).into_iter().collect();
    while let Some(i) = pending.pop() {
        if std::mem::replace(&mut taken[i], true) {
            continue;
        }
        // Its `dependencies = [ "a", "b 1.0", ]` list, one quoted name a line.
        let list = entries[i]
            .split_once("dependencies = [")
            .map_or("", |(_, l)| l);
        let list = list.split(']').next().unwrap_or("");
        pending.extend(
            list.lines()
                .map(|l| l.trim().trim_end_matches(',').trim_matches('"'))
                .filter(|d| !d.is_empty())
                .filter_map(find),
        );
    }
    let mut closure: Vec<String> = (0..entries.len())
        .filter(|&i| taken[i])
        .map(|i| entries[i].trim().to_string())
        .collect();
    closure.sort();
    closure
}

/// The Cargo.lock of the workspace building the lab: the first above the lab's
/// directory as Cargo names it (through a local package's link, that package's).
fn workspace_lock(manifest_dir: &Path) -> Option<PathBuf> {
    manifest_dir
        .ancestors()
        .skip(1)
        .map(|dir| dir.join("Cargo.lock"))
        .find(|lock| lock.is_file())
}

fn main() {
    // The lab may be built through a link to it (a local package's workspace):
    // the root is where the lab really is.
    let named_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let manifest_dir = fs::canonicalize(&named_dir).unwrap();
    let root = manifest_dir.parent().unwrap().to_path_buf();
    println!("cargo:rustc-env=LAB_ROOT={}", root.display());

    // The fingerprint: the compiler, every source the lab is built from, and
    // its dependencies' resolved versions.
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let version = Command::new(&rustc)
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=LAB_RUSTC={version}");
    let mut hash = DefaultHasher::new();
    hash_dir(&manifest_dir.join("src"), &root, &mut hash);
    hash.write(&fs::read(manifest_dir.join("Cargo.toml")).unwrap());
    let derive = manifest_dir.join("derive");
    hash_dir(&derive.join("src"), &root, &mut hash);
    hash.write(&fs::read(derive.join("Cargo.toml")).unwrap());
    let utility = root.join("packages/render_lab/src");
    hash_dir(&utility, &root, &mut hash);
    let lock = workspace_lock(&named_dir);
    if let Some(lock) = &lock {
        for entry in lock_closure(&fs::read_to_string(lock).unwrap(), "render-lab-core") {
            hash.write(entry.as_bytes());
        }
    }
    println!("cargo:rustc-env=LAB_SOURCE_HASH={:016x}", hash.finish());
    // The lab's own files are named relative to it (a build script runs in its
    // package's directory), so a build through the link prints the same
    // instructions as the repository's and does not rerun this; render_lab's
    // code is named by its real path, and the lock is the building workspace's.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=derive");
    println!("cargo:rerun-if-changed={}", utility.display());
    if let Some(lock) = &lock {
        println!("cargo:rerun-if-changed={}", lock.display());
    }

    render_lab_check::check_package(&root.join("packages/render_lab"), Path::new("src"));
}
