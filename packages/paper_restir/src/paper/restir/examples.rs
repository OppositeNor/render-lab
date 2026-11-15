//! The math scenes' example world (script/*.lam): a small, seeded stand-in for the room's lights, so a
//! demonstration on the canvas shows the method's arithmetic on numbers that read clearly (no light behind a
//! pixel, none covering it). It is the lab's illustration, not the paper's scene or the room's.
//!
//! A cross-section of a room: 24 lights above a floor pixel P (facing up) at distances and angles drawn from a
//! seed, each with a power; a second pixel Q beside P, facing up and to the side. What a light brings to a pixel
//! is its power, times the scene's intensity, times the cosine at the pixel, over the squared distance (the shape of
//! the room's target, without its spheres and BRDF). The lights circle slowly as
//! `phase` grows, as the room's do (`lightPhase`). The cheap choice picks a light by its share of the power.
//! Streams of candidates are drawn from it and run through the lab's reservoir (cpu.rs `Reservoir::update_with`,
//! demos/ris.rs `trace`).

use super::cpu::Sample;
use super::demos::ris::{Candidate, Trace, trace};
use glam::Vec2;

/// The example's lights.
pub const LIGHTS: usize = 24;

/// What a light brings to a pixel, and its share of the lights' power (the chance the cheap choice picks it).
#[derive(Clone, Copy, Debug, Default)]
pub struct LightAt {
    pub brings: f32,
    pub share: f32,
}

/// A seeded stream of numbers in [0, 1) (xorshift64*).
struct Numbers(u64);

impl Numbers {
    fn new(seed: u64) -> Numbers {
        Numbers(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }

    fn next(&mut self) -> f32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// One light of the example: its angle above the floor (radians), its distance from P, its power, and how fast it
/// circles (radians per unit of `lightPhase`: the story sweeps the phase by a few hundredths, so a light swings by a
/// few tenths of a radian).
struct Light {
    angle: f32,
    distance: f32,
    power: f32,
    speed: f32,
}

fn lights() -> Vec<Light> {
    let mut n = Numbers::new(2020);
    (0..LIGHTS)
        .map(|_| Light {
            // Above the floor, kept clear of the horizon, so every light reaches P.
            angle: 0.2 + 2.74 * n.next(),
            distance: 1.0 + 2.0 * n.next(),
            power: 0.25 + n.next(),
            speed: (n.next() - 0.5) * 40.0,
        })
        .collect()
}

/// What each light brings to a pixel and its share of the power, at the lights' `phase` and `intensity`: the pixel
/// `which` is P (0: at the origin, facing up) or Q (1: 0.6 to P's right, facing up and 40° to the right).
pub fn lights_at(which: i64, phase: f32, intensity: f32) -> Vec<LightAt> {
    let all = lights();
    let total: f32 = all.iter().map(|l| l.power).sum();
    let (at, normal) = if which == 0 {
        (Vec2::ZERO, Vec2::Y)
    } else {
        (
            Vec2::new(0.6, 0.0),
            Vec2::new(40f32.to_radians().sin(), 40f32.to_radians().cos()),
        )
    };
    all.iter()
        .map(|l| {
            // The light's place on its circle about P, then the cosine and squared distance from the pixel.
            let angle = (l.angle + l.speed * phase).clamp(0.12, std::f32::consts::PI - 0.12);
            let place = Vec2::new(angle.cos(), angle.sin()) * l.distance;
            let to = place - at;
            let d2 = to.length_squared().max(0.5);
            let cos = normal.dot(to / d2.sqrt()).max(0.0);
            LightAt {
                brings: l.power * intensity * cos / d2,
                share: l.power / total,
            }
        })
        .collect()
}

/// P's stream in the example: `count` candidates drawn by the cheap choice (by power) with the numbers of `seed`,
/// each weighed by what it brings over its share (light-level importance sampling's weight), streamed through the
/// lab's reservoir in two halves of eight and merged (`trace`).
pub fn stream(seed: u64, count: usize) -> Trace {
    let at = lights_at(0, 0.0, 1.0);
    let mut n = Numbers::new(seed);
    let candidates: Vec<Candidate> = (0..count)
        .map(|_| {
            // The light whose running share first passes the number, then its weight and the update's number.
            let u = n.next();
            let mut run = 0.0;
            let light = at
                .iter()
                .position(|l| {
                    run += l.share;
                    u < run
                })
                .unwrap_or(LIGHTS - 1);
            let l = at[light];
            Candidate {
                x: Sample {
                    light: light as u32,
                    uv: 0,
                },
                point: glam::Vec3::ZERO,
                p: l.share,
                p_hat: l.brings,
                w: l.brings / l.share,
                taken: false,
                u: n.next(),
            }
        })
        .collect();
    trace(&candidates, n.next())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_light_reaches_p_and_the_shares_sum_to_one() {
        let p = lights_at(0, 0.0, 1.0);
        assert_eq!(p.len(), LIGHTS);
        assert!(p.iter().all(|l| l.brings > 0.0));
        assert!((p.iter().map(|l| l.share).sum::<f32>() - 1.0).abs() < 1e-5);
        let q = lights_at(1, 0.0, 1.0);
        assert!(
            q.iter()
                .zip(&p)
                .any(|(a, b)| (a.brings - b.brings).abs() > 1e-3)
        );
        let t = stream(7, 16);
        assert_eq!(t.steps.len(), 16);
        assert!(t.steps.iter().all(|s| s.w > 0.0));
        assert_eq!(t.steps[0].kept, Some(0));
    }
}
