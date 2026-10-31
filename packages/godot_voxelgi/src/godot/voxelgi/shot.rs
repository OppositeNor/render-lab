//! The demonstrations of the VoxelGI investigation: their files (what each
//! shows by default, in demos/, and every value the dock may set, in
//! values.rs; lab/src/demo.rs reads them), and the viewer's Play animations.

use lab::animation::{Clip, Motion, Wave};
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
    Compose,
}

impl Demo {
    pub const ALL: [Demo; 11] = [Demo::Room, Demo::Bounds, Demo::Octree, Demo::Cells, Demo::Field, Demo::Light, Demo::Bounce, Demo::Mips, Demo::Dynamic, Demo::Pixel, Demo::Compose];

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

/// The viewer's Play animation of a demonstration: looping motions that sweep
/// what it is about.
pub fn play_clip(demo: Demo) -> Clip {
    let wave = |param: &str, center: f32, amplitude: f32, period: f32| Motion::Wave(Wave::new(param, center, amplitude, period));
    Clip(match demo {
        Demo::Room | Demo::Compose => vec![wave("voxelgiMix", 0.5, 0.5, 10.0)],
        Demo::Bounds => vec![wave("boundsStep", 1.0, 1.0, 10.0)],
        Demo::Octree => vec![wave("descentLevel", 3.5, 3.5, 14.0)],
        Demo::Cells => vec![wave("cutHeight", 1.6, 1.5, 12.0)],
        Demo::Field => vec![wave("sliceHeight", 1.2, 1.0, 14.0)],
        Demo::Light => vec![wave("lightX", 0.0, 2.2, 14.0)],
        Demo::Bounce => vec![wave("coneSteps", 8.0, 8.0, 12.0)],
        Demo::Mips => vec![wave("propagation", 0.5, 0.5, 12.0)],
        Demo::Dynamic => vec![wave("dynamicX", 0.5, 1.0, 14.0)],
        Demo::Pixel => vec![wave("pixelAzimuth", 0.0, 60.0, 16.0)],
    })
}

/// The viewer's Play animation: parameter values `t` seconds after it started.
pub fn demo_animation(demo: Demo, t: f32) -> Vec<(&'static str, f32)> {
    lab::utility::demo::animation(demos(), &play_clip(demo), t)
}
