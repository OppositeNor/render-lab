//! The viewer's Play animation of a demonstration (`Investigation::demo_animation`)
//! from the clip that sweeps it.

use crate::animation::Clip;
use crate::demo::Demos;

/// The parameter values `clip` sets `t` seconds after Play started; a
/// parameter it does not move starts from its default in `demos`. Panics on a
/// parameter `demos` does not declare.
pub fn animation(demos: &'static Demos, clip: &Clip, t: f32) -> Vec<(&'static str, f32)> {
    let name = |param: &str| demos.params.iter().find(|p| p.name == param).map(|p| p.name).unwrap_or_else(|| panic!("{param} is not a shot parameter"));
    clip.sample(t, |param| demos.param_default(param).unwrap_or(0.0)).into_iter().map(|(param, value)| (name(&param), value)).collect()
}
