//! The demonstrations of the SSR investigation: their files (what each shows
//! by default, in demos/, and every value the dock may set, in values.rs;
//! lab/src/demo.rs reads them), and the viewer's Play animations.

use lab::animation::{Clip, Motion, Wave};
use lab::demo::Demos;
use lab::shot::{DemoId, ShotState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Demo {
    Room,
    Inputs,
    Pyramid,
    Trace,
    Hit,
    Blur,
    Compose,
}

impl Demo {
    pub const ALL: [Demo; 7] = [Demo::Room, Demo::Inputs, Demo::Pyramid, Demo::Trace, Demo::Hit, Demo::Blur, Demo::Compose];

    pub fn key(self) -> &'static str {
        match self {
            Demo::Room => "room",
            Demo::Inputs => "inputs",
            Demo::Pyramid => "pyramid",
            Demo::Trace => "trace",
            Demo::Hit => "hit",
            Demo::Blur => "blur",
            Demo::Compose => "compose",
        }
    }

    pub fn of(id: DemoId) -> Demo {
        Demo::ALL.into_iter().find(|d| d.key() == id.key()).unwrap_or_else(|| panic!("{} is not a demonstration of the Godot SSR investigation", id.key()))
    }

    pub fn id(self) -> DemoId {
        DemoId::new(self.key())
    }
}

/// The demonstrations' files (demos/; lab/src/demo.rs), with the values they
/// set (values.rs).
pub fn demos() -> &'static Demos {
    static DEMOS: std::sync::OnceLock<Demos> = std::sync::OnceLock::new();
    DEMOS.get_or_init(|| Demos::with_values::<super::values::SsrValues>("godot_ssr").unwrap_or_else(|e| panic!("{e:#}")))
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
        // The floor from a mirror to the roughness where SSR stops.
        Demo::Room => vec![wave("floorRoughness", 0.35, 0.35, 14.0)],
        // The ray's iterations, one after another.
        Demo::Trace => vec![wave("traceStep", 16.0, 16.0, 16.0)],
        Demo::Inputs => vec![wave("pickX", 255.0, 120.0, 16.0)],
        Demo::Pyramid => vec![wave("hizLevel", 4.5, 4.5, 16.0)],
        Demo::Hit => vec![wave("ssrFadeOut", 2.5, 1.5, 14.0)],
        Demo::Blur => vec![wave("floorRoughness", 0.35, 0.33, 14.0)],
        Demo::Compose => vec![wave("ssrComposeStep", 1.5, 1.5, 12.0)],
    })
}

/// The viewer's Play animation: parameter values `t` seconds after it started.
pub fn demo_animation(demo: Demo, t: f32) -> Vec<(&'static str, f32)> {
    lab::utility::demo::animation(demos(), &play_clip(demo), t)
}
