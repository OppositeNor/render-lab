//! The dock's Sliders tab for each demonstration of the lighting
//! investigation, built from the lab's dock kit (src/lab/controls.rs).

use super::demos::{self, DIFFUSE, LIGHT};
use super::shot::Demo;
use crate::lab::controls::{Dock, Unit};
use crate::lab::i18n::t;
use crate::lab::palette;
use crate::lab::scene::ids;
use crate::lab::ui::ViewerAction;
use egui::Ui;

/// The scene pass's views (brdf::view), in order.
fn view_names() -> [&'static str; 16] {
    [
        t("final", "最终"),
        t("diffuse lighting", "漫反射光照"),
        t("specular lighting", "高光光照"),
        "D (GGX)",
        "G (Smith)",
        "F (Schlick)",
        t("multiscatter compensation", "多次散射补偿"),
        "diffuseResponse",
        "NdotL",
        t("radius attenuation", "半径衰减"),
        t("diffuse IBL", "漫反射 IBL"),
        t("specular IBL", "高光 IBL"),
        t("lights per tile", "每个 tile 的光源数"),
        t("probe stencil", "probe stencil"),
        t("probe blend weight", "probe 混合权重"),
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
    let mut toggles: Vec<(&'static str, &str)> = vec![("lightOn", t("light on", "点亮光源"))];
    match demo {
        Demo::Room => toggles.extend([("labels", t("labels", "标签")), ("vectors", t("vectors", "向量")), ("lobe", t("lobes", "波瓣"))]),
        Demo::Culling => toggles.extend([("cullTile", t("the point's tile", "该点的 tile")), ("labels", t("labels", "标签"))]),
        Demo::Surface => toggles.extend([("vectors", t("N, L, V, H", "N、L、V、H")), ("sphereOcclusion", t("occlusion maps", "遮蔽贴图")), ("labels", t("labels", "标签"))]),
        Demo::Ssao => toggles.extend([("ssao", "SSAO"), ("ssaoBlur", t("blur", "模糊")), ("walls", t("walls", "墙"))]),
        Demo::Reflections => toggles.extend([("probes", t("probes", "probe")), ("probeParallax", t("parallax correction", "视差校正")), ("probeBoxes", t("probe boxes", "probe 盒")), ("walls", t("walls", "墙"))]),
        Demo::Transmission => toggles.extend([("sphereLight", t("point light (sphere)", "point light（球形）")), ("vectors", t("N, L, V", "N、L、V")), ("labels", t("labels", "标签"))]),
        Demo::Decals => toggles.extend([("decals", t("decals", "decal")), ("decalBoxes", t("decal boxes", "decal 盒")), ("vectors", t("N, L, V, H", "N、L、V、H"))]),
        Demo::ClearCoat => toggles.extend([("vectors", t("N, L, V, H", "N、L、V、H")), ("sphereClearCoat", t("clear coat", "clear coat")), ("labels", t("labels", "标签"))]),
        Demo::Specular | Demo::Diffuse | Demo::Multiscatter => toggles.extend([("vectors", t("N, L, V, H", "N、L、V、H")), ("lobe", t("lobes", "波瓣")), ("labels", t("labels", "标签"))]),
        Demo::PointLight => toggles.extend([("falloff", t("falloff curve", "衰减曲线")), ("radius", t("attenuation radius", "衰减半径")), ("vectors", t("vectors", "向量"))]),
        Demo::Ibl => toggles.extend([("vectors", t("N, V, R", "N、V、R")), ("walls", t("walls", "墙"))]),
    }
    toggles.extend([("multiscatter", t("multiscatter", "多次散射补偿")), ("specularAA", "specular AA"), ("ibl", "IBL")]);
    dock.show(ui, &toggles);
}

fn view(dock: &mut Dock, ui: &mut Ui) {
    let names = view_names();
    let current = (dock.state.p("viewMode").round().max(0.0) as usize).min(names.len() - 1);
    let hint = t("What the scene pass writes: the lit result, one of its parts, or one BRDF term in false colour.", "场景 pass 输出的内容：光照结果、其中一部分，或以伪彩色显示的某一项 BRDF。");
    dock.group(ui, t("view", "视图"), Some(hint), |d, ui| {
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
            d.slider(ui, "exposure", t("exposure", "曝光"), palette::TEXT, 0.2..=4.0, Unit::Factor);
            d.slider(ui, "formula", t("formula term", "公式项"), palette::TEXT, -1.0..=terms, Unit::Index);
        });
    });
}

