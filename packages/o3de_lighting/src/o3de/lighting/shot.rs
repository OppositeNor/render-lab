//! The demonstrations of the lighting investigation: their files (what each
//! shows by default, in demos/, and every value the dock may set, in
//! values.rs and the shader's tags; lab/src/demo.rs reads them), and the
//! viewer's Play animations.

use lab::animation::{Clip, Ease, Motion, Orbit, Repeat, Wave};
use lab::demo::Demos;
use lab::shot::{DemoId, ShotState};

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
    Cascades,
}

impl Demo {
    pub const ALL: [Demo; 14] = [Demo::Room, Demo::Culling, Demo::Surface, Demo::Decals, Demo::Specular, Demo::ClearCoat, Demo::Diffuse, Demo::Transmission, Demo::Multiscatter, Demo::PointLight, Demo::Cascades, Demo::Ibl, Demo::Reflections, Demo::Ssao];

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
            Demo::Cascades => "cascades",
        }
    }

    pub fn of(id: DemoId) -> Demo {
        Demo::ALL.into_iter().find(|d| d.key() == id.key()).unwrap_or_else(|| panic!("{} is not a demonstration of the O3DE lighting investigation", id.key()))
    }

    pub fn id(self) -> DemoId {
        DemoId::new(self.key())
    }
}

/// The top of the sphere, where the close-ups inspect the lighting.
pub const SPHERE_TOP: [f32; 3] = [-1.05, 1.24, -0.55];

/// The light of the close-ups, `angle` degrees from the normal at the top of
/// the sphere towards -x, 1.5 m away.
pub fn close_up_light(angle: f32) -> [(&'static str, f32); 2] {
    let a = angle.to_radians();
    [("lightX", SPHERE_TOP[0] - 1.5 * a.sin()), ("lightY", SPHERE_TOP[1] + 1.5 * a.cos())]
}

/// The demonstrations' files (demos/; lab/src/demo.rs), with the values
/// they set (values.rs, and the shader's tagged ones).
pub fn demos() -> &'static Demos {
    static DEMOS: std::sync::OnceLock<Demos> = std::sync::OnceLock::new();
    DEMOS.get_or_init(|| Demos::with_values::<super::values::LightingValues>("o3de_lighting").unwrap_or_else(|e| panic!("{e:#}")))
}

/// A demonstration's starting state, from its file.
pub fn for_demo(demo: Demo) -> ShotState {
    demos().shot_state(demo.id())
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
        // The inner volumes shrink and grow, so the blend region widens and narrows; the floor's
        // roughness walks its reflections through the cubemaps' mips, as the cross's mip does.
        Demo::Reflections => vec![wave("probeInnerScale", 0.55, 0.45, 12.0), cosine("floorRoughness", 0.25, -0.1, 9.0), cosine("probeCubemapMip", 2.5, -2.5, 14.0)],
        // The sampling radius grows and shrinks: wider and softer, then tighter.
        Demo::Ssao => vec![wave("ssaoRadius", 0.05, 0.04, 10.0)],
        // Roughness walks through the specular cubemap's mips.
        Demo::Ibl => vec![cosine("sphereRoughness", 0.5, -0.5, 12.0)],
        // The observer looks around, so the slices turn and the snapped boxes hold still; the split ratio swings.
        Demo::Cascades => vec![wave("observerYaw", 0.0, 35.0, 16.0), cosine("cascadeSplitRatio", 0.6, 0.3, 11.0)],
    })
}

/// The viewer's Play animation: parameter values `t` seconds after it started.
pub fn demo_animation(demo: Demo, t: f32) -> Vec<(&'static str, f32)> {
    lab::utility::demo::animation(demos(), &play_clip(demo), t)
}
