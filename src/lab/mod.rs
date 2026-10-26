//! The lab: everything an investigation reuses. The renderer (wgpu and WGSL)
//! draws the study room, its distance fields, cards, Surface Cache, probes,
//! and the overlays of a frame; the recorder renders a timeline frame by
//! frame into FFmpeg; the viewer plays a timeline with its narration or lets
//! the user explore the demonstrations; the homepage chooses the engine and
//! the feature. What a feature shows comes from its `Investigation`
//! (src/lab/investigation.rs).
//!
//! Modules marked `commercial` build only when the investigations of
//! commercial engines are present (src/commercial). They are not published
//! (see .gitignore and build.rs): they derive from such an engine and live in
//! src/commercial/lab. Without them the Surface Cache, the distance-field
//! fixtures, and the probes are absent, and the shaders compose
//! shaders/wgsl/surface_fallback.wgsl instead of their lookups.

pub mod audio;
pub mod camera;
pub mod config;
pub mod controls;
pub mod formats;
pub mod frame;
pub mod frost;
pub mod gpu;
pub mod home;
pub mod i18n;
pub mod investigation;
pub mod layout;
pub mod mesh;
pub mod palette;
pub mod pipelines;
pub mod pseudocode;
pub mod readback;
pub mod record;
pub mod renderer;
pub mod scene;
pub mod shaders;
pub mod shot;
pub mod sources;
pub mod text;
pub mod ui;
pub mod viewer;

#[cfg(commercial)]
#[path = "../commercial/lab/fixtures.rs"]
pub mod fixtures;
#[cfg(commercial)]
#[path = "../commercial/lab/reflection_rays.rs"]
pub mod reflection_rays;
#[cfg(commercial)]
#[path = "../commercial/lab/surface_cache.rs"]
pub mod surface_cache;
#[cfg(commercial)]
#[path = "../commercial/lab/surface_cache_gpu.rs"]
pub mod surface_cache_gpu;
