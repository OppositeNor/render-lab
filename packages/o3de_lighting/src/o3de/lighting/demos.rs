//! What each shot of the lighting investigation shows: the study room shaded
//! by shaders/standard_lighting.wgsl under one simple point light, the
//! vectors and the specular lobe at the inspected point, the light's falloff,
//! and the readout of every term there, computed by the CPU mirror (brdf.rs).

use super::brdf::{self, BrdfMap, DiffuseModel, Material, PointLight, Shading};
use super::shot::Demo;
use super::transmission::{SphereLight, Transmission, TransmissionMode};
use lab::animation::Ease;
use lab::frame::{DrawLists, FrameContent, Label, Readout, Statics, FLAG_ARROW, FLAG_DASHED};
use lab::i18n::object_name;
use crate::o3de::lighting::text::tr;
use lab::layout::{FrameUniforms, ObjectData};
use lab::palette;
use lab::scene::ids;
use lab::shot::ShotState;
use egui::Color32;
use glam::{Vec2, Vec3};

/// Where the decal records start in gShadingFrame (FRAME_DECALS in the shader),
/// after the materials and up to 49 lights.
pub const FRAME_DECALS: usize = 128;
/// The teapot's transmission record and the sphere lights, after up to eight decals.
pub const FRAME_TRANSMISSION: usize = 176;
pub const FRAME_SPHERE_LIGHTS: usize = 184;
/// The reflection probes, after up to four sphere lights.
pub const FRAME_PROBES: usize = 200;
/// The SSAO settings, after up to four probes.
pub const FRAME_SSAO: usize = 220;
/// The lab's views of a probe's baked cubemap, after the SSAO settings.
pub const FRAME_CUBEMAP_VIEW: usize = 224;

/// The SSAO pass constants: (strength, sampling radius, blur on, unused),
/// (constant falloff, depth falloff threshold, depth falloff strength,
/// unused). Off, SsaoComputePass sets the strength to 0, so the AO is 1.
pub fn ssao_records(state: &ShotState) -> Vec<[f32; 4]> {
    let on = state.p("ssao") > 0.5;
    let strength = if on { state.p("ssaoStrength").clamp(0.0, 1.0) } else { 0.0 };
    let blur = if on && state.p("ssaoBlur") > 0.5 { 1.0 } else { 0.0 };
    vec![[strength, state.p("ssaoRadius"), blur, 0.0], [state.p("ssaoBlurConstFalloff"), state.p("ssaoBlurDepthThreshold"), state.p("ssaoBlurDepthStrength"), 0.0]]
}

/// The light's colour; its intensity scales it (candelas).
pub const LIGHT_COLOR: Vec3 = Vec3::new(1.0, 0.92, 0.82);
pub const LIGHT: Color32 = palette::rgb(0xffd98a);
pub const DIFFUSE: Color32 = palette::rgb(0x8fe3a0);
pub const SPECULAR: Color32 = palette::rgb(0xff8fc8);
/// The half vector H and what is measured from it; also the clear coat's own lobe.
pub const HALF: Color32 = palette::rgb(0xb49cff);
/// GGX's G term (and the IBL's F0 · A + B), apart from D's pink.
pub const TERM_G: Color32 = palette::rgb(0xffa14a);

/// The object a roughness or metallic parameter belongs to.
fn material_params(object: usize) -> Option<(&'static str, &'static str)> {
    match object {
        ids::SPHERE => Some(("sphereRoughness", "sphereMetallic")),
        ids::TEAPOT => Some(("teapotRoughness", "teapotMetallic")),
        ids::BOX => Some(("boxRoughness", "boxMetallic")),
        ids::FLOOR => Some(("floorRoughness", "floorMetallic")),
        _ => None,
    }
}

/// The study room's records with the shot's roughness; the ceiling panel
/// gives no light here (the point light does).
pub fn posed_objects(state: &ShotState, statics: &Statics) -> Vec<ObjectData> {
    pose_objects(statics.scene.records(), state)
}

/// The room's records with the shot's materials.
pub fn pose_objects(mut objects: Vec<ObjectData>, state: &ShotState) -> Vec<ObjectData> {
    for (i, o) in objects.iter_mut().enumerate() {
        if let Some((roughness, _)) = material_params(i) {
            o.albedo_roughness[3] = state.p(roughness);
        }
        if i == ids::FLOOR {
            // baseColor.factor scales the floor's authored colour.
            for c in &mut o.albedo_roughness[..3] {
                *c *= state.p("floorBaseColorFactor");
            }
        }
        if i == ids::PANEL {
            o.emissive = [0.0; 4];
        }
    }
    objects
}

pub fn light(state: &ShotState) -> PointLight {
    let radius = state.p("attenuationRadius").max(0.01);
    PointLight { position: Vec3::new(state.p("lightX"), state.p("lightY"), state.p("lightZ")), inv_attenuation_radius_squared: 1.0 / (radius * radius), rgb_intensity_candelas: LIGHT_COLOR * state.p("lightIntensity"), lighting_channel_mask: state.p("lightChannels").round() as u32 }
}

/// The colours of the light-culling demonstration's lights, in turn.
const EXTRA_LIGHT_COLORS: [Vec3; 4] = [Vec3::new(1.0, 0.55, 0.35), Vec3::new(0.45, 0.75, 1.0), Vec3::new(0.6, 1.0, 0.55), Vec3::new(1.0, 0.85, 0.45)];

/// The light-culling demonstration's lights: `extraLights` small simple point
/// lights over the room (a golden-angle spiral, at three heights), in four
/// colours. `extraLightMotion` turns each height's ring about the room's
/// centre, at its own rate and direction, so lights cross tiles and bins.
pub fn extra_lights(state: &ShotState) -> Vec<PointLight> {
    let count = state.p("extraLights").round().clamp(0.0, 48.0) as usize;
    let radius = state.p("extraLightRadius").max(0.05);
    let colors = EXTRA_LIGHT_COLORS;
    let motion = state.p("extraLightMotion").to_radians();
    (0..count)
        .map(|i| {
            let r = 2.5 * ((i as f32 + 0.5) / 48.0).sqrt();
            let a = i as f32 * 2.399_963 + motion * [1.0, -0.6, 0.35][i % 3];
            let position = Vec3::new(r * a.cos(), [0.2, 0.9, 1.7][i % 3], -0.3 + r * a.sin());
            PointLight { position, inv_attenuation_radius_squared: 1.0 / (radius * radius), rgb_intensity_candelas: colors[i % 4] * state.p("extraLightIntensity"), lighting_channel_mask: brdf::ALL_LIGHTING_CHANNELS }
        })
        .collect()
}

/// Each object's material from the shot: metallic, specularF0Factor,
/// lighting channels, occlusion, and clear coat (`shading` adds the
/// teapot's transmission and each mesh's forward-pass probe).
fn base_materials(state: &ShotState) -> [Material; 12] {
    let mut materials = [Material { metallic: 0.0, specular_f0_factor: state.p("specularF0Factor"), diffuse_occlusion: None, specular_occlusion: None, clear_coat: None, lighting_channels: brdf::ALL_LIGHTING_CHANNELS, transmission: None, forward_probe: None }; 12];
    materials[ids::SPHERE].lighting_channels = state.p("sphereLightingChannels").round() as u32;
    for (i, m) in materials.iter_mut().enumerate() {
        if let Some((_, metallic)) = material_params(i) {
            m.metallic = state.p(metallic);
        }
    }
    // The sphere's occlusion maps (maps.rs), when assigned.
    if state.p("sphereOcclusion") > 0.5 {
        let sphere = &mut materials[ids::SPHERE];
        sphere.diffuse_occlusion = Some(state.p("diffuseOcclusionFactor"));
        sphere.specular_occlusion = Some(state.p("specularOcclusionFactor"));
    }
    // The sphere's clear coat (the clearCoat property group), when enabled.
    if state.p("sphereClearCoat") > 0.5 {
        materials[ids::SPHERE].clear_coat = Some((state.p("clearCoatFactor"), state.p("clearCoatRoughness")));
    }
    materials
}

/// gShading's derived members (shaders/standard_lighting.wgsl; the lab packs
/// its values, lab/src/shader_data.rs): the light's inverse squared radius
/// and intensity, the inspected object, and each object's material.
pub fn shader_data(block: &mut lab::shader_data::Block, state: &ShotState) {
    let light = light(state);
    block.set("invAttenuationRadiusSquared", light.inv_attenuation_radius_squared);
    block.set("rgbIntensityCandelas", light.rgb_intensity_candelas.to_array());
    block.set("inspectedObject", inspected_object(state) as f32);
    block.set("pick", [state.p("inspectX"), state.p("inspectY"), state.p("inspectZ"), if state.p("pickShown") > 0.5 { 1.0 } else { 0.0 }]);
    let materials: Vec<f32> = base_materials(state).iter().flat_map(|m| [m.metallic, m.specular_f0_factor, m.diffuse_occlusion.unwrap_or(-1.0), m.specular_occlusion.unwrap_or(-1.0)]).collect();
    block.write("materials", &materials);
}

/// The option of the choice `key` the shot holds (the last for an index past them).
fn choice<T: lab::values::Choice>(state: &ShotState, key: &str) -> T {
    let last = T::choices().options.len() as u32 - 1;
    T::from_index((state.p(key).round().max(0.0) as u32).min(last)).expect("an option")
}

pub fn shading(state: &ShotState) -> Shading {
    let mut materials = base_materials(state);
    // The teapot as EnhancedPBR with transmission, when its mode is not None.
    materials[ids::TEAPOT].transmission = transmission(state);
    // Each mesh's forward-pass probe, from the room's fixed bounds.
    let probes = super::reflections::scene_probes(state);
    for (o, m) in lab::scene::Scene::study_room().records().iter().zip(materials.iter_mut()) {
        m.forward_probe = super::reflections::forward_probe(&probes, Vec3::from(o.world_center), Vec3::from(o.world_extent));
    }
    Shading {
        light: light(state),
        light_on: state.p("lightOn") > 0.5,
        extra_lights: extra_lights(state),
        view: choice(state, "viewMode"),
        multiscatter: state.p("multiscatter") > 0.5,
        diffuse_model: choice(state, "diffuseModel"),
        inspected: inspected_object(state),
        ibl: state.p("ibl") > 0.5,
        ibl_exposure: state.p("iblExposure"),
        // The albedo and specularF0 targets are 8-bit (R8G8B8A8_UNORM), as in O3DE.
        quantize: true,
        specular_aa: state.p("specularAA") > 0.5,
        materials,
        decals: super::decals::scene_decals(state),
        sphere_lights: sphere_lights(state),
        probes,
    }
}

