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

/// One column of an alias table: the part of the column its own light keeps (the rest is its alias's).
#[derive(Clone, Copy, Debug)]
pub struct AliasColumn {
    pub own: f32,
    pub alias: usize,
}

/// An alias table over the example's shares, built by Vose's method (Vose, "A linear algorithm for generating random
/// numbers with a given distribution", IEEE TSE 1991): each share scaled by the number of lights; a column under 1
/// is topped up by a light over 1, which keeps what is left. The lab's illustration of the paper's structure: the
/// lab itself picks with its CDF (`cpu::bisect`).
pub fn alias_table(shares: &[f32]) -> Vec<AliasColumn> {
    let n = shares.len();
    let mut scaled: Vec<f32> = shares.iter().map(|s| s * n as f32).collect();
    let mut table = vec![AliasColumn { own: 1.0, alias: 0 }; n];
    let (mut small, mut large): (Vec<usize>, Vec<usize>) = (0..n).partition(|&i| scaled[i] < 1.0);
    while let (Some(&l), Some(&g)) = (small.last(), large.last()) {
        // Column l keeps its scaled share and takes the rest from g, which moves to the list its remainder fits.
        small.pop();
        large.pop();
        table[l] = AliasColumn {
            own: scaled[l],
            alias: g,
        };
        scaled[g] -= 1.0 - scaled[l];
        if scaled[g] < 1.0 {
            small.push(g);
        } else {
            large.push(g);
        }
    }
    for i in small.into_iter().chain(large) {
        table[i] = AliasColumn { own: 1.0, alias: i };
    }
    table
}

/// A piece of the alias table: part of a light's share, in a column from `lo` to `hi` (a column's height is 1), and
/// where that part lies along the shares laid end to end (`start` and `length`, of the whole).
#[derive(Clone, Copy, Debug)]
pub struct AliasPiece {
    pub light: usize,
    pub column: usize,
    pub lo: f32,
    pub hi: f32,
    pub start: f32,
    pub length: f32,
}

/// The table's pieces, each light's in turn: its own column's part, then its parts of the columns it tops up.
pub fn alias_pieces(shares: &[f32], table: &[AliasColumn]) -> Vec<AliasPiece> {
    let n = shares.len() as f32;
    let mut pieces = Vec::new();
    let mut start = 0.0;
    for (light, share) in shares.iter().enumerate() {
        // The light's own part first, then each column whose alias it is, laid along its share.
        let mut at = start;
        let own = table[light].own;
        let tops = table
            .iter()
            .enumerate()
            .filter(|(c, t)| t.alias == light && *c != light && t.own < 1.0)
            .map(|(c, t)| (c, t.own, 1.0));
        for (column, lo, hi) in std::iter::once((light, 0.0, own)).chain(tops) {
            let length = (hi - lo) / n;
            pieces.push(AliasPiece {
                light,
                column,
                lo,
                hi,
                start: at,
                length,
            });
            at += length;
        }
        start += share;
    }
    pieces
}

/// The example's shares (P's lights at the start).
pub fn shares() -> Vec<f32> {
    lights_at(0, 0.0, 1.0).iter().map(|l| l.share).collect()
}

/// The lab's pick of a light for the number `u` over the example's shares (the running sum and `cpu::bisect`):
/// each halving's range and middle (lo, hi, mid), with the running sum at the middle, then the light found.
pub fn search(u: f32) -> (Vec<(usize, usize, usize, f32)>, usize) {
    let cdf: Vec<f32> = shares()
        .iter()
        .scan(0.0, |run, s| {
            *run += s;
            Some(*run)
        })
        .collect();
    let mut steps = Vec::new();
    let found = super::cpu::bisect(&cdf, u, |lo, hi, mid| steps.push((lo, hi, mid, cdf[mid])));
    (steps, found)
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

    #[test]
    fn the_alias_table_keeps_every_share() {
        let shares = shares();
        let pieces = alias_pieces(&shares, &alias_table(&shares));
        for (light, share) in shares.iter().enumerate() {
            let sum: f32 = pieces
                .iter()
                .filter(|p| p.light == light)
                .map(|p| p.length)
                .sum();
            assert!(
                (sum - share).abs() < 1e-4,
                "light {light}: {sum} for {share}"
            );
        }
        let (steps, found) = search(0.5);
        assert!(!steps.is_empty());
        let before: f32 = shares[..found].iter().sum();
        assert!(before <= 0.5 && before + shares[found] > 0.5);
    }
}
