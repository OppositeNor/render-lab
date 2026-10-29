//! The dock's Sliders tab for each demonstration of the lighting
//! investigation, built from the lab's dock kit (lab/src/controls.rs).

use super::demos;
use super::shot::Demo;
use lab::controls::{Dock, Unit};
use crate::o3de::lighting::text::tr;
use lab::palette;
use lab::scene::ids;
use egui::Ui;

pub fn parameters(dock: &mut Dock, ui: &mut Ui) {
    let demo = Demo::of(dock.state.demo);
    view(dock, ui);
    if demo == Demo::Room {
        dock.camera(ui);
    }
    light(dock, ui, demo);
    material(dock, ui);
    if demo == Demo::Surface {
        occlusion(dock, ui);
    }
    if demo == Demo::ClearCoat {
        clear_coat(dock, ui);
    }
    if demo == Demo::Decals {
        decals(dock, ui);
    }
    if demo == Demo::Transmission {
        transmission(dock, ui);
    }
    if demo == Demo::Reflections {
        probes(dock, ui);
    }
    if demo == Demo::Ssao {
        ssao(dock, ui);
    }
    if demo == Demo::Culling {
        culling_lights(dock, ui);
    }
    if demo == Demo::PointLight {
        lighting_channels(dock, ui);
    }
    let mut toggles: Vec<(&'static str, &str)> = vec![("lightOn", tr("controls.light_on"))];
    match demo {
        Demo::Room => toggles.extend([("labels", tr("controls.labels")), ("vectors", tr("controls.vectors")), ("lobe", tr("controls.lobes"))]),
        Demo::Culling => toggles.extend([("cullTile", tr("controls.the_point_s_tile")), ("lightMarkers", tr("controls.light_markers")), ("labels", tr("controls.labels"))]),
        Demo::Surface => toggles.extend([("vectors", tr("controls.n_l_v_h")), ("sphereOcclusion", tr("controls.occlusion_maps")), ("labels", tr("controls.labels"))]),
        Demo::Ssao => toggles.extend([("ssao", "SSAO"), ("ssaoBlur", tr("controls.blur")), ("walls", tr("controls.walls"))]),
        Demo::Reflections => toggles.extend([("probes", tr("controls.probes")), ("probeParallax", tr("controls.parallax_correction")), ("probeBoxes", tr("controls.probe_boxes")), ("walls", tr("controls.walls")), ("probeCubemap", tr("controls.cubemap_cross")), ("probeBall", tr("controls.mirror_ball"))]),
        Demo::Transmission => toggles.extend([("sphereLight", tr("common.point_light_sphere")), ("vectors", tr("controls.n_l_v")), ("labels", tr("controls.labels"))]),
        Demo::Decals => toggles.extend([("decals", tr("controls.decals")), ("decalBoxes", tr("controls.decal_boxes")), ("vectors", tr("controls.n_l_v_h"))]),
        Demo::ClearCoat => toggles.extend([("vectors", tr("controls.n_l_v_h")), ("sphereClearCoat", tr("controls.clear_coat")), ("labels", tr("controls.labels"))]),
        Demo::Specular | Demo::Diffuse | Demo::Multiscatter => toggles.extend([("vectors", tr("controls.n_l_v_h")), ("lobe", tr("controls.lobes")), ("labels", tr("controls.labels"))]),
        Demo::PointLight => toggles.extend([("falloff", tr("controls.falloff_curve")), ("radius", tr("common.attenuation_radius")), ("vectors", tr("controls.vectors"))]),
        Demo::Ibl => toggles.extend([("vectors", tr("controls.n_v_r")), ("walls", tr("controls.walls"))]),
    }
    toggles.extend([("multiscatter", tr("controls.multiscatter")), ("specularAA", "specular AA"), ("ibl", "IBL")]);
    dock.show(ui, &toggles);
}

fn view(dock: &mut Dock, ui: &mut Ui) {
    let hint = tr("controls.output_hint");
    dock.group(ui, tr("controls.view"), Some(hint), |d, ui| {
        // The choices' options (brdf::ViewMode, brdf::DiffuseModel).
        d.choice(ui, "viewMode");
        if d.state.demo == Demo::Diffuse.id() {
            d.choice(ui, "diffuseModel");
        }
        let terms = super::formulas::terms(Demo::of(d.state.demo)) as f32;
        d.rows(ui, "o3de-exposure", |d, ui| {
            d.value(ui, "exposure");
            d.slider(ui, "formula", tr("controls.formula_term"), palette::text(), -1.0..=terms, Unit::Index);
        });
    });
}

fn light(dock: &mut Dock, ui: &mut Ui, demo: Demo) {
    let hint = tr("controls.point_light_hint");
    dock.group(ui, tr("common.simple_point_light"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-light", |d, ui| {
            // The position and the IBL exposure are gShading's (shaders/standard_lighting.wgsl), the rest values.rs's.
            for key in ["lightPosition", "lightIntensity", "attenuationRadius"] {
                d.value(ui, key);
            }
            if demo != Demo::PointLight {
                d.value(ui, "iblExposure");
            }
        });
    });
}