/// The teapot's transmission inputs (SubsurfaceAndTransmissionPropertyGroup,
/// without a thickness map), packed as SubsurfaceTransmissionParameterFunctor
/// packs them; tint, scatter colour, scatter distance, and shrink factor keep
/// the property group's defaults.
pub fn transmission(state: &ShotState) -> Option<Transmission> {
    let mode = match state.p("teapotTransmission").round() as i32 {
        1 => TransmissionMode::ThickObject,
        2 => TransmissionMode::ThinObject,
        _ => return None,
    };
    let scale = state.p("transmissionScale");
    let params = match mode {
        TransmissionMode::ThickObject => glam::Vec4::new(state.p("transmissionAttenuation"), state.p("transmissionPower"), state.p("transmissionDistortion"), scale),
        TransmissionMode::ThinObject => glam::Vec4::new(0.005, state.p("transmissionNdLBias"), state.p("transmissionDistanceAttenuation"), scale),
    };
    Some(Transmission { mode, tint: Vec3::new(1.0, 0.8, 0.6), thickness: state.p("transmissionThickness"), params, scatter_distance: Vec3::new(1.0, 0.27, 0.13) * 8.0 })
}

/// The O3DE point light (a sphere area light without shadows) when `sphereLight` is on.
pub fn sphere_lights(state: &ShotState) -> Vec<SphereLight> {
    if state.p("sphereLight") < 0.5 {
        return Vec::new();
    }
    let radius = state.p("sphereLightRadius").max(0.01);
    vec![SphereLight {
        position: Vec3::new(state.p("sphereLightX"), state.p("sphereLightY"), state.p("sphereLightZ")),
        inv_attenuation_radius_squared: 1.0 / (radius * radius),
        rgb_intensity_candelas: SPHERE_LIGHT_COLOR * state.p("sphereLightIntensity"),
        bulb_radius: state.p("sphereLightBulb"),
        lighting_channel_mask: brdf::ALL_LIGHTING_CHANNELS,
    }]
}

/// The sphere light's marker colour.
pub const SPHERE_LIGHT: Color32 = Color32::from_rgb(0xff, 0xb3, 0x6b);

/// The sphere light's colour, a warm white.
pub const SPHERE_LIGHT_COLOR: Vec3 = Vec3::new(1.0, 0.86, 0.7);

pub fn inspected_object(state: &ShotState) -> u32 {
    state.p("inspectObject").round().clamp(0.0, ids::TEAPOT as f32) as u32
}

/// Where the readout and the vectors see the inspected point from.
pub fn observer(state: &ShotState, eye: Vec3) -> Vec3 {
    if state.p("observer") > 0.5 { Vec3::new(state.p("observerX"), state.p("observerY"), state.p("observerZ")) } else { eye }
}

pub fn inspected_point(state: &ShotState) -> (Vec3, Vec3) {
    let p = Vec3::new(state.p("inspectX"), state.p("inspectY"), state.p("inspectZ"));
    let n = Vec3::new(state.p("inspectNX"), state.p("inspectNY"), state.p("inspectNZ")).normalize_or(Vec3::Y);
    (p, n)
}

fn f3(v: Vec3) -> String {
    format!("({:.3}, {:.3}, {:.3})", v.x, v.y, v.z)
}

fn f1(v: f32) -> String {
    if v.abs() >= 1000.0 { format!("{v:.0}") } else if v.abs() >= 10.0 { format!("{v:.2}") } else { format!("{v:.4}") }
}

/// How the normal at `p` changes to the next pixel across and down, as the
/// scene pass's ddx_fine and ddy_fine see it: the same object hit one pixel
/// over (zero off its edge, or where `p` is off screen).
pub fn normal_derivatives(objects: &[ObjectData], camera: &lab::layout::CameraData, p: Vec3, object: u32) -> [Vec3; 2] {
    let projector = lab::camera::Projector::new(camera);
    let Some((pixel, _)) = projector.project(p) else { return [Vec3::ZERO; 2] };
    let eye = Vec3::from(camera.position);
    let normal_at = |offset: glam::Vec2| -> Option<Vec3> {
        let (origin, dir) = projector.ray(pixel.floor() + glam::Vec2::splat(0.5) + offset);
        let hit = lab::scene::trace_scene(objects, origin, dir, 100.0, ids::PANEL as u32);
        (hit.is_hit() && hit.object == object).then(|| if hit.normal.dot(eye - (origin + dir * hit.t)) < 0.0 { -hit.normal } else { hit.normal })
    };
    let Some(here) = normal_at(glam::Vec2::ZERO) else { return [Vec3::ZERO; 2] };
    let across = normal_at(glam::Vec2::X).map_or(Vec3::ZERO, |n| n - here);
    let down = normal_at(glam::Vec2::Y).map_or(Vec3::ZERO, |n| n - here);
    [across, down]
}

/// The faces of the unfolded cubemap the merge pass draws (probeCubemapViews
/// in the shader: four cells across, three down, -Z at the centre), named at
/// their cells' tops, and the cubemap and mip under the cross.
fn cubemap_cross_labels(labels: &mut Vec<Label>, state: &ShotState, camera: &lab::layout::CameraData) {
    let projector = lab::camera::Projector::new(camera);
    let size = projector.viewport;
    let cell = (size.x / 4.0).min(size.y / 3.0) * 0.94;
    let origin = (size - Vec2::new(4.0, 3.0) * cell) * 0.5;
    let at = |x: f32, y: f32| {
        let (eye, dir) = projector.ray(origin + Vec2::new(x, y) * cell);
        eye + dir
    };
    for (x, y, name) in [(1.0, 0.0, "+Y"), (0.0, 1.0, "-X"), (1.0, 1.0, "-Z"), (2.0, 1.0, "+X"), (3.0, 1.0, "+Z"), (1.0, 2.0, "-Y")] {
        labels.push(Label { anchor: at(x + 0.5, y + 0.12), text: name.into(), color: palette::text() });
    }
    let mip = state.p("probeCubemapMip").clamp(0.0, (super::ibl::SPECULAR_MIPS - 1) as f32);
    let texels = super::ibl::SPECULAR_SIZE >> mip.round() as u32;
    let text = format!("{} {} · mip {:.1} · {texels} × {texels}", tr("demos.probe"), super::reflections::shown_cubemap(state) + 1, mip);
    labels.push(Label { anchor: at(3.0, 2.5), text, color: palette::accent() });
}

/// A direction perpendicular to `n` in the plane of `n` and `l`.
fn in_plane(n: Vec3, l: Vec3) -> Vec3 {
    let t = l - n * n.dot(l);
    if t.length_squared() > 1e-6 { t.normalize() } else { n.any_orthonormal_vector() }
}

