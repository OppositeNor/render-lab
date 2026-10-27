//! The demonstrations of the lighting investigation and its shot parameters:
//! what each demonstration shows by default, every numeric parameter the
//! dock may set, and the viewer's Play animations.

use crate::lab::animation::{Clip, Ease, Motion, Orbit, Repeat, Wave};
use crate::lab::camera::OrbitCamera;
use crate::o3de::lighting::text::tr;
use crate::lab::scene::ids;
use crate::lab::shot::{DemoId, PanelMode, RecordRef, ShotState};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Demo {
    Room,
    Culling,
    Surface,
    Decals,
    Specular,
    ClearCoat,
    Diffuse,
    Transmission,
    Multiscatter,
    PointLight,
    Ibl,
    Reflections,
    Ssao,
}

impl Demo {
    pub const ALL: [Demo; 13] = [Demo::Room, Demo::Culling, Demo::Surface, Demo::Decals, Demo::Specular, Demo::ClearCoat, Demo::Diffuse, Demo::Transmission, Demo::Multiscatter, Demo::PointLight, Demo::Ibl, Demo::Reflections, Demo::Ssao];

    pub fn key(self) -> &'static str {
        match self {
            Demo::Room => "room",
            Demo::Culling => "culling",
            Demo::Surface => "surface",
            Demo::Decals => "decals",
            Demo::Specular => "specular",
            Demo::ClearCoat => "clearcoat",
            Demo::Diffuse => "diffuse",
            Demo::Transmission => "transmission",
            Demo::Multiscatter => "multiscatter",
            Demo::PointLight => "pointlight",
            Demo::Ibl => "ibl",
            Demo::Reflections => "reflections",
            Demo::Ssao => "ssao",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Demo::Room => tr("shot.one_point_light_standardpbr"),
            Demo::Culling => tr("common.tiled_light_culling"),
            Demo::Surface => tr("shot.the_metallic_workflow"),
            Demo::Decals => tr("shot.decals"),
            Demo::Specular => tr("shot.ggx_specular_d_g_and"),
            Demo::ClearCoat => tr("shot.clear_coat"),
            Demo::Diffuse => tr("shot.diffuse_and_its_response"),
            Demo::Transmission => tr("shot.transmission_enhancedpbr"),
            Demo::Multiscatter => tr("shot.multiscatter_compensation"),
            Demo::PointLight => tr("shot.the_simple_point_light"),
            Demo::Ibl => tr("shot.image_based_lighting"),
            Demo::Reflections => tr("common.reflection_probes"),
            Demo::Ssao => tr("shot.screen_space_ambient_occlusion"),
        }
    }

    pub fn tab(self) -> &'static str {
        match self {
            Demo::Room => tr("shot.room"),
            Demo::Culling => tr("shot.culling"),
            Demo::Surface => tr("shot.surface"),
            Demo::Decals => tr("shot.decals"),
            Demo::Specular => tr("shot.specular"),
            Demo::ClearCoat => tr("shot.clear_coat"),
            Demo::Diffuse => tr("shot.diffuse"),
            Demo::Transmission => tr("shot.transmission"),
            Demo::Multiscatter => tr("shot.multiscatter"),
            Demo::PointLight => tr("shot.point_light"),
            Demo::Ibl => "IBL",
            Demo::Reflections => tr("shot.probes"),
            Demo::Ssao => "SSAO",
        }
    }

    /// The listing the Code tab opens first (pseudocode/).
    pub fn listing(self) -> &'static str {
        match self {
            Demo::Room => "forward-pass",
            Demo::Culling => "light-culling",
            Demo::Surface => "surface-setup",
            Demo::Decals => "decals",
            Demo::Specular => "specular-ggx",
            Demo::ClearCoat => "clear-coat",
            Demo::Diffuse => "diffuse-response",
            Demo::Transmission => "transmission",
            Demo::Multiscatter => "multiscatter",
            Demo::PointLight => "simple-point-light",
            Demo::Ibl => "ibl-passes",
            Demo::Reflections => "reflection-probes",
            Demo::Ssao => "ssao",
        }
    }

    pub fn of(id: DemoId) -> Demo {
        Demo::ALL.into_iter().find(|d| d.key() == id.key()).unwrap_or_else(|| panic!("{} is not a demonstration of the O3DE lighting investigation", id.key()))
    }

    pub fn id(self) -> DemoId {
        DemoId::new(self.key())
    }
}

