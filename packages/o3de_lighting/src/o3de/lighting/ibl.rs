//! Image-based lighting as O3DE builds and applies it for opaque StandardPBR
//! surfaces. Offline, O3DE's image processor turns an environment cubemap
//! into a specular cubemap (GGX-prefiltered into six mips, one per roughness
//! step of 0.2) and a diffuse one (cosine-filtered), with CubeMapGen's
//! `FilterCubeSurfacesGGX` and cosine filter; at run time two full-screen
//! passes, `DiffuseGlobalFullscreen` and `ReflectionGlobalFullscreen`, read
//! them with what the forward pass wrote to its 8-bit targets. This module
//! builds both cubemaps on the CPU from the lab's own environment and
//! mirrors the shader's lookups (shaders/standard_lighting.wgsl).
//!
//! The prefiltering is ported from CubeMapGen (Copyright (c) 2011, Advanced
//! Micro Devices, Inc., modified BSD license) as modified in the Open 3D
//! Engine (Copyright Contributors to the Open 3D Engine Project, MIT); see
//! THIRD_PARTY.md.

use glam::{DVec3, Vec2, Vec3};
use std::sync::OnceLock;

/// IBLSpecular.preset: 256 x 256 faces; the GGX filter keeps six mips.
pub const SPECULAR_SIZE: usize = 256;
pub const SPECULAR_MIPS: usize = 6;
/// Cubemap.cpp's SampleCountGGX.
pub const GGX_SAMPLES: u32 = 256;
/// IBLDiffuse.preset allows 32 to 128; the lab's brute-force cosine filter
/// runs at the smallest size (irradiance has no detail to lose).
pub const DIFFUSE_SIZE: usize = 32;

const PI: f32 = 3.14159265;

/// CubeMapGen's face table (sgFace2DMapping): the u direction, the v
/// direction, and the face axis of +X, -X, +Y, -Y, +Z, -Z.
const FACES: [[Vec3; 3]; 6] = [
    [Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(1.0, 0.0, 0.0)],
    [Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(-1.0, 0.0, 0.0)],
    [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 1.0, 0.0)],
    [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, -1.0, 0.0)],
    [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)],
    [Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, -1.0)],
];

/// The skylight's level: the mean radiance of the lab's environment
/// (lab/src/environment.rs, a photographed sky with its sun clipped), in the
/// units of the point light's illuminance. It is the mean of the procedural
/// sky this investigation had before, so the two lights keep their balance.
pub const SKY_LEVEL: f32 = 0.31;

/// The skylight's environment: the lab's, at the level lighting reads.
pub fn environment(dir: Vec3) -> Vec3 {
    lab::environment::radiance(dir) * SKY_LEVEL
}

/// Six square faces of `size` texels, face-major then row-major (v down, u across).
#[derive(Clone)]
pub struct Cubemap {
    pub size: usize,
    pub texels: Vec<Vec3>,
}

/// TexelCoordToVect: the direction through texel coordinate (u, v) of a face.
pub fn texel_dir(face: usize, u: f32, v: f32, size: usize) -> Vec3 {
    let nvc_u = 2.0 * (u + 0.5) / size as f32 - 1.0;
    let nvc_v = 2.0 * (v + 0.5) / size as f32 - 1.0;
    (FACES[face][0] * nvc_u + FACES[face][1] * nvc_v + FACES[face][2]).normalize()
}

/// TexelCoordSolidAngle: the two triangles between the texel's corner directions.
pub fn texel_solid_angle(face: usize, u: f32, v: f32, size: usize) -> f32 {
    let c = |du: f32, dv: f32| texel_dir(face, u + du, v + dv, size).as_dvec3().normalize();
    let (c0, c1, c2, c3) = (c(-0.5, -0.5), c(-0.5, 0.5), c(0.5, -0.5), c(0.5, 0.5));
    let area = |a: DVec3, b: DVec3, o: DVec3| 0.5 * (a - o).cross(b - o).length();
    (area(c1, c2, c0) + area(c2, c3, c1)) as f32
}

