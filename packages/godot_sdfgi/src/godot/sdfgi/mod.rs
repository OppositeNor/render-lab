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
use lab::investigation::{Engine, Investigation, SelftestOptions, Wording};
use lab::pseudocode::Listing;
use lab::shot::{PanelMode, ShotState};
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
    fn images(&self) -> &'static [ImageSpec] {
        IMAGES
    }
    fn annotations(&self) -> &'static [lab::utility::annotations::AnnotationSpec] {
        ANNOTATIONS
    }
    /// The sizes chapter 2 says (`godot_sdfgi_natives`): the cascade count the resources are made for, and a
    /// cascade's cells along an axis.
    fn script_natives(&self, r: &mut lab::script::lamlet::embed::NativeRegistry) -> Result<(), lab::script::lamlet::embed::Error> {
        r.function("godot_sdfgi/cascades", |_, ()| Ok(gpu::CASCADES as i64)).pure().doc("Fn([], Int). `sdfgi_cascades`, the cascade count.").register()?;
        r.function("godot_sdfgi/cascade-cells", |_, ()| Ok(model::CASCADE_SIZE as i64)).pure().doc("Fn([], Int). A cascade's cells along an axis.").register()?;
        // The numbers chapter 2 says of the default settings, so the narration says what the code holds. Metres are
        // rounded to a tenth, as said.
        let tenth = |x: f64| (x * 10.0).round() / 10.0;
        let cell = model::Settings::default().min_cell_size as f64;
        r.function("godot_sdfgi/cell-centimetres", move |_, ()| Ok((cell * 100.0).round() as i64)).pure().doc("Fn([], Int). The finest cascade's cell, in centimetres (`sdfgi_min_cell_size`).").register()?;
        r.function("godot_sdfgi/first-span", move |_, ()| Ok(tenth(cell * model::CASCADE_SIZE as f64))).pure().doc("Fn([], Float). How far the finest cascade spans, in metres, to a tenth.").register()?;
        r.function("godot_sdfgi/probe-gaps", |_, ()| Ok(model::PROBE_DIVISOR as i64)).pure().doc("Fn([], Int). The gaps between probes along a cascade's axis (`PROBE_DIVISOR`).").register()?;
        r.function("godot_sdfgi/probe-cells", |_, ()| Ok(model::PROBE_CELLS as i64)).pure().doc("Fn([], Int). Cells from one probe to the next.").register()?;
        r.function("godot_sdfgi/drag-margin", |_, ()| Ok(((model::CASCADE_SIZE / model::PROBE_DIVISOR) / 2) as i64)).pure().doc("Fn([], Int). How many cells the camera may stray before a cascade scrolls (`drag_margin`).").register()?;
        r.function("godot_sdfgi/probe-axis-count", |_, ()| Ok(model::PROBE_AXIS_COUNT as i64)).pure().doc("Fn([], Int). Probes along a cascade's axis (`probe_axis_count`).").register()?;
        r.function("godot_sdfgi/probe-spacing", move |_, ()| Ok(tenth(cell * model::PROBE_CELLS as f64))).pure().doc("Fn([], Float). Metres between the finest cascade's probes, to a tenth.").register()?;
        Ok(())
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

    fn shading_hooks(&self) -> &'static [(&'static str, lab::investigation::ShadingHook)] {
        use lab::investigation::ShadingHook;
        static HOOKS: &[(&str, ShadingHook)] = &[
            ("data", ShadingHook::Data(sim::shading_data)),
            ("gi-divisor", ShadingHook::Divisor(gi_divisor)),
            ("plan", ShadingHook::Program { init: gpu::init, plan: gpu::plan }),
        ];
        HOOKS
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