fn light(dock: &mut Dock, ui: &mut Ui, demo: Demo) {
    let hint = t("The SimplePointLight record: its position, its intensity in candelas, and the radius at which it fades to zero.", "SimplePointLight 记录：位置、以 candela 计的强度，以及衰减到零的半径。");
    dock.group(ui, t("simple point light", "simple point light"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-light", |d, ui| {
            d.slider(ui, "lightX", "x", palette::AXIS_X, -2.8..=2.8, Unit::Metres);
            d.slider(ui, "lightY", "y", palette::AXIS_Y, 0.1..=2.9, Unit::Metres);
            d.slider(ui, "lightZ", "z", palette::AXIS_Z, -2.8..=2.8, Unit::Metres);
            d.slider(ui, "lightIntensity", t("candelas", "candela"), LIGHT, 0.0..=30.0, Unit::Plain);
            d.slider(ui, "attenuationRadius", t("radius", "半径"), LIGHT, 0.5..=12.0, Unit::Metres);
            if demo != Demo::PointLight {
                d.slider(ui, "iblExposure", t("IBL exposure", "IBL 曝光"), palette::TEXT_DIM, -4.0..=4.0, Unit::Plain);
            }
        });
    });
}

/// The point light's and the sphere's lighting-channel masks (five channels).
fn lighting_channels(dock: &mut Dock, ui: &mut Ui) {
    let hint = t("A light shades a surface only when their lighting-channel masks share a bit (five channels; the editor's default is all of them, 31).", "只有当光源与表面的 lighting channel 掩码有共同位时，光源才照亮该表面（五个 channel；编辑器默认全部开启，即 31）。");
    dock.group(ui, t("lighting channels", "lighting channel"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-channels", |d, ui| {
            d.slider(ui, "lightChannels", t("light mask", "光源掩码"), LIGHT, 0.0..=31.0, Unit::Index);
            d.slider(ui, "sphereLightingChannels", t("sphere mask", "球体掩码"), palette::TEXT, 0.0..=31.0, Unit::Index);
        });
    });
}

/// The light-culling demonstration's extra simple point lights.
fn culling_lights(dock: &mut Dock, ui: &mut Ui) {
    let hint = t("More simple point lights over the room; each 16 x 16 tile keeps only those whose sphere meets its box and its depth mask. Click a surface to inspect its tile.", "房间中更多的 simple point light；每个 16 x 16 的 tile 只保留球体与其包围盒和深度掩码相交的光源。点击表面以查看它所在的 tile。");
    dock.group(ui, t("lights · culling", "光源 · 剔除"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-culling", |d, ui| {
            d.slider(ui, "extraLights", t("lights", "光源数"), LIGHT, 0.0..=48.0, Unit::Index);
            d.slider(ui, "extraLightRadius", t("radius", "半径"), LIGHT, 0.2..=2.5, Unit::Metres);
            d.slider(ui, "extraLightIntensity", "candelas", LIGHT, 0.0..=3.0, Unit::Plain);
        });
    });
}

/// SSAO's settings (AoSettings, or the passes' defaults without an AO component).
fn ssao(dock: &mut Dock, ui: &mut Ui) {
    let hint = t("SSAO at half resolution: 52 samples per pixel from the depth alone, a depth-aware blur down and across, an upsample by depth, then the diffuse target times the result.", "半分辨率的 SSAO：每个像素仅从深度取 52 个采样，竖直与水平的深度感知模糊，按深度上采样，最后用结果乘以漫反射 target。");
    dock.group(ui, "SSAO", Some(hint), |d, ui| {
        d.rows(ui, "o3de-ssao", |d, ui| {
            d.slider(ui, "ssaoStrength", "strength", palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "ssaoRadius", t("sampling radius", "采样半径"), palette::ACCENT, 0.0..=0.2, Unit::Plain);
            d.slider(ui, "ssaoBlurConstFalloff", t("blur falloff", "模糊衰减"), palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "ssaoBlurDepthThreshold", t("depth threshold", "深度阈值"), palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "ssaoBlurDepthStrength", t("depth strength", "深度强度"), palette::ACCENT, 0.0..=400.0, Unit::Plain);
        });
    });
}

