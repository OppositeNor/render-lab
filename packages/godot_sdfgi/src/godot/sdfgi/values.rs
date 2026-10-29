//! The investigation's values: every number a demonstration, the dock, a
//! timeline, or a story sets, its default, and what it is, as a struct the lab
//! reaches by key (`#[derive(Values)]`, lab/src/values.rs). A shot stores them
//! by key (`state.p("key")`); a demonstration's file overrides some
//! (lab/src/demo.rs). A field's `label` names the dock group that shows it
//! (controls.rs). **A value's default must change nothing**, so existing frames
//! stay identical. The scene shading's values (sdfgiMix, sdfgiEnergy,
//! normalBias) are declared by their tags in shaders/sdfgi_shading.wgsl.

use lab::palette;

#[derive(Clone, Debug, PartialEq, lab::Values, serde::Serialize, serde::Deserialize)]
#[values(package = "godot_sdfgi", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct SdfgiValues {
    /// How far the camera has walked along +x from where SDFGI was created, metres; the cascades follow it (`SDFGI::update`).
    #[value(default = 0.0, range = -12.0..=12.0, unit = m, text = "controls.camera_walk", color = palette::THEME_ACCENT, label(cascades))]
    pub camera_x: f32,
    /// How many of the cascades' boxes are drawn, from the finest (0 to 4).
    #[value(default = 0.0, range = 0.0..=4.0, unit = index, text = "controls.cascades_shown", color = palette::THEME_ACCENT, label(cascades))]
    pub cascades_shown: f32,
    /// The environment's sdfgi_y_scale for the boxes drawn: 0 = 50 %, 1 = 75 % (the default), 2 = 100 %; fractions blend.
    #[value(default = 1.0, range = 0.0..=2.0, unit = index, text = "controls.y_scale", color = palette::THEME_ACCENT, label(cascades))]
    pub y_scale: f32,
    /// The probes of cascade 0, as points (the ones inside the room brighter).
    #[value(default = 0.0, toggle)]
    pub probes: f32,
    /// The dirty slab of the walk and the regions handed to the culling.
    #[value(default = 0.0, toggle)]
    pub regions: f32,
    /// A top view of the walk's first row of cells (the scroll worked step by step).
    #[value(default = 0.0, toggle)]
    pub scroll_view: f32,
    /// Cascade 0's solid voxels, as points in their albedo.
    #[value(default = 0.0, toggle)]
    pub voxels: f32,
    /// How many of the voxelizer's three passes (x, y, z views) have run (0 to 3).
    #[value(default = 3.0, range = 0.0..=3.0, unit = index, text = "controls.voxel_passes", color = palette::THEME_ACCENT, label(voxels))]
    pub voxel_passes: f32,
    /// Voxels are drawn only below this height, metres (a cut through the room).
    #[value(default = 4.0, range = 0.0..=4.0, unit = m, text = "controls.cut_height", color = palette::THEME_ACCENT, label(voxels))]
    pub cut_height: f32,
    /// The distance field's slice (an inset): its height, metres.
    #[value(default = 0.62, range = 0.0..=3.0, unit = m, text = "controls.slice_height", color = palette::OUTSIDE, label(field))]
    pub slice_height: f32,
    /// The slice inset shown (0 none, 1 distance field, 2 jump-flood seeds, 3 occlusion of one probe).
    #[value(default = 0.0, range = 0.0..=3.0, unit = index, text = "controls.slice", color = palette::OUTSIDE, label(field))]
    pub slice: f32,
    /// The jump flood's step shown: 0 the half-size seeds, 1 to 6 the half-size passes, 7 the upscale, 8 the full-size pass.
    #[value(default = 8.0, range = 0.0..=8.0, unit = index, text = "controls.flood_step", color = palette::OUTSIDE, label(field))]
    pub flood_step: f32,
    /// The occlusion shells shown around the probe (|x| + |y| + |z| up to this, 0 to 22).
    #[value(default = 22.0, range = 0.0..=22.0, unit = index, text = "controls.occlusion_shell", color = palette::OUTSIDE, label(field))]
    pub occlusion_shell: f32,
    /// The solid cells, coloured by the light the light pass stored in them.
    #[value(default = 0.0, toggle)]
    pub cell_light: f32,
    /// Which quarter of the cells this frame's light pass updates (0 to 3; 4 shows all).
    #[value(default = 4.0, range = 0.0..=4.0, unit = index, text = "controls.light_phase", color = palette::INCIDENT, label(light))]
    pub light_phase: f32,
    /// The extra omni light (for the light pass's worked cell): on or off.
    #[value(default = 0.0, toggle)]
    pub extra_light: f32,
    /// Its height above the floor, metres.
    #[value(default = 2.0, range = 0.2..=2.9, unit = m, text = "controls.light_height", color = palette::INCIDENT, label(light))]
    pub light_height: f32,
    /// Its x, metres.
    #[value(default = 1.6, range = -2.8..=2.8, unit = m, text = "controls.light_x", color = palette::INCIDENT, label(light))]
    pub light_x: f32,
    /// Its z, metres.
    #[value(default = 1.0, range = -2.8..=2.8, unit = m, text = "controls.light_z", color = palette::INCIDENT, label(light))]
    pub light_z: f32,
    /// The worked floor cell's x and z, metres; below -9 it lies under the light.
    #[value(default = -10.0, range = -10.0..=2.8, unit = m, text = "controls.cell_x", color = palette::SELECTED, label(light))]
    pub cell_x: f32,
    #[value(default = 0.0, range = -2.8..=2.8, unit = m, text = "controls.cell_z", color = palette::SELECTED, label(light))]
    pub cell_z: f32,
    /// Its bake mode: 0 Dynamic (every update), 1 Static (written into the cells when a region is built).
    #[value(default = 0.0, range = 0.0..=1.0, unit = index, text = "controls.light_bake", color = palette::INCIDENT, label(light))]
    pub light_bake: f32,
    /// Scrolls of cascade 0 after a static light was baked (0 to 3): the stored static light of a kept cell.
    #[value(default = 0.0, range = 0.0..=3.0, unit = index, text = "controls.scrolls", color = palette::INCIDENT, label(light))]
    pub scrolls: f32,
    /// The environment's sdfgi_bounce_feedback the readout's cells use (the room's light keeps the default 0.5).
    #[value(default = 0.5, range = 0.0..=1.99, text = "controls.bounce_feedback", color = palette::INCIDENT, label(light))]
    pub bounce_feedback: f32,
    /// The shadow ray of the worked cell, with a sphere per step.
    #[value(default = 0.0, toggle)]
    pub shadow_ray: f32,
    /// The probe the probe demonstrations follow, and its rays.
    #[value(default = 0.0, toggle)]
    pub probe_rays: f32,
    /// How many frames of the probe's rays are drawn (8 rays a frame, 0 to 30).
    #[value(default = 1.0, range = 0.0..=30.0, unit = index, text = "controls.ray_frames", color = palette::INCIDENT, label(probes))]
    pub ray_frames: f32,
    /// The followed ray's march: how many steps are drawn (0 to 40).
    #[value(default = 0.0, range = 0.0..=40.0, unit = index, text = "controls.ray_steps", color = palette::INCIDENT, label(probes))]
    pub ray_steps: f32,
    /// Frames since SDFGI was created, for the probe's history (0 to 60).
    #[value(default = 60.0, range = 0.0..=60.0, unit = index, text = "controls.frames", color = palette::INCIDENT, label(probes))]
    pub frames: f32,
    /// The followed probe's octahedral maps (an inset).
    #[value(default = 0.0, toggle)]
    pub probe_maps: f32,
    /// The inspected pixel's point on the red wall, along z, metres.
    #[value(default = -0.4, range = -2.8..=2.8, unit = m, text = "controls.pixel_z", color = palette::VIEW, label(pixel))]
    pub pixel_z: f32,
    /// Its height, metres.
    #[value(default = 0.9, range = 0.1..=2.9, unit = m, text = "controls.pixel_y", color = palette::VIEW, label(pixel))]
    pub pixel_y: f32,
    /// The pixel's eight probes, their weights, and the cascades it reads.
    #[value(default = 0.0, toggle)]
    pub pixel_probes: f32,
    /// The sphere's roughness (the screen pass's specular source depends on it).
    #[value(default = 0.5, range = 0.0..=1.0, text = "controls.roughness", color = palette::VIEW, label(pixel))]
    pub sphere_roughness: f32,
    /// The walk is one jump (a single `update`), not 0.2 m steps.
    #[value(default = 0.0, toggle)]
    pub camera_jump: f32,
    /// The settings readout's row lit (-1: none).
    #[value(default = -1.0, range = -1.0..=11.0, unit = index, text = "controls.focus_row", color = palette::THEME_ACCENT, label(cascades))]
    pub focus_row: f32,
    /// Voxels coloured by albedo (0) or by their facing bits (1).
    #[value(default = 0.0, toggle)]
    pub voxel_facing: f32,
    /// A sphere trace from the field's worked cell along +x, a circle per step.
    #[value(default = 0.0, toggle)]
    pub field_march: f32,
    /// The followed probe's irradiance over the frames since creation (an inset chart).
    #[value(default = 0.0, toggle)]
    pub history_chart: f32,
    /// The uniform-sky gain row of the maps readout.
    #[value(default = 0.0, toggle)]
    pub show_gain: f32,
    /// A test point on the camera's +x axis this far out, metres: its cascade and blend.
    #[value(default = 0.0, range = 0.0..=30.0, unit = m, text = "controls.blend_point", color = palette::AXIS_Y, label(pixel))]
    pub blend_point: f32,
    /// The pixel on the sphere's front instead of the red wall (the specular sources).
    #[value(default = 0.0, toggle)]
    pub sphere_pixel: f32,
    /// Labels on what is drawn.
    #[value(default = 1.0, toggle)]
    pub labels: f32,
    /// How much the camera sways as a timeline plays (the timeline player reads it).
    #[value(default = 0.0, range = 0.0..=2.0)]
    pub camera_drift: f32,
}
