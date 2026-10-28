//! The lab: everything an investigation reuses, built as a Rust dylib that the
//! binary (src/main.rs in the repository) and every package's library link. The renderer (wgpu and WGSL)
//! draws the study room, its distance fields, cards, Surface Cache, probes,
//! and the overlays of a frame; the recorder renders a timeline frame by
//! frame into FFmpeg; the viewer plays a timeline with its narration or lets
//! the user explore the demonstrations; the homepage chooses the engine and
//! the feature. What a feature shows comes from its `Investigation`
//! (src/investigation.rs); the investigations come from the packages'
//! libraries, which the loader opens at run time (src/packages.rs,
//! src/plugin.rs, src/investigations.rs).
//!
//! Modules marked `commercial` build only when the optional commercial
//! module is present (the repository's src/commercial). They are not
//! published (see .gitignore and build.rs): they derive from such an engine
//! and live in src/commercial/lab. Without them the Surface Cache, the distance-field
//! fixtures, and the probes are absent, and the shaders compose the lab's
//! `render_lab.wgsl/surface-lookup` (packages/render_lab/shaders/surface_fallback.wgsl),
//! which another package may override.

#![cfg_attr(not(commercial), allow(dead_code))]

// So `#[derive(Values)]`'s `::lab::values` paths resolve inside the lab too.
extern crate self as lab;

/// `#[derive(Values)]`: a struct's fields reachable by key; `#[derive(Choice)]`:
/// an enum's variants as a choice's options (src/values.rs).
pub use render_lab_derive::{Choice, Values};

pub mod animation;
pub mod audio;
pub mod camera;
pub mod cli;
pub mod config;
pub mod controls;
pub mod cook;
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
pub mod investigations;
pub mod layout;
pub mod loading;
pub mod mesh;
pub mod packages;
pub mod palette;
pub mod popup;
pub mod pipeline;
pub mod pipelines;
pub mod plugin;
pub mod pseudocode;
pub mod readback;
pub mod record;
pub mod renderer;
pub mod resources;
/// The environment's encoding, which the resource pipeline builds with.
pub mod rgb9e5;
pub mod rich_text;
pub mod scene;
pub mod shader_data;
pub mod shaders;
pub mod shot;
pub mod sources;
pub mod terrain;
pub mod text;
pub mod ui;
pub mod values;
pub mod viewer;
pub mod volume;

#[cfg(commercial)]
#[path = "../../src/commercial/lab/fixtures.rs"]
pub mod fixtures;
#[cfg(commercial)]
#[path = "../../src/commercial/lab/reflection_rays.rs"]
pub mod reflection_rays;
#[cfg(commercial)]
#[path = "../../src/commercial/lab/surface_cache.rs"]
pub mod surface_cache;
#[cfg(commercial)]
#[path = "../../src/commercial/lab/surface_cache_gpu.rs"]
pub mod surface_cache_gpu;
