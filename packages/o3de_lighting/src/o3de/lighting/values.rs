//! The investigation's values: every number a demonstration, the dock, a
//! timeline, or a story sets, its default, and what it is, as a struct the
//! lab reaches by key (`#[derive(Values)]`, lab/src/values.rs): the
//! declarations of a shot's values. A shot stores them by key
//! (`state["key"]`, `state.p("key")`), and converts them to this struct
//! when code wants them typed (`state.view::<LightingValues>()`). A
//! demonstration's file overrides some (lab/src/demo.rs).
//!
//! The values gShading holds (the light's position, the view, the flags) are
//! declared by their tags in shaders/standard_lighting.wgsl instead;
//! `render-lab --investigation o3de/lighting values` lists both. A field's
//! `label` names the dock group that shows it (controls.rs); `toggle` marks
//! a number used as on or off. **A value's default must change nothing**, so
//! existing frames stay identical.

use super::demos::{DIFFUSE, LIGHT, SPECULAR, SPHERE_LIGHT};
use lab::palette;

#[derive(Clone, Debug, PartialEq, lab::Values, serde::Serialize, serde::Deserialize)]
#[values(package = "o3de_lighting", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct LightingValues {
    /// Light intensity, candelas (m_rgbIntensityCandelas over the light colour).
    #[value(default = 6.0, range = 0.0..=30.0, text = "controls.candelas", color = LIGHT, label(light))]
    pub light_intensity: f32,
    /// Attenuation radius, metres (m_invAttenuationRadiusSquared = 1 / r^2).
    #[value(default = 7.0, range = 0.5..=12.0, unit = m, text = "controls.radius", color = LIGHT, label(light))]
    pub attenuation_radius: f32,
    /// The point light's lighting-channel mask (the editor's default: all five channels, 0x1F).
    #[value(default = 31.0, range = 0.0..=31.0, unit = index, text = "controls.light_mask", color = LIGHT, label(channels))]
    pub light_channels: f32,
    /// The sphere mesh's lighting-channel mask (0x1F by default).
    #[value(default = 31.0, range = 0.0..=31.0, unit = index, text = "controls.sphere_mask", color = palette::TEXT, label(channels))]
    pub sphere_lighting_channels: f32,
    /// More simple point lights over the room, for light culling (0 to 48).
    #[value(default = 0.0, range = 0.0..=48.0, unit = index, text = "controls.lights", color = LIGHT, label(culling))]
    pub extra_lights: f32,
    /// Their attenuation radius, metres.
    #[value(default = 1.0, range = 0.2..=2.5, unit = m, text = "controls.radius", color = LIGHT, label(culling))]
    pub extra_light_radius: f32,
    /// Their intensity, candelas.
    #[value(default = 0.4, range = 0.0..=3.0, name = "candelas", color = LIGHT, label(culling))]
    pub extra_light_intensity: f32,
    /// How far the lights have moved, degrees: each height's ring turns about the room's centre, the three at different rates and directions.
    #[value(default = 0.0, range = -360.0..=360.0, unit = deg, text = "controls.light_motion", color = LIGHT, label(culling))]
    pub extra_light_motion: f32,
    /// A marker on each of those lights; the ones the inspected point's bin keeps (with cullTile on) are ringed and joined to the point.
    #[value(default = 0.0, toggle)]
    pub light_markers: f32,
    /// The inspected point's 16 x 16 tile: its view-space box, its depth bins, and the lights its bin keeps.
    #[value(default = 0.0, toggle)]
    pub cull_tile: f32,
    /// Two decals on the floor beside the sphere: a paint splat and a panel grid (decals.rs).
    #[value(default = 0.0, toggle)]
    pub decals: f32,
    /// The decals' m_opacity (the component's Opacity).
    #[value(default = 1.0, range = 0.0..=1.0, name = "opacity", color = palette::ACCENT, label(decals))]
    pub decal_opacity: f32,
    /// The decals' m_normalMapOpacity.
    #[value(default = 1.0, range = 0.0..=1.0, text = "controls.normal_map_opacity", color = palette::NORMAL, label(decals))]
    pub decal_normal_opacity: f32,
    /// The decals' m_angleAttenuation (the component's Attenuation Angle).
    #[value(default = 1.0, range = 0.0..=1.0, text = "controls.attenuation_angle", color = palette::ACCENT, label(decals))]
    pub decal_angle_attenuation: f32,
    /// The splat's sort key; the grid keeps the default 16, and larger keys draw on top.
    #[value(default = 17.0, range = 0.0..=32.0, unit = index, text = "controls.splat_sort_key", color = palette::ACCENT, label(decals))]
    pub decal_splat_sort_key: f32,
    /// The splat's tilt about x, degrees: the angle between its projection axis and the floor's normal.
    #[value(default = 0.0, range = -80.0..=80.0, unit = deg, text = "controls.splat_tilt", color = palette::ACCENT, label(decals))]
    pub decal_tilt: f32,
    /// The panel grid's turn about the vertical, degrees.
    #[value(default = 0.0, range = -180.0..=180.0, unit = deg, text = "controls.grid_turn", color = palette::REFLECTED, label(decals))]
    pub decal_turn: f32,
    /// The decals' boxes and projection axes.
    #[value(default = 0.0, toggle)]
    pub decal_boxes: f32,
    /// An O3DE point light: a sphere area light, without shadows (simple point lights do no back lighting).
    #[value(default = 0.0, toggle)]
    pub sphere_light: f32,
    /// The sphere light's position x, metres.
    #[value(default = -0.85, range = -2.8..=2.8, unit = m, name = "x", color = palette::AXIS_X, label(sphere_light))]
    pub sphere_light_x: f32,
    /// The sphere light's position y, metres.
    #[value(default = 0.3, range = 0.0..=2.9, unit = m, name = "y", color = palette::AXIS_Y, label(sphere_light))]
    pub sphere_light_y: f32,
    /// The sphere light's position z, metres.
    #[value(default = -2.6, range = -2.9..=2.8, unit = m, name = "z", color = palette::AXIS_Z, label(sphere_light))]
    pub sphere_light_z: f32,
    /// The sphere light's intensity, candelas.
    #[value(default = 2.5, range = 0.0..=10.0, name = "candelas", color = SPHERE_LIGHT, label(sphere_light))]
    pub sphere_light_intensity: f32,
    /// The sphere light's attenuation radius, metres.
    #[value(default = 3.0, range = 0.5..=6.0, unit = m, text = "controls.radius", color = SPHERE_LIGHT, label(sphere_light))]
    pub sphere_light_radius: f32,
    /// The sphere light's bulb radius, metres (the editor's default sphere shape, 0.05 m).
    #[value(default = 0.05, range = 0.0..=0.5, unit = m, text = "controls.bulb_radius", color = SPHERE_LIGHT, label(sphere_light))]
    pub sphere_light_bulb: f32,
    /// Screen-space ambient occlusion (on by default in the main pipeline).
    #[value(default = 1.0, toggle)]
    pub ssao: f32,
    /// SSAO strength (DefaultSsaoStrength 1).
    #[value(default = 1.0, range = 0.0..=1.0, name = "strength", color = palette::ACCENT, label(ssao))]
    pub ssao_strength: f32,
    /// SSAO sampling radius, a fraction of the half-res height (DefaultSsaoSamplingRadius 0.05).
    #[value(default = 0.05, range = 0.0..=0.2, text = "controls.sampling_radius", color = palette::ACCENT, label(ssao))]
    pub ssao_radius: f32,
    /// The SSAO's depth-aware blur.
    #[value(default = 1.0, toggle)]
    pub ssao_blur: f32,
    /// The blur's constant falloff (2/3 without an AO component; the component's default is 0.85).
    #[value(default = 0.6667, range = 0.0..=1.0, text = "controls.blur_falloff", color = palette::ACCENT, label(ssao))]
    pub ssao_blur_const_falloff: f32,
    /// The blur's depth falloff threshold (0).
    #[value(default = 0.0, range = 0.0..=1.0, text = "controls.depth_threshold", color = palette::ACCENT, label(ssao))]
    pub ssao_blur_depth_threshold: f32,
    /// The blur's depth falloff strength (50 without an AO component; the component's default is 200).
    #[value(default = 50.0, range = 0.0..=400.0, text = "controls.depth_strength", color = palette::ACCENT, label(ssao))]
    pub ssao_blur_depth_strength: f32,
    /// Two reflection probes in the room, their cubemaps baked from the room demonstration's start.
    #[value(default = 0.0, toggle)]
    pub probes: f32,
    /// The probes' parallax correction (the component's default: on).
    #[value(default = 1.0, toggle)]
    pub probe_parallax: f32,
    /// The probes' inner extents as a fraction of the placed ones (clamped to the outer extents).
    #[value(default = 1.0, range = 0.0..=1.0, text = "controls.inner_extents", color = palette::ACCENT, label(probes))]
    pub probe_inner_scale: f32,
    /// The probes' render exposure, stops (default 0).
    #[value(default = 0.0, range = -4.0..=4.0, text = "controls.exposure_stops", color = palette::ACCENT, label(probes))]
    pub probe_exposure: f32,
    /// The probes' outer and inner boxes.
    #[value(default = 0.0, toggle)]
    pub probe_boxes: f32,
    /// The probe whose baked cubemap the cubemap views show (1 or 2).
    #[value(default = 1.0, range = 1.0..=2.0, unit = index, text = "demos.probe", color = palette::ACCENT, label(cubemap))]
    pub probe_shown: f32,
    /// That cubemap unfolded as a cross over the viewport (the lab's view, not an O3DE pass; a fraction fades it in).
    #[value(default = 0.0, toggle)]
    pub probe_cubemap: f32,
    /// The cross's mip (0 the 256-texel capture, 5 the roughest prefiltered mip; a fraction blends two).
    #[value(default = 0.0, range = 0.0..=5.0, name = "mip", color = palette::ACCENT, label(cubemap))]
    pub probe_cubemap_mip: f32,
    /// How many of the cross's six faces are baked (the video fills them in, in the cubemap's face order: +X, -X, +Y, -Y, +Z, -Z).
    #[value(default = 6.0, range = 0.0..=6.0, text = "controls.faces_baked", color = palette::ACCENT, label(cubemap))]
    pub probe_bake_faces: f32,
    /// A mirror ball at the probe's position reflecting its cubemap (a fraction fades it in).
    #[value(default = 0.0, toggle)]
    pub probe_ball: f32,
    /// The mirror ball's radius, metres.
    #[value(default = 0.22)]
    pub probe_ball_radius: f32,
    /// The mip the mirror ball reads.
    #[value(default = 0.0, range = 0.0..=5.0, text = "controls.ball_mip", color = palette::REFLECTED, label(cubemap))]
    pub probe_ball_mip: f32,
    /// The teapot as EnhancedPBR with transmission: 0 None (the default), 1 ThickObject, 2 ThinObject.
    #[value(default = 0.0, range = 0.0..=2.0, unit = index, text = "controls.mode", color = palette::ACCENT, label(transmission))]
    pub teapot_transmission: f32,
    /// Transmission thickness (the property's default 0.5).
    #[value(default = 0.5, range = 0.0..=1.0, name = "thickness", color = palette::ACCENT, label(transmission))]
    pub transmission_thickness: f32,
    /// Thick mode: transmission power (default 6).
    #[value(default = 6.0, range = 0.0..=20.0, name = "power", color = palette::ACCENT, label(transmission))]
    pub transmission_power: f32,
    /// Thick mode: transmission distortion (default 0.1).
    #[value(default = 0.1, range = 0.0..=1.0, name = "distortion", color = palette::ACCENT, label(transmission))]
    pub transmission_distortion: f32,
    /// Thick mode: transmission attenuation coefficient (default 4).
    #[value(default = 4.0, range = 0.0..=10.0, name = "attenuation", color = palette::ACCENT, label(transmission))]
    pub transmission_attenuation: f32,
    /// Transmission scale (default 1).
    #[value(default = 1.0, range = 0.0..=4.0, name = "scale", color = palette::ACCENT, label(transmission))]
    pub transmission_scale: f32,
    /// Thin mode: the NdotL bias (default 0.1).
    #[value(default = 0.1, range = -1.0..=1.0, text = "controls.ndotl_bias_thin", color = palette::ACCENT, label(transmission))]
    pub transmission_nd_l_bias: f32,
    /// Thin mode: the distance attenuation (default 0.5).
    #[value(default = 0.5, range = 0.0..=4.0, text = "controls.distance_attenuation_thin", color = palette::ACCENT, label(transmission))]
    pub transmission_distance_attenuation: f32,
    /// The sphere's clearCoat.enable (false by default).
    #[value(default = 0.0, toggle)]
    pub sphere_clear_coat: f32,
    /// ClearCoat.factor (1 by default).
    #[value(default = 1.0, range = 0.0..=1.0, name = "factor", color = palette::ACCENT, label(clear_coat))]
    pub clear_coat_factor: f32,
    /// ClearCoat.roughness, linear (0 by default).
    #[value(default = 0.0, range = 0.0..=1.0, name = "roughness", color = palette::ACCENT, label(clear_coat))]
    pub clear_coat_roughness: f32,
    /// The sphere's diffuse AO and specular cavity maps assigned (1) or not (0, O3DE's default: no texture, occlusion 1).
    #[value(default = 0.0, toggle)]
    pub sphere_occlusion: f32,
    /// DiffuseOcclusion.factor: the exponent on the diffuse AO map.
    #[value(default = 1.0, range = 0.0..=2.0, text = "controls.diffuse_factor", color = DIFFUSE, label(occlusion))]
    pub diffuse_occlusion_factor: f32,
    /// SpecularOcclusion.factor: the exponent on the specular cavity map.
    #[value(default = 1.0, range = 0.0..=2.0, text = "controls.specular_factor", color = SPECULAR, label(occlusion))]
    pub specular_occlusion_factor: f32,
    /// The room's walls drawn (global IBL ignores them either way).
    #[value(default = 1.0, toggle)]
    pub walls: f32,
    /// StandardPBR specularF0.factor of every object (0.5: F0 = 0.04).
    #[value(default = 0.5, range = 0.0..=1.0, name = "specularF0 factor", color = SPECULAR, label(material))]
    pub specular_f0_factor: f32,
    /// Sphere roughness (linear, as authored).
    #[value(default = 0.35, range = 0.0..=1.0, name = "roughness", color = palette::ACCENT, label(material))]
    pub sphere_roughness: f32,
    /// Sphere metallic.
    #[value(default = 0.0, range = 0.0..=1.0, name = "metallic", color = palette::TEXT, label(material))]
    pub sphere_metallic: f32,
    /// Teapot roughness.
    #[value(default = 0.3, range = 0.0..=1.0, name = "roughness", color = palette::ACCENT, label(material))]
    pub teapot_roughness: f32,
    /// Teapot metallic.
    #[value(default = 0.0, range = 0.0..=1.0, name = "metallic", color = palette::TEXT, label(material))]
    pub teapot_metallic: f32,
    /// Rotated box roughness.
    #[value(default = 0.55, range = 0.0..=1.0, name = "roughness", color = palette::ACCENT, label(material))]
    pub box_roughness: f32,
    /// Rotated box metallic.
    #[value(default = 0.0, range = 0.0..=1.0, name = "metallic", color = palette::TEXT, label(material))]
    pub box_metallic: f32,
    /// Floor roughness.
    #[value(default = 0.45, range = 0.0..=1.0, name = "roughness", color = palette::ACCENT, label(material))]
    pub floor_roughness: f32,
    /// Floor metallic.
    #[value(default = 0.0, range = 0.0..=1.0, name = "metallic", color = palette::TEXT, label(material))]
    pub floor_metallic: f32,
    /// The floor's baseColor.factor, scaling its authored colour (0.16, 0.17, 0.19).
    #[value(default = 1.0, range = 0.0..=5.0, unit = factor, name = "baseColor factor", color = palette::TEXT, label(material))]
    pub floor_base_color_factor: f32,
    /// Inspected point x, metres (a click sets it).
    #[value(default = -0.621)]
    pub inspect_x: f32,
    /// Inspected point y.
    #[value(default = 0.900)]
    pub inspect_y: f32,
    /// Inspected point z.
    #[value(default = -0.201)]
    pub inspect_z: f32,
    /// Inspected point's normal x.
    #[value(default = 0.693)]
    pub inspect_n_x: f32,
    /// Inspected point's normal y.
    #[value(default = 0.452)]
    pub inspect_n_y: f32,
    /// Inspected point's normal z.
    #[value(default = 0.562)]
    pub inspect_n_z: f32,
    /// The object of the inspected point (scene ids).
    #[value(default = 5.0)]
    pub inspect_object: f32,
    /// The readout and vectors see the point from the observer (1) or the camera (0).
    #[value(default = 0.0, toggle)]
    pub observer: f32,
    /// Observer position x, metres.
    #[value(default = -0.362)]
    pub observer_x: f32,
    /// Observer position y.
    #[value(default = 2.223)]
    pub observer_y: f32,
    /// Observer position z.
    #[value(default = -0.55)]
    pub observer_z: f32,
    /// N, L, V, and H at the inspected point.
    #[value(default = 0.0, toggle)]
    pub vectors: f32,
    /// The specular lobe at the inspected point, in the plane of N and L.
    #[value(default = 0.0, toggle)]
    pub lobe: f32,
    /// The light's illuminance along the floor, as a curve.
    #[value(default = 0.0, toggle)]
    pub falloff: f32,
    /// The attenuation radius around the light.
    #[value(default = 0.0, toggle)]
    pub radius: f32,
    /// Object labels.
    #[value(default = 1.0, toggle)]
    pub labels: f32,
    /// Tonemapper exposure.
    #[value(default = 1.0, range = 0.2..=4.0, unit = factor, text = "controls.exposure", color = palette::TEXT, label(display))]
    pub exposure: f32,
    /// How many of N, L, V, H, and R are drawn, in that order (the video reveals them one by one; a fraction grows the next one).
    #[value(default = 5.0)]
    pub vectors_shown: f32,
    /// How many readout rows are shown (the video reveals them step by step).
    #[value(default = 99.0)]
    pub readout_rows: f32,
    /// The term lit in the demonstration's formula above the readout (0: none; -1 hides the formula).
    #[value(default = 0.0)]
    pub formula: f32,
    /// GGX sample directions drawn around R at the inspected point, as the specular prefilter takes them (N = V = R).
    #[value(default = 0.0)]
    pub ggx_samples: f32,
    /// Camera sway, as in the lab's other investigations.
    #[value(default = 0.0)]
    pub camera_drift: f32,
}