pub fn build_frame(state: &ShotState, statics: &Statics, viewport: [f32; 2], pixel_scale: f32, time: f32) -> FrameContent {
    let demo = Demo::of(state.demo);
    let objects = posed_objects(state, statics);
    let shading = shading(state);
    let lighting = statics.scene.lighting;
    let camera = state.camera.camera_data(viewport);
    let eye = Vec3::from(camera.position);
    let exposure = state.p("exposure");
    let walls = (1 << ids::BACK_WALL) | (1 << ids::LEFT_WALL) | (1 << ids::RIGHT_WALL);
    // Every scene's light panel (the study room's is ids::PANEL; the terrain's is its second object).
    let panels = objects.iter().enumerate().filter(|(_, o)| o.shape_kind == lab::scene::SHAPE_PANEL).fold(0u32, |m, (i, _)| m | 1 << i);
    let hidden: u32 = panels | if state.p("walls") < 0.5 { walls } else { 0 };
    let uniforms = FrameUniforms {
        camera,
        light_center_radiance: [lighting.light.center.x, lighting.light.center.y, lighting.light.center.z, 0.0],
        light_axis_u: lighting.light.axis_u.extend(0.0).to_array(),
        light_axis_v: lighting.light.axis_v.extend(0.0).to_array(),
        light_color: lighting.light.color.extend(1.0).to_array(),
        // The sky box's level (gFrame.ambientColor scales skyRadiance).
        ambient_color: [super::ibl::SKY_LEVEL, super::ibl::SKY_LEVEL, super::ibl::SKY_LEVEL, 1.0],
        floor_params: [state.p("floorRoughness"), lighting.f0, 0.0, 1.0],
        overlay_params: [exposure, pixel_scale, time, 0.0],
        counts: [objects.len() as u32, ids::FLOOR as u32, 0, hidden],
        view_params: [1.0, 0.0, 0.0, 0.0],
    };
    // gShading is the lab's to pack (`shader_data`).
    let mut content = FrameContent::new(objects, uniforms);
    // The full-screen passes rebuild positions from depth (shaders/standard_lighting.wgsl).
    let inverse = glam::Mat4::from_cols_array_2d(&uniforms.camera.world_to_clip).inverse();
    content.shading_frame = inverse.to_cols_array_2d().to_vec();
    content.shading_frame.extend(shading.frame_materials());
    content.shading_frame.extend(shading.frame_lights());
    content.shading_frame.resize(FRAME_DECALS, [0.0; 4]);
    content.shading_frame.extend(super::decals::frame_records(&shading.decals));
    content.shading_frame.resize(FRAME_TRANSMISSION, [0.0; 4]);
    let transmissive = shading.materials.iter().position(|m| m.transmission.is_some()).unwrap_or(0);
    content.shading_frame.extend(super::transmission::frame_record(shading.materials[transmissive].transmission.as_ref(), transmissive));
    content.shading_frame.resize(FRAME_SPHERE_LIGHTS, [0.0; 4]);
    content.shading_frame.extend(super::transmission::frame_lights(&shading.sphere_lights));
    content.shading_frame.resize(FRAME_PROBES, [0.0; 4]);
    content.shading_frame.extend(super::reflections::frame_records(&shading.probes));
    content.shading_frame.resize(FRAME_SSAO, [0.0; 4]);
    content.shading_frame.extend(ssao_records(state));
    content.shading_frame.resize(FRAME_CUBEMAP_VIEW, [0.0; 4]);
    content.shading_frame.extend(super::reflections::cubemap_view_records(state));
    content.shading_frame.resize(FRAME_CASCADES, [0.0; 4]);
    content.shading_frame.extend(cascade_records(state, &uniforms.camera));

    let map = BrdfMap::get();
    let (p, n) = inspected_point(state);
    let object = inspected_object(state);
    let seen_from = observer(state, eye);
    let derivatives = normal_derivatives(&content.objects, &camera, p, object);
    let px = brdf::pixel(&shading, &content.objects, p, n, seen_from, object, exposure, map, derivatives);
    // SSAO's occluders for the inspected point's half-res pixel (ssao::samples_at):
    // the first `ssaoSamples` of its 52, each coloured by saturate(cos) against
    // the normal rebuilt from depth (dark green unoccluding, bright occluding)
    // and sized by its weight 1 / (d^2 + 0.07).
    let shown_samples = state.p("ssaoSamples").clamp(0.0, 52.0);
    let ssao_overlay = (shown_samples > 0.0).then(|| ssao_samples(state, &content.objects, &camera)).flatten();
    let draw = &mut content.draw;
    let labels = &mut content.labels;

    if let Some((world_p, world_n, samples)) = &ssao_overlay {
        draw.segment(*world_p, *world_p + *world_n * 0.25, palette::NORMAL, 1.0, 3.0, FLAG_ARROW);
        for (k, (o, cosine, weight)) in samples.iter().enumerate() {
            let fade = (shown_samples - k as f32).clamp(0.0, 1.0);
            let color = mix_color(palette::rgb(0x3f9e6a), palette::rgb(0xd8ff8a), *cosine);
            draw.point(*o, 2.5 + 5.0 * (weight / (1.0 / 0.07)).sqrt(), color, fade);
            draw.segment(*world_p, *o, color, 0.35 * fade, 1.0, 0);
        }
    }
    // The light, and its attenuation radius.
    let lp = shading.light.position;
    if shading.light_on {
        draw.point(lp, 11.0, LIGHT, 1.0);
        if state.p("labels") > 0.0 {
            labels.push(Label { anchor: lp, text: format!("{} · {:.1} cd", tr("common.simple_point_light"), state.p("lightIntensity")), color: LIGHT });
        }
    }
    // The sphere lights: the bulb, as a point at its size.
    for light in &shading.sphere_lights {
        draw.point(light.position, 14.0, SPHERE_LIGHT, 1.0);
        if state.p("labels") > 0.0 || demo == Demo::Transmission {
            labels.push(Label { anchor: light.position, text: format!("{} · {:.1} cd", tr("common.point_light_sphere"), state.p("sphereLightIntensity")), color: SPHERE_LIGHT });
        }
    }
    let radius = shading.light.attenuation_radius();
    if state.p("radius") > 0.0 {
        attenuation_radius(draw, lp, radius, state.p("radius"));
    }
    if state.p("falloff") > 0.0 {
        falloff_curve(draw, labels, &shading.light, state.p("falloff"));
    }

    // The inspected point's vectors and lobe.
    let vectors = state.p("vectors");
    if vectors > 0.0 {
        let len = 0.5;
        let l = px.light.dir_to_light;
        let v = px.lighting.dir_to_camera;
        draw.point(p, 8.0, palette::SELECTED, vectors);
        // N, L, V, H, then R, the first `vectorsShown` of them.
        let mut shown = vec![(n, palette::NORMAL, "N")];
        if shading.light_on {
            shown.push((l, LIGHT, "L"));
        }
        shown.push((v, palette::VIEW, "V"));
        if shading.light_on {
            shown.push((px.terms.half_vector, HALF, "H"));
        }
        if demo == Demo::Ibl {
            shown.push((px.ibl.reflect_dir, palette::REFLECTED, "R"));
        }
        // A fractional count grows the next vector out of the point.
        let shown_count = state.p("vectorsShown").max(0.0);
        let count = shown_count.ceil() as usize;
        for (i, &(dir, color, name)) in shown.iter().take(count).enumerate() {
            let grown = if shown_count >= (i + 1) as f32 { 1.0 } else { Ease::Smooth.apply(shown_count - i as f32) };
            draw.segment(p, p + dir * len * grown, color, vectors, 4.0, FLAG_ARROW);
            if grown > 0.6 {
                labels.push(Label { anchor: p + dir * (len * grown + 0.04), text: name.into(), color });
            }
        }
        if shading.light_on && shown.iter().take(count).any(|s| s.2 == "L") {
            draw.segment(p, lp, LIGHT, vectors * 0.5, 1.5, FLAG_DASHED);
        }
        if state.p("observer") > 0.5 {
            draw.point(seen_from, 9.0, palette::VIEW, vectors);
            draw.segment(p, seen_from, palette::VIEW, vectors * 0.5, 1.5, FLAG_DASHED);
            labels.push(Label { anchor: seen_from, text: tr("demos.observer").into(), color: palette::VIEW });
        }
    }
    if state.p("lobe") > 0.0 {
        lobes(draw, labels, &px, n, state.p("lobe"));
    }
    let samples = state.p("ggxSamples").round().clamp(0.0, 256.0) as u32;
    if samples > 0 {
        ggx_samples(draw, &px, samples);
    }

    if state.p("labels") > 0.0 {
        for (i, anchor) in [(ids::SPHERE, Vec3::new(-1.05, 1.36, -0.55)), (ids::TEAPOT, Vec3::new(-0.25, 0.72, -1.75)), (ids::BOX, Vec3::new(1.05, 1.02, 0.15))] {
            let Some((roughness, metallic)) = material_params(i) else { continue };
            let text = format!("{} · metallic {:.2} · roughness {:.2}", object_name(statics.scene.objects[i].name), state.p(metallic), state.p(roughness));
            labels.push(Label { anchor, text, color: palette::text() });
        }
    }

    if state.p("probeBoxes") > 0.0 {
        for probe in &shading.probes {
            let color = [palette::accent(), palette::REFLECTED][probe.cubemap.min(1)];
            obb_edges(&mut content.draw, probe.position, probe.rotation, probe.outer_half, color, state.p("probeBoxes") * 0.6, 2.0);
            obb_edges(&mut content.draw, probe.position, probe.rotation, probe.inner_half, color, state.p("probeBoxes"), 3.0);
            content.draw.point(probe.position, 9.0, color, state.p("probeBoxes"));
            content.labels.push(Label { anchor: probe.position, text: format!("{} {}", tr("demos.probe"), probe.cubemap + 1), color });
        }
    }
    if state.p("decalBoxes") > 0.0 {
        decal_boxes(&mut content.draw, &mut content.labels, &shading.decals, state.p("decalBoxes"));
    }
    if demo == Demo::Culling && (state.p("cullTile") > 0.0 || state.p("lightMarkers") > 0.0) {
        let (_, tile) = culling_view(state, &shading, &content.objects, &uniforms.camera);
        if state.p("cullTile") > 0.0 {
            tile_overlay(&mut content.draw, &uniforms.camera, &tile, state.p("cullTile"));
        }
        if state.p("lightMarkers") > 0.0 {
            light_markers(&mut content.draw, &shading, (state.p("cullTile") > 0.0).then_some(&tile), state.p("lightMarkers"));
        }
    }
    if state.p("cascades") > 0.0 {
        cascade_overlay(&mut content.draw, &mut content.labels, state, p, n);
    }
    let readout = readout(demo, state, &shading, &px, statics, &content.objects, &uniforms.camera);
    content.readout = Some(readout);
    content.notes = notes(demo, &shading);
    // The unfolded cubemap covers the room, so the room's markers and labels give way to its faces' names.
    if state.p("probeCubemap") > 0.5 {
        content.draw = DrawLists::default();
        content.labels.clear();
        cubemap_cross_labels(&mut content.labels, state, &content.uniforms.camera);
    }
    content
}

/// The observer camera of the cascades demonstration: in the room's front
/// corner, looking at the sphere, turned by `observerYaw`, with the shadow
/// far as its depth (cascades.rs).
pub fn cascade_observer(state: &ShotState) -> super::cascades::Observer {
    let position = Vec3::new(2.3, 1.6, 2.3);
    let look = (Vec3::new(-1.05, 0.9, -0.55) - position).normalize();
    let forward = glam::Quat::from_rotation_y(state.p("observerYaw").to_radians()) * look;
    super::cascades::Observer { position, forward, fov_y: 60f32.to_radians(), aspect: 16.0 / 9.0, near: 0.1, far: state.p("shadowFar").max(0.2) }
}

/// With `splitView`, two views from the same camera: on the left the frame in
/// `splitViewMode`, on the right (the main view) as `viewMode` shows it, each
/// titled with its view mode.
pub fn panes(state: &ShotState) -> Vec<lab::investigation::Pane> {
    if state.p("splitView") < 0.5 {
        return Vec::new();
    }
    use lab::values::Choice;
    let options = brdf::ViewMode::choices().options;
    let title = |mode: f32| {
        let option = &options[(mode.round().max(0.0) as usize).min(options.len() - 1)];
        option.text.map(tr).or(option.label).unwrap_or(option.name).to_string()
    };
    let mut left = state.clone();
    left.set("viewMode", state.p("splitViewMode"));
    vec![
        lab::investigation::Pane { state: left, title: Some(title(state.p("splitViewMode"))) },
        lab::investigation::Pane { state: state.clone(), title: Some(title(state.p("viewMode"))) },
    ]
}