/// The face of a direction (its largest component) and the direction on that face.
fn face_of(dir: Vec3) -> (usize, Vec3) {
    let a = dir.abs();
    let (face, max) = if a.x >= a.y && a.x >= a.z {
        (if dir.x >= 0.0 { 0 } else { 1 }, a.x)
    } else if a.y >= a.x && a.y >= a.z {
        (if dir.y >= 0.0 { 2 } else { 3 }, a.y)
    } else {
        (if dir.z >= 0.0 { 4 } else { 5 }, a.z)
    };
    (face, dir / max)
}

impl Cubemap {
    pub fn from_fn(size: usize, f: impl Fn(Vec3) -> Vec3 + Sync) -> Cubemap {
        Cubemap { size, texels: parallel(6 * size * size, |i| f(texel_dir(i / (size * size), (i % size) as f32, ((i / size) % size) as f32, size))) }
    }

    pub fn texel(&self, face: usize, u: usize, v: usize) -> Vec3 {
        self.texels[(face * self.size + v) * self.size + u]
    }

    /// FilterImage with MipGenType::box from the top mip: each texel is the
    /// mean of the block of `self` it covers.
    pub fn box_down(&self, size: usize) -> Cubemap {
        let k = self.size / size;
        let texels = parallel(6 * size * size, |i| {
            let (face, u, v) = (i / (size * size), i % size, (i / size) % size);
            let mut sum = Vec3::ZERO;
            for dv in 0..k {
                for du in 0..k {
                    sum += self.texel(face, u * k + du, v * k + dv);
                }
            }
            sum / (k * k) as f32
        });
        Cubemap { size, texels }
    }

    /// GetCubeMapTexelBilinear: VectToTexelCoord maps the face's [-1, 1] onto
    /// [0, size - 1] (texel indices, not texel centres) and blends the four
    /// texels after it, clamped to the face.
    pub fn bilinear_cubemapgen(&self, dir: Vec3) -> Vec3 {
        let (face, on_face) = face_of(dir);
        let s = self.size as f32;
        let u = (s - 1.0) * 0.5 * (FACES[face][0].dot(on_face) + 1.0);
        let v = (s - 1.0) * 0.5 * (FACES[face][1].dot(on_face) + 1.0);
        let (u0, v0) = (u as usize, v as usize);
        let (u1, v1) = ((u0 + 1).min(self.size - 1), (v0 + 1).min(self.size - 1));
        let (du, dv) = (u - u0 as f32, v - v0 as f32);
        let top = self.texel(face, u0, v0) * (1.0 - du) + self.texel(face, u1, v0) * du;
        let bottom = self.texel(face, u0, v1) * (1.0 - du) + self.texel(face, u1, v1) * du;
        top * (1.0 - dv) + bottom * dv
    }

    /// The run-time lookup (the shader's cubeSample): bilinear between texel
    /// centres, clamped at the face's edge (a GPU's seamless filtering would
    /// blend across it).
    pub fn sample(&self, dir: Vec3) -> Vec3 {
        let (face, on_face) = face_of(dir);
        let s = self.size as f32;
        let p = (Vec2::new(FACES[face][0].dot(on_face), FACES[face][1].dot(on_face)) + 1.0) * 0.5 * s - 0.5;
        let p = p.clamp(Vec2::ZERO, Vec2::splat(s - 1.0));
        let (u0, v0) = (p.x.floor() as usize, p.y.floor() as usize);
        let (u1, v1) = ((u0 + 1).min(self.size - 1), (v0 + 1).min(self.size - 1));
        let w = p - p.floor();
        let top = self.texel(face, u0, v0).lerp(self.texel(face, u1, v0), w.x);
        let bottom = self.texel(face, u0, v1).lerp(self.texel(face, u1, v1), w.x);
        top.lerp(bottom, w.y)
    }
}

/// `f(i)` for every i below `count`, on every core.
fn parallel(count: usize, f: impl Fn(usize) -> Vec3 + Sync) -> Vec<Vec3> {
    let mut out = vec![Vec3::ZERO; count];
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let per = count.div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (chunk, slice) in out.chunks_mut(per).enumerate() {
            let f = &f;
            scope.spawn(move || {
                for (i, texel) in slice.iter_mut().enumerate() {
                    *texel = f(chunk * per + i);
                }
            });
        }
    });
    out
}

