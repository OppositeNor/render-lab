//! The dock's Sliders tab for each demonstration of the lighting
//! investigation, built from the lab's dock kit (lab/src/controls.rs).

use super::demos::{self, DIFFUSE, LIGHT};
use super::shot::Demo;
use lab::controls::{Dock, Unit};
use crate::o3de::lighting::text::tr;
use lab::palette;
use lab::scene::ids;
use lab::ui::ViewerAction;
use egui::Ui;

/// The scene pass's views (brdf::view), in order.
fn view_names() -> [&'static str; 16] {
    [
        tr("controls.final"),
        tr("controls.diffuse_lighting"),
        tr("controls.specular_lighting"),
        "D (GGX)",
        "G (Smith)",
        "F (Schlick)",
        tr("controls.multiscatter_compensation"),
        "diffuseResponse",
        "NdotL",
        tr("controls.radius_attenuation"),
        tr("common.diffuse_ibl"),
        tr("common.specular_ibl"),
        tr("controls.lights_per_tile"),
        tr("controls.probe_stencil"),
        tr("controls.probe_blend_weight"),
        "SSAO",
    ]
}

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
    let names = view_names();
    let current = (dock.state.p("viewMode").round().max(0.0) as usize).min(names.len() - 1);
    let hint = tr("controls.output_hint");
    dock.group(ui, tr("controls.view"), Some(hint), |d, ui| {
        egui::ComboBox::from_id_salt("o3de-view").selected_text(names[current]).width(250.0).show_ui(ui, |ui| {
            for (i, name) in names.iter().enumerate() {
                if ui.selectable_label(i == current, *name).clicked() {
                    d.actions.push(ViewerAction::SetParam("viewMode", i as f32));
                }
            }
        });
        if d.state.demo == Demo::Diffuse.id() {
            let disney = d.state.p("diffuseModel") > 0.5;
            for (value, label) in [(0.0, "DiffuseLambertian"), (1.0, "NormalizedDisneyDiffuse")] {
                if ui.selectable_label(disney == (value > 0.5), label).clicked() {
                    d.actions.push(ViewerAction::SetParam("diffuseModel", value));
                }
            }
        }
        let terms = super::formulas::terms(Demo::of(d.state.demo)) as f32;
        d.rows(ui, "o3de-exposure", |d, ui| {
            d.slider(ui, "exposure", tr("controls.exposure"), palette::TEXT, 0.2..=4.0, Unit::Factor);
            d.slider(ui, "formula", tr("controls.formula_term"), palette::TEXT, -1.0..=terms, Unit::Index);
        });
    });
}

fn light(dock: &mut Dock, ui: &mut Ui, demo: Demo) {
    let hint = tr("controls.point_light_hint");
    dock.group(ui, tr("common.simple_point_light"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-light", |d, ui| {
            d.slider(ui, "lightX", "x", palette::AXIS_X, -2.8..=2.8, Unit::Metres);
            d.slider(ui, "lightY", "y", palette::AXIS_Y, 0.1..=2.9, Unit::Metres);
            d.slider(ui, "lightZ", "z", palette::AXIS_Z, -2.8..=2.8, Unit::Metres);
            d.slider(ui, "lightIntensity", tr("controls.candelas"), LIGHT, 0.0..=30.0, Unit::Plain);
            d.slider(ui, "attenuationRadius", tr("controls.radius"), LIGHT, 0.5..=12.0, Unit::Metres);
            if demo != Demo::PointLight {
                d.slider(ui, "iblExposure", tr("controls.ibl_exposure"), palette::TEXT_DIM, -4.0..=4.0, Unit::Plain);
            }
        });
    });
}

/// The point light's and the sphere's lighting-channel masks (five channels).
fn lighting_channels(dock: &mut Dock, ui: &mut Ui) {
    let hint = tr("controls.lighting_channels_hint");
    dock.group(ui, tr("controls.lighting_channels"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-channels", |d, ui| {
            d.slider(ui, "lightChannels", tr("controls.light_mask"), LIGHT, 0.0..=31.0, Unit::Index);
            d.slider(ui, "sphereLightingChannels", tr("controls.sphere_mask"), palette::TEXT, 0.0..=31.0, Unit::Index);
        });
    });
}