/// Every numeric shot parameter, its default, and what it is.
pub const PARAMS: &[(&str, f32, &str)] = &[
    ("lightX", 0.55, "simple point light position x, metres"),
    ("lightY", 1.85, "simple point light position y, metres"),
    ("lightZ", 0.95, "simple point light position z, metres"),
    ("lightIntensity", 6.0, "light intensity, candelas (m_rgbIntensityCandelas over the light colour)"),
    ("attenuationRadius", 7.0, "attenuation radius, metres (m_invAttenuationRadiusSquared = 1 / r^2)"),
    ("lightOn", 1.0, "the point light on (1) or off (0)"),
    ("lightChannels", 31.0, "the point light's lighting-channel mask (the editor's default: all five channels, 0x1F)"),
    ("sphereLightingChannels", 31.0, "the sphere mesh's lighting-channel mask (0x1F by default)"),
    ("extraLights", 0.0, "more simple point lights over the room, for light culling (0 to 48)"),
    ("extraLightRadius", 1.0, "their attenuation radius, metres"),
    ("extraLightIntensity", 0.4, "their intensity, candelas"),
    ("extraLightMotion", 0.0, "how far the lights have moved, degrees: each height's ring turns about the room's centre, the three at different rates and directions"),
    ("lightMarkers", 0.0, "a marker on each of those lights; the ones the inspected point's bin keeps (with cullTile on) are ringed and joined to the point"),
    ("cullTile", 0.0, "the inspected point's 16 x 16 tile: its view-space box, its depth bins, and the lights its bin keeps"),
    ("decals", 0.0, "two decals on the floor beside the sphere: a paint splat and a panel grid (decals.rs)"),
    ("decalOpacity", 1.0, "the decals' m_opacity (the component's Opacity)"),
    ("decalAngleAttenuation", 1.0, "the decals' m_angleAttenuation (the component's Attenuation Angle)"),
    ("decalNormalOpacity", 1.0, "the decals' m_normalMapOpacity"),
    ("decalSplatSortKey", 17.0, "the splat's sort key; the grid keeps the default 16, and larger keys draw on top"),
    ("decalTurn", 0.0, "the panel grid's turn about the vertical, degrees"),
    ("decalTilt", 0.0, "the splat's tilt about x, degrees: the angle between its projection axis and the floor's normal"),
    ("decalBoxes", 0.0, "the decals' boxes and projection axes"),
    ("sphereLight", 0.0, "an O3DE point light: a sphere area light, without shadows (simple point lights do no back lighting)"),
    ("sphereLightX", -0.85, "the sphere light's position x, metres"),
    ("sphereLightY", 0.3, "the sphere light's position y, metres"),
    ("sphereLightZ", -2.6, "the sphere light's position z, metres"),
    ("sphereLightIntensity", 2.5, "the sphere light's intensity, candelas"),
    ("sphereLightRadius", 3.0, "the sphere light's attenuation radius, metres"),
    ("sphereLightBulb", 0.05, "the sphere light's bulb radius, metres (the editor's default sphere shape, 0.05 m)"),
    ("ssao", 1.0, "screen-space ambient occlusion (on by default in the main pipeline)"),
    ("ssaoStrength", 1.0, "SSAO strength (DefaultSsaoStrength 1)"),
    ("ssaoRadius", 0.05, "SSAO sampling radius, a fraction of the half-res height (DefaultSsaoSamplingRadius 0.05)"),
    ("ssaoBlur", 1.0, "the SSAO's depth-aware blur"),
    ("ssaoBlurConstFalloff", 0.6667, "the blur's constant falloff (2/3 without an AO component; the component's default is 0.85)"),
    ("ssaoBlurDepthThreshold", 0.0, "the blur's depth falloff threshold (0)"),
    ("ssaoBlurDepthStrength", 50.0, "the blur's depth falloff strength (50 without an AO component; the component's default is 200)"),
    ("probes", 0.0, "two reflection probes in the room, their cubemaps baked from the room demonstration's start"),
    ("probeParallax", 1.0, "the probes' parallax correction (the component's default: on)"),
    ("probeInnerScale", 1.0, "the probes' inner extents as a fraction of the placed ones (clamped to the outer extents)"),
    ("probeExposure", 0.0, "the probes' render exposure, stops (default 0)"),
    ("probeBoxes", 0.0, "the probes' outer and inner boxes"),
    ("teapotTransmission", 0.0, "the teapot as EnhancedPBR with transmission: 0 None (the default), 1 ThickObject, 2 ThinObject"),
    ("transmissionThickness", 0.5, "transmission thickness (the property's default 0.5)"),
    ("transmissionPower", 6.0, "thick mode: transmission power (default 6)"),
    ("transmissionDistortion", 0.1, "thick mode: transmission distortion (default 0.1)"),
    ("transmissionAttenuation", 4.0, "thick mode: transmission attenuation coefficient (default 4)"),
    ("transmissionScale", 1.0, "transmission scale (default 1)"),
    ("transmissionNdLBias", 0.1, "thin mode: the NdotL bias (default 0.1)"),
    ("transmissionDistanceAttenuation", 0.5, "thin mode: the distance attenuation (default 0.5)"),
    ("viewMode", 0.0, "what the scene pass shows (brdf::view)"),
    ("multiscatter", 1.0, "o_specularF0_enableMultiScatterCompensation"),
    ("specularAA", 1.0, "specular anti-aliasing (the applySpecularAA material property, true by default)"),
    ("sphereClearCoat", 0.0, "the sphere's clearCoat.enable (false by default)"),
    ("clearCoatFactor", 1.0, "clearCoat.factor (1 by default)"),
    ("clearCoatRoughness", 0.0, "clearCoat.roughness, linear (0 by default)"),
    ("sphereOcclusion", 0.0, "the sphere's diffuse AO and specular cavity maps assigned (1) or not (0, O3DE's default: no texture, occlusion 1)"),
    ("diffuseOcclusionFactor", 1.0, "diffuseOcclusion.factor: the exponent on the diffuse AO map"),
    ("specularOcclusionFactor", 1.0, "specularOcclusion.factor: the exponent on the specular cavity map"),
    ("diffuseModel", 0.0, "0 DiffuseLambertian (StandardPBR), 1 NormalizedDisneyDiffuse (Skin; EnhancedPBR with subsurface scattering)"),
    ("ibl", 1.0, "the full-screen IBL passes (global skylight) on (1) or off (0)"),
    ("iblExposure", 0.0, "the skylight's exposure, stops (m_iblExposure)"),
    ("walls", 1.0, "the room's walls drawn (global IBL ignores them either way)"),
    ("specularF0Factor", 0.5, "StandardPBR specularF0.factor of every object (0.5: F0 = 0.04)"),
    ("sphereRoughness", 0.35, "sphere roughness (linear, as authored)"),
    ("sphereMetallic", 0.0, "sphere metallic"),
    ("teapotRoughness", 0.3, "teapot roughness"),
    ("teapotMetallic", 0.0, "teapot metallic"),
    ("boxRoughness", 0.55, "rotated box roughness"),
    ("boxMetallic", 0.0, "rotated box metallic"),
    ("floorRoughness", 0.45, "floor roughness"),
    ("inspectX", -0.621, "inspected point x, metres (a click sets it)"),
    ("inspectY", 0.900, "inspected point y"),
    ("inspectZ", -0.201, "inspected point z"),
    ("inspectNX", 0.693, "inspected point's normal x"),
    ("inspectNY", 0.452, "inspected point's normal y"),
    ("inspectNZ", 0.562, "inspected point's normal z"),
    ("inspectObject", 5.0, "the object of the inspected point (scene ids)"),
    ("observer", 0.0, "the readout and vectors see the point from the observer (1) or the camera (0)"),
    ("observerX", -0.362, "observer position x, metres"),
    ("observerY", 2.223, "observer position y"),
    ("observerZ", -0.55, "observer position z"),
    ("vectors", 0.0, "N, L, V, and H at the inspected point"),
    ("lobe", 0.0, "the specular lobe at the inspected point, in the plane of N and L"),
    ("falloff", 0.0, "the light's illuminance along the floor, as a curve"),
    ("radius", 0.0, "the attenuation radius around the light"),
    ("labels", 1.0, "object labels"),
    ("exposure", 1.0, "tonemapper exposure"),
    ("vectorsShown", 5.0, "how many of N, L, V, H, and R are drawn, in that order (the video reveals them one by one; a fraction grows the next one)"),
    ("readoutRows", 99.0, "how many readout rows are shown (the video reveals them step by step)"),
    ("formula", 0.0, "the term lit in the demonstration's formula above the readout (0: none; -1 hides the formula)"),
    ("ggxSamples", 0.0, "GGX sample directions drawn around R at the inspected point, as the specular prefilter takes them (N = V = R)"),
    ("cameraDrift", 0.0, "camera sway, as in the lab's other investigations"),
];