/// CubeMapGen's ImportanceSampleGGX (it takes alpha squared).
fn importance_sample_ggx(xi: Vec2, alpha_roughness_sqr: f32, n: Vec3) -> Vec3 {
    let phi = 2.0 * PI * xi.x;
    let cos_theta = ((1.0 - xi.y) / (1.0 + (alpha_roughness_sqr - 1.0) * xi.y)).sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
    let h = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta);
    let up = if n.z.abs() < 0.999 { Vec3::Z } else { Vec3::X };
    let tangent_x = up.cross(n).normalize();
    let tangent_y = n.cross(tangent_x);
    tangent_x * h.x + tangent_y * h.y + n * h.z
}

/// The specular and diffuse cubemaps of one environment.
pub struct Ibl {
    /// Mip m is prefiltered for perceptual roughness m / 5.
    pub specular: Vec<Cubemap>,
    pub diffuse: Cubemap,
}

/// The roughness a specular mip is filtered for (FilterCubeSurfacesGGX).
pub fn mip_roughness(mip: usize) -> f32 {
    let smoothness = (1.0 - mip as f32 / (SPECULAR_MIPS - 1) as f32).max(0.0);
    1.0 - smoothness
}

impl Ibl {
    pub fn build(env: impl Fn(Vec3) -> Vec3 + Sync, size: usize, diffuse_size: usize, samples: u32) -> Ibl {
        let specular = Ibl::build_specular(env, size, samples);
        let top = &specular[0];
        let small = if diffuse_size == size { top.clone() } else { top.box_down(diffuse_size) };
        Ibl { diffuse: cosine_filter(&small), specular }
    }

    /// The specular cubemap alone: mip 0 copied, the others GGX-filtered.
    pub fn build_specular(env: impl Fn(Vec3) -> Vec3 + Sync, size: usize, samples: u32) -> Vec<Cubemap> {
        let top = Cubemap::from_fn(size, env);
        // The box-filtered mips of the source, which the GGX filter reads.
        let source: Vec<Cubemap> = (0..SPECULAR_MIPS).map(|m| if m == 0 { top.clone() } else { top.box_down(size >> m) }).collect();
        let mut specular = vec![top];
        for mip in 1..SPECULAR_MIPS {
            specular.push(prefilter_ggx(&source, mip, samples));
        }
        specular
    }

    /// The lab environment's cubemaps, built once per run.
    pub fn get() -> &'static Ibl {
        static IBL: OnceLock<Ibl> = OnceLock::new();
        IBL.get_or_init(|| Ibl::build(environment, SPECULAR_SIZE, DIFFUSE_SIZE, GGX_SAMPLES))
    }

    /// SampleLevel(..., GetRoughnessMip(roughness)): trilinear between mips.
    pub fn sample_specular(&self, dir: Vec3, mip: f32) -> Vec3 {
        let m = mip.clamp(0.0, (SPECULAR_MIPS - 1) as f32);
        let m0 = m.floor() as usize;
        let m1 = (m0 + 1).min(SPECULAR_MIPS - 1);
        self.specular[m0].sample(dir).lerp(self.specular[m1].sample(dir), m - m0 as f32)
    }

    pub fn sample_diffuse(&self, dir: Vec3) -> Vec3 {
        self.diffuse.sample(dir)
    }
}