/// Where gShadingFrame holds the sun's cascades (shaders/standard_lighting.wgsl FRAME_CASCADES).
pub const FRAME_CASCADES: usize = 240;

/// The camera the sun's cascades cover: the camera that renders, as O3DE
/// takes the active camera's (DirectionalLightComponentController::
/// GetCameraConfiguration), its far plane the light's shadow far; or, with
/// `cascadeCamera`, the fixed observer.
pub fn shadow_camera(state: &ShotState, camera: &lab::layout::CameraData) -> super::cascades::Observer {
    if state.p("cascadeCamera") > 0.5 {
        return cascade_observer(state);
    }
    let view = glam::Mat4::from_cols_array_2d(&camera.world_to_view);
    let forward = -Vec3::new(view.x_axis.z, view.y_axis.z, view.z_axis.z).normalize();
    let aspect = camera.viewport_size[0] / camera.viewport_size[1].max(1.0);
    super::cascades::Observer { position: Vec3::from(camera.position), forward, fov_y: camera.vertical_fov, aspect, near: camera.near_plane, far: state.p("shadowFar").min(camera.far_plane).max(0.2) }
}

/// gShadingFrame[FRAME_CASCADES..]: each cascade's world to shadow-map
/// transform (cascades::world_to_shadowmap, by column), then each cascade's
/// (near, far, texel in metres, depth range in metres), then the sun: (its direction of
/// travel, the cascade count), (white times sunLux, on), and
/// (normalShadowBias, shadowBias, shadowBlur, cascadeColors), and (the
/// cascades' camera depth span, unused).
pub fn cascade_records(state: &ShotState, camera: &lab::layout::CameraData) -> Vec<[f32; 4]> {
    let observer = shadow_camera(state, camera);
    let set = super::cascades::cascades(observer, sun_direction(state), state.p("cascadeSplitRatio"));
    let mut out = Vec::new();
    for i in 0..super::cascades::CASCADES {
        out.extend(set.world_to_shadowmap(i).to_cols_array_2d());
    }
    for c in &set.cascades {
        out.push([c.near, c.far, c.texel(), c.max_y - c.min_y]);
    }
    let d = sun_direction(state);
    let lux = state.p("sunLux");
    out.push([d.x, d.y, d.z, super::cascades::CASCADES as f32]);
    out.push([lux, lux, lux, if lux > 0.0 { 1.0 } else { 0.0 }]);
    out.push([state.p("normalShadowBias"), state.p("shadowBias"), state.p("shadowBlur"), state.p("cascadeColors")]);
    // m_far_minus_near: the camera's depth span the cascades cover, which
    // GetThickness scales a cascade's depth difference by.
    out.push([observer.far - observer.near, 0.0, 0.0, 0.0]);
    out
}

/// The directional light's direction, from `sunYaw` and `sunPitch` (below the horizon).
pub fn sun_direction(state: &ShotState) -> Vec3 {
    let (yaw, pitch) = (state.p("sunYaw").to_radians(), state.p("sunPitch").to_radians());
    Vec3::new(pitch.cos() * yaw.sin(), -pitch.sin(), pitch.cos() * yaw.cos()).normalize()
}

pub const CASCADE_COLORS: [Color32; 4] = [palette::rgb(0x64dfd1), palette::rgb(0x8fb8ff), palette::rgb(0xffd98a), palette::rgb(0xff8fc8)];

/// The observer's frustum split into its cascades (the first `cascadesShown`,
/// a fraction growing the next), each cascade's bounding sphere and snapped
/// light-space box, the light's direction, and the inspected point's cascade.
fn cascade_overlay(draw: &mut DrawLists, labels: &mut Vec<Label>, state: &ShotState, p: Vec3, n: Vec3) {
    use super::cascades;
    let alpha = state.p("cascades");
    let observer = cascade_observer(state);
    let dir = sun_direction(state);
    let set = cascades::cascades(observer, dir, state.p("cascadeSplitRatio"));
    let shown = state.p("cascadesShown").clamp(0.0, cascades::CASCADES as f32);
    draw.point(observer.position, 12.0, palette::VIEW, alpha);
    labels.push(Label { anchor: observer.position + Vec3::Y * 0.12, text: tr("demos.observer_camera").into(), color: palette::VIEW });
    let near = observer.corners(observer.near);
    for (i, c) in set.cascades.iter().enumerate() {
        let grown = (shown - i as f32).clamp(0.0, 1.0);
        if grown <= 0.0 {
            break;
        }
        let color = CASCADE_COLORS[i];
        let far = c.near + (c.far - c.near) * Ease::Smooth.apply(grown);
        let (a, b) = (observer.corners(c.near), observer.corners(far));
        let ring: Vec<Vec3> = b.iter().chain(std::iter::once(&b[0])).copied().collect();
        draw.polyline(&ring, color, alpha, 3.0);
        for k in 0..4 {
            draw.segment(if i == 0 { near[k] } else { a[k] }, b[k], color, alpha * 0.8, 2.0, 0);
        }
        if grown >= 1.0 {
            labels.push(Label { anchor: (b[2] + b[3]) * 0.5, text: format!("{} {i} · {:.2} m", tr("demos.cascade"), c.far), color });
        }
        let spheres = state.p("cascadeSpheres") * grown;
        if spheres > 0.0 {
            for (u, v) in [(set.frame.x, set.frame.y), (set.frame.y, set.frame.z), (set.frame.x, set.frame.z)] {
                let circle: Vec<Vec3> = (0..=64).map(|t| {
                    let a = t as f32 / 64.0 * std::f32::consts::TAU;
                    c.center + (u * a.cos() + v * a.sin()) * c.radius
                }).collect();
                draw.polyline(&circle, color, alpha * spheres * 0.55, 1.5);
            }
            draw.point(c.center, 7.0, color, alpha * spheres);
        }
        let boxes = state.p("cascadeBoxes") * grown;
        if boxes > 0.0 {
            let (lo, hi) = (c.snapped_min, c.snapped_max);
            let corner = |x: bool, y: bool, z: bool| set.frame.to_world(Vec3::new(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z }));
            for (x0, y0, z0, x1, y1, z1) in [
                (0, 0, 0, 1, 0, 0), (0, 1, 0, 1, 1, 0), (0, 0, 1, 1, 0, 1), (0, 1, 1, 1, 1, 1),
                (0, 0, 0, 0, 1, 0), (1, 0, 0, 1, 1, 0), (0, 0, 1, 0, 1, 1), (1, 0, 1, 1, 1, 1),
                (0, 0, 0, 0, 0, 1), (1, 0, 0, 1, 0, 1), (0, 1, 0, 0, 1, 1), (1, 1, 0, 1, 1, 1),
            ] {
                draw.segment(corner(x0 == 1, y0 == 1, z0 == 1), corner(x1 == 1, y1 == 1, z1 == 1), color, alpha * boxes * 0.7, 1.5, 0);
            }
        }
    }
    // The light, shining along its direction.
    let from = Vec3::new(-2.2, 2.7, -2.2) - dir * 0.9;
    draw.segment(from, from + dir * 0.9, LIGHT, alpha, 4.0, FLAG_ARROW);
    labels.push(Label { anchor: from - dir * 0.1, text: tr("demos.directional_light").into(), color: LIGHT });
    // The inspected point's cascade.
    draw.point(p, 9.0, palette::SELECTED, alpha);
    let text = match set.select(p, n) {
        Some(i) if (i as f32) < shown => format!("{} {i}", tr("demos.cascade")),
        Some(_) => String::new(),
        None => tr("demos.outside_every_cascade").into(),
    };
    if !text.is_empty() {
        labels.push(Label { anchor: p + Vec3::Y * 0.1, text, color: palette::SELECTED });
    }
}

/// The names the demonstration gives the scene's decals (decals::scene_decals).
fn decal_name(texture: usize) -> &'static str {
    [tr("demos.splat"), tr("demos.grid")][texture.min(1)]
}

/// The twelve edges of a box.
fn obb_edges(draw: &mut DrawLists, center: Vec3, rotation: glam::Quat, half: Vec3, color: Color32, alpha: f32, width: f32) {
    let [x, y, z] = [rotation * Vec3::X * half.x, rotation * Vec3::Y * half.y, rotation * Vec3::Z * half.z];
    let corner = |i: u32| center + x * if i & 1 != 0 { 1.0 } else { -1.0 } + y * if i & 2 != 0 { 1.0 } else { -1.0 } + z * if i & 4 != 0 { 1.0 } else { -1.0 };
    for i in 0..8u32 {
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                draw.segment(corner(i), corner(i | bit), color, alpha, width, 0);
            }
        }
    }
}

/// Each decal's box (m_position, m_quaternion, m_halfSize), its projection
/// axis (local z, the decalUp of GetDecalAttenuation), and its sort key.
fn decal_boxes(draw: &mut DrawLists, labels: &mut Vec<Label>, decals: &[super::decals::Decal], alpha: f32) {
    for d in decals {
        let color = [palette::accent(), palette::REFLECTED][d.texture.min(1)];
        let [x, y, z] = d.axes();
        let corner = |i: u32| d.position + x * d.half_size.x * if i & 1 != 0 { 1.0 } else { -1.0 } + y * d.half_size.y * if i & 2 != 0 { 1.0 } else { -1.0 } + z * d.half_size.z * if i & 4 != 0 { 1.0 } else { -1.0 };
        for i in 0..8u32 {
            for bit in [1, 2, 4] {
                if i & bit == 0 {
                    draw.segment(corner(i), corner(i | bit), color, alpha * 0.8, 2.0, 0);
                }
            }
        }
        draw.segment(d.position, d.position + z * (d.half_size.z + 0.25), color, alpha, 3.0, FLAG_ARROW);
        labels.push(Label { anchor: d.position + z * (d.half_size.z + 0.3), text: format!("{} · sortKey {}", decal_name(d.texture), d.sort_key), color });
    }
}

