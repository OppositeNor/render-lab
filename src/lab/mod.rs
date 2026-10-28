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
//! fixtures, and the probes are absent, and the shaders compose the lab's
//! `wgsl/surface-lookup` (shaders/wgsl/surface_fallback.wgsl), which an
//! optional module's package may override.

pub mod animation;
pub mod audio;
pub mod camera;
pub mod config;
pub mod controls;
pub mod demo;
pub mod environment;
pub mod formats;
pub mod formula;
pub mod frame;
pub mod frost;
pub mod gpu;
pub mod home;
pub mod i18n;
pub mod investigation;
pub mod layout;
pub mod loading;
pub mod mesh;
pub mod palette;
pub mod popup;
pub mod pipeline;
pub mod pipelines;
pub mod pseudocode;
pub mod readback;
pub mod record;
pub mod renderer;
pub mod resources;
/// The environment's encoding, which the resource pipeline builds with.
pub mod rgb9e5;
pub mod rich_text;
pub mod scene;
pub mod shaders;
pub mod shot;
pub mod sources;
pub mod terrain;
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
