//! SDFGI, the signed distance field global illumination of Godot's Forward+
//! renderer (Godot 4.7.2-stable, the commit in source-index.json): nested
//! cascades of voxels that follow the camera, a jump-flood distance field,
//! light injected into the solid voxels, probes that trace the field and keep
//! a history of spherical harmonics, and the screen pass that reads them.
//!
//! `model` mirrors the engine's passes on the CPU, at the engine's sizes. The
//! demonstrations and parameters are in `shot`, what each shot shows in
//! `demos`, the dock in `controls`, the Godot source index in
//! source-index.json (written by source_index.py), and the pseudo-code in
//! pseudocode/.

pub mod actions;
pub mod controls;
pub mod demos;
pub mod light;
pub mod model;
pub mod occlusion;
pub mod probes;
pub mod pseudocode;
pub mod screen;
pub mod selftest;
pub mod shot;
pub mod sim;
pub mod sources;
pub mod text;
pub mod values;
pub mod voxels;

use crate::godot::sdfgi::text::tr;
use lab::camera::Projector;
use lab::controls::Dock;
use lab::demo::Demos;
use lab::frame::{FrameContent, Statics};
use lab::investigation::{Engine, Investigation, SceneShading, SelftestOptions, Wording};
use lab::pseudocode::Listing;
use lab::shot::{DemoId, PanelMode, ShotState};
use lab::sources::SourceIndex;
use lab::utility::inset::ImageSpec;
use glam::Vec2;
use shot::Demo;

pub struct Sdfgi;

pub static SDFGI: Sdfgi = Sdfgi;

static WORDING: Wording = Wording {
    engine_short: "Godot",
    engine_tag: "Godot",
    checkout_dir: "godot/",
};

/// The scene pass shades with shaders/sdfgi_shading.wgsl: the lab's direct
/// light plus the WGSL port of Godot's screen pass reading the probe maps of
/// the converged CPU world (sim.rs).
pub static SHADING: SceneShading = SceneShading {
    module: "godot_sdfgi.shader/sdfgi-shading",
    fragment: "sdfgiForwardFS",
    data: sim::shading_data,
    verify: &["sdfgiProcessTests", "sdfgiLightTests", "sdfgiProbeTests", "sdfgiStoreTests"],
    targets: &[],
    outputs: &[],
    passes: &[],
    prepass: None,
    early: &[],
    buffers: &[],
    scene_reads: &[],
};

/// The images its story can show (`{@ image=key}`), which `demos::field_demo` publishes.
pub static IMAGES: &[ImageSpec] = &[
    ImageSpec { key: "distanceField", doc: "A slice of cascade 0's distance field at sliceHeight, the worked cell marked." },
    ImageSpec { key: "floodSeeds", doc: "The same slice's jump-flood seeds at floodStep, the cells the step changed in orange." },
];

/// The points its story's arrows and labels can name, which `demos::field_demo` publishes.
pub static ANNOTATIONS: &[lab::utility::annotations::AnnotationSpec] = &[
    lab::utility::annotations::AnnotationSpec { key: "field.worked", kind: lab::utility::annotations::AnnotationKind::Point, doc: "The field demonstration's worked cell, at sliceHeight." },
    lab::utility::annotations::AnnotationSpec { key: "field.seed", kind: lab::utility::annotations::AnnotationKind::Point, doc: "The seed the jump flood has given that cell at floodStep (none before it has one)." },
];

impl Investigation for Sdfgi {
    fn engine(&self) -> &'static Engine {
        &crate::godot::ENGINE
    }
    fn actions(&self) -> &'static lab::actions::Actions {
        &Self::ACTIONS
    }
    fn images(&self) -> &'static [ImageSpec] {
        IMAGES
    }
    fn annotations(&self) -> &'static [lab::utility::annotations::AnnotationSpec] {
        ANNOTATIONS
    }

    fn id(&self) -> &'static str {
        "sdfgi"
    }

    fn title(&self) -> &'static str {
        tr("investigation.title")
    }

    fn summary(&self) -> &'static str {
        tr("investigation.summary")
    }

    fn wording(&self) -> &'static Wording {
        &WORDING
    }

    fn footer(&self) -> &'static str {
        tr("investigation.footer")
    }

    fn sources(&self) -> &'static SourceIndex {
        sources::INDEX.get()
    }

    fn listings(&self) -> &'static [Listing] {
        pseudocode::all()
    }

    fn package(&self) -> Option<&'static str> {
        Some("godot_sdfgi")
    }

    fn demo_specs(&self) -> Option<&'static Demos> {
        Some(shot::demos())
    }

    fn text(&self, key: &'static str) -> &'static str {
        text::tr_key(key)
    }

    fn backdrop(&self, t: f32) -> Option<ShotState> {
        let mut state = shot::for_demo(Demo::Room);
        state.panel = PanelMode::Hidden;
        state.camera.yaw += 3.0 * t;
        state.camera = lab::shot::camera_drift(state.camera, 0.0, t, 1.0);
        Some(state)
    }

    fn animation(&self, demo: DemoId, t: f32) -> Vec<(&'static str, f32)> {
        shot::demo_animation(Demo::of(demo), t)
    }

    fn build_frame(&self, state: &ShotState, statics: &Statics, viewport: [f32; 2], pixel_scale: f32, time: f32) -> FrameContent {
        demos::build_frame(state, statics, viewport, pixel_scale, time)
    }

    fn scene_shading(&self) -> Option<&'static SceneShading> {
        Some(&SHADING)
    }

    fn shader_data(&self, block: &mut lab::shader_data::Block, state: &ShotState) {
        demos::shader_data(block, state)
    }

    fn dock(&self, dock: &mut Dock, ui: &mut egui::Ui) {
        controls::parameters(dock, ui);
    }

    fn click(&self, _shown: &ShotState, _state: &mut ShotState, _content: &FrameContent, _statics: &Statics, _projector: &Projector, _pixel: Vec2) -> bool {
        false
    }

    fn selftest(&self, options: &SelftestOptions) -> anyhow::Result<bool> {
        selftest::run(options)
    }
}