/// The light-culling demonstration's extra simple point lights.
fn culling_lights(dock: &mut Dock, ui: &mut Ui) {
    let hint = tr("controls.culling_hint");
    dock.group(ui, tr("controls.lights_culling"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-culling", |d, ui| {
            d.slider(ui, "extraLights", tr("controls.lights"), LIGHT, 0.0..=48.0, Unit::Index);
            d.slider(ui, "extraLightRadius", tr("controls.radius"), LIGHT, 0.2..=2.5, Unit::Metres);
            d.slider(ui, "extraLightIntensity", "candelas", LIGHT, 0.0..=3.0, Unit::Plain);
            d.slider(ui, "extraLightMotion", tr("controls.light_motion"), LIGHT, -360.0..=360.0, Unit::Degrees);
        });
    });
}

/// SSAO's settings (AoSettings, or the passes' defaults without an AO component).
fn ssao(dock: &mut Dock, ui: &mut Ui) {
    let hint = tr("controls.ssao_hint");
    dock.group(ui, "SSAO", Some(hint), |d, ui| {
        d.rows(ui, "o3de-ssao", |d, ui| {
            d.slider(ui, "ssaoStrength", "strength", palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "ssaoRadius", tr("controls.sampling_radius"), palette::ACCENT, 0.0..=0.2, Unit::Plain);
            d.slider(ui, "ssaoBlurConstFalloff", tr("controls.blur_falloff"), palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "ssaoBlurDepthThreshold", tr("controls.depth_threshold"), palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "ssaoBlurDepthStrength", tr("controls.depth_strength"), palette::ACCENT, 0.0..=400.0, Unit::Plain);
        });
    });
}

/// The two reflection probes' settings.
fn probes(dock: &mut Dock, ui: &mut Ui) {
    let hint = tr("controls.probes_hint");
    dock.group(ui, tr("controls.reflection_probes"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-probes", |d, ui| {
            d.slider(ui, "probeInnerScale", tr("controls.inner_extents"), palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "probeExposure", tr("controls.exposure_stops"), palette::ACCENT, -4.0..=4.0, Unit::Plain);
        });
    });
    let hint = tr("controls.cubemap_hint");
    dock.group(ui, tr("controls.baked_cubemap"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-cubemap", |d, ui| {
            d.slider(ui, "probeShown", tr("demos.probe"), palette::ACCENT, 1.0..=2.0, Unit::Index);
            d.slider(ui, "probeCubemapMip", "mip", palette::ACCENT, 0.0..=5.0, Unit::Plain);
            d.slider(ui, "probeBakeFaces", tr("controls.faces_baked"), palette::ACCENT, 0.0..=6.0, Unit::Plain);
            d.slider(ui, "probeBallMip", tr("controls.ball_mip"), palette::REFLECTED, 0.0..=5.0, Unit::Plain);
        });
    });
}

