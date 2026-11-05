//! The investigation's values: every number a demonstration, the dock, a
//! timeline, or a story sets, its default, and what it is, as a struct the lab
//! reaches by key (`#[derive(Values)]`, lab/src/values.rs). A shot stores them
//! by key (`state.p("key")`); a demonstration's file overrides some
//! (lab/src/demo.rs). A field's `label` names the dock group that shows it
//! (controls.rs). **A value's default must change nothing**, so existing frames
//! stay identical. The scene shading's values (voxelgiMix, quality, interior,
//! viewMode) are declared by their tags in shaders/voxelgi_shading.wgsl. A
//! value with `engine` is one of Godot's settings (the Experiments tab).

use lab::palette;

#[derive(Clone, Debug, PartialEq, lab::Values, serde::Serialize, serde::Deserialize)]
#[values(package = "godot_voxelgi", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct VoxelgiValues {
    /// The VoxelGI node's box, drawn (0 to 1 fades it in).
    #[value(default = 0.0, range = 0.0..=1.0, text = "controls-show_box", color = palette::AABB, label(bake), help = "help-showBox")]
    pub show_box: f32,
    /// The bake's bounds step: 0 the box as set, 1 the power-of-two cube, 2 its cells per axis.
    #[value(default = 0.0, range = 0.0..=2.0, unit = index, text = "controls-bounds_step", color = palette::AABB, label(bake), help = "help-boundsStep")]
    pub bounds_step: f32,
    /// The octree level whose cells are drawn (-1 none, 0 the root, 7 the leaves).
    #[value(default = -1.0, range = -1.0..=7.0, unit = index, text = "controls-octree_level", color = palette::AABB, label(bake), help = "help-octreeLevel")]
    pub octree_level: f32,
    /// What the drawn cells show: 0 albedo, 1 coverage (alpha), 2 normal, 3 emission, 4 the light texture, 5 direct light.
    #[value(default = 0.0, range = 0.0..=5.0, unit = index, text = "controls-cell_view", color = palette::AABB, label(bake), help = "help-cellView")]
    pub cell_view: f32,
    /// Cells are drawn only below this height, metres (a cut through the room).
    #[value(default = 3.2, range = 0.0..=3.2, unit = m, text = "controls-cut_height", color = palette::AABB, label(bake), help = "help-cutHeight")]
    pub cut_height: f32,
    /// The worked triangle (one of the teapot's) and the boxes it went down through.
    #[value(default = 0.0, toggle, text = "values-workedTriangle", help = "help-workedTriangle")]
    pub worked_triangle: f32,
    /// How deep the worked triangle's descent is drawn (0 the root to 7 the leaves).
    #[value(default = 7.0, range = 0.0..=7.0, unit = index, text = "controls-descent_level", color = palette::SELECTED, label(bake), help = "help-descentLevel")]
    pub descent_level: f32,
    /// The separating-axis test's stage drawn on one child: 0 the box and triangle, 1 an edge axis, 2 the box's axes, 3 the plane.
    #[value(default = 0.0, range = 0.0..=3.0, unit = index, text = "controls-sat_step", color = palette::SELECTED, label(bake), help = "help-satStep")]
    pub sat_step: f32,
    /// The worked leaf's 4 x 4 columns, drawn.
    #[value(default = 0.0, toggle, text = "values-workedLeaf", help = "help-workedLeaf")]
    pub worked_leaf: f32,
    /// How many of its columns are drawn (0 to 16).
    #[value(default = 16.0, range = 0.0..=16.0, unit = index, text = "controls-leaf_columns", color = palette::SELECTED, label(bake), help = "help-leafColumns")]
    pub leaf_columns: f32,
    /// The worked cell's records in the readout: 0 its sums before `_fixup_plot`, 1 after.
    #[value(default = 1.0, range = 0.0..=1.0, unit = index, text = "controls-fixup_step", color = palette::SELECTED, label(cells), help = "help-fixupStep")]
    pub fixup_step: f32,
    /// The readout row lit (-1: none).
    #[value(default = -1.0, range = -1.0..=11.0, unit = index, text = "controls-focus_row", color = palette::THEME_ACCENT, label(cells), help = "help-focusRow")]
    pub focus_row: f32,
    /// The picked leaf cell (cells), or -1 for the demonstration's worked cell.
    #[value(default = -1.0, range = -1.0..=127.0, unit = index, text = "controls-cell_x", color = palette::SELECTED, label(cells), help = "help-cellX")]
    pub cell_x: f32,
    #[value(default = -1.0, range = -1.0..=63.0, unit = index, text = "controls-cell_y", color = palette::SELECTED, label(cells), help = "help-cellY")]
    pub cell_y: f32,
    #[value(default = -1.0, range = -1.0..=127.0, unit = index, text = "controls-cell_z", color = palette::SELECTED, label(cells), help = "help-cellZ")]
    pub cell_z: f32,
    /// The sorted cell array, as a bar coloured by level (an inset).
    #[value(default = 0.0, toggle, text = "values-cellArray", help = "help-cellArray")]
    pub cell_array: f32,
    /// The slice inset: 0 none, 1 the distance field, 2 the light texture's mip `mipLevel`.
    #[value(default = 0.0, range = 0.0..=2.0, unit = index, text = "controls-slice", color = palette::OUTSIDE, label(field), help = "help-slice")]
    pub slice: f32,
    /// The slice's height, metres.
    #[value(default = 0.3, range = 0.0..=3.1, unit = m, text = "controls-slice_height", color = palette::OUTSIDE, label(field), help = "help-sliceHeight")]
    pub slice_height: f32,
    /// The distance transform's passes run (0 the seeds, 1 along z, 2 then y, 3 then x).
    #[value(default = 3.0, range = 0.0..=3.0, unit = index, text = "controls-edt_pass", color = palette::OUTSIDE, label(field), help = "help-edtPass")]
    pub edt_pass: f32,
    /// The light texture's mip the slice shows (0 the leaves to 7 the root).
    #[value(default = 0.0, range = 0.0..=7.0, unit = index, text = "controls-mip_level", color = palette::OUTSIDE, label(field), help = "help-mipLevel")]
    pub mip_level: f32,
    /// The extra omni light with shadows (OmniLight3D): on or off.
    #[value(default = 0.0, toggle, text = "values-extraLight", help = "help-extraLight")]
    pub extra_light: f32,
    /// Its x, height, and z, metres.
    #[value(default = 0.9, range = -2.8..=2.8, unit = m, text = "controls-light_x", color = palette::INCIDENT, label(light), help = "help-lightX")]
    pub light_x: f32,
    #[value(default = 1.4, range = 0.2..=2.9, unit = m, text = "controls-light_height", color = palette::INCIDENT, label(light), help = "help-lightHeight")]
    pub light_height: f32,
    #[value(default = -0.4, range = -2.8..=2.8, unit = m, text = "controls-light_z", color = palette::INCIDENT, label(light), help = "help-lightZ")]
    pub light_z: f32,
    /// The worked light cell's steps: 0 the cell, 1 the light vector, 2 attenuation, 3 the cosine, 4 the result.
    #[value(default = 4.0, range = 0.0..=4.0, unit = index, text = "controls-light_step", color = palette::INCIDENT, label(light), help = "help-lightStep")]
    pub light_step: f32,
    /// The worked light cell and its vectors, drawn.
    #[value(default = 0.0, toggle, text = "values-workedLight", help = "help-workedLight")]
    pub worked_light: f32,
    /// How many steps of its shadow march are drawn (0 to 40).
    #[value(default = 40.0, range = 0.0..=40.0, unit = index, text = "controls-march_steps", color = palette::INCIDENT, label(light), help = "help-marchSteps")]
    pub march_steps: f32,
    /// The VoxelGIData's dynamic_range the drawn cells and slices use.
    #[value(default = 2.0, range = 1.0..=8.0, text = "controls-dynamic_range", color = palette::INCIDENT, label(light), help = "help-dynamicRange")]
    pub dynamic_range: f32,
    /// Its propagation.
    #[value(default = 0.5, range = 0.0..=1.0, text = "controls-propagation", color = palette::INCIDENT, label(bounce), help = "help-propagation")]
    pub propagation: f32,
    /// Its passes: 1 direct light only, 2 with the second bounce (use_two_bounces).
    #[value(default = 2.0, range = 1.0..=2.0, unit = index, text = "controls-bounces", color = palette::INCIDENT, label(bounce), help = "help-bounces")]
    pub bounces: f32,
    /// The bounce's worked cell and its cones, drawn.
    #[value(default = 0.0, toggle, text = "values-workedBounce", help = "help-workedBounce")]
    pub worked_bounce: f32,
    /// The worked parent cell of the mip pass and its children, drawn.
    #[value(default = 0.0, toggle, text = "values-workedParent", help = "help-workedParent")]
    pub worked_parent: f32,
    /// How many cones are drawn (0 to 6).
    #[value(default = 6.0, range = 0.0..=6.0, unit = index, text = "controls-cones_shown", color = palette::VIEW, label(cones), help = "help-conesShown")]
    pub cones_shown: f32,
    /// How many steps of each cone are drawn (0 to 16).
    #[value(default = 16.0, range = 0.0..=16.0, unit = index, text = "controls-cone_steps", color = palette::VIEW, label(cones), help = "help-coneSteps")]
    pub cone_steps: f32,
    /// The specular cone, drawn.
    #[value(default = 0.0, toggle, text = "values-specCone", help = "help-specCone")]
    pub spec_cone: f32,
    /// The worked pixel (on the sphere) and its cones, drawn.
    #[value(default = 0.0, toggle, text = "values-workedPixel", help = "help-workedPixel")]
    pub worked_pixel: f32,
    /// The worked pixel's place on the sphere: its azimuth from the red wall and its elevation, degrees.
    #[value(default = 60.0, range = -180.0..=180.0, unit = deg, text = "controls-pixel_azimuth", color = palette::VIEW, label(cones), help = "help-pixelAzimuth")]
    pub pixel_azimuth: f32,
    #[value(default = 10.0, range = -30.0..=80.0, unit = deg, text = "controls-pixel_elevation", color = palette::VIEW, label(cones), help = "help-pixelElevation")]
    pub pixel_elevation: f32,
    /// The sphere's roughness (the reflection cone follows it).
    #[value(default = 0.5, range = 0.0..=1.0, text = "controls-roughness", color = palette::VIEW, label(cones), help = "help-sphereRoughness")]
    pub sphere_roughness: f32,
    /// The teapot as a dynamic object: its plotted voxels, drawn.
    #[value(default = 0.0, toggle, text = "values-dynamicVoxels", help = "help-dynamicVoxels")]
    pub dynamic_voxels: f32,
    /// How far the dynamic teapot has moved along x, metres.
    #[value(default = 0.0, range = -1.0..=2.0, unit = m, text = "controls-dynamic_x", color = palette::SELECTED, label(dynamic), help = "help-dynamicX")]
    pub dynamic_x: f32,
    /// The view shown in the insets (0 +x, 1 +y, 2 +z, 3 -x, 4 -y, 5 -z; -1 none).
    #[value(default = -1.0, range = -1.0..=5.0, unit = index, text = "controls-dynamic_view", color = palette::SELECTED, label(dynamic), help = "help-dynamicView")]
    pub dynamic_view: f32,
    /// Which map of its shrink chain the light inset shows (0 the 16x map to 11 the coarsest mip).
    #[value(default = 0.0, range = 0.0..=11.0, unit = index, text = "controls-shrink_level", color = palette::SELECTED, label(dynamic), help = "help-shrinkLevel")]
    pub shrink_level: f32,
    /// The mip of the plotted voxels drawn (0 to 7).
    #[value(default = 0.0, range = 0.0..=7.0, unit = index, text = "controls-dynamic_mip", color = palette::SELECTED, label(dynamic), help = "help-dynamicMip")]
    pub dynamic_mip: f32,
    /// Labels on what is drawn.
    #[value(default = 1.0, toggle, text = "values-labels", help = "help-labels")]
    pub labels: f32,
    /// How much the camera sways as a timeline plays (the timeline player reads it).
    #[value(default = 0.0, range = 0.0..=2.0, text = "values-cameraDrift", help = "help-cameraDrift")]
    pub camera_drift: f32,
    // The scene's VoxelGI node, as the engine's settings (the Experiments
    // tab): VoxelGIData's properties (scene/3d/voxel_gi.cpp:234-239, their
    // ranges; defaults voxel_gi.h:50-56). The demonstrations' worked cells
    // keep their own dynamicRange, propagation, and bounces.
    /// VoxelGIData.dynamic_range of the scene's node: the light texture's range, baked into it (a change relights it).
    #[value(default = 2.0, range = 1.0..=8.0, step = 0.01, text = "controls-gi_dynamic_range", label(gi), engine = "VoxelGIData.dynamic_range", help = "help-giDynamicRange")]
    pub gi_dynamic_range: f32,
    /// VoxelGIData.energy: multiplies the light the screen pass reads (dynamic_range × energy).
    #[value(default = 1.0, range = 0.0..=64.0, step = 0.01, text = "controls-gi_energy", label(gi), engine = "VoxelGIData.energy", help = "help-giEnergy")]
    pub gi_energy: f32,
    /// VoxelGIData.bias, in cells: how far along the normal and the cone the trace starts.
    #[value(default = 1.5, range = 0.0..=8.0, step = 0.01, text = "controls-gi_bias", label(gi), engine = "VoxelGIData.bias", help = "help-giBias")]
    pub gi_bias: f32,
    /// VoxelGIData.normal_bias, in cells: the start's offset along the normal.
    #[value(default = 0.0, range = 0.0..=8.0, step = 0.01, text = "controls-gi_normal_bias", label(gi), engine = "VoxelGIData.normal_bias", help = "help-giNormalBias")]
    pub gi_normal_bias: f32,
    /// VoxelGIData.propagation of the scene's node: how much of the first bounce the second carries (a change relights the texture).
    #[value(default = 0.5, range = 0.0..=1.0, step = 0.01, text = "controls-gi_propagation", label(gi), engine = "VoxelGIData.propagation", help = "help-giPropagation")]
    pub gi_propagation: f32,
    /// VoxelGIData.use_two_bounces of the scene's node (a change relights the texture).
    #[value(default = 1.0, toggle, text = "controls-gi_two_bounces", label(gi), engine = "VoxelGIData.use_two_bounces", help = "help-giTwoBounces")]
    pub gi_two_bounces: f32,
    /// rendering/global_illumination/gi/use_half_resolution: the GI pass traces one pixel of each 2 × 2 block into half-size buffers, which the opaque pass reads bilinearly.
    #[value(default = 0.0, toggle, text = "controls-gi_half_resolution", label(gi), engine = "ProjectSettings.rendering/global_illumination/gi/use_half_resolution", help = "help-giHalfResolution")]
    pub gi_half_resolution: f32,
    /// The lab's measurement, not a setting of Godot's: run the light passes every frame (Godot runs them only when the lights or the data change), so the performance panel times them.
    #[value(default = 0.0, toggle, text = "controls-gi_relight_each_frame", label(gi), help = "help-giRelightEachFrame")]
    pub gi_relight_each_frame: f32,
    /// The cost demonstration's split: the frame at full resolution on the left, as the values set it on the right.
    #[value(default = 0.0, toggle, text = "controls-cost_split", help = "help-costSplit")]
    pub cost_split: f32,
}