/// The two reflection probes' settings.
fn probes(dock: &mut Dock, ui: &mut Ui) {
    let hint = t("Two reflection probes, baked from the room. Inside a probe's inner box its cubemap replaces the global one; between the inner and outer boxes it blends by distance. Views: probe stencil, probe blend weight.", "两个从房间烘焙的反射 probe。在 probe 的 inner 盒内，它的 cubemap 取代全局 cubemap；在 inner 与 outer 盒之间按距离混合。视图：probe stencil、probe 混合权重。");
    dock.group(ui, t("reflection probes", "反射 probe"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-probes", |d, ui| {
            d.slider(ui, "probeInnerScale", t("inner extents", "inner 尺寸"), palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "probeExposure", t("exposure (stops)", "曝光（stop）"), palette::ACCENT, -4.0..=4.0, Unit::Plain);
        });
    });
}

/// The teapot's transmission (EnhancedPBR) and the sphere light behind it.
fn transmission(dock: &mut Dock, ui: &mut Ui) {
    let hint = t("The teapot as EnhancedPBR with transmission, lit from behind by an O3DE point light (a sphere area light without shadows). Mode: 0 None, 1 ThickObject, 2 ThinObject.", "茶壶作为带透射的 EnhancedPBR，由身后的 O3DE point light（无阴影的球形面光源）照亮。模式：0 None、1 ThickObject、2 ThinObject。");
    dock.group(ui, t("transmission · teapot", "透射 · 茶壶"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-transmission", |d, ui| {
            d.slider(ui, "teapotTransmission", t("mode", "模式"), palette::ACCENT, 0.0..=2.0, Unit::Index);
            d.slider(ui, "transmissionThickness", "thickness", palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "transmissionPower", "power", palette::ACCENT, 0.0..=20.0, Unit::Plain);
            d.slider(ui, "transmissionDistortion", "distortion", palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "transmissionAttenuation", "attenuation", palette::ACCENT, 0.0..=10.0, Unit::Plain);
            d.slider(ui, "transmissionScale", "scale", palette::ACCENT, 0.0..=4.0, Unit::Plain);
            d.slider(ui, "transmissionNdLBias", t("NdotL bias (thin)", "NdotL bias（thin）"), palette::ACCENT, -1.0..=1.0, Unit::Plain);
            d.slider(ui, "transmissionDistanceAttenuation", t("distance attenuation (thin)", "distance attenuation（thin）"), palette::ACCENT, 0.0..=4.0, Unit::Plain);
        });
    });
    dock.group(ui, t("point light (sphere)", "point light（球形）"), None, |d, ui| {
        d.rows(ui, "o3de-sphere-light", |d, ui| {
            d.slider(ui, "sphereLightX", "x", palette::AXIS_X, -2.8..=2.8, Unit::Metres);
            d.slider(ui, "sphereLightY", "y", palette::AXIS_Y, 0.0..=2.9, Unit::Metres);
            d.slider(ui, "sphereLightZ", "z", palette::AXIS_Z, -2.9..=2.8, Unit::Metres);
            d.slider(ui, "sphereLightIntensity", "candelas", super::demos::SPHERE_LIGHT, 0.0..=10.0, Unit::Plain);
            d.slider(ui, "sphereLightRadius", t("radius", "半径"), super::demos::SPHERE_LIGHT, 0.5..=6.0, Unit::Metres);
            d.slider(ui, "sphereLightBulb", t("bulb radius", "灯泡半径"), super::demos::SPHERE_LIGHT, 0.0..=0.5, Unit::Metres);
        });
    });
}

