//! The demonstrations of the ReSTIR investigation: their files (what each shows
//! by default, in demos/, and every value the dock may set, in values.rs and
//! the shaders' tags; lab/src/demo.rs reads them).

use lab::demo::Demos;
use lab::shot::{DemoId, ShotState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Demo {
    Room,
    Ris,
    Stage0,
    Accum,
    Spatial,
    Bias,
}

impl Demo {
    pub const ALL: [Demo; 6] = [
        Demo::Room,
        Demo::Ris,
        Demo::Stage0,
        Demo::Accum,
        Demo::Spatial,
        Demo::Bias,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Demo::Room => "room",
            Demo::Ris => "ris",
            Demo::Stage0 => "stage0",
            Demo::Accum => "accum",
            Demo::Bias => "bias",
            Demo::Spatial => "spatial",
        }
    }

    pub fn of(id: DemoId) -> Demo {
        Demo::ALL
            .into_iter()
            .find(|d| d.key() == id.key())
            .unwrap_or_else(|| {
                panic!(
                    "{} is not a demonstration of the ReSTIR investigation",
                    id.key()
                )
            })
    }

    pub fn id(self) -> DemoId {
        DemoId::new(self.key())
    }
}

/// The demonstrations' files (demos/; lab/src/demo.rs), with the values they set (values.rs).
pub fn demos() -> &'static Demos {
    static DEMOS: std::sync::OnceLock<Demos> = std::sync::OnceLock::new();
    DEMOS.get_or_init(|| {
        Demos::with_values::<super::values::RestirValues>("paper_restir")
            .unwrap_or_else(|e| panic!("{e:#}"))
    })
}

/// A demonstration's starting state, from its file.
pub fn for_demo(demo: Demo) -> ShotState {
    demos().shot_state(demo.id())
}
