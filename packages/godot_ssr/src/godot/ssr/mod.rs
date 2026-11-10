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
use lab::investigation::{Engine, Investigation, SelftestOptions, Wording};
use lab::pseudocode::Listing;
use lab::shot::{PanelMode, ShotState};
use lab::sources::SourceIndex;
use shot::Demo;

pub struct Ssr;

pub static SSR: Ssr = Ssr;

static WORDING: Wording = Wording {
    engine_short: "Godot",
    engine_tag: "Godot",
    checkout_dir: "godot/",
};

impl Investigation for Ssr {
    fn engine(&self) -> &'static Engine {
        &crate::godot::ENGINE
    }

    fn id(&self) -> &'static str {
        "ssr"
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
        Some("godot_ssr")
    }

    fn demo_specs(&self) -> Option<&'static Demos> {
        Some(shot::demos())
    }

    fn text(&self, key: &'static str) -> &'static str {
        text::tr_key(key)
    }

    fn backdrop(&self, t: f32) -> Option<ShotState> {
        // The title screen's backdrop: the room without its panel, the camera turning slowly and drifting.
        let mut state = shot::for_demo(Demo::Room);
        state.panel = PanelMode::Hidden;
        state.camera.yaw += 3.0 * t;
        state.camera = lab::shot::camera_drift(state.camera, 0.0, t, 1.0);
        Some(state)
    }

    fn movable_objects(&self) -> bool {
        true
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
            "ssrPrepassFS" => tr("pass-ssrPrepassFS"),
            "ssrForwardFS" => tr("pass-ssrForwardFS"),
            "ssrBestFit" => tr("pass-ssrBestFit"),
            "ssrDownsample" => tr("pass-ssrDownsample"),
            "ssrHiz" => tr("pass-ssrHiz"),
            "ssrTrace" => tr("pass-ssrTrace"),
            "ssrFilter" => tr("pass-ssrFilter"),
            "ssrResolve" => tr("pass-ssrResolve"),
            "ssrCopyLastFrame" => tr("pass-ssrCopyLastFrame"),
            _ => return None,
        })
    }

    fn shading_hooks(&self) -> &'static [(&'static str, lab::investigation::ShadingHook)] {
        gpu::HOOKS
    }

    fn dock(&self, dock: &mut Dock, ui: &mut egui::Ui) {
        controls::parameters(dock, ui);
    }

    /// A click on the scene picks the SSR pixel under it (half size): the trace demonstrations follow it.
    /// The depth map as a height field and the trace as a cross-section, each a
    /// view of its own over the main one (demos.rs `views`).
    fn views(&self, state: &ShotState) -> Vec<lab::investigation::OverView> {
        demos::views(state)
    }

    /// A view over the main one draws on the canvas (lab/src/view.rs); the main view its demonstration's scene.
    fn scene(&self, state: &ShotState) -> lab::scene::SceneId {
        if lab::view::name().is_some() {
            // A view over the main one draws on the empty canvas.
            lab::scene::SceneId::CANVAS
        } else {
            self.demo_scene(state.demo)
        }
    }

    /// A click on the scene is its script's (script/logic.lam `scene`: in the trace demonstration, the
    /// half-resolution pixel under it).
    fn click(
        &self,
        _shown: &ShotState,
        _state: &mut ShotState,
        _content: &FrameContent,
        _statics: &Statics,
        _projector: &Projector,
        _pixel: Vec2,
    ) -> bool {
        false
    }

    /// The fade's ceiling the narration says (chapter 5, the summary): with the default fades, the most the
    /// screen-edge and length fades leave of a pixel's validity, and the ray length where they do, in hundredths.
    fn script_natives(
        &self,
        natives: &mut lab::script::lamlet::embed::NativeRegistry,
    ) -> Result<(), lab::script::lamlet::embed::Error> {
        fn peak() -> (f32, f32) {
            // Sample the fade at 2000 ray lengths across the screen, and keep the one where it is highest.
            let d = shot::demos();
            let (fade_in, fade_out) = (
                d.param_default("ssrFadeIn").unwrap_or(0.15),
                d.param_default("ssrFadeOut").unwrap_or(2.0),
            );
            (1..2000)
                .map(|k| k as f32 / 2000.0)
                .map(|l| (l, trace::fade(l, fade_in, fade_out)))
                .fold((0.0, 0.0), |a, b| {
                    if b.1 > a.1 {
                        b
                    } else {
                        a
                    }
                })
        }
        natives
            .function("godot_ssr/fade-peak", |_, ()| {
                Ok((peak().1 * 100.0).round() as i64)
            })
            .pure()
            .doc("Fn([], Int). The default fades' peak, in hundredths of validity.")
            .register()?;
        natives.function("godot_ssr/fade-peak-length", |_, ()| Ok((peak().0 * 100.0).round() as i64)).pure().doc("Fn([], Int). The ray length where the default fades peak, in hundredths of the screen.").register()?;
        Ok(())
    }

    fn selftest(&self, options: &SelftestOptions) -> anyhow::Result<bool> {
        selftest::run(options)
    }
}
