//! WGSL shader modules, composed from a shared prelude (types, bindings,
//! common math, and src/commercial's surface_lookup.wgsl, or
//! shaders/wgsl/surface_fallback.wgsl without it) and one pass file each.
//! An investigation's own scene shading is composed after `SCENE` at run time
//! (`Investigation::scene_shading`). build.rs validates the same
//! compositions with naga and exports the struct layouts.

#[cfg(commercial)]
macro_rules! wgsl_module {
    ($pass:literal) => {
        concat!(
            include_str!("../../shaders/wgsl/types.wgsl"),
            include_str!("../../shaders/wgsl/bindings.wgsl"),
            include_str!("../../shaders/wgsl/common.wgsl"),
            include_str!("../commercial/shaders/surface_lookup.wgsl"),
            include_str!($pass),
        )
    };
}

#[cfg(not(commercial))]
macro_rules! wgsl_module {
    ($pass:literal) => {
        concat!(
            include_str!("../../shaders/wgsl/types.wgsl"),
            include_str!("../../shaders/wgsl/bindings.wgsl"),
            include_str!("../../shaders/wgsl/common.wgsl"),
            include_str!("../../shaders/wgsl/surface_fallback.wgsl"),
            include_str!($pass),
        )
    };
}

pub const SCENE: &str = wgsl_module!("../../shaders/wgsl/scene.wgsl");
pub const OVERLAY: &str = wgsl_module!("../../shaders/wgsl/overlay.wgsl");
pub const VERIFY: &str = wgsl_module!("../../shaders/wgsl/verify.wgsl");
#[cfg(commercial)]
pub const SURFACE_CACHE: &str = wgsl_module!("../commercial/shaders/surface_cache.wgsl");

/// The type declarations the struct panel displays.
pub const TYPES_SOURCE: &str = include_str!("../../shaders/wgsl/types.wgsl");

/// Struct layouts computed by naga for types.wgsl (see build.rs).
pub const LAYOUT_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/wgsl-layout.json"));
pub const MANIFEST: &str = include_str!(concat!(env!("OUT_DIR"), "/shader-manifest.json"));
