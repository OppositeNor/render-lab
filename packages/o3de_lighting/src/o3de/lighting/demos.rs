//! What each shot of the lighting investigation shows: the study room shaded
//! by shaders/standard_lighting.wgsl under one simple point light, the
//! vectors and the specular lobe at the inspected point, the light's falloff,
//! and the readout of every term there, computed by the CPU mirror (brdf.rs).

mod overlays;
mod readout;
mod shadows;
pub use overlays::*;
use readout::*;
pub use shadows::*;

use super::brdf::{self, BrdfMap, DiffuseModel, Material, PointLight, Shading};
use super::shot::Demo;
use super::transmission::{SphereLight, Transmission, TransmissionMode};
use crate::o3de::lighting::text::tr;
use egui::Color32;
use glam::{Vec2, Vec3};
use lab::animation::Ease;
use lab::frame::{DrawLists, FLAG_ARROW, FLAG_DASHED, FrameContent, Label, Readout, Statics};
use lab::i18n::object_name;
use lab::layout::{FrameUniforms, ObjectData};
use lab::palette;
use lab::scene::ids;
use lab::shot::ShotState;

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
    let strength = if on {
        state.p("ssaoStrength").clamp(0.0, 1.0)
    } else {
        0.0
    };
    let blur = if on && state.p("ssaoBlur") > 0.5 {
        1.0
    } else {
        0.0
    };
    vec![
        [strength, state.p("ssaoRadius"), blur, 0.0],
        [
            state.p("ssaoBlurConstFalloff"),
            state.p("ssaoBlurDepthThreshold"),
            state.p("ssaoBlurDepthStrength"),
            0.0,
        ],
    ]
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
    PointLight {
        position: Vec3::new(state.p("lightX"), state.p("lightY"), state.p("lightZ")),
        inv_attenuation_radius_squared: 1.0 / (radius * radius),
        rgb_intensity_candelas: LIGHT_COLOR * state.p("lightIntensity"),
        lighting_channel_mask: state.p("lightChannels").round() as u32,
    }
}

/// The colours of the light-culling demonstration's lights, in turn.
const EXTRA_LIGHT_COLORS: [Vec3; 4] = [
    Vec3::new(1.0, 0.55, 0.35),
    Vec3::new(0.45, 0.75, 1.0),
    Vec3::new(0.6, 1.0, 0.55),
    Vec3::new(1.0, 0.85, 0.45),
];

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
            PointLight {
                position,
                inv_attenuation_radius_squared: 1.0 / (radius * radius),
                rgb_intensity_candelas: colors[i % 4] * state.p("extraLightIntensity"),
                lighting_channel_mask: brdf::ALL_LIGHTING_CHANNELS,
            }
        })
        .collect()
}

/// Each object's material from the shot: metallic, specularF0Factor,
/// lighting channels, occlusion, and clear coat (`shading` adds the
/// teapot's transmission and each mesh's forward-pass probe).
fn base_materials(state: &ShotState) -> [Material; 12] {
    let mut materials = [Material {
        metallic: 0.0,
        specular_f0_factor: state.p("specularF0Factor"),
        diffuse_occlusion: None,
        specular_occlusion: None,
        clear_coat: None,
        lighting_channels: brdf::ALL_LIGHTING_CHANNELS,
        transmission: None,
        forward_probe: None,
    }; 12];
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
        materials[ids::SPHERE].clear_coat =
            Some((state.p("clearCoatFactor"), state.p("clearCoatRoughness")));
    }
    materials
}

