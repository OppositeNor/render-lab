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
    /// The SSR buffer's size (half size) that pickX and pickY were chosen at; 0, the current one. Another size
    /// moves the pick to the same direction from the camera (its vertical field of view is fixed), so a story's
    /// pick lands on the same spot in the viewer as in its video.
    #[value(default = 0.0, range = 0.0..=2048.0, unit = index, text = "controls.pick_width", color = palette::SELECTED, label(pick), help = "help.pickWidth")]
    pub pick_width: f32,
    #[value(default = 0.0, range = 0.0..=2048.0, unit = index, text = "controls.pick_height", color = palette::SELECTED, label(pick), help = "help.pickHeight")]
    pub pick_height: f32,
    /// The trace's iterations shown, from the first (0: none yet; at the end, every one).
    #[value(default = 64.0, range = 0.0..=64.0, unit = index, text = "controls.trace_step", color = palette::SELECTED, label(pick), help = "help.traceStep")]
    pub trace_step: f32,
    /// The depth pyramid's level shown (0: the SSR size).
    #[value(default = 0.0, range = 0.0..=9.0, unit = index, text = "controls.hiz_level", color = palette::THEME_ACCENT, label(pick), help = "help.hizLevel")]
    pub hiz_level: f32,
    /// The depth map drawn as a height field (lab::utility::height_field): how far it has faded in over the
    /// room (0: the room alone; 1: the field, on a dark ground).
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls.field_show", color = palette::THEME_ACCENT, label(field), help = "help.fieldShow")]
    pub field_show: f32,
    /// The field from flat (0: the depth map as the screen shows it) to tilted back with its columns risen (1).
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls.field_morph", color = palette::THEME_ACCENT, label(field), help = "help.fieldMorph")]
    pub field_morph: f32,
    /// The pyramid level the field's columns are of; between two levels, each 2 x 2 rises to its tallest and merges.
    #[value(default = 0.0, range = 0.0..=9.0, unit = plain, text = "controls.field_level", color = palette::THEME_ACCENT, label(field), help = "help.fieldLevel")]
    pub field_level: f32,
    /// From the whole field (0) to a close window around P (1).
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls.field_zoom", color = palette::THEME_ACCENT, label(field), help = "help.fieldZoom")]
    pub field_zoom: f32,
    /// The tilted field turned about its own up, in degrees.
    #[value(default = 0.0, range = -180.0..=180.0, unit = deg, text = "controls.field_turn", color = palette::THEME_ACCENT, label(field), help = "help.fieldTurn")]
    pub field_turn: f32,
    /// How far the risen field lies back from facing the camera, in degrees (0: seen from straight above it).
    #[value(default = 50.0, range = 0.0..=85.0, unit = deg, text = "controls.field_tilt", color = palette::THEME_ACCENT, label(field), help = "help.fieldTilt")]
    pub field_tilt: f32,
    /// The trace as a cross-section along the ray, over the view (0: hidden; 1: shown): bars of the level tested,
    /// the ray, and where it leaves the cell and reaches its depth.
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls.profile_show", color = palette::THEME_ACCENT, label(field), help = "help.profileShow")]
    pub profile_show: f32,
}
