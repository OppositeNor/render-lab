//! The investigation's values: every number a demonstration, the dock, a
//! timeline, or a story sets, its default, and what it is, as a struct the lab
//! reaches by key (`#[derive(Values)]`, lab/src/values.rs). A shot stores them
//! by key (`state.p("key")`); a demonstration's file overrides some
//! (lab/src/demo.rs). A field's `label` names the dock group that shows it
//! (controls.rs). The Environment's SSR settings (ssrEnabled, ssrMaxSteps,
//! ssrFadeIn, ssrFadeOut, ssrDepthTolerance) are declared by their tags in
//! shaders/ssr_shading.wgsl.

use lab::palette;

#[derive(Clone, Debug, PartialEq, lab::Values, serde::Serialize, serde::Deserialize)]
#[values(package = "godot_ssr", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct SsrValues {
    /// The floor's roughness (its material; the study room's is 1).
    #[value(default = 1.0, range = 0.0..=1.0, unit = plain, text = "controls.floor_roughness", color = palette::THEME_ACCENT, label(surface), help = "help.floorRoughness")]
    pub floor_roughness: f32,
    /// The teapot's roughness.
    #[value(default = 0.35, range = 0.0..=1.0, unit = plain, text = "controls.teapot_roughness", color = palette::THEME_ACCENT, label(surface), help = "help.teapotRoughness")]
    pub teapot_roughness: f32,
    /// The picked SSR pixel (half size), x and y; below 0, the pixel P in front of the teapot.
    #[value(default = -1.0, range = -1.0..=1024.0, unit = index, text = "controls.pick_x", color = palette::SELECTED, label(pick), help = "help.pickX")]
    pub pick_x: f32,
    #[value(default = -1.0, range = -1.0..=1024.0, unit = index, text = "controls.pick_y", color = palette::SELECTED, label(pick), help = "help.pickY")]
    pub pick_y: f32,
    /// The trace's iterations shown, from the first (0: none yet; at the end, every one).
    #[value(default = 64.0, range = 0.0..=64.0, unit = index, text = "controls.trace_step", color = palette::SELECTED, label(pick), help = "help.traceStep")]
    pub trace_step: f32,
    /// The depth pyramid's level shown (0: the SSR size).
    #[value(default = 0.0, range = 0.0..=9.0, unit = index, text = "controls.hiz_level", color = palette::THEME_ACCENT, label(pick), help = "help.hizLevel")]
    pub hiz_level: f32,
}
