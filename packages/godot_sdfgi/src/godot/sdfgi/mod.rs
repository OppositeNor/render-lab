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
pub mod gpu;
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
use lab::investigation::{Engine, Investigation, SceneShading, SelftestOptions, ShadingPass, ShadingTarget, Wording};
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

/// The frame is Godot's (shaders/sdfgi_shading.wgsl, gpu.rs): the program's
/// GPU passes (Godot's own, ported) write the probe maps; the depth pre-pass
/// writes the normal-roughness buffer; the screen gather (gi.glsl's
/// sdfgi_process) writes the AMBIENT and REFLECTION buffers; and the scene
/// pass adds them to the lab's direct light. The converged CPU world's data (sim.rs) is what the selftest's
/// kernels read.
pub static SHADING: SceneShading = SceneShading {
    module: "godot_sdfgi.shader/sdfgi-shading",
    fragment: "sdfgiForwardFS",
    data: sim::shading_data,
    verify: &["sdfgiProcessTests", "sdfgiLightTests", "sdfgiProbeTests", "sdfgiStoreTests"],
    // Godot's normal-roughness buffer (R8G8B8A8_UNORM) and the screen gather's AMBIENT and
    // REFLECTION buffers (R16G16B16A16_SFLOAT), gi.cpp:3986-3999, rfc.cpp:263-269.
    targets: &[
        ShadingTarget { name: "normalRoughness", format: wgpu::TextureFormat::Rgba8Unorm, divisor: 1 },
        ShadingTarget { name: "ambient", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
        ShadingTarget { name: "reflection", format: wgpu::TextureFormat::Rgba16Float, divisor: 1 },
    ],
    outputs: &[],
    passes: &[],
    prepass: Some("sdfgiPrepassFS"),
    prepass_outputs: &["normalRoughness"],
    early: &[ShadingPass { entry: "sdfgiGatherCS", reads: &["normalRoughness", "depth", "lightprobeData", "screenData"], writes: &["ambient", "reflection"], workgroup: [8, 8] }],
    buffers: &[],
    scene_reads: &["ambient", "reflection"],
    depth_views: None,
    volumes: &[],
    update: None,
    resolution: Some(&RESOLUTION),
    history: &[],
    // How the inspector shows the program's resources (slices through the floor of cascade 0).
    shows: &[
        ("renderAlbedo", "hash?slice=0.4&bits=1,15"),
        ("renderGeomFacing", "hash?slice=0.4"),
        ("renderEmission", "color?encoding=3&slice=0.4"),
        ("renderEmissionAniso", "hash?slice=0.4"),
        ("renderOcclusion", "ramp?slice=0.4"),
        ("renderSdf", "hash?slice=0.4"),
        ("renderSdfHalf", "hash?slice=0.4"),
        ("occlusionData", "count?slice=0.1&bits=12,4&range=0,15"),
        ("sdf", "ramp?slice=0.4&range=0,0.1"),
        ("lightData", "color?encoding=3&slice=0.4&exposure=3"),
        ("lightAniso0", "unorm?slice=0.4"),
        ("lightAniso1", "unorm?slice=0.4"),
        ("lightprobeHistory", "signed?range=-600,600"),
        ("lightprobeAverage", "signed?range=-18000,18000"),
        ("lightprobeHistoryScroll", "signed?range=-600,600"),
        ("lightprobeAverageScroll", "signed?range=-18000,18000"),
        ("lightprobeData", "color?encoding=3&exposure=1"),
    ],
    program: Some(&gpu::PROGRAM),
};

/// rendering/global_illumination/gi/use_half_resolution: the gather's buffers at
/// half the viewport's size (gi.cpp:3987-3996, `size >>= 1`).
static RESOLUTION: lab::investigation::ShadingResolution = lab::investigation::ShadingResolution { targets: &["ambient", "reflection"], divisor: gi_divisor };

fn gi_divisor(state: &lab::shot::ShotState) -> u32 {
    if state.p("giHalfResolution") > 0.5 { 2 } else { 1 }
}

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

    fn pass_about(&self, node: &str) -> Option<&'static str> {
        Some(match node {
            "sdfgiForwardFS" => tr("pass.sdfgiForwardFS"),
            "sdfgiPrepassFS" => tr("pass.sdfgiPrepassFS"),
            "sdfgiGatherCS" => tr("pass.sdfgiGatherCS"),
            "sdfgiVoxelize" => tr("pass.sdfgiVoxelize"),
            "sdfgiScroll" => tr("pass.sdfgiScroll"),
            "sdfgiScrollOcclusion" => tr("pass.sdfgiScrollOcclusion"),
            "sdfgiProbeScroll" => tr("pass.sdfgiProbeScroll"),
            "sdfgiProbeScrollStore" => tr("pass.sdfgiProbeScrollStore"),
            "sdfgiProbeStoreScroll" => tr("pass.sdfgiProbeStoreScroll"),
            "sdfgiJumpFloodInit" => tr("pass.sdfgiJumpFloodInit"),
            "sdfgiJumpFlood" => tr("pass.sdfgiJumpFlood"),
            "sdfgiJumpFloodShared" => tr("pass.sdfgiJumpFloodShared"),
            "sdfgiUpscale" => tr("pass.sdfgiUpscale"),
            "sdfgiJumpFloodFull" => tr("pass.sdfgiJumpFloodFull"),
            "sdfgiOcclusion" => tr("pass.sdfgiOcclusion"),
            "sdfgiStore" => tr("pass.sdfgiStore"),
            "sdfgiStaticLight" => tr("pass.sdfgiStaticLight"),
            "sdfgiDynamicLight" => tr("pass.sdfgiDynamicLight"),
            "sdfgiProbeProcess" => tr("pass.sdfgiProbeProcess"),
            "sdfgiProbeStore" => tr("pass.sdfgiProbeStore"),
            _ => return None,
        })
    }

    fn scene_shading(&self) -> Option<&'static SceneShading> {
        Some(&SHADING)
    }

    fn shader_data(&self, block: &mut lab::shader_data::Block, state: &ShotState) {
        demos::shader_data(block, state)
    }

    /// The probe maps converged again for the scene's settings when they are not the defaults.
    fn scene_data(&self, state: &ShotState) -> Option<lab::investigation::SceneData> {
        let settings = sim::SceneSettings::of(state);
        if settings == sim::SceneSettings::default() {
            return None;
        }
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (settings.bounce_feedback.to_bits(), settings.probe_bias.to_bits(), settings.ray_count, settings.history_size, settings.frames_to_update_light, settings.read_sky).hash(&mut hasher);
        Some(lab::investigation::SceneData { key: hasher.finish(), build: Box::new(move || sim::shading_data_for(&settings)) })
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
