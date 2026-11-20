//! The CPU mirror of the WGSL passes (shaders/restir_shading.wgsl and
//! restir_passes.wgsl): the same random numbers, the same light sample, target,
//! reservoir update, candidates (Alg. 3), and combination (Alg. 4 and Alg. 6),
//! so the selftest can recompute a pixel's reservoir from the GPU's inputs and
//! compare.

use super::alias::{self, AliasColumn};
use super::environment::{self, MARKER as ENVIRONMENT};
use super::lights::Light;
use glam::{UVec2, Vec3};
use lab::layout::ObjectData;
use std::f32::consts::PI;

pub const NONE: u32 = u32::MAX;
/// How far a shadow ray toward the sky goes (`RESTIR_SKY_RAY`): past everything in the room.
pub const SKY_RAY: f32 = 1000.0;
/// How far along its direction a demonstration draws a sample of the sky.
pub const SKY_DRAWN: f32 = 1.2;

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

/// A light sample: a light and a point on it, or the environment map's marker and a direction (`Sample`).
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
        let u = rng.rand();
        self.update_with(x, w, u);
    }

    /// The same update with its uniform number `u` given: whether the candidate replaced the kept one.
    pub fn update_with(&mut self, x: Sample, w: f32, u: f32) -> bool {
        self.wsum += w;
        self.m += 1.0;
        let taken = w > 0.0 && u * self.wsum < w;
        if taken {
            self.y = x;
        }
        taken
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

/// Temporal reuse's count (`restirTemporalCS`): last frame's M clamped to `clamp` times this frame's, then the two
/// added (§5).
pub fn temporal_m(current: f32, previous: f32, clamp: f32) -> f32 {
    current + previous.min(clamp * current)
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

/// What the passes read besides the images: the lights, their shares and alias table, the sky, the eye, the
/// objects, and the parameters.
pub struct Scene<'a> {
    pub lights: &'a [Light],
    pub shares: &'a [f32],
    pub table: &'a [AliasColumn],
    /// The sky's level (`ambientColor`); 0 when the room has no environment map.
    pub sky: f32,
    pub eye: Vec3,
    /// The triangles the shadow rays to the lights trace (`occluders`).
    pub triangles: &'a [Triangle],
    /// The walls and floor, which the shadow rays to the sky trace too (`planes`).
    pub planes: &'a [Triangle],
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
    /// Where a sample is seen from `from`: its point on a light, or a point `SKY_DRAWN` along its direction.
    pub fn point(&self, from: Vec3, x: Sample) -> Vec3 {
        if x.light == ENVIRONMENT {
            return from + SKY_DRAWN * environment::direction(x.uv);
        }
        let l = &self.lights[x.light as usize];
        l.center + l.radius * sample_dir(x.uv)
    }

    /// The share of the candidates drawn from the environment map (`restirEnvironmentShare`).
    pub fn sky_share(&self) -> f32 {
        if self.sky > 0.0 {
            environment::SHARE
        } else {
            0.0
        }
    }

    /// The light picked by power (`restirPickLight`).
    pub fn pick(&self, u: f32) -> u32 {
        alias::draw(self.table, u) as u32
    }

    /// One candidate and its p (`restirDrawCandidate`): from the environment map for a share of them when the room
    /// has one, else a light by power and a uniform point on it.
    pub fn draw(&self, rng: &mut Rng) -> (Sample, f32) {
        let x = if self.sky > 0.0 && rng.rand() < environment::SHARE {
            let u = [rng.rand(), rng.rand(), rng.rand(), rng.rand()];
            Sample {
                light: ENVIRONMENT,
                uv: environment::table().draw(u),
            }
        } else {
            let light = self.pick(rng.rand());
            let uv = (rng.next() >> 16) | (rng.next() & 0xffff_0000);
            Sample { light, uv }
        };
        (x, self.source_pdf(x))
    }

    /// p(x) (`restirSourcePdf`): per unit area on a light, per steradian for the environment map, each times the
    /// share of the candidates drawn that way.
    pub fn source_pdf(&self, x: Sample) -> f32 {
        if x.light == ENVIRONMENT {
            return environment::SHARE * environment::table().pdf(x.uv);
        }
        let i = x.light as usize;
        let r = self.lights[i].radius;
        (1.0 - self.sky_share()) * self.shares[i] / (4.0 * PI * r * r)
    }

    /// ρ · Le · G (`restirContribution`); for the environment map, ρ · Le · cos θ per steradian.
    pub fn contribution(&self, s: &Surface, x: Sample) -> Vec3 {
        if !s.valid || x.light == NONE {
            return Vec3::ZERO;
        }
        if x.light == ENVIRONMENT {
            let wi = environment::direction(x.uv);
            let cos_surface = s.normal.dot(wi);
            if cos_surface <= 0.0 {
                return Vec3::ZERO;
            }
            let v = (self.eye - s.position).normalize();
            let le = self.sky * environment::table().radiance(x.uv);
            return brdf(s, wi, v) * le * cos_surface;
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

    /// V (`restirVisible`): the room's objects before the lights, the surface itself skipped; toward the sky, its
    /// walls and floor too.
    pub fn visible(&self, s: &Surface, x: Sample) -> bool {
        let origin = s.position + s.normal * 1e-3;
        let blocks =
            |t: &Triangle, dir: Vec3, dist: f32| t.object != s.object && t.hit(origin, dir, dist);
        if x.light == ENVIRONMENT {
            let dir = environment::direction(x.uv);
            return !self
                .triangles
                .iter()
                .chain(self.planes)
                .any(|t| blocks(t, dir, SKY_RAY));
        }
        let to_light = self.point(s.position, x) - s.position;
        let dist = to_light.length();
        let dir = to_light / dist.max(1e-6);
        !self.triangles.iter().any(|t| blocks(t, dir, dist * 0.999))
    }

    /// Alg. 3: the candidates of one pixel (`restirCandidates`).
    pub fn candidates(&self, s: &Surface, rng: &mut Rng) -> Reservoir {
        let mut r = Reservoir::empty();
        for _ in 0..self.candidates {
            // Generate x_i ~ p, weighted p̂ / p.
            let (x, p) = self.draw(rng);
            r.update(x, self.target(s, x) / p, rng);
        }
        r.finish(self.target(s, r.y), r.m);
        r
    }

    /// Whether p̂ at a source's surface is not zero for a sample (`restirReachable`).
    pub fn reachable(&self, s: &Surface, x: Sample) -> bool {
        self.target(s, x) > 0.0 && (!self.visibility_reuse || self.visible(s, x))
    }

    /// Alg. 4 or Alg. 6 (restirTemporalCS and restirSpatialCS stream their sources the same way): entry 0 is the pixel's own.
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

/// A triangle of an object that blocks light, in world space: what the GPU's shadow rays trace (the TLAS's
/// instances of `MASK_OCCLUDER`, lab/src/ray_tracing.rs).
#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    pub object: u32,
    pub p: [Vec3; 3],
}

impl Triangle {
    /// Whether the ray from `o` along the unit `d` meets it before `t_max` (Möller and Trumbore 1997).
    pub fn hit(&self, o: Vec3, d: Vec3, t_max: f32) -> bool {
        let (e1, e2) = (self.p[1] - self.p[0], self.p[2] - self.p[0]);
        let h = d.cross(e2);
        let a = e1.dot(h);
        if a.abs() < 1e-12 {
            return false;
        }
        let f = 1.0 / a;
        let s = o - self.p[0];
        let u = f * s.dot(h);
        let q = s.cross(e1);
        let v = f * d.dot(q);
        let t = f * e2.dot(q);
        (0.0..=1.0).contains(&u) && v >= 0.0 && u + v <= 1.0 && t > 0.0 && t < t_max
    }
}

/// Every occluder's raster mesh (`meshes`, each object's range, in its local space) at its record's transform.
pub fn occluders(meshes: &lab::scene::Meshes, objects: &[ObjectData]) -> Vec<Triangle> {
    triangles(meshes, objects, lab::ray_tracing::MASK_OCCLUDER)
}

/// The walls, floor, and panels (the TLAS's `MASK_OTHER`), which only the shadow rays toward the sky trace.
pub fn planes(meshes: &lab::scene::Meshes, objects: &[ObjectData]) -> Vec<Triangle> {
    triangles(meshes, objects, lab::ray_tracing::MASK_OTHER)
}

fn triangles(meshes: &lab::scene::Meshes, objects: &[ObjectData], mask: u8) -> Vec<Triangle> {
    let mut out = Vec::new();
    for (i, (o, range)) in objects.iter().zip(&meshes.ranges).enumerate() {
        // Only the TLAS's instances of this kind.
        if lab::ray_tracing::mask(o) != mask {
            continue;
        }
        let m = glam::Mat4::from_cols_array_2d(&o.local_to_world);
        let corner = |k: u32| {
            m.transform_point3(Vec3::from(
                meshes.vertices[meshes.indices[k as usize] as usize].position,
            ))
        };
        for k in range.clone().step_by(3) {
            out.push(Triangle {
                object: i as u32,
                p: [corner(k), corner(k + 1), corner(k + 2)],
            });
        }
    }
    out
}
