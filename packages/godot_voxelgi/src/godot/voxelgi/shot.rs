//! The demonstrations of the VoxelGI investigation: their files (what each
//! shows by default, in demos/, and every value the dock may set, in
//! values.rs; lab/src/demo.rs reads them); the viewer's Play animations are script/animations.lam.

use lab::demo::Demos;
use lab::shot::{DemoId, ShotState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Demo {
    Room,
    Bounds,
    Octree,
    Cells,
    Field,
    Light,
    Bounce,
    Mips,
    Dynamic,
    Pixel,
    Cost,
    Compose,
}

impl Demo {
    pub const ALL: [Demo; 12] = [Demo::Room, Demo::Bounds, Demo::Octree, Demo::Cells, Demo::Field, Demo::Light, Demo::Bounce, Demo::Mips, Demo::Dynamic, Demo::Pixel, Demo::Cost, Demo::Compose];

    pub fn key(self) -> &'static str {
        match self {
            Demo::Room => "room",
            Demo::Bounds => "bounds",
            Demo::Octree => "octree",
            Demo::Cells => "cells",
            Demo::Field => "field",
            Demo::Light => "light",
            Demo::Bounce => "bounce",
            Demo::Mips => "mips",
            Demo::Dynamic => "dynamic",
            Demo::Pixel => "pixel",
            Demo::Cost => "cost",
            Demo::Compose => "compose",
        }
    }

    pub fn of(id: DemoId) -> Demo {
        Demo::ALL.into_iter().find(|d| d.key() == id.key()).unwrap_or_else(|| panic!("{} is not a demonstration of the Godot VoxelGI investigation", id.key()))
    }

    pub fn id(self) -> DemoId {
        DemoId::new(self.key())
    }
}

/// The demonstrations' files (demos/; lab/src/demo.rs), with the values they
/// set (values.rs).
pub fn demos() -> &'static Demos {
    static DEMOS: std::sync::OnceLock<Demos> = std::sync::OnceLock::new();
    DEMOS.get_or_init(|| Demos::with_values::<super::values::VoxelgiValues>("godot_voxelgi").unwrap_or_else(|e| panic!("{e:#}")))
}

/// A demonstration's starting state, from its file.
pub fn for_demo(demo: Demo) -> ShotState {
    demos().shot_state(demo.id())
}