/// The two floor decals (decals.rs): the decal component's properties.
fn decals(dock: &mut Dock, ui: &mut Ui) {
    let hint = t("Two decals on the floor: a splat and a panel grid. ApplyDecal blends each into the surface before lighting, in sort-key order; tilt the splat to see the angle attenuation.", "地面上的两个 decal：一个泼溅和一个面板网格。ApplyDecal 在光照之前按 sort key 顺序把它们混合进表面；倾斜泼溅可以看到角度衰减。");
    dock.group(ui, t("decals · floor", "decal · 地面"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-decals", |d, ui| {
            d.slider(ui, "decalOpacity", "opacity", palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "decalNormalOpacity", t("normal map opacity", "normal map opacity"), palette::NORMAL, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "decalAngleAttenuation", t("attenuation angle", "attenuation angle"), palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "decalSplatSortKey", t("splat sort key", "泼溅 sort key"), palette::ACCENT, 0.0..=32.0, Unit::Index);
            d.slider(ui, "decalTilt", t("splat tilt", "泼溅倾斜"), palette::ACCENT, -80.0..=80.0, Unit::Degrees);
            d.slider(ui, "decalTurn", t("grid turn", "网格旋转"), palette::REFLECTED, -180.0..=180.0, Unit::Degrees);
        });
    });
}

/// The sphere's clear coat (the clearCoat property group).
fn clear_coat(dock: &mut Dock, ui: &mut Ui) {
    let hint = t("The sphere's clear coat: a dielectric layer (F0 0.04) over the base, with its own roughness (toggle it under show).", "球体的 clear coat：底层之上的电介质层（F0 0.04），有自己的 roughness（在“显示”中开关）。");
    dock.group(ui, t("clear coat · sphere", "clear coat · 球体"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-clear-coat", |d, ui| {
            d.slider(ui, "clearCoatFactor", "factor", palette::ACCENT, 0.0..=1.0, Unit::Plain);
            d.slider(ui, "clearCoatRoughness", "roughness", palette::ACCENT, 0.0..=1.0, Unit::Plain);
        });
    });
}

/// The sphere's occlusion maps: GetOcclusionInput raises each to its factor.
fn occlusion(dock: &mut Dock, ui: &mut Ui) {
    let hint = t("The sphere's diffuse AO and specular cavity maps (toggle them under show): each is raised to the power of its factor.", "球体的 diffuse AO 与 specular cavity 贴图（在“显示”中开关）：每张贴图取其 factor 次幂。");
    dock.group(ui, t("occlusion · sphere", "遮蔽 · 球体"), Some(hint), |d, ui| {
        d.rows(ui, "o3de-occlusion", |d, ui| {
            d.slider(ui, "diffuseOcclusionFactor", t("diffuse factor", "diffuse factor"), DIFFUSE, 0.0..=2.0, Unit::Plain);
            d.slider(ui, "specularOcclusionFactor", t("specular factor", "specular factor"), super::demos::SPECULAR, 0.0..=2.0, Unit::Plain);
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
        ids::FLOOR => (Some("floorRoughness"), None),
        _ => (None, None),
    };
    let name = crate::lab::i18n::object_name(dock.input.statics.scene.objects[object].name);
    let hint = t("The StandardPBR inputs of the inspected object (click another object to edit it); specularF0 factor applies to every object.", "选中物体的 StandardPBR 输入（点击其他物体以编辑它）；specularF0 factor 作用于所有物体。");
    dock.group(ui, &format!("{} · {name}", t("material", "材质")), Some(hint), |d, ui| {
        d.rows(ui, "o3de-material", |d, ui| {
            if let Some(roughness) = roughness {
                d.slider(ui, roughness, "roughness", palette::ACCENT, 0.0..=1.0, Unit::Plain);
            }
            if let Some(metallic) = metallic {
                d.slider(ui, metallic, "metallic", palette::TEXT, 0.0..=1.0, Unit::Plain);
            }
            d.slider(ui, "specularF0Factor", "specularF0 factor", super::demos::SPECULAR, 0.0..=1.0, Unit::Plain);
        });
    });
}
