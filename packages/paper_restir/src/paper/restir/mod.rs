//! ReSTIR, Reservoir-based SpatioTemporal Importance Resampling (Bitterli,
//! Wyman, Pharr, Shirley, Lefohn, and Jarosz 2020, "Spatiotemporal reservoir
//! resampling for real-time ray tracing with dynamic direct lighting", ACM TOG
//! 39(4), Article 148), implemented as the paper describes it: streaming RIS of
//! each pixel's light candidates, visibility reuse, temporal reuse, spatial
//! reuse, biased (Alg. 4) or unbiased (Alg. 6).
//!
//! The passes are the lab program in `gpu` (shaders/restir_passes.wgsl), the
//! scene pass and what the passes share in shaders/restir_shading.wgsl; `cpu`
//! mirrors them. The room's lights are `lights`, the demonstrations `shot` and
//! `demos`, the dock `controls`, the paper's citation index source-index.json
//! (written by source_index.py from the PDF), and the pseudo-code pseudocode/.

pub mod controls;
pub mod cpu;
pub mod demos;
pub mod gpu;
pub mod lights;
pub mod pseudocode;
pub mod selftest;
pub mod shot;
pub mod sources;
pub mod text;
pub mod values;

use crate::paper::restir::text::tr;
use lab::controls::Dock;
use lab::demo::Demos;
use lab::frame::{FrameContent, Statics};
use lab::investigation::{Engine, Investigation, SelftestOptions, Wording};
use lab::pseudocode::Listing;
use lab::shot::{PanelMode, ShotState};
use lab::sources::SourceIndex;
use shot::Demo;

pub struct Restir;

pub static RESTIR: Restir = Restir;

static WORDING: Wording = Wording {
    engine_short: "Paper",
    engine_tag: "ReSTIR",
    checkout_dir: "",
};

impl Investigation for Restir {
    fn engine(&self) -> &'static Engine {
        &crate::paper::PAPERS
    }

    fn id(&self) -> &'static str {
        "restir"
    }

    fn title(&self) -> &'static str {
        tr("investigation-title")
    }

    fn summary(&self) -> &'static str {
        tr("investigation-summary")
    }

    fn wording(&self) -> &'static Wording {
        &WORDING
    }

    fn footer(&self) -> &'static str {
        tr("investigation-footer")
    }

    fn sources(&self) -> &'static SourceIndex {
        sources::INDEX.get()
    }

    fn listings(&self) -> &'static [Listing] {
        pseudocode::all()
    }

    fn package(&self) -> Option<&'static str> {
        Some("paper_restir")
    }

    fn demo_specs(&self) -> Option<&'static Demos> {
        Some(shot::demos())
    }

    fn text(&self, key: &'static str) -> &'static str {
        text::tr_key(key)
    }

    fn backdrop(&self, t: f32) -> Option<ShotState> {
        // The title screen's backdrop: the room, its lights moving, the camera turning slowly.
        let mut state = shot::for_demo(Demo::Room);
        state.panel = PanelMode::Hidden;
        state.camera.yaw += 3.0 * t;
        state.set("lightPhase", (t / 60.0).fract());
        Some(state)
    }

    fn build_frame(
        &self,
        state: &ShotState,
        statics: &Statics,
        viewport: [f32; 2],
        pixel_scale: f32,
        time: f32,
    ) -> FrameContent {
        demos::build_frame(state, statics, viewport, pixel_scale, time)
    }

    fn pass_about(&self, node: &str) -> Option<&'static str> {
        Some(match node {
            "restirPrepassFS" => tr("pass-restirPrepassFS"),
            "restirForwardFS" => tr("pass-restirForwardFS"),
            "restirCandidates" => tr("pass-restirCandidates"),
            "restirVisibility" => tr("pass-restirVisibility"),
            "restirTemporal" => tr("pass-restirTemporal"),
            "restirSpatialTA" | "restirSpatialAB" | "restirSpatialBA" => tr("pass-restirSpatial"),
            "restirKeep" => tr("pass-restirKeep"),
            "restirShade" => tr("pass-restirShade"),
            _ => return None,
        })
    }

    fn shading_hooks(&self) -> &'static [(&'static str, lab::investigation::ShadingHook)] {
        gpu::HOOKS
    }

    /// A click on the scene picks the pixel under it as P, which the per-pixel demonstrations follow.
    fn click(
        &self,
        _shown: &ShotState,
        state: &mut ShotState,
        content: &FrameContent,
        _statics: &Statics,
        _projector: &lab::camera::Projector,
        pixel: glam::Vec2,
    ) -> bool {
        let size = content.uniforms.camera.viewport_size;
        state.set("pickU", (pixel.x / size[0]).clamp(0.0, 1.0));
        state.set("pickV", (pixel.y / size[1]).clamp(0.0, 1.0));
        true
    }

    fn dock(&self, dock: &mut Dock, ui: &mut egui::Ui) {
        controls::parameters(dock, ui);
    }

    fn selftest(&self, options: &SelftestOptions) -> anyhow::Result<bool> {
        selftest::run(options)
    }
}
