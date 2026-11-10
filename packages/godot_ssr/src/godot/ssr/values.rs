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
    #[value(default = 1.0, range = 0.0..=1.0, unit = plain, text = "controls-floor_roughness", color = palette::THEME_ACCENT, label(surface), help = "help-floorRoughness")]
    pub floor_roughness: f32,
    /// The teapot's roughness.
    #[value(default = 0.35, range = 0.0..=1.0, unit = plain, text = "controls-teapot_roughness", color = palette::THEME_ACCENT, label(surface), help = "help-teapotRoughness")]
    pub teapot_roughness: f32,
    /// The picked SSR pixel (half size), x and y; below 0, the pixel P in front of the teapot.
    #[value(default = -1.0, range = -1.0..=1024.0, unit = index, text = "controls-pick_x", color = palette::SELECTED, label(pick), help = "help-pickX")]
    pub pick_x: f32,
    #[value(default = -1.0, range = -1.0..=1024.0, unit = index, text = "controls-pick_y", color = palette::SELECTED, label(pick), help = "help-pickY")]
    pub pick_y: f32,
    /// The SSR buffer's size (half size) that pickX and pickY were chosen at; 0, the current one. Another size
    /// moves the pick to the same direction from the camera (its vertical field of view is fixed), so a story's
    /// pick lands on the same spot in the viewer as in its video.
    #[value(default = 0.0, range = 0.0..=2048.0, unit = index, text = "controls-pick_width", color = palette::SELECTED, label(pick), help = "help-pickWidth")]
    pub pick_width: f32,
    #[value(default = 0.0, range = 0.0..=2048.0, unit = index, text = "controls-pick_height", color = palette::SELECTED, label(pick), help = "help-pickHeight")]
    pub pick_height: f32,
    /// The trace's iterations shown, from the first (0: none yet; at the end, every one).
    #[value(default = 64.0, range = 0.0..=64.0, unit = index, text = "controls-trace_step", color = palette::SELECTED, label(pick), help = "help-traceStep")]
    pub trace_step: f32,
    /// How far the geometric normal at P is built (the inputs): 0 not shown; 1 the depths two pixels to each side
    /// of P on each axis; 2 each side's straight-line prediction of P's depth; 3 the side kept; 4 the tangents
    /// along the surface; 5 their cross product, the normal.
    #[value(default = 0.0, range = 0.0..=5.0, unit = index, text = "controls-geo_step", color = palette::SELECTED, label(pick), help = "help-geoStep")]
    pub geo_step: f32,
    /// How far P's normal is packed into three bytes (the inputs): 0 not shown; 1 the normal and the cube from
    /// minus one to one; 2 the normal scaled to touch the cube; 3 shortened to the stored length; 4 the alpha
    /// byte, where the roughness lies in it.
    #[value(default = 0.0, range = 0.0..=4.0, unit = index, text = "controls-encode_step", color = palette::SELECTED, label(pick), help = "help-encodeStep")]
    pub encode_step: f32,
    /// P's alpha byte as a moving object would store it (0: still, the scaled roughness; 1: moving, one minus it).
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-alpha_dynamic", color = palette::INCIDENT, label(pick), help = "help-alphaDynamic")]
    pub alpha_dynamic: f32,
    /// How much of the line from the camera to P is drawn (the inputs): P's place on screen and its depth, back
    /// through the projection to its point in the room.
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-unproject", color = palette::SELECTED, label(pick), help = "help-unproject")]
    pub unproject: f32,
    /// How far the mirror direction at P is drawn (the inputs): the view ray arriving at P, reflected about P's
    /// stored normal.
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-mirror", color = palette::REFLECTED, label(pick), help = "help-mirror")]
    pub mirror: f32,
    /// The trace's start at P (the trace): 0 not shown; 1 P's stored normal and the view ray reflected about it;
    /// 2 the true (geometric) normal beside it, and the push off the surface over the angle between the two.
    #[value(default = 0.0, range = 0.0..=2.0, unit = index, text = "controls-start_step", color = palette::SELECTED, label(pick), help = "help-startStep")]
    pub start_step: f32,
    /// An angle between the stored and the true normal marked on the push chart, in degrees (0: P's own only).
    #[value(default = 0.0, range = 0.0..=90.0, unit = deg, text = "controls-push_tilt", color = palette::SELECTED, label(pick), help = "help-pushTilt")]
    pub push_tilt: f32,
    /// P's hit in the room (the hit): 0 not shown; 1 P's ray to the point it hit; 2 also the line from that point
    /// to the camera, the way the trace takes it back onto a screen to read its colour.
    #[value(default = 0.0, range = 0.0..=2.0, unit = index, text = "controls-hit_show", color = palette::REFLECTED, label(pick), help = "help-hitShow")]
    pub hit_show: f32,
    /// The brighter of the two texels the compression chart averages, its luminance.
    #[value(default = 6.0, range = 0.5..=16.0, unit = plain, text = "controls-bright_texel", color = palette::INCIDENT, label(pick), help = "help-brightTexel")]
    pub bright_texel: f32,
    /// The direction the best-fit demonstration packs instead of P's normal (the inputs), in degrees from facing
    /// the camera toward its right; below 0, P's own normal. Its chart shows the table's error over the length.
    #[value(default = -1.0, range = -1.0..=90.0, unit = deg, text = "controls-fit_angle", color = palette::SELECTED, label(pick), help = "help-fitAngle")]
    pub fit_angle: f32,
    /// A stored length marked on the best-fit chart (a click on the chart sets it), its 8-bit error read out;
    /// 0, none.
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-fit_len", color = palette::REFLECTED, label(pick), help = "help-fitLen")]
    pub fit_len: f32,
    /// A ray length marked on the fade chart (a click on the chart sets it), its fade read out; below 0, none.
    #[value(default = -1.0, range = -1.0..=1.0, unit = plain, text = "controls-fade_probe", color = palette::REFLECTED, label(pick), help = "help-fadeProbe")]
    pub fade_probe: f32,
    /// Whether the trace's back-face test applies (1, Godot's) or is left out (0), to see what it rejects.
    #[value(default = 1.0, range = 0.0..=1.0, unit = plain, text = "controls-back_rule", color = palette::REFLECTED, label(pick), help = "help-backRule")]
    pub back_rule: f32,
    /// A hit taken to face away from P's ray, this many SSR pixels along it from the start (the hit); below 0,
    /// not shown. Inside the two-pixel box around the start it is rejected; beyond it, never tested.
    #[value(default = -1.0, range = -1.0..=8.0, unit = plain, text = "controls-back_sweep", color = palette::REFLECTED, label(pick), help = "help-backSweep")]
    pub back_sweep: f32,
    /// Last frame's camera, this one turned back by this many degrees about its target (the hit): P's hit
    /// reprojected into it, where last frame's image is read; 0, not shown.
    #[value(default = 0.0, range = 0.0..=15.0, unit = deg, text = "controls-last_yaw", color = palette::INCIDENT, label(pick), help = "help-lastYaw")]
    pub last_yaw: f32,
    /// P's group of eight by eight threads (the pyramid and the trace): 0 not shown; 1 the group and P's thread;
    /// 2 each thread's iterations of the trace; 3 the longest, which the whole group waits for.
    #[value(default = 0.0, range = 0.0..=3.0, unit = index, text = "controls-lane_step", color = palette::SELECTED, label(pick), help = "help-laneStep")]
    pub lane_step: f32,
    /// The chart beside P's validity (the hit): 0 the two fades over the ray's length; 1 the confidence over the
    /// gap between the ray's end and the surface; 2 the compression of a colour by its luminance.
    #[value(default = 0.0, range = 0.0..=2.0, unit = index, text = "controls-hit_chart", color = palette::THEME_ACCENT, label(pick), help = "help-hitChart")]
    pub hit_chart: f32,
    /// The depth pyramid's level shown (0: the SSR size).
    #[value(default = 0.0, range = 0.0..=9.0, unit = index, text = "controls-hiz_level", color = palette::THEME_ACCENT, label(pick), help = "help-hizLevel")]
    pub hiz_level: f32,
    /// The depth map drawn as a height field (lab::utility::height_field): how far it has faded in over the
    /// room (0: the room alone; 1: the field, on a dark ground).
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-field_show", color = palette::THEME_ACCENT, label(field), help = "help-fieldShow")]
    pub field_show: f32,
    /// The field from flat (0: the depth map as the screen shows it) to tilted back with its columns risen (1).
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-field_morph", color = palette::THEME_ACCENT, label(field), help = "help-fieldMorph")]
    pub field_morph: f32,
    /// The pyramid level the field's columns are of; between two levels, each 2 x 2 rises to its tallest and merges.
    #[value(default = 0.0, range = 0.0..=9.0, unit = plain, text = "controls-field_level", color = palette::THEME_ACCENT, label(field), help = "help-fieldLevel")]
    pub field_level: f32,
    /// From the whole field (0) to a close window around P (1).
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-field_zoom", color = palette::THEME_ACCENT, label(field), help = "help-fieldZoom")]
    pub field_zoom: f32,
    /// The tilted field turned about its own up, in degrees.
    #[value(default = 0.0, range = -180.0..=180.0, unit = deg, text = "controls-field_turn", color = palette::THEME_ACCENT, label(field), help = "help-fieldTurn")]
    pub field_turn: f32,
    /// How far the risen field lies back from facing the camera, in degrees (0: seen from straight above it).
    #[value(default = 50.0, range = 0.0..=85.0, unit = deg, text = "controls-field_tilt", color = palette::THEME_ACCENT, label(field), help = "help-fieldTilt")]
    pub field_tilt: f32,
    /// The trace as a cross-section along the ray, over the view (0: hidden; 1: shown): bars of the level tested,
    /// the ray, and where it leaves the cell and reaches its depth.
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-profile_show", color = palette::THEME_ACCENT, label(field), help = "help-profileShow")]
    pub profile_show: f32,
}
