//! The demonstrations of the SDFGI investigation: their files (what each shows
//! by default, in demos/, and every value the dock may set, in values.rs;
//! lab/src/demo.rs reads them), and the viewer's Play animations.

use lab::animation::{Clip, Motion, Wave};
use lab::demo::Demos;
use lab::shot::{DemoId, ShotState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Demo {
    Room,
    Cascades,
    Regions,
    Voxels,
    Field,
    Occlusion,
    Light,
    Probes,
    Maps,
    Pixel,
}

impl Demo {
    pub const ALL: [Demo; 10] = [Demo::Room, Demo::Cascades, Demo::Regions, Demo::Voxels, Demo::Field, Demo::Occlusion, Demo::Light, Demo::Probes, Demo::Maps, Demo::Pixel];

    pub fn key(self) -> &'static str {
        match self {
            Demo::Room => "room",
            Demo::Cascades => "cascades",
            Demo::Regions => "regions",
            Demo::Voxels => "voxels",
            Demo::Field => "field",
            Demo::Occlusion => "occlusion",
            Demo::Light => "light",
            Demo::Probes => "probes",
            Demo::Maps => "maps",
            Demo::Pixel => "pixel",
        }
    }

    pub fn of(id: DemoId) -> Demo {
        Demo::ALL.into_iter().find(|d| d.key() == id.key()).unwrap_or_else(|| panic!("{} is not a demonstration of the Godot SDFGI investigation", id.key()))
    }

    pub fn id(self) -> DemoId {
        DemoId::new(self.key())
    }
}

/// The demonstrations' files (demos/; lab/src/demo.rs), with the values they
/// set (values.rs).
pub fn demos() -> &'static Demos {
    static DEMOS: std::sync::OnceLock<Demos> = std::sync::OnceLock::new();
    DEMOS.get_or_init(|| Demos::with_values::<super::values::SdfgiValues>("godot_sdfgi").unwrap_or_else(|e| panic!("{e:#}")))
}

/// A demonstration's starting state, from its file.
pub fn for_demo(demo: Demo) -> ShotState {
    demos().shot_state(demo.id())
}

/// The viewer's Play animation of a demonstration: looping motions that sweep
/// what it is about.
pub fn play_clip(demo: Demo) -> Clip {
    let wave = |param: &str, center: f32, amplitude: f32, period: f32| Motion::Wave(Wave::new(param, center, amplitude, period));
    Clip(match demo {
        Demo::Room => vec![wave("sdfgiMix", 0.5, 0.5, 10.0)],
        // The camera walks along x, so cascade 0 jumps one probe each 1.6 m.
        Demo::Cascades | Demo::Regions => vec![wave("cameraX", 0.0, 4.0, 20.0)],
        Demo::Voxels => vec![wave("cutHeight", 2.0, 2.0, 12.0)],
        Demo::Field => vec![wave("sliceHeight", 1.2, 1.0, 14.0)],
        Demo::Occlusion => vec![wave("occlusionShell", 11.0, 11.0, 12.0)],
        Demo::Light => vec![wave("lightX", 0.0, 2.6, 14.0)],
        Demo::Probes => vec![wave("rayFrames", 15.0, 15.0, 16.0)],
        Demo::Maps => vec![wave("frames", 30.0, 30.0, 16.0)],
        Demo::Pixel => vec![wave("pixelZ", 0.0, 2.6, 16.0)],
    })
}

/// The viewer's Play animation: parameter values `t` seconds after it started.
pub fn demo_animation(demo: Demo, t: f32) -> Vec<(&'static str, f32)> {
    let params = &demos().params;
    let name = |param: &str| params.iter().find(|p| p.name == param).map(|p| p.name).unwrap_or_else(|| panic!("{param} is not a shot parameter"));
    play_clip(demo).sample(t, |param| demos().param_default(param).unwrap_or(0.0)).into_iter().map(|(param, value)| (name(&param), value)).collect()
}
