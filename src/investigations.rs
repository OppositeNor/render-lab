//! Every investigation the lab has, grouped by engine on the homepage. The
//! investigations of commercial engines live in src/commercial, which is not
//! published (see .gitignore); this build lists them first when they are
//! present, then the public ones.

use crate::lab::investigation::{Engine, Investigation};
use std::sync::OnceLock;

/// The investigations whose source is published with the lab.
static PUBLIC: [&dyn Investigation; 1] = [&crate::o3de::lighting::LIGHTING];

pub fn all() -> &'static [&'static dyn Investigation] {
    static ALL: OnceLock<Vec<&'static dyn Investigation>> = OnceLock::new();
    ALL.get_or_init(|| {
        #[cfg_attr(not(commercial), allow(unused_mut))]
        let mut all: Vec<&'static dyn Investigation> = Vec::new();
        #[cfg(commercial)]
        all.extend(crate::commercial::INVESTIGATIONS);
        all.extend(PUBLIC);
        all
    })
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
