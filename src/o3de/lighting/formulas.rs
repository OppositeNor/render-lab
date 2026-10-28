//! The formulas above the readout: one formula tag per demonstration in this
//! investigation's text tables (`formula.<demo>`), typeset by
//! src/lab/formula.rs. Its terms are marked `\lit{n}{…}`, and the tag's
//! `colors` give each term's readout colour; the `formula` parameter picks the
//! lit term.

use super::shot::Demo;
use super::text::tr;
use crate::lab::rich_text;

/// The demonstration's formula tag.
pub fn text(demo: Demo) -> &'static str {
    match demo {
        Demo::Room => tr("formula.room"),
        Demo::Culling => tr("formula.culling"),
        Demo::Surface => tr("formula.surface"),
        Demo::Decals => tr("formula.decals"),
        Demo::Specular => tr("formula.specular"),
        Demo::ClearCoat => tr("formula.clearcoat"),
        Demo::Diffuse => tr("formula.diffuse"),
        Demo::Transmission => tr("formula.transmission"),
        Demo::Multiscatter => tr("formula.multiscatter"),
        Demo::PointLight => tr("formula.pointlight"),
        Demo::Ibl => tr("formula.ibl"),
        Demo::Reflections => tr("formula.reflections"),
        Demo::Ssao => tr("formula.ssao"),
    }
}

/// How many terms the demonstration's formula can light: one colour each.
pub fn terms(demo: Demo) -> usize {
    rich_text::first_formula(text(demo)).map_or(0, |(_, _, colors)| colors.len())
}

/// The readout's formula and its lit term for the `formula` parameter: none
/// when it is negative, no term lit at 0 or past the last term.
pub fn readout(demo: Demo, term: f32) -> (Option<String>, Option<(usize, f32)>) {
    let term = term.round();
    if term < 0.0 {
        return (None, None);
    }
    let lit = (term >= 1.0 && term as usize <= terms(demo)).then_some((term as usize, 1.0));
    (Some(text(demo).to_string()), lit)
}
