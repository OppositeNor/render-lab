//! Screen-space reflections of Godot's Forward+ renderer (Godot 4.7.2-stable,
//! the commit in source-index.json): the half-size downsample of depth and
//! normal-roughness, the Hi-Z pyramid, the trace through it, its validation and
//! colour from the last frame, the roughness filter, the resolve to full size,
//! and the opaque pass that blends the result into the indirect specular.
//!
//! The passes are the lab program in `gpu` (shaders/ssr_passes.wgsl), the
//! opaque pass and pre-pass in shaders/ssr_shading.wgsl; `trace` mirrors them
//! on the CPU. The demonstrations and parameters are in `shot`, what each shot
//! shows in `demos`, the dock in `controls`, the Godot source index in
//! source-index.json (written by source_index.py), and the pseudo-code in
//! pseudocode/.

pub mod controls;
pub mod cpu;
pub mod demos;
pub mod gpu;
pub mod pseudocode;
pub mod selftest;
pub mod shot;
pub mod sources;
pub mod text;
pub mod trace;
pub mod values;

use crate::godot::ssr::text::tr;
use glam::Vec2;
use lab::camera::Projector;
use lab::controls::Dock;
use lab::demo::Demos;
use lab::frame::{FrameContent, Statics};
use lab::investigation::{Engine, Investigation, SceneShading, SelftestOptions, ShadingTarget, Wording};
use lab::pseudocode::Listing;
use lab::shot::{DemoId, PanelMode, ShotState};
use lab::sources::SourceIndex;
use shot::Demo;

pub struct Ssr;

pub static SSR: Ssr = Ssr;

static WORDING: Wording = Wording {
    engine_short: "Godot",
    engine_tag: "Godot",
    checkout_dir: "godot/",
};

/// The frame is Godot's: the depth pre-pass writes depth and the
/// normal-roughness buffer; the program's passes (gpu.rs) downsample them,
/// build the Hi-Z pyramid, trace, filter, and resolve; the opaque pass blends
/// the result into the indirect specular; and after it the frame is kept for
/// the next frame's trace.
pub static SHADING: SceneShading = SceneShading {
    module: "godot_ssr.shader/ssr-shading",
    fragment: "ssrForwardFS",
    data: Vec::new,
    verify: &[],
    // Godot's normal-roughness buffer, R8G8B8A8_UNORM (rfc.cpp:263-265).
    targets: &[ShadingTarget { name: "normalRoughness", format: wgpu::TextureFormat::Rgba8Unorm, divisor: 1 }],
    outputs: &[],
    passes: &[],
    prepass: Some("ssrPrepassFS"),
    prepass_outputs: &["normalRoughness"],
    early: &[],
    buffers: &[],
    // The resolved reflections, a program image the scene pass reads at group 3.
    scene_reads: &["final"],
    depth_views: None,
    volumes: &[],
    update: None,
    resolution: None,
    history: &[],
    shows: &[("hiz", "ramp?range=0,0.04"), ("mipLevel", "ramp?range=0,0.4"), ("bestFit", "ramp?range=0.5,1")],
    program: Some(&gpu::PROGRAM),
};

impl Investigation for Ssr {
    fn engine(&self) -> &'static Engine {
        &crate::godot::ENGINE
    }

    fn id(&self) -> &'static str {
        "ssr"
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
        Some("godot_ssr")
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
            "ssrPrepassFS" => tr("pass.ssrPrepassFS"),
            "ssrForwardFS" => tr("pass.ssrForwardFS"),
            "ssrBestFit" => tr("pass.ssrBestFit"),
            "ssrDownsample" => tr("pass.ssrDownsample"),
            "ssrHiz" => tr("pass.ssrHiz"),
            "ssrTrace" => tr("pass.ssrTrace"),
            "ssrFilter" => tr("pass.ssrFilter"),
            "ssrResolve" => tr("pass.ssrResolve"),
            "ssrCopyLastFrame" => tr("pass.ssrCopyLastFrame"),
            _ => return None,
        })
    }

    fn scene_shading(&self) -> Option<&'static SceneShading> {
        Some(&SHADING)
    }

    fn dock(&self, dock: &mut Dock, ui: &mut egui::Ui) {
        controls::parameters(dock, ui);
    }

    /// A click on the scene picks the SSR pixel under it (half size): the trace demonstrations follow it.
    /// While the depth map is shown as a height field, the pointer turns the field
    /// itself (its turn and tilt; the wheel zooms it toward P), so the camera that
    /// renders the depth map keeps its place.
    fn pointer_values(&self, shown: &ShotState) -> Option<lab::investigation::PointerValues> {
        let field = matches!(Demo::of(shown.demo), Demo::Pyramid | Demo::Trace) && shown.p("fieldShow") > 0.5 && shown.p("fieldMorph") > 0.05 && shown.p("profileShow") < 0.5;
        field.then_some(lab::investigation::PointerValues { yaw: "fieldTurn", pitch: "fieldTilt", zoom: Some("fieldZoom") })
    }

    fn click(&self, shown: &ShotState, state: &mut ShotState, _content: &FrameContent, _statics: &Statics, _projector: &Projector, pixel: Vec2) -> bool {
        if Demo::of(shown.demo) != Demo::Trace {
            return false;
        }
        state.set("pickX", (pixel.x / 2.0).floor());
        state.set("pickY", (pixel.y / 2.0).floor());
        true
    }

    fn selftest(&self, options: &SelftestOptions) -> anyhow::Result<bool> {
        selftest::run(options)
    }
}