/// gShading's derived members (shaders/standard_lighting.wgsl; the lab packs
/// its values, lab/src/shader_data.rs): the light's inverse squared radius
/// and intensity, the inspected object, and each object's material.
pub fn shader_data(block: &mut lab::shader_data::Block, state: &ShotState) {
    let light = light(state);
    block.set(
        "invAttenuationRadiusSquared",
        light.inv_attenuation_radius_squared,
    );
    block.set(
        "rgbIntensityCandelas",
        light.rgb_intensity_candelas.to_array(),
    );
    block.set("inspectedObject", inspected_object(state) as f32);
    block.set(
        "pick",
        [
            state.p("inspectX"),
            state.p("inspectY"),
            state.p("inspectZ"),
            if state.p("pickShown") > 0.5 {
                1.0
            } else {
                0.0
            },
        ],
    );
    let materials: Vec<f32> = base_materials(state)
        .iter()
        .flat_map(|m| {
            [
                m.metallic,
                m.specular_f0_factor,
                m.diffuse_occlusion.unwrap_or(-1.0),
                m.specular_occlusion.unwrap_or(-1.0),
            ]
        })
        .collect();
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
    for (o, m) in lab::scene::Scene::study_room()
        .records()
        .iter()
        .zip(materials.iter_mut())
    {
        m.forward_probe = super::reflections::forward_probe(
            &probes,
            Vec3::from(o.world_center),
            Vec3::from(o.world_extent),
        );
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
        TransmissionMode::ThickObject => glam::Vec4::new(
            state.p("transmissionAttenuation"),
            state.p("transmissionPower"),
            state.p("transmissionDistortion"),
            scale,
        ),
        TransmissionMode::ThinObject => glam::Vec4::new(
            0.005,
            state.p("transmissionNdLBias"),
            state.p("transmissionDistanceAttenuation"),
            scale,
        ),
    };
    Some(Transmission {
        mode,
        tint: Vec3::new(1.0, 0.8, 0.6),
        thickness: state.p("transmissionThickness"),
        params,
        scatter_distance: Vec3::new(1.0, 0.27, 0.13) * 8.0,
    })
}

