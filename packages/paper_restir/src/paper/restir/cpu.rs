//! The CPU mirror of the WGSL passes (shaders/restir_shading.wgsl and
//! restir_passes.wgsl): the same random numbers, the same light sample, target,
//! reservoir update, candidates (Alg. 3), and combination (Alg. 4 and Alg. 6),
//! so the selftest can recompute a pixel's reservoir from the GPU's inputs and
//! compare.

use super::lights::{FIRST_LIGHT, Light};
use glam::{UVec2, Vec3};
use lab::layout::ObjectData;
use std::f32::consts::PI;

pub const NONE: u32 = u32::MAX;

/// The PCG hash (Jarzynski and Olano 2020), as `restirHash`.
pub fn hash(x: u32) -> u32 {
    let state = x.wrapping_mul(747796405).wrapping_add(2891336453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277803737);
    (word >> 22) ^ word
}

/// The random numbers of one pixel, frame, and pass (`restirRng`).
pub struct Rng(u32);

impl Rng {
    pub fn new(pixel: UVec2, frame: u32, pass: u32) -> Rng {
        Rng(hash(
            pixel.x ^ hash(pixel.y ^ hash(frame.wrapping_mul(16).wrapping_add(pass))),
        ))
    }

    pub fn next(&mut self) -> u32 {
        self.0 = hash(self.0);
        self.0
    }

    pub fn rand(&mut self) -> f32 {
        (self.next() >> 8) as f32 * (1.0 / 16777216.0)
    }
}

/// A light sample: a light and a point on it (`Sample`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub light: u32,
    pub uv: u32,
}

/// A reservoir (`Reservoir`): the sample, the weight sum, M, and W.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reservoir {
    pub y: Sample,
    pub wsum: f32,
    pub m: f32,
    pub w: f32,
}

impl Reservoir {
    pub fn empty() -> Reservoir {
        Reservoir {
            y: Sample { light: NONE, uv: 0 },
            wsum: 0.0,
            m: 0.0,
            w: 0.0,
        }
    }

    /// As the GPU stores it: light, point, W's bits, M's bits.
    pub fn unpack(v: [u32; 4]) -> Reservoir {
        Reservoir {
            y: Sample {
                light: v[0],
                uv: v[1],
            },
            wsum: 0.0,
            m: f32::from_bits(v[3]),
            w: f32::from_bits(v[2]),
        }
    }

    /// Alg. 2, update: add a candidate of weight w, keeping it with probability w / wsum.
    pub fn update(&mut self, x: Sample, w: f32, rng: &mut Rng) {
        self.wsum += w;
        self.m += 1.0;
        let u = rng.rand();
        if w > 0.0 && u * self.wsum < w {
            self.y = x;
        }
    }

    /// W from the weight sum (Eq. 6): over p̂ of the sample, over the count.
    pub fn finish(&mut self, target: f32, count: f32) {
        self.w = if target > 0.0 && count > 0.0 {
            self.wsum / (target * count)
        } else {
            0.0
        };
    }
}

/// A pixel's surface, as the candidate pass stores it (`Surface`).
#[derive(Clone, Copy, Debug)]
pub struct Surface {
    pub position: Vec3,
    pub distance: f32,
    pub normal: Vec3,
    pub roughness: f32,
    pub albedo: Vec3,
    pub object: u32,
    pub valid: bool,
}

impl Surface {
    /// From the three images' texels (`restirSurface`).
    pub fn from_texels(p: [f32; 4], m: [f32; 4], a: [f32; 4]) -> Surface {
        Surface {
            position: Vec3::new(p[0], p[1], p[2]),
            distance: p[3],
            normal: Vec3::new(m[0], m[1], m[2]),
            roughness: a[3],
            albedo: Vec3::new(a[0], a[1], a[2]),
            object: m[3].max(0.0) as u32,
            valid: m[3] >= 0.0,
        }
    }
}

/// What the passes read besides the images: the lights, their CDF, the eye, the objects, and the parameters.
pub struct Scene<'a> {
    pub lights: &'a [Light],
    pub cdf: &'a [f32],
    pub eye: Vec3,
    pub objects: &'a [ObjectData],
    pub candidates: u32,
    pub unbiased: bool,
    pub visibility_reuse: bool,
}

pub fn sample_dir(uv: u32) -> Vec3 {
    let u = ((uv & 0xffff) as f32 + 0.5) / 65536.0;
    let v = ((uv >> 16) as f32 + 0.5) / 65536.0;
    let z = 1.0 - 2.0 * u;
    let r = (1.0 - z * z).max(0.0).sqrt();
    let phi = 2.0 * PI * v;
    Vec3::new(r * phi.cos(), z, r * phi.sin())
}

