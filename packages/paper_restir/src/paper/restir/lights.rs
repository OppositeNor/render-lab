//! The room's many lights: small emissive spheres, the lab's stand-in for the
//! paper's emissive triangles (its scenes have thousands to millions, 148:10
//! §6; this room has at most `MAX_LIGHTS`). Where each is, how large, and its
//! radiance follow from the shot (`place`), so a story can move them as the
//! paper's dynamic lights move; `pack` writes them for the shaders, with the CDF
//! of their power the candidates are drawn from (§5, "Candidate Generation").

use glam::{Vec3, Vec4};
use lab::layout::ObjectData;
use lab::shot::ShotState;

/// The lights the scene holds (scenes/restir-room.toml).
pub const MAX_LIGHTS: usize = 64;
/// The object index of the first light: the study room's nine objects come first.
pub const FIRST_LIGHT: usize = 9;
/// The ceiling panel's object index: dark in this room.
pub const PANEL: usize = 4;
/// The radius the scene's light spheres are made with (scenes/restir-room.toml).
const SCENE_RADIUS: f32 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub center: Vec3,
    pub radius: f32,
    pub radiance: Vec3,
}

impl Light {
    /// Its power, up to the constant every light shares: luminance times surface area (Φ ∝ Le · A).
    pub fn power(&self) -> f32 {
        luminance(self.radiance) * self.radius * self.radius
    }
}

pub fn luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// A colour from a hue in turns, saturated `s`, bright 1.
fn hue(h: f32, s: f32) -> Vec3 {
    let k = |n: f32| {
        let x = (n + h * 6.0).rem_euclid(6.0);
        1.0 - s * (x.min(4.0 - x).clamp(0.0, 1.0))
    };
    Vec3::new(k(5.0), k(3.0), k(1.0))
}

/// Light `i` of the room at a phase of its motion (0 to 1, one loop): three groups, a ring under the ceiling, a
/// ring around the objects at mid height, and lights circling low beside them.
fn light(i: usize, phase: f32, intensity: f32) -> Light {
    let turn = std::f32::consts::TAU;
    let (center, radius): (Vec3, f32) = match i {
        0..24 => {
            // The ceiling ring turns slowly one way.
            let a = turn * (i as f32 / 24.0 + phase);
            (Vec3::new(2.3 * a.cos(), 2.6, 2.3 * a.sin() - 0.3), 0.05)
        }
        24..48 => {
            // The middle ring turns the other way and bobs.
            let k = (i - 24) as f32;
            let a = turn * (k / 24.0 - phase);
            let y = 1.25 + 0.25 * (turn * (2.0 * phase + k / 6.0)).sin();
            (Vec3::new(1.7 * a.cos(), y, 1.7 * a.sin() - 0.4), 0.04)
        }
        _ => {
            // The low lights each circle a point of their own.
            let k = (i - 48) as f32;
            let base = Vec3::new(
                -2.2 + 0.3 * k,
                0.35 + 0.05 * (k % 3.0),
                -2.2 + 0.28 * ((k * 7.0) % 16.0),
            );
            let a = turn * (phase * 2.0 + k / 16.0);
            (base + Vec3::new(0.25 * a.cos(), 0.0, 0.25 * a.sin()), 0.035)
        }
    };
    // Colours around the hue circle, every fourth light white; brighter where the light is smaller.
    let color = if i.is_multiple_of(4) {
        Vec3::splat(1.0)
    } else {
        hue((i as f32 * 0.618_034).fract(), 0.65)
    };
    let strength = 18.0 * intensity * (0.05 / radius).powi(2);
    Light {
        center,
        radius,
        radiance: color * strength,
    }
}

/// The lights the shot shows: the first `lightCount`, at its `lightPhase`.
pub fn place(state: &ShotState) -> Vec<Light> {
    let count = (state.p("lightCount").round() as usize).clamp(1, MAX_LIGHTS);
    let phase = state.p("lightPhase");
    let intensity = state.p("lightIntensity");
    (0..count).map(|i| light(i, phase, intensity)).collect()
}

/// The shaders' light list (restir_shading.wgsl, gShadingFrame): a header (count, total power, the first light's
/// object index), then each light's centre and radius, and its radiance and the CDF of the power up to it.
pub fn pack(lights: &[Light]) -> Vec<[f32; 4]> {
    let total: f32 = lights.iter().map(Light::power).sum();
    let mut out = vec![[lights.len() as f32, total, FIRST_LIGHT as f32, 0.0]];
    let mut running = 0.0;
    for (i, l) in lights.iter().enumerate() {
        // The last light's CDF is exactly 1, so a draw below 1 always finds one.
        running += l.power();
        let cdf = if i + 1 == lights.len() {
            1.0
        } else {
            running / total
        };
        out.push(l.center.extend(l.radius).to_array());
        out.push(l.radiance.extend(cdf).to_array());
    }
    out
}

/// The CDF entries `pack` writes, for the CPU mirror.
pub fn cdf(lights: &[Light]) -> Vec<f32> {
    pack(lights)
        .iter()
        .skip(2)
        .step_by(2)
        .map(|v| v[3])
        .collect()
}

/// The scene's light spheres moved, sized, and lit as `lights` says; those past the count put out and away.
pub fn apply(objects: &mut [ObjectData], lights: &[Light]) {
    objects[PANEL].emissive = [0.0; 4];
    for i in 0..MAX_LIGHTS {
        let o = &mut objects[FIRST_LIGHT + i];
        let (center, radius, radiance) = match lights.get(i) {
            Some(l) => (l.center, l.radius, l.radiance),
            None => (Vec3::new(0.0, -50.0, 0.0), 1e-4, Vec3::ZERO),
        };
        // The record's transform is the sphere scaled from the scene's radius and moved to the centre.
        let m = glam::Mat4::from_scale_rotation_translation(
            Vec3::splat(radius / SCENE_RADIUS),
            glam::Quat::IDENTITY,
            center,
        );
        o.local_to_world = m.to_cols_array_2d();
        o.world_to_local = m.inverse().to_cols_array_2d();
        o.world_center = center.to_array();
        o.world_extent = [radius; 3];
        o.bounding_radius = radius;
        let peak = radiance.max_element().max(1e-6);
        o.emissive = Vec4::from((radiance / peak, peak)).to_array();
    }
}
