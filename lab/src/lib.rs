//! The lab: everything an investigation reuses, built as a Rust dylib that the
//! binary (src/main.rs in the repository) and every package's library link. The renderer (wgpu and WGSL)
//! draws the study room and the overlays of a frame; the recorder renders a timeline frame by
//! frame into FFmpeg; the viewer plays a timeline with its narration or lets
//! the user explore the demonstrations; the homepage chooses the engine and
//! the feature. What a feature shows comes from its `Investigation`
//! (src/investigation.rs); the investigations come from the packages'
//! libraries, which the loader opens at run time (src/packages.rs,
//! src/plugin.rs, src/investigations.rs).

// So `#[derive(Values)]`'s `::lab::values` paths resolve inside the lab too.
extern crate self as lab;

/// `#[derive(Values)]`: a struct's fields reachable by key; `#[derive(Choice)]`:
/// an enum's variants as a choice's options (src/values.rs); `#[actions]`:
/// an `impl` block's functions as actions a story runs (src/actions.rs).
pub use render_lab_derive::{actions, Choice, Values};

pub mod actions;
pub mod animation;
pub mod audio;
pub mod camera;
pub mod cli;
pub mod config;
pub mod controls;
pub mod cook;
pub mod demo;
pub mod diagram;
pub mod entity;
pub mod environment;
/// A page drawn with egui into a texture: the loading page, the homepage.
pub mod egui_pass;
pub mod experiment;
pub mod feature;
pub mod quiz;
pub mod formats;
pub mod formula;
pub mod fetch;
pub mod frame;
pub mod free_view;
pub mod frost;
pub mod gizmo;
pub mod gpu;
pub mod graph;
pub mod graph_view;
pub mod shader_source;
pub mod code_view;
pub mod home;
pub mod i18n;
pub mod investigation;
pub mod investigations;
pub mod layered;
pub mod layout;
pub mod loading;
pub mod mesh;
pub mod metrics;
pub mod packages;
pub mod palette;
/// The cards the lab's panels and viewport overlays are drawn on.
pub mod paint;
pub mod popup;
pub mod pipeline;
pub mod pipelines;
pub mod program;
pub mod plugin;
pub mod pseudocode;
pub mod readback;
pub mod record;
pub mod renderer;
pub mod research;
pub mod resources;
/// The environment's encoding, which the resource pipeline builds with.
pub mod rgb9e5;
pub mod rich_text;
pub mod scene;
pub mod script;
pub mod shader_data;
pub mod shading_file;
pub mod shaders;
pub mod shot;
mod story;
mod story_refs;
/// The knowledge graph and an investigation's audience (the Prerequisites tab, the knowledge panel).
pub mod knowledge;
pub mod sources;
pub mod terrain;
pub mod text;
pub mod theme;
pub mod timing;
pub mod ui;
/// The standard package's code (packages/render_lab/src): the picture inset, a
/// selftest's pieces, a package's source index and text table.
#[path = "../../packages/render_lab/src/mod.rs"]
pub mod utility;
pub mod values;
pub mod view;
pub mod viewer;
pub mod views;
pub mod volume;
/// What the homepage's and the viewer's windows share: the window, its swapchain, pointer input.
pub mod window;