/// The teapot's transmission (EnhancedPBR) and the sphere light behind it.
fn transmission(dock: &mut Dock, ui: &mut Ui) {
    let hint = tr("controls.transmission_hint");
    dock.group(ui, tr("controls.transmission_teapot"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-transmission", |d, ui| {
            d.slider(ui, "teapotTransmission", tr("controls.mode"), palette::ACCENT, 0.0..=2.0, Unit::Index);
            d.slider(ui, "transmissionThickness", "thickness", palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "transmissionPower", "power", palette::ACCENT, 0.0..=20.0, Unit::Plain);
            d.slider(ui, "transmissionDistortion", "distortion", palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "transmissionAttenuation", "attenuation", palette::ACCENT, 0.0..=10.0, Unit::Plain);
            d.slider(ui, "transmissionScale", "scale", palette::ACCENT, 0.0..=4.0, Unit::Plain);
            d.slider(ui, "transmissionNdLBias", tr("controls.ndotl_bias_thin"), palette::ACCENT, -1.0..=1.0, Unit::Plain);
            d.slider(ui, "transmissionDistanceAttenuation", tr("controls.distance_attenuation_thin"), palette::ACCENT, 0.0..=4.0, Unit::Plain);
        });
    });
    dock.group(ui, tr("common.point_light_sphere"), None, |d, ui| {
        d.rows(ui, "o3de-sphere-light", |d, ui| {
            d.slider(ui, "sphereLightX", "x", palette::AXIS_X, -2.8..=2.8, Unit::Metres);
            d.slider(ui, "sphereLightY", "y", palette::AXIS_Y, 0.0..=2.9, Unit::Metres);
            d.slider(ui, "sphereLightZ", "z", palette::AXIS_Z, -2.9..=2.8, Unit::Metres);
            d.slider(ui, "sphereLightIntensity", "candelas", super::demos::SPHERE_LIGHT, 0.0..=10.0, Unit::Plain);
            d.slider(ui, "sphereLightRadius", tr("controls.radius"), super::demos::SPHERE_LIGHT, 0.5..=6.0, Unit::Metres);
            d.slider(ui, "sphereLightBulb", tr("controls.bulb_radius"), super::demos::SPHERE_LIGHT, 0.0..=0.5, Unit::Metres);
        });
    });
}

/// The two floor decals (decals.rs): the decal component's properties.
fn decals(dock: &mut Dock, ui: &mut Ui) {
    let hint = tr("controls.decals_hint");
    dock.group(ui, tr("controls.decals_floor"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-decals", |d, ui| {
            d.slider(ui, "decalOpacity", "opacity", palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "decalNormalOpacity", tr("controls.normal_map_opacity"), palette::NORMAL, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "decalAngleAttenuation", tr("controls.attenuation_angle"), palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "decalSplatSortKey", tr("controls.splat_sort_key"), palette::ACCENT, 0.0..=32.0, Unit::Index);
            d.slider(ui, "decalTilt", tr("controls.splat_tilt"), palette::ACCENT, -80.0..=80.0, Unit::Degrees);
            d.slider(ui, "decalTurn", tr("controls.grid_turn"), palette::REFLECTED, -180.0..=180.0, Unit::Degrees);
        });
    });
}

/// The sphere's clear coat (the clearCoat property group).
fn clear_coat(dock: &mut Dock, ui: &mut Ui) {
    let hint = tr("controls.clear_coat_hint");
    dock.group(ui, tr("controls.clear_coat_sphere"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-clear-coat", |d, ui| {
            d.slider(ui, "clearCoatFactor", "factor", palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "clearCoatRoughness", "roughness", palette::ACCENT, 0.0..=1.0, Unit::Plain);
        });
    });
}

/// The sphere's occlusion maps: GetOcclusionInput raises each to its factor.
fn occlusion(dock: &mut Dock, ui: &mut Ui) {
    let hint = tr("controls.maps_hint");
    dock.group(ui, tr("controls.occlusion_sphere"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-occlusion", |d, ui| {
            d.slider(ui, "diffuseOcclusionFactor", tr("controls.diffuse_factor"), DIFFUSE, 0.0..=2.0, Unit::Plain);
            d.slider(ui, "specularOcclusionFactor", tr("controls.specular_factor"), super::demos::SPECULAR, 0.0..=2.0, Unit::Plain);
        });
    });
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
            if let Some(roughness) = roughness {
                d.slider(ui, roughness, "roughness", palette::ACCENT, 0.0..=1.0, Unit::Plain);
            }
            if let Some(metallic) = metallic {
                d.slider(ui, metallic, "metallic", palette::TEXT, 0.0..=1.0, Unit::Plain);
            }
            if object == ids::FLOOR {
                d.slider(ui, "floorBaseColorFactor", "baseColor factor", palette::TEXT, 0.0..=5.0, Unit::Factor);
            }
            d.slider(ui, "specularF0Factor", "specularF0 factor", super::demos::SPECULAR, 0.0..=1.0, Unit::Plain);
        });
    });
}