/// Three great circles around the light at its attenuation radius, and the
/// circle where that sphere meets the floor.
fn attenuation_radius(draw: &mut DrawLists, center: Vec3, radius: f32, alpha: f32) {
    let circle = |axis_a: Vec3, axis_b: Vec3, r: f32, c: Vec3| -> Vec<Vec3> { (0..=96).map(|k| k as f32 / 96.0 * std::f32::consts::TAU).map(|a| c + (axis_a * a.cos() + axis_b * a.sin()) * r).collect() };
    for (a, b) in [(Vec3::X, Vec3::Y), (Vec3::Y, Vec3::Z), (Vec3::X, Vec3::Z)] {
        draw.polyline(&circle(a, b, radius, center), LIGHT, alpha * 0.35, 1.5);
    }
    if center.y < radius {
        let r = (radius * radius - center.y * center.y).sqrt();
        draw.polyline(&circle(Vec3::X, Vec3::Z, r, Vec3::new(center.x, 0.005, center.z)), LIGHT, alpha * 0.9, 3.0);
    }
}

/// The illuminance on the floor along +x from below the light, as a curve
/// standing on the floor: inverse-square alone (dashed) and with
/// SimplePointLight's radius attenuation (solid), both on one scale.
fn falloff_curve(draw: &mut DrawLists, labels: &mut Vec<Label>, light: &PointLight, alpha: f32) {
    let foot = Vec3::new(light.position.x, 0.0, light.position.z);
    let dir = Vec3::new(1.0, 0.0, 0.0);
    let reach = 3.0;
    let height = 1.2;
    let candelas = light.rgb_intensity_candelas.y;
    // Scaled so the inverse-square value one light height away is 1.
    let unit = candelas / (light.position.y * light.position.y);
    let point = |x: f32, value: f32| foot + dir * x + Vec3::Y * (height * (value / unit).min(1.4));
    let mut plain = Vec::new();
    let mut attenuated = Vec::new();
    for k in 0..=120 {
        let x = reach * k as f32 / 120.0;
        let s = brdf::simple_point_light(light, foot + dir * x);
        plain.push(point(x, candelas / s.distance_squared));
        attenuated.push(point(x, if s.falloff < 1.0 { s.intensity.y } else { 0.0 }));
    }
    draw.segment(foot, foot + dir * reach, palette::text_faint(), alpha * 0.6, 1.5, FLAG_ARROW);
    for w in plain.windows(2) {
        draw.segment(w[0], w[1], palette::text_dim(), alpha * 0.8, 2.0, FLAG_DASHED);
    }
    draw.polyline(&attenuated, LIGHT, alpha, 3.5);
    labels.push(Label { anchor: plain[40], text: "I / d²".into(), color: palette::text_dim() });
    labels.push(Label { anchor: attenuated[70], text: "I / d² · (1 − (d²/r²)²)²".into(), color: LIGHT });
}

/// The diffuse and specular lobes at the inspected point, for an outgoing
/// direction swept through the plane of N and L, horizon to horizon, as a
/// radius (each lobe on its own scale). The diffuse lobe is the term itself,
/// the radiance seen from that direction, which Lambert keeps constant until
/// the response flattens it. The specular lobe is the light the term reflects
/// into that direction, the term times N·V: its radiance grows toward the
/// horizon, where G's division by N·V outpaces it, and would stretch the lobe
/// flat along the surface instead of around the mirror direction.
fn lobes(draw: &mut DrawLists, labels: &mut Vec<Label>, px: &brdf::Pixel, n: Vec3, alpha: f32) {
    let s = &px.surface;
    let l = px.light.dir_to_light;
    let t = in_plane(n, l);
    let samples = 90;
    let mut specular = Vec::new();
    let mut diffuse = Vec::new();
    for k in 0..=samples {
        let a = (k as f32 / samples as f32 - 0.5) * std::f32::consts::PI;
        let w = n * a.cos() + t * a.sin();
        let n_dot_v = w.dot(n).clamp(0.0, 1.0);
        let spec = brdf::specular_ggx(w, l, n, s.specular_f0, n_dot_v, s.roughness_a2, px.lighting.multi_scatter_compensation);
        let response = Vec3::ONE - brdf::fresnel_schlick_with_roughness(n_dot_v, s.specular_f0, s.roughness_linear);
        let diff = brdf::diffuse_lambertian(s.albedo, n, l, response);
        specular.push((w, spec.dot(Vec3::splat(1.0 / 3.0)) * n_dot_v));
        diffuse.push((w, diff.dot(Vec3::splat(1.0 / 3.0))));
    }
    let p = s.position;
    for (lobe, color, size) in [(&specular, SPECULAR, 0.8), (&diffuse, DIFFUSE, 0.4)] {
        let peak = lobe.iter().map(|x| x.1).fold(0.0f32, f32::max);
        if peak <= 0.0 {
            continue;
        }
        let points: Vec<Vec3> = std::iter::once(p).chain(lobe.iter().map(|(w, v)| p + *w * (size * v / peak))).chain(std::iter::once(p)).collect();
        draw.polyline(&points, color, alpha, 3.0);
        // Say what the specular lobe is, at its tip.
        if color == SPECULAR {
            let tip = lobe.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map_or(n, |x| x.0);
            labels.push(Label { anchor: p + tip * (size + 0.06), text: tr("demos.specular_lobe").into(), color: SPECULAR });
        }
    }
    // The mirror direction of L, where the specular lobe of a smooth surface peaks.
    let mirror = (2.0 * n.dot(l) * n - l).normalize();
    draw.segment(p, p + mirror * 0.65, SPECULAR, alpha * 0.6, 1.5, FLAG_DASHED);
}

/// The first `count` of the 256 directions the specular prefilter samples for
/// a texel along R (N = V = R), at the inspected surface's roughness.
fn ggx_samples(draw: &mut DrawLists, px: &brdf::Pixel, count: u32) {
    let r = px.ibl.reflect_dir;
    let p = px.surface.position;
    for i in 0..count {
        let h = brdf::importance_sample_ggx(brdf::hammersley(i, 256), px.surface.roughness_a, r);
        let l = 2.0 * r.dot(h) * h - r;
        let weight = r.dot(l);
        if weight > 0.0 {
            draw.segment(p, p + l * 0.55, SPECULAR, 0.25 + 0.6 * weight, 1.5, 0);
        }
    }
}

/// The inspected point's tile as the culling demonstration shows it.
pub struct CullTile {
    pub tile: [u32; 2],
    pub data: super::culling::TileLightData,
    /// The point's bin, and the lights the bin keeps (group 1 of its remapped list).
    pub bin: u32,
    pub kept: Vec<u32>,
    /// The simple point lights the tile's list holds, for any bin.
    pub listed: Vec<u32>,
    pub point: Vec3,
}

/// The culling readout and the inspected point's tile, from the CPU mirror on
/// the tile's ray-cast depths.
pub fn culling_view(state: &ShotState, shading: &Shading, objects: &[ObjectData], camera: &lab::layout::CameraData) -> (Vec<(String, String, Color32)>, CullTile) {
    use super::culling;
    let (p, _) = inspected_point(state);
    let projector = lab::camera::Projector::new(camera);
    let pixel = projector.project(p).map_or(Vec2::ZERO, |(px, _)| px);
    let tile = [(pixel.x.max(0.0) as u32) / culling::TILE_DIM, (pixel.y.max(0.0) as u32) / culling::TILE_DIM];
    let data = culling::cpu_tile(camera, objects, tile);
    let lights = shading.lights();
    let list = culling::cull_tile(camera, &data, tile, &lights, &shading.decals, &shading.sphere_lights);
    let distance = -glam::Mat4::from_cols_array_2d(&camera.world_to_view).transform_point3(p).z;
    let bin = data.bin(distance);
    let bins = culling::remap(&list);
    let kept: Vec<u32> = bins[bin as usize].split(|&v| v == culling::END_OF_GROUP).nth(culling::GROUP_SIMPLE_POINT as usize).unwrap_or(&[]).to_vec();
    let in_tile = list.len() - culling::LIGHT_GROUPS as usize;
    let listed: Vec<u32> = list.split(|&e| e >> 16 == culling::END_OF_GROUP).nth(culling::GROUP_SIMPLE_POINT as usize).unwrap_or(&[]).iter().map(|e| e >> 16).collect();
    let reach = lights.iter().filter(|l| (l.position - p).length_squared() * l.inv_attenuation_radius_squared < 1.0).count();
    let rows = vec![
        (tr("demos.tile_16_x_16_px").to_string(), format!("({}, {})", tile[0], tile[1]), palette::text()),
        ("zNear … zFar".to_string(), format!("{:.2} … {:.2} m", data.z_near, data.z_far), palette::accent()),
        (tr("demos.depth_bins_2_logmaxbins").to_string(), format!("{}", 1u32 << data.log_max_bins), palette::accent()),
        (tr("demos.32_slice_mask").to_string(), format!("0x{:08X}", data.mask), palette::accent()),
        (tr("demos.simple_point_lights").to_string(), format!("{}", lights.len()), LIGHT),
        (tr("demos.in_the_tile_s_list").to_string(), format!("{in_tile}"), LIGHT),
        (tr("demos.the_point_s_bin").to_string(), format!("{bin}  ({:.2} m)", distance), palette::accent()),
        (tr("demos.in_that_bin").to_string(), format!("{}", kept.len()), LIGHT),
        (tr("demos.lights_reaching_the_point").to_string(), format!("{reach}"), DIFFUSE),
    ];
    (rows, CullTile { tile, data, bin, kept, listed, point: p })
}

