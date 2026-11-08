//! The viewer's Play animation of a demonstration (`Investigation::demo_animation`)
//! from the clip that sweeps it.

use crate::animation::Clip;
use crate::demo::Demos;
use crate::values::Reflect;

/// The parameter values `clip` sets `t` seconds after Play started; a
/// parameter it does not move starts from its default in `demos`. A clip may
/// also move the camera (`camera.yaw`, src/camera.rs). Panics on a parameter
/// that is neither declared in `demos` nor the camera's.
pub fn animation(demos: &'static Demos, clip: &Clip, t: f32) -> Vec<(&'static str, f32)> {
    let camera = || {
        crate::camera::OrbitCamera::schema()
            .iter()
            .flat_map(|e| std::iter::once(e.key).chain(e.components.iter().copied()))
    };
    let name = |param: &str| {
        demos
            .params
            .iter()
            .find(|p| p.name == param)
            .map(|p| p.name)
            .or_else(|| camera().find(|k| *k == param))
            .unwrap_or_else(|| panic!("{param} is not a shot parameter"))
    };
    clip.sample(t, |param| demos.param_default(param).unwrap_or(0.0))
        .into_iter()
        .map(|(param, value)| (name(&param), value))
        .collect()
}