/// The point light's and the sphere's lighting-channel masks (five channels).
fn lighting_channels(dock: &mut Dock, ui: &mut Ui) {
    dock.values(ui, tr("controls.lighting_channels"), Some(tr("controls.lighting_channels_hint")), "channels");
}

/// The light-culling demonstration's extra simple point lights.
fn culling_lights(dock: &mut Dock, ui: &mut Ui) {
    dock.values(ui, tr("controls.lights_culling"), Some(tr("controls.culling_hint")), "culling");
}

/// SSAO's settings (AoSettings, or the passes' defaults without an AO component).
fn ssao(dock: &mut Dock, ui: &mut Ui) {
    dock.values(ui, "SSAO", Some(tr("controls.ssao_hint")), "ssao");
}

/// The two reflection probes' settings, and the baked cubemap's views.
fn probes(dock: &mut Dock, ui: &mut Ui) {
    dock.values(ui, tr("controls.reflection_probes"), Some(tr("controls.probes_hint")), "probes");
    dock.values(ui, tr("controls.baked_cubemap"), Some(tr("controls.cubemap_hint")), "cubemap");
}

/// The teapot's transmission (EnhancedPBR) and the sphere light behind it.
fn transmission(dock: &mut Dock, ui: &mut Ui) {
    dock.values(ui, tr("controls.transmission_teapot"), Some(tr("controls.transmission_hint")), "transmission");
    dock.values(ui, tr("common.point_light_sphere"), None, "sphere_light");
}

/// The two floor decals (decals.rs): the decal component's properties.
fn decals(dock: &mut Dock, ui: &mut Ui) {
    dock.values(ui, tr("controls.decals_floor"), Some(tr("controls.decals_hint")), "decals");
}

/// The sphere's clear coat (the clearCoat property group).
fn clear_coat(dock: &mut Dock, ui: &mut Ui) {
    dock.values(ui, tr("controls.clear_coat_sphere"), Some(tr("controls.clear_coat_hint")), "clear_coat");
}

/// The sphere's occlusion maps: GetOcclusionInput raises each to its factor.
fn occlusion(dock: &mut Dock, ui: &mut Ui) {
    dock.values(ui, tr("controls.occlusion_sphere"), Some(tr("controls.maps_hint")), "occlusion");
}

fn material(dock: &mut Dock, ui: &mut Ui) {
    let object = demos::inspected_object(dock.state) as usize;
    // Walls and the small box keep the study room's material.
    let (roughness, metallic) = match object {
        ids::SPHERE => (Some("sphereRoughness"), Some("sphereMetallic")),
        ids::TEAPOT => (Some("teapotRoughness"), Some("teapotMetallic")),
        ids::BOX => (Some("boxRoughness"), Some("boxMetallic")),
        ids::FLOOR => (Some("floorRoughness"), Some("floorMetallic")),
        _ => (None, None),
    };
    let name = lab::i18n::object_name(dock.input.statics.scene.objects[object].name);
    let hint = tr("controls.material_hint");
    dock.group(ui, &format!("{} · {name}", tr("controls.material")), Some(hint), |d, ui| {
        d.rows(ui, "o3de-material", |d, ui| {
            for key in [roughness, metallic, (object == ids::FLOOR).then_some("floorBaseColorFactor"), Some("specularF0Factor")].into_iter().flatten() {
                d.value(ui, key);
            }
        });
    });
}