/// The O3DE point light (a sphere area light without shadows) when `sphereLight` is on.
pub fn sphere_lights(state: &ShotState) -> Vec<SphereLight> {
    if state.p("sphereLight") < 0.5 {
        return Vec::new();
    }
    let radius = state.p("sphereLightRadius").max(0.01);
    vec![SphereLight {
        position: Vec3::new(
            state.p("sphereLightX"),
            state.p("sphereLightY"),
            state.p("sphereLightZ"),
        ),
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
    state
        .p("inspectObject")
        .round()
        .clamp(0.0, ids::TEAPOT as f32) as u32
}

/// Where the readout and the vectors see the inspected point from.
pub fn observer(state: &ShotState, eye: Vec3) -> Vec3 {
    if state.p("observer") > 0.5 {
        Vec3::new(
            state.p("observerX"),
            state.p("observerY"),
            state.p("observerZ"),
        )
    } else {
        eye
    }
}

pub fn inspected_point(state: &ShotState) -> (Vec3, Vec3) {
    let p = Vec3::new(
        state.p("inspectX"),
        state.p("inspectY"),
        state.p("inspectZ"),
    );
    let n = Vec3::new(
        state.p("inspectNX"),
        state.p("inspectNY"),
        state.p("inspectNZ"),
    )
    .normalize_or(Vec3::Y);
    (p, n)
}

fn f3(v: Vec3) -> String {
    format!("({:.3}, {:.3}, {:.3})", v.x, v.y, v.z)
}

fn f1(v: f32) -> String {
    if v.abs() >= 1000.0 {
        format!("{v:.0}")
    } else if v.abs() >= 10.0 {
        format!("{v:.2}")
    } else {
        format!("{v:.4}")
    }
}

/// How the normal at `p` changes to the next pixel across and down, as the
/// scene pass's ddx_fine and ddy_fine see it: the same object hit one pixel
/// over (zero off its edge, or where `p` is off screen).
pub fn normal_derivatives(
    objects: &[ObjectData],
    camera: &lab::layout::CameraData,
    p: Vec3,
    object: u32,
) -> [Vec3; 2] {
    let projector = lab::camera::Projector::new(camera);
    let Some((pixel, _)) = projector.project(p) else {
        return [Vec3::ZERO; 2];
    };
    let eye = Vec3::from(camera.position);
    let normal_at = |offset: glam::Vec2| -> Option<Vec3> {
        let (origin, dir) = projector.ray(pixel.floor() + glam::Vec2::splat(0.5) + offset);
        let hit = lab::scene::trace_scene(objects, origin, dir, 100.0, ids::PANEL as u32);
        (hit.is_hit() && hit.object == object).then(|| {
            if hit.normal.dot(eye - (origin + dir * hit.t)) < 0.0 {
                -hit.normal
            } else {
                hit.normal
            }
        })
    };
    let Some(here) = normal_at(glam::Vec2::ZERO) else {
        return [Vec3::ZERO; 2];
    };
    let across = normal_at(glam::Vec2::X).map_or(Vec3::ZERO, |n| n - here);
    let down = normal_at(glam::Vec2::Y).map_or(Vec3::ZERO, |n| n - here);
    [across, down]
}

/// The faces of the unfolded cubemap the merge pass draws (probeCubemapViews
/// in the shader: four cells across, three down, -Z at the centre), named at
/// their cells' tops, and the cubemap and mip under the cross.
fn cubemap_cross_labels(
    labels: &mut Vec<Label>,
    state: &ShotState,
    camera: &lab::layout::CameraData,
) {
    let projector = lab::camera::Projector::new(camera);
    let size = projector.viewport;
    let cell = (size.x / 4.0).min(size.y / 3.0) * 0.94;
    let origin = (size - Vec2::new(4.0, 3.0) * cell) * 0.5;
    let at = |x: f32, y: f32| {
        let (eye, dir) = projector.ray(origin + Vec2::new(x, y) * cell);
        eye + dir
    };
    for (x, y, name) in [
        (1.0, 0.0, "+Y"),
        (0.0, 1.0, "-X"),
        (1.0, 1.0, "-Z"),
        (2.0, 1.0, "+X"),
        (3.0, 1.0, "+Z"),
        (1.0, 2.0, "-Y"),
    ] {
        labels.push(Label {
            anchor: at(x + 0.5, y + 0.12),
            text: name.into(),
            color: palette::text(),
        });
    }
    let mip = state
        .p("probeCubemapMip")
        .clamp(0.0, (super::ibl::SPECULAR_MIPS - 1) as f32);
    let texels = super::ibl::SPECULAR_SIZE >> mip.round() as u32;
    let text = format!(
        "{} {} · mip {:.1} · {texels} × {texels}",
        tr("demos-probe"),
        super::reflections::shown_cubemap(state) + 1,
        mip
    );
    labels.push(Label {
        anchor: at(3.0, 2.5),
        text,
        color: palette::accent(),
    });
}

/// The near plane of O3DE's camera (AzFramework CameraState and the Camera
/// component's default near clip distance), in metres.
pub const CAMERA_NEAR: f32 = 0.1;

/// The rendering camera's clip-to-world matrix, which the full-screen passes
/// rebuild positions from depth with: `shading_frame`'s first four vectors.
fn clip_to_world(camera: &lab::layout::CameraData) -> [[f32; 4]; 4] {
    glam::Mat4::from_cols_array_2d(&camera.world_to_clip)
        .inverse()
        .to_cols_array_2d()
}

/// The frame drawn from another camera (the free camera): the passes rebuild
/// positions with that camera's matrix; everything else (the cascades fitted
/// to the main camera, the demonstration) stays as built.
pub fn view_from(content: &mut FrameContent) {
    content.shading_frame[..4].copy_from_slice(&clip_to_world(&content.uniforms.camera));
}

pub fn build_frame(
    state: &ShotState,
    statics: &Statics,
    viewport: [f32; 2],
    pixel_scale: f32,
    time: f32,
) -> FrameContent {
    let demo = Demo::of(state.demo);
    let objects = posed_objects(state, statics);
    let shading = shading(state);
    // O3DE's camera near plane (Camera::Configuration's default, 0.1 m), which the cascades split from.
    let base = state.camera.camera_data(viewport);
    let camera = state.camera.camera_data_like(&lab::layout::CameraData {
        near_plane: CAMERA_NEAR,
        ..base
    });
    let eye = Vec3::from(camera.position);
    let exposure = state.p("exposure");
    let uniforms = frame_uniforms(
        state,
        statics,
        (&objects, camera),
        [exposure, pixel_scale, time],
    );
    // gShading is the lab's to pack (`shader_data`).
    let mut content = FrameContent::new(objects, uniforms);
    pack_shading_frame(&mut content, state, &shading, &uniforms.camera);

    let map = BrdfMap::get();
    let (p, n) = inspected_point(state);
    let object = inspected_object(state);
    let seen_from = observer(state, eye);
    let derivatives = normal_derivatives(&content.objects, &camera, p, object);
    let px = brdf::pixel(
        &shading,
        &content.objects,
        &brdf::Fragment {
            object,
            p,
            n,
            derivatives,
        },
        seen_from,
        exposure,
        map,
    );
    // SSAO's occluders for the inspected point's half-res pixel (ssao::samples_at):
    // the first `ssaoSamples` of its 52, each coloured by saturate(cos) against
    // the normal rebuilt from depth (dark green unoccluding, bright occluding)
    // and sized by its weight 1 / (d^2 + 0.07).
    let shown_samples = state.p("ssaoSamples").clamp(0.0, 52.0);
    let ssao_overlay = (shown_samples > 0.0)
        .then(|| ssao_samples(state, &content.objects, &camera))
        .flatten();
    scene_marks(
        &mut content,
        (state, &shading, demo),
        (&ssao_overlay, shown_samples),
    );
    let draw = &mut content.draw;
    let labels = &mut content.labels;
    // The light's attenuation radius and falloff, and the inspected point's vectors, lobes, and GGX samples are its
    // overlay script's (script/overlay.lam, from `script::Frame`), here.
    let overlay_at = lab::frame::DrawMark {
        segments: draw.segments.len(),
        points: draw.points.len(),
        labels: labels.len(),
    };

    material_labels(labels, state, statics);

    demo_overlays(
        &mut content,
        (state, &shading, demo),
        &uniforms.camera,
        (p, n),
    );
    let readout = readout(
        demo,
        state,
        &shading,
        &px,
        statics,
        &content.objects,
        &uniforms.camera,
    );
    content.readout = Some(readout);
    content.overlay_at = Some(overlay_at);
    let surface_channels = shading.material(shading.inspected).lighting_channels;
    content.script = Some(std::sync::Arc::new(super::script::Frame {
        light_on: shading.light_on,
        light: shading.light,
        surface_channels,
        normal: n,
        pixel: px,
    }));
    content.notes = notes(demo, &shading);
    // The unfolded cubemap covers the room, so the room's markers and labels give way to its faces' names.
    if state.p("probeCubemap") > 0.5 {
        content.draw = DrawLists::default();
        content.labels.clear();
        cubemap_cross_labels(&mut content.labels, state, &content.uniforms.camera);
    }
    content
}

/// The material labels (name, metallic, roughness) of the sphere, teapot, and box, when labels are on.
fn material_labels(labels: &mut Vec<Label>, state: &ShotState, statics: &Statics) {
    if state.p("labels") > 0.0 {
        for (i, anchor) in [
            (ids::SPHERE, Vec3::new(-1.05, 1.36, -0.55)),
            (ids::TEAPOT, Vec3::new(-0.25, 0.72, -1.75)),
            (ids::BOX, Vec3::new(1.05, 1.02, 0.15)),
        ] {
            let Some((roughness, metallic)) = material_params(i) else {
                continue;
            };
            let text = format!(
                "{} · metallic {:.2} · roughness {:.2}",
                object_name(statics.scene.objects[i].name),
                state.p(metallic),
                state.p(roughness)
            );
            labels.push(Label {
                anchor,
                text,
                color: palette::text(),
            });
        }
    }
}

/// The demonstrations' own overlays: the probes' and decals' boxes, the culling tile and light markers, and the
/// cascades around the inspected point `(p, n)`.
fn demo_overlays(
    content: &mut FrameContent,
    (state, shading, demo): (&ShotState, &Shading, Demo),
    camera: &lab::layout::CameraData,
    (p, n): (Vec3, Vec3),
) {
    if state.p("probeBoxes") > 0.0 {
        probe_boxes(content, shading, state.p("probeBoxes"));
    }
    if state.p("decalBoxes") > 0.0 {
        decal_boxes(
            &mut content.draw,
            &mut content.labels,
            &shading.decals,
            state.p("decalBoxes"),
        );
    }
    if demo == Demo::Culling && (state.p("cullTile") > 0.0 || state.p("lightMarkers") > 0.0) {
        let (_, tile) = culling_view(state, shading, &content.objects, camera);
        if state.p("cullTile") > 0.0 {
            tile_overlay(&mut content.draw, camera, &tile, state.p("cullTile"));
        }
        if state.p("lightMarkers") > 0.0 {
            light_markers(
                &mut content.draw,
                shading,
                (state.p("cullTile") > 0.0).then_some(&tile),
                state.p("lightMarkers"),
            );
        }
    }
    if state.p("cascades") > 0.0 {
        cascade_overlay(&mut content.draw, &mut content.labels, state, camera, p, n);
    }
}

/// The frame's uniforms: the camera, the study room's light panel, the sky's level, the floor, the overlay's
/// exposure, pixel scale, and time, and the objects hidden (every light panel, and the walls when they are off).
fn frame_uniforms(
    state: &ShotState,
    statics: &Statics,
    (objects, camera): (&[ObjectData], lab::layout::CameraData),
    [exposure, pixel_scale, time]: [f32; 3],
) -> FrameUniforms {
    let lighting = statics.scene.lighting;
    let walls = (1 << ids::BACK_WALL) | (1 << ids::LEFT_WALL) | (1 << ids::RIGHT_WALL);
    // Every scene's light panel (the study room's is ids::PANEL; the terrain's is its second object).
    let panels = objects
        .iter()
        .enumerate()
        .filter(|(_, o)| o.shape_kind == lab::scene::SHAPE_PANEL)
        .fold(0u32, |m, (i, _)| m | 1 << i);
    let hidden: u32 = panels
        | if state.p("walls") < 0.5 {
            walls
        } else {
            0
        };
    FrameUniforms {
        camera,
        light_center_radiance: [
            lighting.light.center.x,
            lighting.light.center.y,
            lighting.light.center.z,
            0.0,
        ],
        light_axis_u: lighting.light.axis_u.extend(0.0).to_array(),
        light_axis_v: lighting.light.axis_v.extend(0.0).to_array(),
        light_color: lighting.light.color.extend(1.0).to_array(),
        // The sky box's level (gFrame.ambientColor scales skyRadiance).
        ambient_color: [
            super::ibl::SKY_LEVEL,
            super::ibl::SKY_LEVEL,
            super::ibl::SKY_LEVEL,
            1.0,
        ],
        floor_params: [state.p("floorRoughness"), lighting.f0, 0.0, 1.0],
        overlay_params: [exposure, pixel_scale, time, 0.0],
        counts: [objects.len() as u32, ids::FLOOR as u32, 0, hidden],
        view_params: [1.0, 0.0, 0.0, 0.0],
    }
}

/// gShading's frame records, packed after the camera's clip-to-world matrix (the full-screen passes rebuild
/// positions from depth, shaders/standard_lighting.wgsl): the materials, lights, decals, transmission, sphere
/// lights, probes, SSAO, the cubemap view, and the cascades, each at its offset.
fn pack_shading_frame(
    content: &mut FrameContent,
    state: &ShotState,
    shading: &Shading,
    camera: &lab::layout::CameraData,
) {
    content.shading_frame = clip_to_world(camera).to_vec();
    content.shading_frame.extend(shading.frame_materials());
    content.shading_frame.extend(shading.frame_lights());
    content.shading_frame.resize(FRAME_DECALS, [0.0; 4]);
    content
        .shading_frame
        .extend(super::decals::frame_records(&shading.decals));
    content.shading_frame.resize(FRAME_TRANSMISSION, [0.0; 4]);
    let transmissive = shading
        .materials
        .iter()
        .position(|m| m.transmission.is_some())
        .unwrap_or(0);
    content
        .shading_frame
        .extend(super::transmission::frame_record(
            shading.materials[transmissive].transmission.as_ref(),
            transmissive,
        ));
    content.shading_frame.resize(FRAME_SPHERE_LIGHTS, [0.0; 4]);
    content
        .shading_frame
        .extend(super::transmission::frame_lights(&shading.sphere_lights));
    content.shading_frame.resize(FRAME_PROBES, [0.0; 4]);
    content
        .shading_frame
        .extend(super::reflections::frame_records(&shading.probes));
    content.shading_frame.resize(FRAME_SSAO, [0.0; 4]);
    content.shading_frame.extend(ssao_records(state));
    content.shading_frame.resize(FRAME_CUBEMAP_VIEW, [0.0; 4]);
    content
        .shading_frame
        .extend(super::reflections::cubemap_view_records(state));
    content.shading_frame.resize(FRAME_CASCADES, [0.0; 4]);
    content.shading_frame.extend(cascade_records(state, camera));
}

/// SSAO's occluders for the inspected point (the first `shown_samples`), the simple point light, and the sphere
/// lights, with their labels.
fn scene_marks(
    content: &mut FrameContent,
    (state, shading, demo): (&ShotState, &Shading, Demo),
    (ssao_overlay, shown_samples): (&Option<SsaoOverlay>, f32),
) {
    let draw = &mut content.draw;
    let labels = &mut content.labels;

    if let Some((world_p, world_n, samples)) = ssao_overlay {
        draw.segment(
            *world_p,
            *world_p + *world_n * 0.25,
            palette::NORMAL,
            1.0,
            3.0,
            FLAG_ARROW,
        );
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
            labels.push(Label {
                anchor: lp,
                text: format!(
                    "{} · {:.1} cd",
                    tr("common-simple_point_light"),
                    state.p("lightIntensity")
                ),
                color: LIGHT,
            });
        }
    }
    // The sphere lights: the bulb, as a point at its size.
    for light in &shading.sphere_lights {
        draw.point(light.position, 14.0, SPHERE_LIGHT, 1.0);
        if state.p("labels") > 0.0 || demo == Demo::Transmission {
            labels.push(Label {
                anchor: light.position,
                text: format!(
                    "{} · {:.1} cd",
                    tr("common-point_light_sphere"),
                    state.p("sphereLightIntensity")
                ),
                color: SPHERE_LIGHT,
            });
        }
    }
}

/// The reflection probes' boxes (outer and inner), at `alpha`, each with its centre and name.
fn probe_boxes(content: &mut FrameContent, shading: &Shading, alpha: f32) {
    for probe in &shading.probes {
        let color = [palette::accent(), palette::REFLECTED][probe.cubemap.min(1)];
        obb_edges(
            &mut content.draw,
            probe.position,
            probe.rotation,
            probe.outer_half,
            color,
            alpha * 0.6,
            2.0,
        );
        obb_edges(
            &mut content.draw,
            probe.position,
            probe.rotation,
            probe.inner_half,
            color,
            alpha,
            3.0,
        );
        content.draw.point(probe.position, 9.0, color, alpha);
        content.labels.push(Label {
            anchor: probe.position,
            text: format!("{} {}", tr("demos-probe"), probe.cubemap + 1),
            color,
        });
    }
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
        option
            .text
            .map(tr)
            .or(option.label)
            .unwrap_or(option.name)
            .to_string()
    };
    let mut left = state.clone();
    left.set("viewMode", state.p("splitViewMode"));
    vec![
        lab::investigation::Pane {
            state: left,
            title: Some(title(state.p("splitViewMode"))),
        },
        lab::investigation::Pane {
            state: state.clone(),
            title: Some(title(state.p("viewMode"))),
        },
    ]
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
        Demo::Room | Demo::PointLight => notes.push((
            tr("demos-simple_point_lights_cast_no").to_string(),
            palette::text_faint(),
        )),
        Demo::Cascades => notes.push((
            tr("demos-cascades_schematic").to_string(),
            palette::text_faint(),
        )),
        Demo::Ssao => {
            notes.push((
                tr("demos-half_resolution_8_bits_a").to_string(),
                palette::text_faint(),
            ));
            notes.push((
                tr("demos-the_diffuse_target_only_light").to_string(),
                palette::text_faint(),
            ));
        }
        Demo::Reflections => {
            notes.push((
                tr("demos-the_lab_bakes_the_probes").to_string(),
                palette::text_faint(),
            ));
            notes.push((
                tr("demos-with_the_room_s_light").to_string(),
                palette::text_faint(),
            ));
        }
        Demo::Transmission => {
            notes.push((
                tr("demos-simple_point_lights_add_no").to_string(),
                palette::text_faint(),
            ));
            notes.push((
                tr("demos-the_sphere_light_here_has").to_string(),
                palette::text_faint(),
            ));
        }
        Demo::Diffuse if shading.diffuse_model == DiffuseModel::NormalizedDisney => notes.push((
            tr("demos-disney_diffuse_note").to_string(),
            palette::MISMATCH,
        )),
        Demo::Multiscatter if !shading.multiscatter => notes.push((
            tr("demos-compensation_off_note").to_string(),
            palette::MISMATCH,
        )),
        Demo::Multiscatter => notes.push((
            tr("demos-compensation_1_f0_1_a").to_string(),
            palette::text_faint(),
        )),
        Demo::Ibl => {
            notes.push((
                tr("demos-the_global_skylight_casts_no").to_string(),
                palette::text_faint(),
            ));
            notes.push((
                tr("demos-walls_or_not_only_occlusion").to_string(),
                palette::text_faint(),
            ));
        }
        _ => {}
    }
    if !shading.ibl {
        notes.push((
            tr("demos-ibl_off_direct_lighting_only").to_string(),
            palette::MISMATCH,
        ));
    }
    if !shading.ibl
        && shading.material(shading.inspected).metallic > 0.5
        && demo != Demo::PointLight
    {
        notes.push((
            tr("demos-metals_have_no_diffuse_term").to_string(),
            palette::text_faint(),
        ));
        notes.push((
            tr("demos-without_ibl_only_the_highlight").to_string(),
            palette::text_faint(),
        ));
    }
    notes
}

