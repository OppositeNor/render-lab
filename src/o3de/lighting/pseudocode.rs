//! The lighting investigation's Python pseudo-code listings (pseudocode/):
//! one per step a demonstration shows, each following the O3DE function its
//! `# source:` header names. Parsed by `Listing::parse` (src/lab/pseudocode.rs).

use crate::lab::pseudocode::Listing;
use std::sync::OnceLock;

const FILES: &[(&str, &str)] = &[
    ("forward-pass", include_str!("pseudocode/forward_pass.py")),
    ("light-culling", include_str!("pseudocode/light_culling.py")),
    ("surface-setup", include_str!("pseudocode/surface_setup.py")),
    ("occlusion", include_str!("pseudocode/occlusion.py")),
    ("decals", include_str!("pseudocode/decals.py")),
    ("decal-culling", include_str!("pseudocode/decal_culling.py")),
    ("specular-ggx", include_str!("pseudocode/specular_ggx.py")),
    ("clear-coat", include_str!("pseudocode/clear_coat.py")),
    ("clear-coat-specular", include_str!("pseudocode/clear_coat_specular.py")),
    ("diffuse-response", include_str!("pseudocode/diffuse_response.py")),
    ("transmission", include_str!("pseudocode/transmission.py")),
    ("multiscatter", include_str!("pseudocode/multiscatter.py")),
    ("brdf-texture", include_str!("pseudocode/brdf_texture.py")),
    ("simple-point-light", include_str!("pseudocode/simple_point_light.py")),
    ("ibl-passes", include_str!("pseudocode/ibl_passes.py")),
    ("ibl-prefilter", include_str!("pseudocode/ibl_prefilter.py")),
    ("reflection-probes", include_str!("pseudocode/reflection_probes.py")),
    ("probe-specular", include_str!("pseudocode/probe_specular.py")),
    ("ssao", include_str!("pseudocode/ssao.py")),
];

pub fn all() -> &'static [Listing] {
    static LISTINGS: OnceLock<Vec<Listing>> = OnceLock::new();
    LISTINGS.get_or_init(|| FILES.iter().map(|(name, text)| Listing::parse(name, text)).collect())
}
