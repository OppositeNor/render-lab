//! Every investigation the lab has, grouped by engine on the homepage, and the
//! resource packages they mount. The optional `commercial` module, not
//! provided in this repo, is an `Extension`: when it is present, this build
//! lists its investigations first, then the public ones, and mounts its
//! packages after the lab's.

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
        all.extend(crate::commercial::EXTENSION.investigations);
        all.extend(PUBLIC);
        all
    })
}

/// The resource packages, in the order they are mounted: the lab's, the
/// extension's, then each investigation's.
pub fn packages() -> Vec<String> {
    let mut packages = vec!["src/lab/resources.toml".to_string()];
    #[cfg(commercial)]
    packages.extend(crate::commercial::EXTENSION.packages.iter().map(|p| p.to_string()));
    packages.extend(all().iter().filter_map(|i| i.package()).map(String::from));
    packages
}

/// The investigation whose backdrop the homepage shows behind every page.
pub fn title() -> &'static dyn Investigation {
    &crate::o3de::lighting::LIGHTING
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