/// An SSAO occluder in world space: its position, saturate(cos), and weight.
type Occluder = (Vec3, f32, f32);

/// SSAO's overlay: the point, its normal rebuilt from depth, and its occluders.
type SsaoOverlay = (Vec3, Vec3, Vec<Occluder>);

/// The inspected point's SSAO occluders in world space: its position and
/// depth-rebuilt normal, and each occluder with its saturate(cos) and weight
/// (ssao::samples_at, on the lab's ray-traced linear depth); None off screen.
fn ssao_samples(
    state: &ShotState,
    objects: &[ObjectData],
    camera: &lab::layout::CameraData,
) -> Option<SsaoOverlay> {
    let (p, _) = inspected_point(state);
    let projector = lab::camera::Projector::new(camera);
    let view = glam::Mat4::from_cols_array_2d(&camera.world_to_view);
    let size = [
        camera.viewport_size[0] as u32,
        camera.viewport_size[1] as u32,
    ];
    let linear_depth = |x: i32, y: i32| -> f32 {
        let (o, d) = projector.ray(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
        let hit = lab::scene::trace_scene(objects, o, d, 100.0, ids::PANEL as u32);
        if hit.is_hit() {
            -view.transform_point3(o + d * hit.t).z
        } else {
            camera.far_plane
        }
    };
    let projection = glam::Mat4::from_cols_array_2d(&camera.view_to_clip);
    let tan = Vec2::new(1.0 / projection.x_axis.x, 1.0 / projection.y_axis.y);
    let settings = super::ssao::Settings::from_records(&ssao_records(state));
    let (pixel, _) = projector.project(p)?;
    let half_size = [size[0].div_ceil(2), size[1].div_ceil(2)];
    let half = |x: i32, y: i32| linear_depth(2 * x, 2 * y);
    let k = super::ssao::samples_at(
        &half,
        half_size,
        &settings,
        tan,
        pixel.x as i32 / 2,
        pixel.y as i32 / 2,
    );
    // GetViewSpacePosition's space: x right, y up, z forward; the lab's view space looks down -z.
    let to_world = view.inverse();
    let world = |v: Vec3| to_world.transform_point3(Vec3::new(v.x, v.y, -v.z));
    let normal = to_world
        .transform_vector3(Vec3::new(k.normal.x, k.normal.y, -k.normal.z))
        .normalize();
    let terms = k.terms();
    Some((
        world(k.position),
        -normal,
        k.occluders
            .iter()
            .zip(terms)
            .map(|(o, (c, w))| (world(*o), c, w))
            .collect(),
    ))
}

/// The colour `t` of the way from `a` to `b`.
fn mix_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}