impl Scene<'_> {
    pub fn point(&self, x: Sample) -> Vec3 {
        let l = &self.lights[x.light as usize];
        l.center + l.radius * sample_dir(x.uv)
    }

    /// The light picked by power (`restirPickLight`).
    pub fn pick(&self, u: f32) -> u32 {
        let (mut lo, mut hi) = (0usize, self.lights.len() - 1);
        while lo < hi {
            // Halve the range around the first CDF entry past u.
            let mid = (lo + hi) / 2;
            if self.cdf[mid] > u {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        lo as u32
    }

    /// p(x) per unit area (`restirSourcePdf`).
    pub fn source_pdf(&self, light: u32) -> f32 {
        let i = light as usize;
        let before = if i > 0 {
            self.cdf[i - 1]
        } else {
            0.0
        };
        let r = self.lights[i].radius;
        (self.cdf[i] - before) / (4.0 * PI * r * r)
    }

    /// ρ · Le · G (`restirContribution`).
    pub fn contribution(&self, s: &Surface, x: Sample) -> Vec3 {
        if !s.valid || x.light == NONE {
            return Vec3::ZERO;
        }
        let l = &self.lights[x.light as usize];
        let dir = sample_dir(x.uv);
        let p = l.center + l.radius * dir;
        let to_light = p - s.position;
        let d2 = to_light.length_squared().max(1e-8);
        let wi = to_light / d2.sqrt();
        let cos_surface = s.normal.dot(wi);
        let cos_light = dir.dot(-wi);
        if cos_surface <= 0.0 || cos_light <= 0.0 {
            return Vec3::ZERO;
        }
        let v = (self.eye - s.position).normalize();
        brdf(s, wi, v) * l.radiance * cos_surface * cos_light / d2
    }

    /// p̂, the contribution's luminance (`restirTarget`).
    pub fn target(&self, s: &Surface, x: Sample) -> f32 {
        super::lights::luminance(self.contribution(s, x))
    }

    /// V (`restirVisible`): the room's objects before the lights, the surface itself skipped.
    pub fn visible(&self, s: &Surface, x: Sample) -> bool {
        let to_light = self.point(x) - s.position;
        let dist = to_light.length();
        let dir = to_light / dist.max(1e-6);
        let origin = s.position + s.normal * 1e-3;
        !self.objects[..FIRST_LIGHT]
            .iter()
            .enumerate()
            .any(|(i, o)| {
                i as u32 != s.object
                    && lab::scene::occludes_light(o)
                    && lab::scene::intersect_object(o, origin, dir, dist * 0.999).is_some()
            })
    }

    /// Alg. 3: the candidates of one pixel (`restirCandidates`).
    pub fn candidates(&self, s: &Surface, rng: &mut Rng) -> Reservoir {
        let mut r = Reservoir::empty();
        for _ in 0..self.candidates {
            // Generate x_i ~ p, weighted p̂ / p.
            let light = self.pick(rng.rand());
            let uv = (rng.next() >> 16) | (rng.next() & 0xffff_0000);
            let x = Sample { light, uv };
            r.update(x, self.target(s, x) / self.source_pdf(light), rng);
        }
        r.finish(self.target(s, r.y), r.m);
        r
    }

    /// Whether p̂ at a source's surface is not zero for a sample (`restirReachable`).
    pub fn reachable(&self, s: &Surface, x: Sample) -> bool {
        self.target(s, x) > 0.0 && (!self.visibility_reuse || self.visible(s, x))
    }

    /// Alg. 4 or Alg. 6 (`restirCombine`): entry 0 is the pixel's own.
    pub fn combine(
        &self,
        s: &Surface,
        sources: &[(Reservoir, Surface)],
        rng: &mut Rng,
    ) -> Reservoir {
        let mut out = Reservoir::empty();
        let mut m = 0.0;
        for (r, _) in sources {
            out.update(r.y, self.target(s, r.y) * r.w * r.m, rng);
            m += r.m;
        }
        out.m = m;
        let z = if self.unbiased {
            // Alg. 6 lines 6-10: the M of the sources whose p̂ at the sample is not zero.
            sources
                .iter()
                .filter(|(_, sn)| self.reachable(sn, out.y))
                .map(|(r, _)| r.m)
                .sum()
        } else {
            m
        };
        out.finish(self.target(s, out.y), z);
        out
    }
}

/// The GGX dielectric over Lambert (`restirBrdf`).
pub fn brdf(s: &Surface, l: Vec3, v: Vec3) -> Vec3 {
    let n = s.normal;
    let no_l = n.dot(l);
    let no_v = n.dot(v).max(1e-4);
    if no_l <= 0.0 {
        return Vec3::ZERO;
    }
    let h = (l + v).normalize();
    let no_h = n.dot(h).max(0.0);
    let vo_h = v.dot(h).max(0.0);
    let a = (s.roughness * s.roughness).max(1e-3);
    let a2 = a * a;
    let d = a2 / (PI * (no_h * no_h * (a2 - 1.0) + 1.0).powi(2));
    let k = a * 0.5;
    let g = (no_l / (no_l * (1.0 - k) + k)) * (no_v / (no_v * (1.0 - k) + k));
    let f = 0.04 + 0.96 * (1.0 - vo_h).powi(5);
    let specular = d * g * f / (4.0 * no_l * no_v);
    s.albedo * (1.0 - f) / PI + Vec3::splat(specular)
}

/// Neighbour j of pixel q (`restirNeighbour`).
pub fn neighbour(q: [i32; 2], j: u32, rotation: [f32; 2], radius: f32, size: [u32; 2]) -> [i32; 2] {
    let e0 = (rotation[0] + (j + 1) as f32 * 0.754_877_7).fract();
    let e1 = (rotation[1] + (j + 1) as f32 * 0.569_840_3).fract();
    let r = radius * e0.sqrt();
    let a = 2.0 * PI * e1;
    let p = [q[0] as f32 + r * a.cos(), q[1] as f32 + r * a.sin()];
    [
        (p[0].round_ties_even() as i32).clamp(0, size[0] as i32 - 1),
        (p[1].round_ties_even() as i32).clamp(0, size[1] as i32 - 1),
    ]
}

/// The geometry test for a neighbour (`restirSimilar`): a surface there, its camera distance within 10% and its
/// normal within 25 degrees of this one's (when the test is on).
pub fn similar(test: bool, s: &Surface, n: &Surface) -> bool {
    n.valid
        && (!test
            || ((n.distance - s.distance).abs() <= 0.1 * s.distance
                && n.normal.dot(s.normal) >= 25f32.to_radians().cos()))
}
