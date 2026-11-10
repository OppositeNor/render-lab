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
    #[value(default = 0.0, range = -12.0..=12.0, unit = m, text = "controls-camera_walk", color = palette::THEME_ACCENT, label(cascades), help = "help-cameraX")]
    pub camera_x: f32,
    /// How far the camera has walked along +z, metres (with `cameraX`, a walk that moves the cascades along two axes).
    #[value(default = 0.0, range = -12.0..=12.0, unit = m, text = "controls-camera_walk_z", color = palette::THEME_ACCENT, label(cascades), help = "help-walkZ")]
    pub walk_z: f32,
    /// How many of the cascades' boxes are drawn, from the finest (0 to 4).
    #[value(default = 0.0, range = 0.0..=4.0, unit = index, text = "controls-cascades_shown", color = palette::THEME_ACCENT, label(cascades), help = "help-cascadesShown")]
    pub cascades_shown: f32,
    /// The environment's sdfgi_y_scale for the boxes drawn: 0 = 50 %, 1 = 75 % (the default), 2 = 100 %; fractions blend.
    #[value(default = 1.0, range = 0.0..=2.0, unit = index, text = "controls-y_scale", color = palette::THEME_ACCENT, label(cascades), help = "help-yScale")]
    pub y_scale: f32,
    /// The environment's sdfgi_min_cell_size for the cascades drawn, metres (the first cascade's cell; the scene keeps 0.2).
    #[value(default = 0.2, range = 0.05..=0.8, unit = m, text = "controls-min_cell_size", color = palette::THEME_ACCENT, label(cascades), help = "help-minCellSize")]
    pub min_cell_size: f32,
    /// One row of cascade 0's probes along x, counted (0 to 17).
    #[value(default = 0.0, range = 0.0..=17.0, unit = index, text = "controls-probe_row", color = palette::THEME_ACCENT, label(cascades), help = "help-probeRow")]
    pub probe_row: f32,
    /// The probes of cascade 0, as points (the ones inside the room brighter).
    #[value(default = 0.0, toggle, text = "values-probes", help = "help-probes")]
    pub probes: f32,
    /// The dirty slab of the walk and the regions handed to the culling.
    #[value(default = 0.0, toggle, text = "values-regions", help = "help-regions")]
    pub regions: f32,
    /// A top view of the walk's first row of cells (the scroll worked step by step).
    #[value(
        default = 0.0,
        toggle,
        text = "values-scrollView",
        help = "help-scrollView"
    )]
    pub scroll_view: f32,
    /// The teapot's GI mode: 0 Static (voxelized), 1 Dynamic, 2 Disabled; for SDFGI both of the last leave it out of
    /// the voxels and still light it (GeometryInstance3D's GI_MODE_DYNAMIC acts as GI_MODE_DISABLED there).
    #[value(default = 0.0, range = 0.0..=2.0, unit = index, text = "controls-teapot_gi_mode", color = palette::THEME_ACCENT, label(voxels), help = "help-teapotGiMode")]
    pub teapot_gi_mode: f32,
    /// Cascade 0's solid voxels, as points in their albedo.
    #[value(default = 0.0, toggle, text = "values-voxels", help = "help-voxels")]
    pub voxels: f32,
    /// How many of the voxelizer's three passes (x, y, z views) have run (0 to 3).
    #[value(default = 3.0, range = 0.0..=3.0, unit = index, text = "controls-voxel_passes", color = palette::THEME_ACCENT, label(voxels), help = "help-voxelPasses")]
    pub voxel_passes: f32,
    /// The levels a channel of the drawn voxels' albedo has (2 to 32; the stored five bits give 32).
    #[value(default = 32.0, range = 2.0..=32.0, unit = index, text = "controls-albedo_levels", color = palette::THEME_ACCENT, label(voxels), help = "help-albedoLevels")]
    pub albedo_levels: f32,
    /// An emissive colour packed as the SDF pass packs emission, as a chart of its fields.
    #[value(
        default = 0.0,
        toggle,
        text = "values-emissionChart",
        help = "help-emissionChart"
    )]
    pub emission_chart: f32,
    /// That colour's brightness, as a power of ten (-3 to 3).
    #[value(default = 0.0, range = -3.0..=3.0, unit = plain, text = "controls-emission_level", color = palette::THEME_ACCENT, label(voxels), help = "help-emissionLevel")]
    pub emission_level: f32,
    /// Voxels are drawn only below this height, metres (a cut through the room).
    #[value(default = 4.0, range = 0.0..=4.0, unit = m, text = "controls-cut_height", color = palette::THEME_ACCENT, label(voxels), help = "help-cutHeight")]
    pub cut_height: f32,
    /// The distance field's slice (an inset): its height, metres.
    #[value(default = 0.62, range = 0.0..=3.0, unit = m, text = "controls-slice_height", color = palette::OUTSIDE, label(field), help = "help-sliceHeight")]
    pub slice_height: f32,
    /// The slice inset shown (0 none, 1 distance field, 2 jump-flood seeds, 3 occlusion of one probe).
    #[value(default = 0.0, range = 0.0..=3.0, unit = index, text = "controls-slice", color = palette::OUTSIDE, label(field), help = "help-slice")]
    pub slice: f32,
    /// The jump flood's step shown: 0 the half-size seeds, 1 to 6 the half-size passes, 7 the upscale, 8 the full-size pass.
    #[value(default = 8.0, range = 0.0..=8.0, unit = index, text = "controls-flood_step", color = palette::OUTSIDE, label(field), help = "help-floodStep")]
    pub flood_step: f32,
    /// The occlusion shells shown around the probe (|x| + |y| + |z| up to this, 0 to 22).
    #[value(default = 22.0, range = 0.0..=22.0, unit = index, text = "controls-occlusion_shell", color = palette::OUTSIDE, label(field), help = "help-occlusionShell")]
    pub occlusion_shell: f32,
    /// The probes at the corners of the readout cell's probe cube drawn (0 to 8), each with the cell's visibility of it.
    #[value(default = 0.0, range = 0.0..=8.0, unit = index, text = "controls-cube_probes", color = palette::OUTSIDE, label(field), help = "help-cubeProbes")]
    pub cube_probes: f32,
    /// The solid cells, coloured by the light the light pass stored in them.
    #[value(
        default = 0.0,
        toggle,
        text = "values-cellLight",
        help = "help-cellLight"
    )]
    pub cell_light: f32,
    /// Which quarter of the cells this frame's light pass updates (0 to 3; 4 shows all).
    #[value(default = 4.0, range = 0.0..=4.0, unit = index, text = "controls-light_phase", color = palette::INCIDENT, label(light), help = "help-lightPhase")]
    pub light_phase: f32,
    /// The extra omni light (for the light pass's worked cell): on or off.
    #[value(
        default = 0.0,
        toggle,
        text = "values-extraLight",
        help = "help-extraLight"
    )]
    pub extra_light: f32,
    /// Its height above the floor, metres.
    #[value(default = 2.0, range = 0.2..=2.9, unit = m, text = "controls-light_height", color = palette::INCIDENT, label(light), help = "help-lightHeight")]
    pub light_height: f32,
    /// Its x, metres.
    #[value(default = 1.6, range = -2.8..=2.8, unit = m, text = "controls-light_x", color = palette::INCIDENT, label(light), help = "help-lightX")]
    pub light_x: f32,
    /// Its z, metres.
    #[value(default = 1.0, range = -2.8..=2.8, unit = m, text = "controls-light_z", color = palette::INCIDENT, label(light), help = "help-lightZ")]
    pub light_z: f32,
    /// The worked floor cell's x and z, metres; below -9 it lies under the light.
    #[value(default = -10.0, range = -10.0..=2.8, unit = m, text = "controls-cell_x", color = palette::SELECTED, label(light), help = "help-cellX")]
    pub cell_x: f32,
    #[value(default = 0.0, range = -2.8..=2.8, unit = m, text = "controls-cell_z", color = palette::SELECTED, label(light), help = "help-cellZ")]
    pub cell_z: f32,
    /// Its bake mode: 0 Dynamic (every update), 1 Static (written into the cells when a region is built).
    #[value(default = 0.0, range = 0.0..=1.0, unit = index, text = "controls-light_bake", color = palette::INCIDENT, label(light), help = "help-lightBake")]
    pub light_bake: f32,
    /// Scrolls of cascade 0 after a static light was baked (0 to 3): the stored static light of a kept cell.
    #[value(default = 0.0, range = 0.0..=3.0, unit = index, text = "controls-scrolls", color = palette::INCIDENT, label(light), help = "help-scrolls")]
    pub scrolls: f32,
    /// The environment's sdfgi_bounce_feedback the readout's cells use (the room's light keeps the default 0.5).
    #[value(default = 0.5, range = 0.0..=1.99, text = "controls-bounce_feedback", color = palette::INCIDENT, label(light), help = "help-bounceFeedback")]
    pub bounce_feedback: f32,
    /// The shadow ray of the worked cell, with a sphere per step.
    #[value(
        default = 0.0,
        toggle,
        text = "values-shadowRay",
        help = "help-shadowRay"
    )]
    pub shadow_ray: f32,
    /// How many of the shadow ray's steps are drawn (0 to 24).
    #[value(default = 24.0, range = 0.0..=24.0, unit = index, text = "controls-shadow_steps", color = palette::OUTSIDE, label(light), help = "help-shadowSteps")]
    pub shadow_steps: f32,
    /// The omni light's falloff as a chart: 1 the engine's default attenuation, 2 the inverse square beside it.
    #[value(default = 0.0, range = 0.0..=2.0, unit = index, text = "controls-falloff_chart", color = palette::INCIDENT, label(light), help = "help-falloffChart")]
    pub falloff_chart: f32,
    /// Where the light pass's distance puts the light: every height times y_mult.
    #[value(
        default = 0.0,
        toggle,
        text = "values-squashedLight",
        help = "help-squashedLight"
    )]
    pub squashed_light: f32,
    /// The worked cell's six faces, as arrows.
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-face_light", color = palette::INCIDENT, label(light), help = "help-faceLight")]
    pub face_light: f32,
    /// Each face's arrow scaled to the share of the cell's light it stores (0 even, 1 by weight).
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-face_weights", color = palette::INCIDENT, label(light), help = "help-faceWeights")]
    pub face_weights: f32,
    /// The probe the probe demonstrations follow, and its rays.
    #[value(
        default = 0.0,
        toggle,
        text = "values-probeRays",
        help = "help-probeRays"
    )]
    pub probe_rays: f32,
    /// How many frames of the probe's rays are drawn (8 rays a frame, 0 to 30).
    #[value(default = 1.0, range = 0.0..=30.0, unit = index, text = "controls-ray_frames", color = palette::INCIDENT, label(probes), help = "help-rayFrames")]
    pub ray_frames: f32,
    /// The followed ray's march: how many steps are drawn (0 to 40).
    #[value(default = 0.0, range = 0.0..=40.0, unit = index, text = "controls-ray_steps", color = palette::INCIDENT, label(probes), help = "help-raySteps")]
    pub ray_steps: f32,
    /// Frames since SDFGI was created, for the probe's history (0 to 60).
    #[value(default = 60.0, range = 0.0..=60.0, unit = index, text = "controls-frames", color = palette::INCIDENT, label(probes), help = "help-frames")]
    pub frames: f32,
    /// The texel of the followed probe's maps that is picked (-1 none; 0 to 35, row by row over the 6 x 6 texels):
    /// its direction from the probe, and its border copies.
    #[value(default = -1.0, range = -1.0..=35.0, unit = index, text = "controls-map_texel", color = palette::INCIDENT, label(probes), help = "help-mapTexel")]
    pub map_texel: f32,
    /// At each drawn ray's hit, the surface's normal from the field's gradient.
    #[value(
        default = 0.0,
        toggle,
        text = "values-hitNormals",
        help = "help-hitNormals"
    )]
    pub hit_normals: f32,
    /// The drawn rays' light projected onto the 16 SH coefficients, read back around the probe as a glyph.
    #[value(default = 0.0, toggle, text = "values-shGlyph", help = "help-shGlyph")]
    pub sh_glyph: f32,
    /// The maps inset shows what a probe lit by a uniform sky of 1 stores (every texel the same).
    #[value(
        default = 0.0,
        toggle,
        text = "values-uniformSky",
        help = "help-uniformSky"
    )]
    pub uniform_sky: f32,
    /// With the uniform sky: the gain from the code's 16 (0) to the 4 pi a sphere's average needs (1).
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-gain_fix", color = palette::MISMATCH, label(probes), help = "help-gainFix")]
    pub gain_fix: f32,
    /// The followed probe's octahedral maps (an inset).
    #[value(
        default = 0.0,
        toggle,
        text = "values-probeMaps",
        help = "help-probeMaps"
    )]
    pub probe_maps: f32,
    /// The inspected pixel's point on the red wall, along z, metres.
    #[value(default = -0.4, range = -2.8..=2.8, unit = m, text = "controls-pixel_z", color = palette::VIEW, label(pixel), help = "help-pixelZ")]
    pub pixel_z: f32,
    /// Its height, metres.
    #[value(default = 0.9, range = 0.1..=2.9, unit = m, text = "controls-pixel_y", color = palette::VIEW, label(pixel), help = "help-pixelY")]
    pub pixel_y: f32,
    /// The pixel's eight probes, their weights, and the cascades it reads.
    #[value(
        default = 0.0,
        toggle,
        text = "values-pixelProbes",
        help = "help-pixelProbes"
    )]
    pub pixel_probes: f32,
    /// The sphere's roughness (the screen pass's specular source depends on it).
    #[value(default = 0.5, range = 0.0..=1.0, text = "controls-roughness", color = palette::VIEW, label(pixel), help = "help-sphereRoughness")]
    pub sphere_roughness: f32,
    /// A fog sample's height, metres (below 0: none): the probe the screen pass reads there and the one the fog reads.
    #[value(default = -1.0, range = -1.0..=2.9, unit = m, text = "controls-fog_height", color = palette::MISMATCH, label(pixel), help = "help-fogHeight")]
    pub fog_height: f32,
    /// The walk is one jump (a single `update`), not 0.2 m steps.
    #[value(
        default = 0.0,
        toggle,
        text = "values-cameraJump",
        help = "help-cameraJump"
    )]
    pub camera_jump: f32,
    /// The settings readout's row lit (-1: none).
    #[value(default = -1.0, range = -1.0..=11.0, unit = index, text = "controls-focus_row", color = palette::THEME_ACCENT, label(cascades), help = "help-focusRow")]
    pub focus_row: f32,
    /// Voxels coloured by albedo (0) or by their facing bits (1).
    #[value(
        default = 0.0,
        toggle,
        text = "values-voxelFacing",
        help = "help-voxelFacing"
    )]
    pub voxel_facing: f32,
    /// A sphere trace from the field's worked cell along +x, a circle per step.
    #[value(
        default = 0.0,
        toggle,
        text = "values-fieldMarch",
        help = "help-fieldMarch"
    )]
    pub field_march: f32,
    /// How many of the field march's steps are drawn (0 to 40; it stops at the surface).
    #[value(default = 40.0, range = 0.0..=40.0, unit = index, text = "controls-field_steps", color = palette::OUTSIDE, label(field), help = "help-fieldSteps")]
    pub field_steps: f32,
    /// The field demonstration's worked cell: its x and z, metres (a click on the slice inset picks it).
    #[value(default = -0.2, range = -3.4..=3.4, unit = m, text = "controls-field_x", color = palette::SELECTED, label(field), help = "help-fieldX")]
    pub field_x: f32,
    #[value(default = -0.55, range = -3.4..=3.4, unit = m, text = "controls-field_z", color = palette::SELECTED, label(field), help = "help-fieldX")]
    pub field_z: f32,
    /// How many cells of the stored byte along the march's line the chart shows (0 none, to 24).
    #[value(default = 0.0, range = 0.0..=24.0, unit = index, text = "controls-byte_profile", color = palette::OUTSIDE, label(field), help = "help-byteProfile")]
    pub byte_profile: f32,
    /// The chart's bars: the stored byte (0) or the byte minus one, as a ray reads it (1).
    #[value(default = 0.0, range = 0.0..=1.0, unit = plain, text = "controls-byte_read", color = palette::OUTSIDE, label(field), help = "help-byteRead")]
    pub byte_read: f32,
    /// The chart's curve: the filtered read between the cells.
    #[value(
        default = 0.0,
        toggle,
        text = "values-byteCurve",
        help = "help-byteCurve"
    )]
    pub byte_curve: f32,
    /// The cells the worked cell compares at the jump flood's step: its 26 neighbours a step away, or at the
    /// upscale the eight voxels of its seed's block.
    #[value(
        default = 0.0,
        toggle,
        text = "values-floodNeighbours",
        help = "help-floodNeighbours"
    )]
    pub flood_neighbours: f32,
    /// The worked voxel's 26 neighbour flags, from its solid-cell record.
    #[value(
        default = 0.0,
        toggle,
        text = "values-neighbourFlags",
        help = "help-neighbourFlags"
    )]
    pub neighbour_flags: f32,
    /// The followed probe's irradiance over the frames since creation (an inset chart).
    #[value(
        default = 0.0,
        toggle,
        text = "values-historyChart",
        help = "help-historyChart"
    )]
    pub history_chart: f32,
    /// The uniform-sky gain row of the maps readout.
    #[value(
        default = 0.0,
        toggle,
        text = "values-showGain",
        help = "help-showGain"
    )]
    pub show_gain: f32,
    /// A test point on the camera's +x axis this far out, metres: its cascade and blend.
    #[value(default = 0.0, range = 0.0..=30.0, unit = m, text = "controls-blend_point", color = palette::AXIS_Y, label(pixel), help = "help-blendPoint")]
    pub blend_point: f32,
    /// The pixel on the sphere's front instead of the red wall (the specular sources).
    #[value(
        default = 0.0,
        toggle,
        text = "values-spherePixel",
        help = "help-spherePixel"
    )]
    pub sphere_pixel: f32,
    /// Labels on what is drawn.
    #[value(default = 1.0, toggle, text = "values-labels", help = "help-labels")]
    pub labels: f32,
    /// How much the camera sways as a timeline plays (the timeline player reads it).
    #[value(default = 0.0, range = 0.0..=2.0, text = "values-cameraDrift", help = "help-cameraDrift")]
    pub camera_drift: f32,
    // The scene's SDFGI, as the engine's settings (the Experiments tab), all
    // baked into the converged probe maps (a change rebuilds them, sim.rs
    // `converged_for`). The demonstrations keep their own bounceFeedback.
    /// Environment.sdfgi_bounce_feedback of the scene (environment.cpp:1421, 0 to 1.99, default 0.5).
    #[value(default = 0.5, range = 0.0..=1.99, step = 0.01, text = "controls-gi_bounce_feedback", label(gi), engine = "Environment.sdfgi_bounce_feedback", help = "help-giBounceFeedback")]
    pub gi_bounce_feedback: f32,
    /// Environment.sdfgi_probe_bias, in cells: the probe rays' start offset (default 1.1).
    #[value(default = 1.1, range = 0.0..=4.0, step = 0.01, text = "controls-gi_probe_bias", label(gi), engine = "Environment.sdfgi_probe_bias", help = "help-giProbeBias")]
    pub gi_probe_bias: f32,
    /// rendering/global_illumination/sdfgi/probe_ray_count: an index into {4, 8, 16, 32, 64, 96, 128} (gi.cpp:1322), default 1 (8 rays).
    #[value(default = 1.0, range = 0.0..=6.0, unit = index, text = "controls-gi_ray_count", label(gi), engine = "ProjectSettings.rendering/global_illumination/sdfgi/probe_ray_count", help = "help-giRayCount")]
    pub gi_ray_count: f32,
    /// rendering/global_illumination/sdfgi/frames_to_converge: an index into {5, 10, 15, 20, 25, 30} frames of history, default 5 (30).
    #[value(default = 5.0, range = 0.0..=5.0, unit = index, text = "controls-gi_frames_to_converge", label(gi), engine = "ProjectSettings.rendering/global_illumination/sdfgi/frames_to_converge", help = "help-giFramesToConverge")]
    pub gi_frames_to_converge: f32,
    /// rendering/global_illumination/sdfgi/frames_to_update_lights: an index into {1, 2, 4, 8, 16}, default 2 (a quarter of the cells a frame).
    #[value(default = 2.0, range = 0.0..=4.0, unit = index, text = "controls-gi_frames_to_update_lights", label(gi), engine = "ProjectSettings.rendering/global_illumination/sdfgi/frames_to_update_lights", help = "help-giFramesToUpdateLights")]
    pub gi_frames_to_update_lights: f32,
    /// Environment.sdfgi_read_sky_light: rays that leave the cascades bring back the sky (else nothing).
    #[value(
        default = 1.0,
        toggle,
        text = "controls-gi_read_sky_light",
        label(gi),
        engine = "Environment.sdfgi_read_sky_light",
        help = "help-giReadSkyLight"
    )]
    pub gi_read_sky_light: f32,
    /// rendering/global_illumination/gi/use_half_resolution: the screen gather writes half-size buffers, one pixel of each 2 × 2 block, which the opaque pass reads bilinearly.
    #[value(
        default = 0.0,
        toggle,
        text = "controls-gi_half_resolution",
        label(gi),
        engine = "ProjectSettings.rendering/global_illumination/gi/use_half_resolution",
        help = "help-giHalfResolution"
    )]
    pub gi_half_resolution: f32,
}
