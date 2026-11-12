//! The ReSTIR investigation's Python pseudo-code listings (pseudocode/): one per
//! algorithm a demonstration shows, each following the paper's algorithm its
//! `# source:` header names. Parsed by `Listing::parse` (lab/src/pseudocode.rs).

use lab::pseudocode::Listing;
use lab::resources;
use std::sync::OnceLock;

/// In the order the viewer lists them; each is the resource `paper_restir.listing/<name>`.
const NAMES: &[&str] = &["frame", "ris", "combine"];

pub fn all() -> &'static [Listing] {
    static LISTINGS: OnceLock<Vec<Listing>> = OnceLock::new();
    LISTINGS.get_or_init(|| {
        NAMES
            .iter()
            .map(|name| {
                Listing::parse(
                    name,
                    resources::get(&format!("paper_restir.listing/{name}")).text(),
                )
            })
            .collect()
    })
}