pub fn param_default(name: &str) -> Option<f32> {
    PARAMS.iter().find(|p| p.0 == name).map(|p| p.1)
}

fn defaults() -> BTreeMap<String, f32> {
    PARAMS.iter().map(|(name, value, _)| (name.to_string(), *value)).collect()
}

/// The overview camera of the study room.
pub fn room_camera() -> OrbitCamera {
    OrbitCamera { target: [0.0, 0.85, -0.3], yaw: 18.0, pitch: 16.0, distance: 6.4, fov: 50.0 }
}

/// The close-ups look at the top of the sphere from the front, so the plane
/// of N, L, and V faces the camera.
fn sphere_camera() -> OrbitCamera {
    OrbitCamera { target: [-1.05, 1.35, -0.55], yaw: 0.0, pitch: 6.0, distance: 3.1, fov: 50.0 }
}

/// Looking down on the floor's decals from the front.
pub fn decal_camera() -> OrbitCamera {
    OrbitCamera { target: [-0.7, 0.0, 0.55], yaw: 10.0, pitch: 55.0, distance: 2.4, fov: 50.0 }
}

/// The top of the sphere, where the close-ups inspect the lighting.
pub const SPHERE_TOP: [f32; 3] = [-1.05, 1.24, -0.55];

/// The light of the close-ups, `angle` degrees from the normal at the top of
/// the sphere towards -x, 1.5 m away.
pub fn close_up_light(angle: f32) -> [(&'static str, f32); 2] {
    let a = angle.to_radians();
    [("lightX", SPHERE_TOP[0] - 1.5 * a.sin()), ("lightY", SPHERE_TOP[1] + 1.5 * a.cos())]
}

/// The close-ups: the point at the top of the sphere, the light 55 degrees to
/// its left, and the observer (the default observerX, observerY) 1.2 m away
/// 35 degrees to its right, all in the plane z = -0.55.
fn close_up(set: &mut impl FnMut(&str, f32)) {
    for (name, value) in close_up_light(55.0) {
        set(name, value);
    }
    for (name, value) in [
        ("lightZ", SPHERE_TOP[2]),
        ("inspectX", SPHERE_TOP[0]),
        ("inspectY", SPHERE_TOP[1]),
        ("inspectZ", SPHERE_TOP[2]),
        ("inspectNX", 0.0),
        ("inspectNY", 1.0),
        ("inspectNZ", 0.0),
        ("observer", 1.0),
        ("labels", 0.0),
        ("vectors", 1.0),
    ] {
        set(name, value);
    }
}

pub fn for_demo(demo: Demo) -> ShotState {
    let mut params = defaults();
    let mut set = |name: &str, value: f32| {
        params.insert(name.to_string(), value);
    };
    let camera = match demo {
        Demo::Room => room_camera(),
        Demo::Culling => {
            // Two dozen small lights over the room, and how many each tile keeps.
            set("extraLights", 24.0);
            set("viewMode", 12.0);
            set("labels", 0.0);
            set("cullTile", 1.0);
            set("lightMarkers", 1.0);
            room_camera()
        }
        Demo::Surface => {
            close_up(&mut set);
            set("labels", 1.0);
            sphere_camera()
        }
        Demo::Decals => {
            // Looking down on the two decals; the point is where they overlap.
            set("decals", 1.0);
            set("decalBoxes", 1.0);
            set("labels", 0.0);
            for (name, value) in [("lightX", -0.6), ("lightY", 1.2), ("lightZ", 0.9), ("inspectObject", ids::FLOOR as f32), ("inspectX", -0.55), ("inspectY", 0.0), ("inspectZ", 0.55), ("inspectNX", 0.0), ("inspectNY", 1.0), ("inspectNZ", 0.0)] {
                set(name, value);
            }
            decal_camera()
        }
        Demo::Specular => {
            close_up(&mut set);
            set("sphereMetallic", 1.0);
            set("lobe", 1.0);
            sphere_camera()
        }
        Demo::ClearCoat => {
            // A glossy coat over a rough metal: the coat's sharp highlight sits on the base's broad one.
            close_up(&mut set);
            set("sphereMetallic", 1.0);
            set("sphereRoughness", 0.55);
            set("sphereClearCoat", 1.0);
            set("clearCoatRoughness", 0.3);
            sphere_camera()
        }
        Demo::Diffuse => {
            close_up(&mut set);
            set("viewMode", 1.0);
            set("lobe", 1.0);
            sphere_camera()
        }
        Demo::Transmission => {
            // The teapot lit from behind by a sphere light, the simple point light off.
            set("sphereLight", 1.0);
            set("teapotTransmission", 1.0);
            set("lightOn", 0.0);
            set("labels", 0.0);
            // A thin teapot, and the skylight a stop down, so the glow shows.
            set("transmissionThickness", 0.15);
            set("iblExposure", -1.0);
            // The point where the view's centre meets the teapot.
            for (name, value) in [("inspectObject", ids::TEAPOT as f32), ("inspectX", -0.1261), ("inspectY", 0.3227), ("inspectZ", -1.573), ("inspectNX", 0.4914), ("inspectNY", 0.4217), ("inspectNZ", 0.7621)] {
                set(name, value);
            }
            OrbitCamera { target: [-0.25, 0.3, -1.75], yaw: 35.0, pitch: 6.0, distance: 1.8, fov: 50.0 }
        }
        Demo::Reflections => {
            // Both probes and their boxes; a glossy floor shows the probes' reflections.
            set("probes", 1.0);
            set("probeBoxes", 1.0);
            set("labels", 0.0);
            set("floorRoughness", 0.15);
            for (name, value) in [("inspectObject", ids::FLOOR as f32), ("inspectX", 0.15), ("inspectY", 0.0), ("inspectZ", -0.5), ("inspectNX", 0.0), ("inspectNY", 1.0), ("inspectNZ", 0.0)] {
                set(name, value);
            }
            room_camera()
        }
        Demo::Ssao => {
            // The upsampled AO over the room; the point is on the floor where it meets the back wall.
            set("viewMode", 15.0);
            set("labels", 0.0);
            for (name, value) in [("inspectObject", ids::FLOOR as f32), ("inspectX", -0.4), ("inspectY", 0.0), ("inspectZ", -2.92), ("inspectNX", 0.0), ("inspectNY", 1.0), ("inspectNZ", 0.0)] {
                set(name, value);
            }
            room_camera()
        }
        Demo::Multiscatter => {
            close_up(&mut set);
            set("sphereMetallic", 1.0);
            set("sphereRoughness", 0.8);
            sphere_camera()
        }
        Demo::PointLight => {
            set("labels", 0.0);
            set("falloff", 1.0);
            set("radius", 1.0);
            set("attenuationRadius", 3.5);
            set("viewMode", 9.0);
            OrbitCamera { target: [0.3, 0.6, 0.2], yaw: 24.0, pitch: 28.0, distance: 7.2, fov: 50.0 }
        }
        Demo::Ibl => {
            close_up(&mut set);
            set("sphereMetallic", 1.0);
            set("sphereRoughness", 0.2);
            set("lightOn", 0.0);
            set("walls", 0.0);
            OrbitCamera { target: [-0.6, 0.8, -0.6], yaw: 20.0, pitch: 10.0, distance: 3.6, fov: 50.0 }
        }
    };
    ShotState {
        demo: demo.id(),
        camera,
        record: RecordRef::None,
        highlight: Vec::new(),
        panel: PanelMode::Values,
        params,
        callout: None,
        code: None,
    }
}

/// The light of the close-ups swinging in their plane, from `from` degrees
/// off the normal at the top of the sphere to `from + degrees` and back, every
/// `period` seconds.
fn close_up_swing(from: f32, degrees: f32, period: f32) -> Motion {
    let start = close_up_light(from).map(|p| p.1).to_vec();
    Motion::Orbit(Orbit {
        params: vec!["lightX".into(), "lightY".into()],
        center: vec![SPHERE_TOP[0], SPHERE_TOP[1]],
        axis: None,
        start: Some(start),
        degrees,
        seconds: period / 2.0,
        ease: Ease::Sine,
        repeat: Repeat::PingPong,
    })
}

/// The viewer's Play animation of a demonstration: looping motions that sweep
/// what it is about.
pub fn play_clip(demo: Demo) -> Clip {
    let wave = |param: &str, center: f32, amplitude: f32, period: f32| Motion::Wave(Wave::new(param, center, amplitude, period));
    let cosine = |param: &str, center: f32, amplitude: f32, period: f32| Motion::Wave(Wave::new(param, center, amplitude, period).cosine());
    // The light circles the room above the sphere.
    let orbit = |radius: f32, period: f32| {
        let center = [-0.9, -0.4];
        Motion::Orbit(Orbit {
            params: vec!["lightX".into(), "lightZ".into()],
            center: center.to_vec(),
            axis: None,
            start: Some(vec![center[0] + radius * 1.0f32.cos(), center[1] + radius * 1.0f32.sin()]),
            degrees: 360.0,
            seconds: period,
            ease: Ease::Linear,
            repeat: Repeat::Loop,
        })
    };
    Clip(match demo {
        Demo::Room => vec![orbit(1.6, 14.0)],
        // The lights' rings turn back and forth, so lights cross tiles and bins, while their radius breathes.
        Demo::Culling => vec![wave("extraLightMotion", 0.0, 200.0, 28.0), wave("extraLightRadius", 0.8, 0.5, 10.0)],
        Demo::Surface => vec![wave("sphereMetallic", 0.5, 0.5, 8.0), wave("sphereRoughness", 0.45, 0.35, 13.0)],
        // The grid turns under the splat, and the splat tilts away from the floor's normal.
        Demo::Decals => vec![cosine("decalTurn", 45.0, -45.0, 16.0), cosine("decalTilt", 30.0, -30.0, 11.0)],
        // The light sweeps from near the normal to grazing and back, while the roughness slowly widens and narrows the lobe.
        Demo::Specular => vec![close_up_swing(10.0, 70.0, 12.0), wave("sphereRoughness", 0.35, 0.2, 19.0)],
        Demo::ClearCoat => vec![wave("clearCoatFactor", 0.5, 0.5, 8.0), wave("clearCoatRoughness", 0.3, 0.25, 11.0)],
        // Roughness changes the response; the light sweeps so N dot L changes too.
        Demo::Diffuse => vec![close_up_swing(10.0, 60.0, 14.0), wave("sphereRoughness", 0.5, 0.45, 9.0)],
        // The light circles behind the teapot.
        Demo::Transmission => vec![wave("sphereLightX", -0.85, 0.5, 12.0), cosine("sphereLightY", 0.3, 0.25, 12.0)],
        Demo::Multiscatter => vec![wave("sphereRoughness", 0.55, 0.42, 10.0)],
        // The radius grows and shrinks while the light moves across the room, its curve with it.
        Demo::PointLight => vec![wave("attenuationRadius", 3.5, 2.0, 10.0), wave("lightX", 0.55, 0.9, 17.0)],
        // The inner volumes shrink and grow, so the blend region widens and narrows.
        Demo::Reflections => vec![wave("probeInnerScale", 0.55, 0.45, 12.0)],
        // The sampling radius grows and shrinks: wider and softer, then tighter.
        Demo::Ssao => vec![wave("ssaoRadius", 0.05, 0.04, 10.0)],
        // Roughness walks through the specular cubemap's mips.
        Demo::Ibl => vec![cosine("sphereRoughness", 0.5, -0.5, 12.0)],
    })
}

/// The viewer's Play animation: parameter values `t` seconds after it started.
pub fn demo_animation(demo: Demo, t: f32) -> Vec<(&'static str, f32)> {
    let name = |param: &str| PARAMS.iter().find(|p| p.0 == param).map(|p| p.0).unwrap_or_else(|| panic!("{param} is not a shot parameter"));
    play_clip(demo).sample(t, |param| param_default(param).unwrap_or(0.0)).into_iter().map(|(param, value)| (name(&param), value)).collect()
}
