//! The lighting investigation's Python pseudo-code listings (pseudocode/):
//! one per step a demonstration shows, each following the O3DE function its
//! `# source:` header names. Parsed by `Listing::parse` (src/lab/pseudocode.rs).

use crate::lab::pseudocode::Listing;
use crate::lab::resources;
use std::sync::OnceLock;

/// In the order the viewer lists them; each is the resource
/// `o3de/lighting/listing/<name>`.
const NAMES: &[&str] = &[
    "forward-pass",
    "light-culling",
    "surface-setup",
    "occlusion",
    "decals",
    "decal-culling",
    "specular-ggx",
    "clear-coat",
    "clear-coat-specular",
    "diffuse-response",
    "transmission",
    "multiscatter",
    "brdf-texture",
    "simple-point-light",
    "ibl-passes",
    "ibl-prefilter",
    "reflection-probes",
    "probe-specular",
    "ssao",
];

pub fn all() -> &'static [Listing] {
    static LISTINGS: OnceLock<Vec<Listing>> = OnceLock::new();
    LISTINGS.get_or_init(|| NAMES.iter().map(|name| Listing::parse(name, resources::get(&format!("o3de/lighting/listing/{name}")).text())).collect())
}