/// FilterCubeSurfacesGGX for one mip: per texel, with the normal and the
/// view both along its direction, 256 GGX samples of the source, each read
/// from the source mip whose texels match the sample's solid angle and
/// weighted by NdotL.
fn prefilter_ggx(source: &[Cubemap], mip: usize, samples: u32) -> Cubemap {
    let size = source[0].size >> mip;
    let max_mip = (SPECULAR_MIPS - 1) as u32;
    let perceptual = mip_roughness(mip);
    let alpha = perceptual * perceptual;
    let a2 = alpha * alpha;
    let w0 = source[0].size as f32;
    let texels = parallel(6 * size * size, |i| {
        let (face, u, v) = (i / (size * size), i % size, (i / size) % size);
        let center = texel_dir(face, u as f32, v as f32, size);
        let mut color = Vec3::ZERO;
        let mut total = 0.0f32;
        for s in 0..samples {
            let xi = super::brdf::hammersley(s, samples);
            let h = importance_sample_ggx(xi, a2, center);
            let v_dot_h = center.dot(h);
            let l = 2.0 * v_dot_h * h - center;
            let n_dot_l = center.dot(l);
            if n_dot_l > 0.0 {
                let dh = a2 / (PI * (v_dot_h * v_dot_h * (a2 - 1.0) + 1.0).powf(2.0));
                let pdf = dh * v_dot_h / (4.0 * v_dot_h);
                let solid_angle_texel = 4.0 * PI / (6.0 * w0 * w0);
                let solid_angle_sample = 1.0 / (samples as f32 * pdf);
                let mip_f = 0.5 * (solid_angle_sample / solid_angle_texel).log2() + 1.0;
                // CubeMapGen casts floor(mip) to an unsigned integer; it is never negative here.
                let mut mip_a = mip_f.floor().max(0.0) as u32;
                let mut mip_b = mip_a + 1;
                let mut t = (mip_f - mip_a as f32).clamp(0.0, 1.0);
                if mip_a >= max_mip {
                    mip_a = max_mip;
                    mip_b = max_mip;
                    t = 0.0;
                }
                let a = source[mip_a as usize].bilinear_cubemapgen(l);
                let b = source[mip_b as usize].bilinear_cubemapgen(l);
                color += (a * (1.0 - t) + b * t) * n_dot_l;
                total += n_dot_l;
            }
        }
        color / total
    });
    Cubemap { size, texels }
}

/// The IBLDiffuse preset's cosine filter over the whole sphere: each texel
/// is the mean of the source weighted by solid angle and the positive
/// cosine, so it holds irradiance / pi (DiffuseGlobalFullscreen multiplies it
/// by albedo with no 1 / pi).
fn cosine_filter(source: &Cubemap) -> Cubemap {
    let size = source.size;
    let taps: Vec<(Vec3, f32)> = (0..6 * size * size)
        .map(|i| {
            let (face, u, v) = (i / (size * size), (i % size) as f32, ((i / size) % size) as f32);
            (texel_dir(face, u, v, size), texel_solid_angle(face, u, v, size))
        })
        .collect();
    let texels = parallel(6 * size * size, |i| {
        let center = taps[i].0;
        let mut color = Vec3::ZERO;
        let mut total = 0.0f32;
        for (j, &(dir, solid_angle)) in taps.iter().enumerate() {
            let tap_dot = dir.dot(center);
            if tap_dot > 0.0 {
                let weight = solid_angle * tap_dot;
                color += source.texels[j] * weight;
                total += weight;
            }
        }
        color / total
    });
    Cubemap { size, texels }
}

/// Where each cubemap starts in gShadingData, after the BRDF texture.
pub fn specular_offset(mip: usize) -> usize {
    let lut = super::brdf::BRDF_MAP_SIZE * super::brdf::BRDF_MAP_SIZE;
    lut + (0..mip).map(|m| 6 * (SPECULAR_SIZE >> m).pow(2)).sum::<usize>()
}

pub fn diffuse_offset() -> usize {
    specular_offset(SPECULAR_MIPS)
}

/// The shader's gShadingData: the BRDF texture, the specular mips, the diffuse cubemap.
pub fn shading_data() -> Vec<[f32; 4]> {
    let mut data = super::brdf::BrdfMap::shading_data();
    let ibl = Ibl::get();
    for cube in ibl.specular.iter().chain(std::iter::once(&ibl.diffuse)) {
        data.extend(cube.texels.iter().map(|t| [t.x, t.y, t.z, 0.0]));
    }
    debug_assert_eq!(data.len(), diffuse_offset() + 6 * DIFFUSE_SIZE * DIFFUSE_SIZE);
    // Then the material textures (maps.rs).
    data.extend(super::maps::occlusion().iter().map(|&v| [v, 0.0, 0.0, 0.0]));
    data.extend_from_slice(super::maps::decals());
    // Then the reflection probes' cubemaps (reflections.rs).
    data.extend(super::reflections::shading_data());
    data
}
