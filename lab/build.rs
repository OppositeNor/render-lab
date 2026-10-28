//! Checks the lab's own package, render_lab, before the lab is built: its
//! shaders validate with naga and its text tables match the lab's code
//! (render-lab-check; every other package's build checks its own). A failure
//! fails the build.
//!
//! It also tells the lab what it is built from: the repository's root
//! (`LAB_ROOT`, the default for where the packages are), and the build's
//! fingerprint, which a package's library must share to be loaded
//! (`LAB_RUSTC`, `LAB_SOURCE_HASH`; src/plugin.rs).
//!
//! And it decides whether the lab's code for the optional commercial module
//! is built. It lives in the repository's src/commercial, which is not
//! published (see .gitignore): when all of `HIDDEN` are present it sets the
//! `commercial` cfg, and when none are, the lab builds without it.

use std::env;
use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The files of src/commercial the lab needs, relative to the repository's
/// root. They derive from commercial engines' source and stay local (.gitignore).
const HIDDEN: &[&str] = &["src/commercial/lab/fixtures.rs", "src/commercial/lab/reflection_rays.rs", "src/commercial/lab/surface_cache.rs", "src/commercial/lab/surface_cache_gpu.rs"];

/// Hashes every file under `dir` (sorted, by path and bytes) into `hash`.
fn hash_dir(dir: &Path, root: &Path, hash: &mut DefaultHasher) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir).map(|e| e.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            hash_dir(&path, root, hash);
        } else if let Ok(bytes) = fs::read(&path) {
            hash.write(path.strip_prefix(root).unwrap_or(&path).to_string_lossy().as_bytes());
            hash.write(&bytes);
        }
    }
}

fn main() {
    // The lab may be built through a link to it (a local package's workspace):
    // the root is where the lab really is.
    let manifest_dir = fs::canonicalize(env::var("CARGO_MANIFEST_DIR").unwrap()).unwrap();
    let root = manifest_dir.parent().unwrap().to_path_buf();
    println!("cargo:rustc-env=LAB_ROOT={}", root.display());

    println!("cargo::rustc-check-cfg=cfg(commercial)");
    for path in HIDDEN {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
    }
    let present: Vec<&str> = HIDDEN.iter().copied().filter(|p| root.join(p).is_file()).collect();
    let commercial = present.len() == HIDDEN.len();
    if commercial {
        println!("cargo::rustc-cfg=commercial");
    } else if !present.is_empty() {
        let missing: Vec<&str> = HIDDEN.iter().copied().filter(|p| !present.contains(p)).collect();
        panic!("src/commercial is only partly present; missing: {}", missing.join(", "));
    }

    // The fingerprint: the compiler, and every source the lab is built from.
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let version = Command::new(&rustc).arg("--version").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    println!("cargo:rustc-env=LAB_RUSTC={version}");
    let mut hash = DefaultHasher::new();
    hash_dir(&manifest_dir.join("src"), &root, &mut hash);
    hash.write(&fs::read(manifest_dir.join("Cargo.toml")).unwrap());
    if commercial {
        hash_dir(&root.join("src/commercial/lab"), &root, &mut hash);
    }
    println!("cargo:rustc-env=LAB_SOURCE_HASH={:016x}", hash.finish());
    println!("cargo:rerun-if-changed={}", manifest_dir.join("src").display());

    render_lab_check::check_package(&root.join("packages/render_lab"), &manifest_dir.join("src"));
}
