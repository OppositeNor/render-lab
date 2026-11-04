//! The lighting model of O3DE's Atom renderer (O3DE 26.05, engine version 2.6.0, the commit in
//! source-index.json): how the StandardPBR material lights a pixel: direct
//! lighting from one simple point light (the metallic workflow, the GGX
//! specular BRDF and its D, G, and F, the Lambertian diffuse and its
//! response, the BRDF texture and multiscatter compensation, the light's
//! radius attenuation), then the global skylight's image-based lighting
//! (its prefiltered cubemaps, `ibl`, and the full-screen passes that apply it).
//!
//! The scene pass shades with shaders/standard_lighting.wgsl, a WGSL port of
//! O3DE's shader code (`SceneShading`); brdf.rs mirrors it on the CPU for the
//! readout and `selftest`. The demonstrations and parameters are in `shot`,
//! what each shot shows in `demos`, the dock in `controls`, the O3DE source
//! index in source-index.json (written by source_index.py), the
//! pseudo-code in pseudocode/, and the readout's formulas in `formulas`.

pub mod brdf;
pub mod cascades;
pub mod controls;
pub mod culling;
pub mod decals;
pub mod demos;
pub mod formulas;
pub mod ibl;
pub mod maps;
pub mod pseudocode;
pub mod reflections;
pub mod script;
pub mod selftest;
pub mod shot;
pub mod ssao;
pub mod sources;
pub mod text;
pub mod transmission;
pub mod values;

use lab::camera::Projector;
use lab::controls::Dock;
use lab::demo::Demos;
use lab::frame::{FrameContent, Statics};
use crate::o3de::lighting::text::tr;
use lab::investigation::{Engine, Investigation, SelftestOptions, Wording};
use lab::pseudocode::Listing;
use lab::shot::{DemoId, PanelMode, ShotState};
use lab::sources::SourceIndex;
use glam::Vec2;
use shot::Demo;

pub struct Lighting;

pub static LIGHTING: Lighting = Lighting;

static WORDING: Wording = Wording {
    engine_short: "O3DE",
    engine_tag: "O3DE",
    checkout_dir: "o3de/",
};

impl Investigation for Lighting {
    fn engine(&self) -> &'static Engine {
        &crate::o3de::ENGINE
    }

    fn id(&self) -> &'static str {
        "lighting"
    }

    fn title(&self) -> &'static str {
        tr("investigation.lighting_model")
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
        Some("o3de_lighting")
    }

    fn demo_specs(&self) -> Option<&'static Demos> {
        Some(shot::demos())
    }

    fn text(&self, key: &'static str) -> &'static str {
        text::tr_key(key)
    }

    fn backdrop(&self, t: f32) -> Option<ShotState> {
        // The room with the light circling it.
        let mut state = shot::for_demo(Demo::Room);
        state.panel = PanelMode::Hidden;
        state.set("labels", 0.0);
        for (name, value) in shot::demo_animation(Demo::Room, t) {
            state.set(name, value);
        }
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

    fn view_from(&self, content: &mut FrameContent) {
        demos::view_from(content)
    }

    fn panes(&self, state: &ShotState) -> Vec<lab::investigation::Pane> {
        demos::panes(state)
    }

    fn choices(&self) -> &'static [&'static lab::values::Choices] {
        use lab::values::Choice;
        static CHOICES: std::sync::OnceLock<[&'static lab::values::Choices; 2]> = std::sync::OnceLock::new();
        CHOICES.get_or_init(|| [brdf::ViewMode::choices(), brdf::DiffuseModel::choices()])
    }

    fn pass_about(&self, node: &str) -> Option<&'static str> {
        Some(match node {
            "o3dePrepassFS" => tr("pass.o3dePrepassFS"),
            "o3deLightCullingTilePrepareCS" => tr("pass.o3deLightCullingTilePrepareCS"),
            "o3deLightCullingCS" => tr("pass.o3deLightCullingCS"),
            "o3deLightCullingRemapCS" => tr("pass.o3deLightCullingRemapCS"),
            "o3deLinearDepthCS" => tr("pass.o3deLinearDepthCS"),
            "o3deFullscreenShadowCS" => tr("pass.o3deFullscreenShadowCS"),
            "o3deShadowBlurVerticalCS" => tr("pass.o3deShadowBlurVerticalCS"),
            "o3deShadowBlurHorizontalCS" => tr("pass.o3deShadowBlurHorizontalCS"),
            "o3deForwardFS" => tr("pass.o3deForwardFS"),
            "o3deDiffuseGlobalCS" => tr("pass.o3deDiffuseGlobalCS"),
            "o3deReflectionsCS" => tr("pass.o3deReflectionsCS"),
            "o3deReflectionCompositeCS" => tr("pass.o3deReflectionCompositeCS"),
            "o3deDepthDownsampleCS" => tr("pass.o3deDepthDownsampleCS"),
            "o3deSsaoCS" => tr("pass.o3deSsaoCS"),
            "o3deSsaoBlurVerticalCS" => tr("pass.o3deSsaoBlurVerticalCS"),
            "o3deSsaoBlurHorizontalCS" => tr("pass.o3deSsaoBlurHorizontalCS"),
            "o3deSsaoUpsampleCS" => tr("pass.o3deSsaoUpsampleCS"),
            "o3deMergeCS" => tr("pass.o3deMergeCS"),
            _ => return None,
        })
    }

    fn shading_hooks(&self) -> &'static [(&'static str, lab::investigation::ShadingHook)] {
        static HOOKS: &[(&str, lab::investigation::ShadingHook)] = &[("data", lab::investigation::ShadingHook::Data(ibl::shading_data))];
        HOOKS
    }

    fn shader_data(&self, block: &mut lab::shader_data::Block, state: &ShotState) {
        demos::shader_data(block, state)
    }

    fn dock(&self, dock: &mut Dock, ui: &mut egui::Ui) {
        controls::parameters(dock, ui);
    }

    /// A click on the scene is its script's (script/picks.lam, `scene`): the lab runs that first.
    fn click(&self, _shown: &ShotState, _state: &mut ShotState, _content: &FrameContent, _statics: &Statics, _projector: &Projector, _pixel: Vec2) -> bool {
        false
    }

    /// What its scripts read from its code (script.rs).
    fn script_natives(&self, natives: &mut lab::script::lamlet::embed::NativeRegistry) -> Result<(), lab::script::lamlet::embed::Error> {
        script::register(natives)
    }

    fn selftest(&self, options: &SelftestOptions) -> anyhow::Result<bool> {
        selftest::run(options)
    }
}