/// The tile's view-space box in the world, its depth bins as slices, and the
/// lights its bin keeps.
fn tile_overlay(draw: &mut DrawLists, camera: &lab::layout::CameraData, tile: &CullTile, alpha: f32) {
    use super::culling;
    let to_world = glam::Mat4::from_cols_array_2d(&camera.world_to_view).inverse();
    let projection = glam::Mat4::from_cols_array_2d(&camera.view_to_clip);
    let grid = Vec2::splat(culling::TILE_DIM as f32) / Vec2::from(camera.viewport_size);
    let corner = |uv: Vec2, distance: f32| {
        let ndc = Vec2::new(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
        to_world.transform_point3(Vec3::new(ndc.x * distance / projection.x_axis.x, ndc.y * distance / projection.y_axis.y, -distance))
    };
    let uv0 = Vec2::new(tile.tile[0] as f32, tile.tile[1] as f32) * grid;
    let uvs = [uv0, uv0 + Vec2::new(grid.x, 0.0), uv0 + grid, uv0 + Vec2::new(0.0, grid.y)];
    let bins = 1u32 << tile.data.log_max_bins;
    for k in 0..=bins {
        let z = tile.data.z_near + (tile.data.z_far - tile.data.z_near) * k as f32 / bins as f32;
        let ring: Vec<Vec3> = uvs.iter().chain(std::iter::once(&uvs[0])).map(|uv| corner(*uv, z)).collect();
        let color = if k == tile.bin || k == tile.bin + 1 { palette::SELECTED } else { palette::accent() };
        draw.polyline(&ring, color, alpha, 2.0);
    }
    for uv in uvs {
        draw.segment(corner(uv, tile.data.z_near), corner(uv, tile.data.z_far), palette::accent(), alpha * 0.7, 1.5, 0);
    }
    draw.point(tile.point, 9.0, palette::SELECTED, alpha);
}

/// A marker on each of the culling demonstration's lights, in its colour:
/// faint when the inspected point's tile does not list it, brighter when the
/// tile lists it for other bins, and lit, ringed, and joined to the point
/// when the point's bin keeps it. List indices count the room's light first;
/// it keeps its own marker, and is ringed and joined the same way.
fn light_markers(draw: &mut DrawLists, shading: &Shading, tile: Option<&CullTile>, alpha: f32) {
    if let Some(t) = tile.filter(|t| shading.light_on && t.kept.contains(&0)) {
        draw.point(shading.light.position, 17.0, palette::text(), alpha);
        draw.point(shading.light.position, 11.0, LIGHT, alpha);
        draw.segment(t.point, shading.light.position, palette::text(), alpha * 0.8, 2.0, FLAG_DASHED);
    }
    for (i, light) in shading.extra_lights.iter().enumerate() {
        let index = i as u32 + 1;
        let [r, g, b] = (EXTRA_LIGHT_COLORS[i % 4] * 255.0).round().to_array();
        let color = Color32::from_rgb(r as u8, g as u8, b as u8);
        let kept = tile.is_some_and(|t| t.kept.contains(&index));
        let listed = tile.is_some_and(|t| t.listed.contains(&index));
        if kept {
            draw.point(light.position, 16.0, palette::text(), alpha);
            draw.point(light.position, 11.0, color, alpha);
            if let Some(t) = tile {
                draw.segment(t.point, light.position, palette::text(), alpha * 0.8, 2.0, FLAG_DASHED);
            }
        } else {
            draw.point(light.position, if listed { 10.0 } else { 8.0 }, color, alpha * if listed { 0.85 } else { 0.4 });
        }
    }
}

fn readout(demo: Demo, state: &ShotState, shading: &Shading, px: &brdf::Pixel, statics: &Statics, objects: &[ObjectData], camera: &lab::layout::CameraData) -> Readout {
    let s = &px.surface;
    let l = &px.lighting;
    let g = &px.terms;
    let seen = if state.p("observer") > 0.5 { tr("demos.seen_from_the_observer") } else { tr("demos.seen_from_the_camera") };
    let object = format!("{} · {seen}", object_name(statics.scene.objects[shading.inspected as usize].name));
    let mut rows: Vec<(String, String, Color32)> = Vec::new();
    let mut row = |name: &str, value: String, color: Color32| rows.push((name.to_string(), value, color));
    let (title, subtitle) = match demo {
        Demo::Room if state.p("readoutMode") >= 1.5 => {
            // EncodeNormalSignedOctahedron, step by step.
            let n = s.normal;
            let l1 = n.x.abs() + n.y.abs() + n.z.abs();
            let o = n / l1;
            let e = brdf::encode_normal_signed_octahedron(n);
            row("normal", f3(n), palette::NORMAL);
            row("|x| + |y| + |z|", format!("{l1:.3}"), palette::text());
            row("n / that (octahedron)", f3(o), palette::NORMAL);
            row("x' = x/2 + y/2 + 1/2", format!("{:.3}", e.x), palette::accent());
            row("y' = -x/2 + y/2 + 1/2", format!("{:.3}", e.y), palette::accent());
            row("z' = sign of z", format!("{:.0}", e.z), palette::accent());
            row("RGB10A2 stores", format!("{} · {} · {}", (e.x * 1023.0).round(), (e.y * 1023.0).round(), (e.z * 1023.0).round()), palette::text());
            ("EncodeNormalSignedOctahedron", object)
        }
        Demo::Room if state.p("readoutMode") >= 0.5 => {
            // The five targets' texels at the point (GetPbrLightingOutput,
            // ForwardPassVertexAndPixel.azsli): what the forward pass leaves.
            let albedo = s.albedo * l.diffuse_response * s.diffuse_ao;
            let flags = brdf::encode_unorm_2bit_flags(shading.ibl, shading.multiscatter);
            row("DiffuseOutput", format!("{} · -1", f3(px.diffuse())), DIFFUSE);
            row("SpecularOutput", format!("{} · 1", f3(px.specular())), SPECULAR);
            row("AlbedoOutput", format!("{} · {:.3}", f3(albedo.map(brdf::unorm8)), brdf::unorm8(s.specular_occlusion)), palette::text());
            row("SpecularF0Output", format!("{} · {:.3}", f3(s.specular_f0.map(brdf::unorm8)), brdf::unorm8(s.roughness_linear)), palette::accent());
            row("NormalOutput", format!("{} · {flags:.3}", f3(brdf::encode_normal_signed_octahedron(s.normal))), palette::NORMAL);
            (tr("demos.forward_targets_at_the_point"), object)
        }
        Demo::Room => {
            row("light position", f3(shading.light.position), LIGHT);
            row(tr("demos.intensity_cd"), f3(shading.light.rgb_intensity_candelas), LIGHT);
            row(tr("common.attenuation_radius"), format!("{:.2} m", shading.light.attenuation_radius()), LIGHT);
            row("", String::new(), palette::text());
            row("diffuseLighting", f3(px.diffuse()), DIFFUSE);
            row("specularLighting", f3(px.specular()), SPECULAR);
            let ibl_on = if shading.ibl { 1.0 } else { 0.0 };
            row(tr("demos.ibl_diffuse"), f3(px.ibl.diffuse * ibl_on), DIFFUSE);
            row(tr("demos.ibl_specular"), f3(px.ibl.specular * ibl_on), SPECULAR);
            row(tr("demos.pixel"), f3(px.diffuse() + px.specular() + (px.ibl.diffuse + px.ibl.specular) * ibl_on), palette::text());
            (tr("demos.forward_pass_at_the_point"), object)
        }
        Demo::Culling => {
            let (rows_, _) = culling_view(state, shading, objects, camera);
            for (name, value, color) in rows_ {
                row(&name, value, color);
            }
            (tr("common.tiled_light_culling"), tr("demos.the_inspected_point_s_tile").to_string())
        }
        Demo::Surface => {
            let material = shading.material(shading.inspected);
            row("baseColor", f3(s.base_color), palette::text());
            row("metallic", f1(s.metallic), palette::text());
            row("specularF0Factor", f1(material.specular_f0_factor), palette::text());
            row("roughness (authored)", f1(content_roughness(state, shading.inspected)), palette::text());
            row("", String::new(), palette::text());
            row("albedo", f3(s.albedo), DIFFUSE);
            row("specularF0", f3(s.specular_f0), SPECULAR);
            row("roughnessLinear", f1(s.roughness_linear), palette::accent());
            row("roughnessA", f1(s.roughness_a), palette::accent());
            row("roughnessA2", f1(s.roughness_a2), palette::accent());
            row("diffuseAmbientOcclusion", f1(s.diffuse_ao), DIFFUSE);
            row("specularOcclusion", f1(s.specular_occlusion), SPECULAR);
            ("SetAlbedoAndSpecularF0", object)
        }
        Demo::Decals => {
            // The surface before its decals, then each decal in SortDecals' order.
            let (p, n) = inspected_point(state);
            let plain = Shading { decals: Vec::new(), ..shading.clone() };
            let before = brdf::pixel(&plain, objects, p, n, p + n, shading.inspected, 1.0, BrdfMap::get(), [Vec3::ZERO; 2]).surface;
            row(tr("demos.albedo_surface"), f3(before.albedo), DIFFUSE);
            if shading.decals.is_empty() {
                row("decals", tr("demos.off").into(), palette::text_dim());
            }
            let sorted = super::decals::sorted(&shading.decals);
            let top = sorted.iter().rev().copied().find(|&i| super::decals::sample(&shading.decals[i], p, n).is_some());
            for i in sorted {
                let d = &shading.decals[i];
                let name = format!("{} (key {})", decal_name(d.texture), d.sort_key);
                let color = [palette::accent(), palette::REFLECTED][d.texture.min(1)];
                match super::decals::sample(d, p, n) {
                    Some(k) => {
                        row(&name, format!("uvw ({:.2}, {:.2}, {:.2})", k.uvw.x, k.uvw.y, k.uvw.z), color);
                        row(tr("demos.opacity_angle"), format!("{:.2}·{:.2}·{:.2} = {:.2}", k.base[3], d.opacity, k.attenuation, k.albedo_opacity), color);
                        // The decal on top: what BlendBaseColor blends (its texture times its colour),
                        // and that colour converted to ACEScg, as a material's would be.
                        if top == Some(i) {
                            row("b", f3(k.blended), color);
                            row(tr("demos.b_in_acescg"), f3(super::decals::linear_srgb_to_acescg(k.blended)), palette::text_dim());
                        }
                    }
                    None => row(&name, tr("demos.outside_its_box").into(), palette::text_dim()),
                }
            }
            row(tr("demos.albedo_after"), f3(s.albedo), DIFFUSE);
            row("specularF0", f3(s.specular_f0), SPECULAR);
            row("normal", f3(s.normal), palette::NORMAL);
            ("ApplyDecal", object)
        }
        Demo::Specular => {
            row("roughnessA2", f1(s.roughness_a2), palette::accent());
            row("NdotL", f1(g.n_dot_l), LIGHT);
            row("NdotV", f1(l.n_dot_v), palette::VIEW);
            row("NdotH", f1(g.n_dot_h), HALF);
            row("HdotV", f1(g.h_dot_v), HALF);
            row("D  NormalDistributionGGX", f1(g.d), SPECULAR);
            row("G  (visibility, ÷ 4 NdotL NdotV)", f1(g.g), TERM_G);
            // GGX's F term, in the text's colour.
            row("F  FresnelSchlick", f3(g.f), palette::text());
            row(tr("demos.compensation"), f3(l.multi_scatter_compensation), palette::text_dim());
            row("D·G·F·NdotL·comp", f3(brdf::specular_ggx(l.dir_to_camera, px.light.dir_to_light, s.normal, s.specular_f0, l.n_dot_v, s.roughness_a2, l.multi_scatter_compensation)), SPECULAR);
            row(tr("demos.light_lux"), f3(px.specular()), SPECULAR);
            ("SpecularGGX", object)
        }
        Demo::ClearCoat => {
            let to_light = px.light.dir_to_light;
            let base = brdf::specular_ggx(l.dir_to_camera, to_light, s.normal, s.specular_f0, l.n_dot_v, s.roughness_a2, l.multi_scatter_compensation);
            match s.clear_coat {
                Some(c) => {
                    let h = (to_light + l.dir_to_camera).normalize();
                    let fc = brdf::fresnel_schlick(brdf::saturate(h.dot(to_light)), Vec3::splat(0.04)) * c.factor;
                    let coated = brdf::clear_coat_specular(to_light, l.dir_to_camera, &c, base);
                    row("clearCoat.factor", f1(c.factor), palette::accent());
                    row("clearCoat.roughness", f1(c.roughness), palette::accent());
                    row(tr("demos.specularf0_base"), f3(s.specular_f0), SPECULAR);
                    row("diffuseResponse", f3(l.diffuse_response), DIFFUSE);
                    row("Fc = F(HdotL, 0.04) · factor", f1(fc.x), HALF);
                    row(tr("demos.base_1_fc"), f3(base * (Vec3::ONE - fc)), SPECULAR);
                    row(tr("demos.coat_specular"), f3(coated - base * (Vec3::ONE - fc)), HALF);
                    row(tr("demos.light_lux"), f3(px.specular()), SPECULAR);
                }
                None => row("clearCoat.enable", tr("demos.off").into(), palette::text_dim()),
            }
            (tr("demos.clearcoatspecular"), object)
        }
        Demo::Transmission => {
            let (_, n) = inspected_point(state);
            let mode = match s.transmission.map(|t| t.mode) {
                Some(super::transmission::TransmissionMode::ThickObject) => "ThickObject",
                Some(super::transmission::TransmissionMode::ThinObject) => "ThinObject",
                None => "None",
            };
            row("o_transmission_mode", mode.into(), palette::accent());
            match (s.transmission, shading.sphere_lights.first()) {
                (Some(tm), Some(light)) => {
                    let to_light = (light.position - s.position).normalize();
                    row(tr("demos.thickness_no_shadow_data"), f1(tm.thickness), palette::accent());
                    if tm.mode == super::transmission::TransmissionMode::ThickObject {
                        let k = super::transmission::thick_terms(&tm, n, l.dir_to_camera, to_light, 0.0);
                        row("V·−(L + N·distortion)", f1(k.v_dot), palette::VIEW);
                        row(tr("demos.power_scale"), f1(k.transmittance), palette::accent());
                        row("exp(−t·k)·saturate(1 − t)", f1(k.lamber_attenuation), palette::accent());
                    } else {
                        let e = s.albedo * brdf::saturate(tm.params.y + (-n).dot(to_light));
                        row(tr("demos.e_albedo_sat_bias_n"), f3(e), DIFFUSE);
                        row(tr("demos.kernel_at_s_0"), f3(super::transmission::transmission_kernel(0.0, Vec3::ONE)), palette::accent());
                    }
                    row(tr("demos.back_lighting_lux"), f3(l.translucent_back_lighting), SPHERE_LIGHT);
                    row(tr("demos.tint"), f3(l.translucent_back_lighting * tm.tint), SPHERE_LIGHT);
                    row("diffuseLighting", f3(px.diffuse()), DIFFUSE);
                }
                (Some(_), None) => row(tr("demos.point_light"), tr("demos.off").into(), palette::text_dim()),
                (None, _) => {}
            }
            (tr("demos.getbacklighting"), object)
        }
        Demo::Diffuse => {
            let lambert = brdf::diffuse_lambertian(s.albedo, s.normal, px.light.dir_to_light, l.diffuse_response);
            let disney = brdf::normalized_disney_diffuse(s.albedo, s.normal, l.dir_to_camera, px.light.dir_to_light, s.roughness_linear);
            row("NdotV", f1(l.n_dot_v), palette::VIEW);
            row("NdotL", f1(g.n_dot_l), LIGHT);
            row("F with roughness", f3(l.specular_response), SPECULAR);
            row("1 − F (response)", f3(l.diffuse_response), DIFFUSE);
            row("albedo", f3(s.albedo), palette::text());
            row("DiffuseLambertian", f3(lambert), DIFFUSE);
            row(tr("demos.disney_skin"), f3(disney), palette::text_dim());
            row(tr("demos.light_lux"), f3(px.diffuse()), DIFFUSE);
            (tr("demos.diffuse_lighting"), object)
        }
        Demo::Multiscatter => {
            let e_ss = l.brdf.x + l.brdf.y;
            let single = brdf::specular_ggx(l.dir_to_camera, px.light.dir_to_light, s.normal, s.specular_f0, l.n_dot_v, s.roughness_a2, Vec3::ONE);
            row("roughnessLinear", f1(s.roughness_linear), palette::accent());
            row(tr("demos.uv_ndotv_smoothness"), format!("({:.3}, {:.3})", l.n_dot_v, 1.0 - s.roughness_linear), palette::text_dim());
            row("brdf.x  (A, scale)", f1(l.brdf.x), palette::accent());
            row("brdf.y  (B, bias)", f1(l.brdf.y), palette::accent());
            row(tr("demos.a_b_single_scattering_energy"), f1(e_ss), palette::accent());
            row("specularF0", f3(s.specular_f0), SPECULAR);
            row(tr("demos.compensation"), f3(l.multi_scatter_compensation), SPECULAR);
            row(tr("demos.single_scattering"), f3(single), palette::text_dim());
            row(tr("demos.compensated"), f3(single * l.multi_scatter_compensation), SPECULAR);
            ("GetMultiScatterCompensation", object)
        }
        Demo::Ibl => {
            let i = &px.ibl;
            row(tr("demos.albedo_target"), f3(i.albedo), DIFFUSE);
            row(tr("demos.irradiance_at_n"), f3(i.irradiance), DIFFUSE);
            row(tr("common.diffuse_ibl"), f3(i.diffuse), DIFFUSE);
            row("", String::new(), palette::text());
            row(tr("demos.roughness_8_bit"), f1(i.roughness), palette::accent());
            row(tr("demos.mip_roughness_5"), f1(i.mip), palette::accent());
            row(tr("demos.prefiltered_at_r"), f3(i.prefiltered), palette::REFLECTED);
            row("NdotV (≥ 0.01)", f1(i.n_dot_v), palette::VIEW);
            row("F0 · A + B", f3(i.specular_f0 * i.brdf.x + Vec3::splat(i.brdf.y)), TERM_G);
            row(tr("demos.compensation"), f3(i.compensation), palette::text_dim());
            row(tr("common.specular_ibl"), f3(i.specular), SPECULAR);
            (tr("demos.global_ibl_passes"), object)
        }
        Demo::Reflections => {
            let r = &px.ibl.reflection;
            let masked = r.stencil & super::reflections::STENCIL_REFLECTION_MASK;
            let region = match masked {
                0..=2 => tr("demos.no_passes"),
                3 => tr("demos.outside_the_inners"),
                _ => tr("demos.inside_an_inner"),
            };
            row("stencil", format!("{masked} · {region}"), palette::accent());
            for probe in &shading.probes {
                let name = format!("{} {}", tr("demos.probe"), probe.cubemap + 1);
                let color = [palette::accent(), palette::REFLECTED][probe.cubemap.min(1)];
                let value = if probe.contains(probe.inner_half, s.position) {
                    tr("demos.inner_weight_1").to_string()
                } else if probe.contains(probe.outer_half, s.position) {
                    format!("{} {:.3}", tr("demos.outer_weight"), probe.blend_weight(s.position))
                } else {
                    tr("demos.outside").to_string()
                };
                row(&name, value, color);
            }
            row(tr("demos.blend_weight_sum"), f1(r.blend_weight), palette::accent());
            row(tr("demos.global_1_min_sum_1"), f1(r.global_weight), palette::text_dim());
            row(tr("demos.reflection_target"), f3(r.reflection), SPECULAR);
            row(tr("demos.composited"), f3(r.composited), SPECULAR);
            (tr("common.reflection_probes"), object)
        }
        Demo::Ssao => {
            let (p, _) = inspected_point(state);
            let projector = lab::camera::Projector::new(camera);
            let view = glam::Mat4::from_cols_array_2d(&camera.world_to_view);
            let size = [camera.viewport_size[0] as u32, camera.viewport_size[1] as u32];
            // DepthToLinearDepth at a pixel: the view distance of the surface its centre sees, or of the far plane.
            let linear_depth = |x: i32, y: i32| -> f32 {
                let (o, d) = projector.ray(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                let hit = lab::scene::trace_scene(objects, o, d, 100.0, ids::PANEL as u32);
                if hit.is_hit() { -view.transform_point3(o + d * hit.t).z } else { camera.far_plane }
            };
            let projection = glam::Mat4::from_cols_array_2d(&camera.view_to_clip);
            let tan = Vec2::new(1.0 / projection.x_axis.x, 1.0 / projection.y_axis.y);
            let settings = super::ssao::Settings::from_records(&ssao_records(state));
            match projector.project(p) {
                Some((pixel, _)) => {
                    let (x, y) = (pixel.x.floor() as i32, pixel.y.floor() as i32);
                    let k = super::ssao::at_pixel(&linear_depth, size, &settings, tan, x, y);
                    row(tr("demos.pixel_half_res"), format!("({x}, {y}) · ({}, {})", k.half_pixel[0], k.half_pixel[1]), palette::text());
                    row(tr("demos.linear_depth"), format!("{:.3} m", k.linear_depth), palette::accent());
                    row(tr("demos.strength_radius"), format!("{:.2} · {:.3}", settings.strength, settings.sampling_radius), palette::accent());
                    row(tr("demos.ssaocompute_52_samples"), f1(k.raw), palette::text());
                    row(tr("demos.blur_vertical"), f1(k.blur_vertical), palette::text());
                    row(tr("demos.blur_horizontal"), f1(k.blurred), palette::text());
                    row(tr("demos.upsampled_ao"), f1(k.upsampled), palette::text());
                    let diffuse = px.diffuse() + px.ibl.diffuse * if shading.ibl { 1.0 } else { 0.0 };
                    row(tr("demos.diffuse"), f3(diffuse), DIFFUSE);
                    row(tr("demos.ao"), f3(diffuse * k.upsampled), DIFFUSE);
                }
                None => row(tr("demos.point"), tr("demos.off_screen").into(), palette::text_dim()),
            }
            ("SsaoCompute", object)
        }
        Demo::Terrain => {
            // TerrainPBR_ForwardPass.azsl's macro surface.
            row("roughnessLinear (macro)", f1(1.0), palette::accent());
            row("specularF0 factor (macro)", format!("0.5 · F0 {:.2}", 0.08 * 0.5), SPECULAR);
            row("metallic (macro)", f1(0.0), palette::text());
            row(tr("demos.terrain_direct_multiscatter"), tr("demos.off").to_string(), palette::text());
            row(tr("demos.terrain_ibl_multiscatter"), if shading.multiscatter { tr("demos.on") } else { tr("demos.off") }.to_string(), palette::text());
            ("TerrainPBR_ForwardPass", object)
        }
        Demo::Cascades => {
            use super::cascades;
            let (p, n) = inspected_point(state);
            // The cascades the frame renders (the overlay draws the observer's, whose frustum can be seen).
            let observer = shadow_camera(state, camera);
            let set = cascades::cascades(observer, sun_direction(state), state.p("cascadeSplitRatio"));
            row(tr("demos.near_shadow_far"), format!("{:.2} … {:.1} m", observer.near, observer.far), palette::text());
            row(tr("demos.split_ratio"), format!("{:.2}", state.p("cascadeSplitRatio")), palette::accent());
            for (i, c) in set.cascades.iter().enumerate() {
                row(&format!("{} {i}", tr("demos.cascade")), format!("{:.2}–{:.2} m · R {:.2}", c.near, c.far, c.radius), CASCADE_COLORS[i]);
            }
            let texels: Vec<String> = set.cascades.iter().map(|c| format!("{:.1}", c.texel() * 1000.0)).collect();
            row(tr("demos.texel_mm"), texels.join(" · "), palette::accent());
            match set.select(p, n) {
                Some(i) => {
                    let t = set.texel_of(i, p, n);
                    row(tr("demos.point_s_cascade"), format!("{i} · texel ({:.0}, {:.0})", t.x, t.y), CASCADE_COLORS[i]);
                }
                None => row(tr("demos.point_s_cascade"), tr("demos.outside_every_cascade").into(), palette::text_dim()),
            }
            ("DirectionalLightFeatureProcessor", object)
        }
        Demo::PointLight => {
            let d2 = px.light.distance_squared;
            row(tr("common.distance_d"), format!("{:.3} m", d2.sqrt()), palette::text());
            row(tr("demos.attenuation_radius_r"), format!("{:.3} m", shading.light.attenuation_radius()), LIGHT);
            row("m_invAttenuationRadiusSquared", f1(shading.light.inv_attenuation_radius_squared), LIGHT);
            row("falloff = d² / r²", f1(px.light.falloff), LIGHT);
            row("(1 − falloff²)²", f1(px.light.radius_attenuation), LIGHT);
            row("I / max(d², 1e-6)", f3(shading.light.rgb_intensity_candelas / d2.max(1e-6)), palette::text_dim());
            row(tr("demos.lightintensity_lux"), f3(px.light.intensity), LIGHT);
            row(tr("demos.skipped_falloff_1"), if px.light.falloff >= 1.0 { tr("demos.yes") } else { tr("demos.no") }.into(), palette::text());
            let surface_channels = shading.material(shading.inspected).lighting_channels;
            let same = shading.light.lighting_channel_mask & surface_channels != 0;
            row(tr("demos.lighting_channels_light_surface"), format!("0x{:02X} & 0x{:02X} → {}", shading.light.lighting_channel_mask, surface_channels, if same { tr("demos.lit") } else { tr("demos.skipped") }), LIGHT);
            ("SimplePointLightUtil", object)
        }
    };
    rows.truncate(state.p("readoutRows").round().max(0.0) as usize);
    let (formula, formula_lit) = super::formulas::readout(demo, state.p("formula"));
    Readout { title: title.to_string(), subtitle, formula, formula_lit, rows }
}

fn content_roughness(state: &ShotState, object: u32) -> f32 {
    match material_params(object as usize) {
        Some((roughness, _)) => state.p(roughness),
        None => 1.0,
    }
}

fn notes(demo: Demo, shading: &Shading) -> Vec<(String, Color32)> {
    let mut notes = Vec::new();
    match demo {
        Demo::Room | Demo::PointLight => notes.push((tr("demos.simple_point_lights_cast_no").to_string(), palette::text_faint())),
        Demo::Cascades => notes.push((tr("demos.cascades_schematic").to_string(), palette::text_faint())),
        Demo::Ssao => {
            notes.push((tr("demos.half_resolution_8_bits_a").to_string(), palette::text_faint()));
            notes.push((tr("demos.the_diffuse_target_only_light").to_string(), palette::text_faint()));
        }
        Demo::Reflections => {
            notes.push((tr("demos.the_lab_bakes_the_probes").to_string(), palette::text_faint()));
            notes.push((tr("demos.with_the_room_s_light").to_string(), palette::text_faint()));
        }
        Demo::Transmission => {
            notes.push((tr("demos.simple_point_lights_add_no").to_string(), palette::text_faint()));
            notes.push((tr("demos.the_sphere_light_here_has").to_string(), palette::text_faint()));
        }
        Demo::Diffuse if shading.diffuse_model == DiffuseModel::NormalizedDisney => {
            notes.push((tr("demos.disney_diffuse_note").to_string(), palette::MISMATCH))
        }
        Demo::Multiscatter if !shading.multiscatter => notes.push((tr("demos.compensation_off_note").to_string(), palette::MISMATCH)),
        Demo::Multiscatter => notes.push((tr("demos.compensation_1_f0_1_a").to_string(), palette::text_faint())),
        Demo::Ibl => {
            notes.push((tr("demos.the_global_skylight_casts_no").to_string(), palette::text_faint()));
            notes.push((tr("demos.walls_or_not_only_occlusion").to_string(), palette::text_faint()));
        }
        _ => {}
    }
    if !shading.ibl {
        notes.push((tr("demos.ibl_off_direct_lighting_only").to_string(), palette::MISMATCH));
    }
    if !shading.ibl && shading.material(shading.inspected).metallic > 0.5 && demo != Demo::PointLight {
        notes.push((tr("demos.metals_have_no_diffuse_term").to_string(), palette::text_faint()));
        notes.push((tr("demos.without_ibl_only_the_highlight").to_string(), palette::text_faint()));
    }
    notes
}

/// The inspected point's SSAO occluders in world space: its position and
/// depth-rebuilt normal, and each occluder with its saturate(cos) and weight
/// (ssao::samples_at, on the lab's ray-traced linear depth); None off screen.
fn ssao_samples(state: &ShotState, objects: &[ObjectData], camera: &lab::layout::CameraData) -> Option<(Vec3, Vec3, Vec<(Vec3, f32, f32)>)> {
    let (p, _) = inspected_point(state);
    let projector = lab::camera::Projector::new(camera);
    let view = glam::Mat4::from_cols_array_2d(&camera.world_to_view);
    let size = [camera.viewport_size[0] as u32, camera.viewport_size[1] as u32];
    let linear_depth = |x: i32, y: i32| -> f32 {
        let (o, d) = projector.ray(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
        let hit = lab::scene::trace_scene(objects, o, d, 100.0, ids::PANEL as u32);
        if hit.is_hit() { -view.transform_point3(o + d * hit.t).z } else { camera.far_plane }
    };
    let projection = glam::Mat4::from_cols_array_2d(&camera.view_to_clip);
    let tan = Vec2::new(1.0 / projection.x_axis.x, 1.0 / projection.y_axis.y);
    let settings = super::ssao::Settings::from_records(&ssao_records(state));
    let (pixel, _) = projector.project(p)?;
    let half_size = [size[0].div_ceil(2), size[1].div_ceil(2)];
    let half = |x: i32, y: i32| linear_depth(2 * x, 2 * y);
    let k = super::ssao::samples_at(&half, half_size, &settings, tan, pixel.x as i32 / 2, pixel.y as i32 / 2);
    // GetViewSpacePosition's space: x right, y up, z forward; the lab's view space looks down -z.
    let to_world = view.inverse();
    let world = |v: Vec3| to_world.transform_point3(Vec3::new(v.x, v.y, -v.z));
    let normal = to_world.transform_vector3(Vec3::new(k.normal.x, k.normal.y, -k.normal.z)).normalize();
    let terms = k.terms();
    Some((world(k.position), -normal, k.occluders.iter().zip(terms).map(|(o, (c, w))| (world(*o), c, w)).collect()))
}

/// The colour `t` of the way from `a` to `b`.
fn mix_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}
