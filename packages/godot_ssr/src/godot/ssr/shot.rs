//! The demonstrations of the SSR investigation: their files (what each shows
//! by default, in demos/, and every value the dock may set, in values.rs;
//! lab/src/demo.rs reads them); the viewer's Play animations are script/animations.lam.

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

