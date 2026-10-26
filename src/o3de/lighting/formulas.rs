//! The formulas above the readout: PNGs that tools/video/formulas.py renders
//! from formulas/formulas.json with KaTeX (build.rs checks they are current).
//! Each demonstration has one, with no term lit (`<demo>`) and with each of
//! its terms lit in its readout colour (`<demo>-<n>`); the `formula`
//! parameter picks the lit term.

use super::shot::Demo;

macro_rules! images {
    ($($key:literal),* $(,)?) => {
        &[$(($key, include_bytes!(concat!("formulas/", $key, ".png")) as &[u8])),*]
    };
}

/// Every image, by key.
pub const IMAGES: &[(&str, &[u8])] = images![
    "room", "room-1", "room-2", "room-3",
    "culling", "culling-1", "culling-2", "culling-3",
    "surface", "surface-1", "surface-2", "surface-3",
    "decals", "decals-1", "decals-2", "decals-3", "decals-4",
    "specular", "specular-1", "specular-2", "specular-3", "specular-4",
    "clearcoat", "clearcoat-1", "clearcoat-2", "clearcoat-3", "clearcoat-4",
    "diffuse", "diffuse-1", "diffuse-2", "diffuse-3",
    "transmission", "transmission-1", "transmission-2", "transmission-3",
    "multiscatter", "multiscatter-1", "multiscatter-2",
    "pointlight", "pointlight-1", "pointlight-2", "pointlight-3",
    "ibl", "ibl-1", "ibl-2", "ibl-3", "ibl-4",
    "reflections", "reflections-1", "reflections-2", "reflections-3",
    "ssao", "ssao-1", "ssao-2", "ssao-3",
];

/// The PNG of a formula.
pub fn bytes(key: &str) -> Option<&'static [u8]> {
    IMAGES.iter().find(|(k, _)| *k == key).map(|(_, b)| *b)
}

/// How many terms the demonstration's formula can light.
pub fn terms(demo: Demo) -> usize {
    IMAGES.iter().filter(|(k, _)| k.strip_prefix(demo.key()).is_some_and(|rest| rest.starts_with('-'))).count()
}

/// The demonstration's formula with term `term` lit (0: none); none when
/// `term` is negative, and the plain formula past its last term.
pub fn key(demo: Demo, term: f32) -> Option<&'static str> {
    let term = term.round();
    if term < 0.0 {
        return None;
    }
    let name = if term >= 1.0 && term as usize <= terms(demo) { format!("{}-{}", demo.key(), term as usize) } else { demo.key().to_string() };
    IMAGES.iter().find(|(k, _)| *k == name).map(|(k, _)| *k)
}
