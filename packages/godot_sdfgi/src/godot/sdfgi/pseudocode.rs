//! The SDFGI investigation's Python pseudo-code listings (pseudocode/): one per
//! step a demonstration shows, each following the Godot function its
//! `# source:` header names. Parsed by `Listing::parse` (lab/src/pseudocode.rs).

use lab::pseudocode::Listing;
use lab::resources;
use std::sync::OnceLock;

/// In the order the viewer lists them; each is the resource
/// `godot_sdfgi.listing/<name>`.
const NAMES: &[&str] = &["frame", "cascade-update", "regions", "voxelize", "jump-flood", "store", "occlusion", "direct-light", "shadow-march", "probe-rays", "probe-store", "screen-pass"];

pub fn all() -> &'static [Listing] {
    static LISTINGS: OnceLock<Vec<Listing>> = OnceLock::new();
    LISTINGS.get_or_init(|| NAMES.iter().map(|name| Listing::parse(name, resources::get(&format!("godot_sdfgi.listing/{name}")).text())).collect())
}
