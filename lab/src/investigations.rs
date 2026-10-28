//! Every investigation the lab has, grouped by engine on the homepage: those
//! the packages' libraries hand the lab when they negotiate (src/packages.rs,
//! src/plugin.rs). The libraries are opened the first time the lab needs the
//! list, in the packages' order; a library that cannot be used is reported,
//! and its package's content stays mounted.

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

/// Every package's library, opened once.
pub fn libraries() -> &'static [Library] {
    static LIBRARIES: OnceLock<Vec<Library>> = OnceLock::new();
    LIBRARIES.get_or_init(|| {
        let libraries: Vec<Library> = resources::packages().iter().filter(|m| m.library.is_some()).map(|m| Library { package: &m.name, loaded: packages::load(m) }).collect();
        for library in &libraries {
            if let Err(e) = &library.loaded {
                eprintln!("{e}");
            }
        }
        libraries
    })
}

pub fn all() -> &'static [&'static dyn Investigation] {
    static ALL: OnceLock<Vec<&'static dyn Investigation>> = OnceLock::new();
    ALL.get_or_init(|| libraries().iter().filter_map(|l| l.loaded.as_ref().ok()?.investigations()).flat_map(|f| f().iter().copied()).collect())
}

/// The investigation whose backdrop the homepage shows behind every page:
/// the one the binary names, or the first.
pub fn title() -> &'static dyn Investigation {
    TITLE.get().and_then(|path| find(path)).or_else(|| all().first().copied()).expect("no package provides an investigation")
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
