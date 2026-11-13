//! Every investigation the lab has, grouped by engine on the homepage: those
//! the packages' libraries hand the lab when they negotiate (src/packages.rs,
//! src/plugin.rs). The libraries are opened the first time the lab needs the
//! list, in the packages' order; a library that cannot be used is reported
//! (and so are the libraries that import it), and its package's content
//! stays mounted.

use crate::investigation::{Engine, Investigation};
use crate::plugin::Handles;
use crate::{packages, resources};
use std::sync::OnceLock;

/// What opening one package's library gave.
pub struct Library {
    pub package: &'static str,
    pub loaded: Result<Handles, String>,
}

static TITLE: OnceLock<&'static str> = OnceLock::new();

/// Names the investigation whose scene the homepage shows (`engine/feature`).
pub fn set_title(path: &'static str) {
    let _ = TITLE.set(path);
}

/// Every package's library, opened once, in the packages' order (each after
/// its imports', which it is handed; src/packages.rs `load_all`).
pub fn libraries() -> &'static [Library] {
    static LIBRARIES: OnceLock<Vec<Library>> = OnceLock::new();
    LIBRARIES.get_or_init(|| {
        let libraries: Vec<Library> = packages::load_all(resources::packages(), packages::load)
            .into_iter()
            .map(|(m, loaded)| Library {
                package: &m.name,
                loaded,
            })
            .collect();
        for library in &libraries {
            if let Err(e) = &library.loaded {
                eprintln!("{e}");
            }
        }
        libraries
    })
}

/// Why a loaded package's investigations cannot run here (its manifest's `requires`), if they cannot.
fn unmet(package: &str) -> Option<&'static str> {
    resources::packages()
        .iter()
        .find(|m| m.name == package)
        .and_then(|m| m.unmet())
}

/// Every investigation the packages' libraries hand the lab, with why it cannot run here, if it cannot: by
/// package name (the libraries open imports first, which would list an importer after the package it imports).
fn handed() -> &'static [(&'static dyn Investigation, Option<&'static str>)] {
    static HANDED: OnceLock<Vec<(&'static dyn Investigation, Option<&'static str>)>> =
        OnceLock::new();
    HANDED.get_or_init(|| {
        let mut libraries: Vec<&Library> = libraries().iter().collect();
        libraries.sort_by_key(|l| l.package);
        libraries
            .into_iter()
            .filter_map(|l| Some((l.package, l.loaded.as_ref().ok()?.investigations()?)))
            .flat_map(|(package, f)| f().iter().map(move |i| (*i, unmet(package))))
            .collect()
    })
}

/// The investigations that cannot run here, each with the reason (what its package requires).
pub fn unavailable() -> Vec<(&'static dyn Investigation, &'static str)> {
    handed()
        .iter()
        .filter_map(|(i, why)| Some((*i, (*why)?)))
        .collect()
}

/// The investigations that can run here.
pub fn all() -> &'static [&'static dyn Investigation] {
    static ALL: OnceLock<Vec<&'static dyn Investigation>> = OnceLock::new();
    ALL.get_or_init(|| {
        let all: Vec<&'static dyn Investigation> = handed()
            .iter()
            .filter(|(_, why)| why.is_none())
            .map(|(i, _)| *i)
            .collect();
        // Their choices' WGSL constants, before any shader composes them.
        crate::shader_data::write_choices(&all);
        all
    })
}

/// The viewer features the packages' libraries add (src/feature/), in the packages' order.
pub fn features() -> Vec<(&'static str, &'static dyn crate::feature::Feature)> {
    libraries()
        .iter()
        .filter_map(|l| Some((l.package, l.loaded.as_ref().ok()?.features()?)))
        .flat_map(|(package, f)| f().iter().map(move |f| (package, *f)))
        .collect()
}

/// The investigation whose backdrop the homepage shows behind every page:
/// the one the binary names, or the first.
pub fn title() -> &'static dyn Investigation {
    TITLE
        .get()
        .and_then(|path| find(path))
        .or_else(|| all().first().copied())
        .expect("no package provides an investigation")
}

/// The engines, in the order their first investigation is listed.
pub fn engines() -> Vec<&'static Engine> {
    let mut engines: Vec<&'static Engine> = Vec::new();
    for i in all() {
        if !engines.iter().any(|e| e.id == i.engine().id) {
            engines.push(i.engine());
        }
    }
    engines
}

/// An investigation by its `engine/feature` path.
pub fn find(path: &str) -> Option<&'static dyn Investigation> {
    all().iter().copied().find(|i| i.path() == path)
}
