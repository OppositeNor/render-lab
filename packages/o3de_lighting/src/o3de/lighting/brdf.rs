//! The CPU mirror of shaders/standard_lighting.wgsl: O3DE Atom's StandardPBR
//! surface setup, BRDF, BRDF texture, multiscatter compensation, and simple
//! point light, function by function and in the same order of operations.
//! The demonstrations read their numbers from here; `selftest` compares
//! every function with the GPU.
//!
//! Ported from the Open 3D Engine, Copyright Contributors to the Open 3D Engine
//! Project, used under the MIT License (THIRD_PARTY.md).

use lab::layout::ObjectData;
use glam::{Vec2, Vec3};
use std::sync::OnceLock;

/// Atom/RPI/Math.azsli.
pub const PI: f32 = 3.14159265;
pub const INV_PI: f32 = 0.31830988;
pub const GGX_EPSILON: f32 = 1e-8;
/// MaterialUtils.azsli.
pub const MAX_DIELECTRIC_SPECULAR_F0: f32 = 0.08;
pub const MIN_ROUGHNESS_LINEAR: f32 = 0.024;
/// Textures/BRDFTexture.attimage.
pub const BRDF_MAP_SIZE: usize = 256;
/// BRDFTextureCS's NumSamples.
pub const BRDF_MAP_SAMPLES: u32 = 1024;

/// What the scene pass shows (gShading.viewMode): a choice (`render-lab
/// values`), whose WGSL constants (VIEW_*) the lab writes into
/// build/generated/view_mode.wgsl, which the shader composes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, lab::Choice, serde::Serialize, serde::Deserialize)]
#[choice(package = "o3de_lighting", wgsl = "VIEW")]
#[serde(rename_all = "camelCase")]
pub enum ViewMode {
    /// The lit result.
    #[choice(text = "controls.final")]
    Final,
    #[choice(text = "controls.diffuse_lighting")]
    Diffuse,
    #[choice(text = "controls.specular_lighting")]
    Specular,
    #[choice(label = "D (GGX)")]
    D,
    #[choice(label = "G (Smith)")]
    G,
    #[choice(label = "F (Schlick)")]
    F,
    #[choice(text = "controls.multiscatter_compensation")]
    Multiscatter,
    #[choice(label = "diffuseResponse")]
    DiffuseResponse,
    #[choice(label = "NdotL")]
    Ndotl,
    #[choice(text = "controls.radius_attenuation")]
    Attenuation,
    #[choice(text = "common.diffuse_ibl")]
    IblDiffuse,
    #[choice(text = "common.specular_ibl")]
    IblSpecular,
    /// The lights in the pixel's tile and depth bin (the forward pass's list; 0 without tiles).
    #[choice(text = "controls.lights_per_tile")]
    LightCount,
    /// The reflection probes' stencil and blend weight.
    #[choice(text = "controls.probe_stencil")]
    ProbeStencil,
    #[choice(text = "controls.probe_blend_weight")]
    ProbeWeight,
    /// The upsampled SSAO, which is screen space: the CPU mirror shows the final pixel.
    #[choice(label = "SSAO")]
    Ssao,
    /// How much MSAAResolveCustom changes the specular, relative to a plain mean (heat ramp, 0 to 10 percent).
    #[choice(text = "controls.resolve_difference")]
    ResolveDifference,
}

pub fn saturate(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn mix(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

/// SurfaceData_StandardPBR after CalculateRoughnessA and SetAlbedoAndSpecularF0.
#[derive(Clone, Copy, Debug)]
pub struct Surface {
    pub position: Vec3,
    pub normal: Vec3,
    pub base_color: Vec3,
    pub metallic: f32,
    pub roughness_linear: f32,
    pub roughness_a: f32,
    pub roughness_a2: f32,
    pub albedo: Vec3,
    pub specular_f0: Vec3,
    /// diffuseAmbientOcclusion and specularOcclusion: 1 without occlusion maps.
    pub diffuse_ao: f32,
    pub specular_occlusion: f32,
    /// ClearCoatSurfaceData, when the material's clearCoat.enable is on.
    pub clear_coat: Option<ClearCoat>,
    /// GetVertexNormal(), and TransmissionSurfaceData when o_transmission_mode is not None.
    pub vertex_normal: Vec3,
    pub transmission: Option<super::transmission::Transmission>,
}

/// ClearCoatSurfaceData (Surfaces/ClearCoatSurfaceData.azsli).
#[derive(Clone, Copy, Debug)]
pub struct ClearCoat {
    pub factor: f32,
    /// Linear roughness of the coat alone.
    pub roughness: f32,
    pub normal: Vec3,
}

/// ApplyClearCoatToSpecularF0 (MaterialUtils.azsli; Filament's base-layer change).
pub fn apply_clear_coat_to_specular_f0(specular_f0: Vec3, factor: f32) -> Vec3 {
    let root = specular_f0.map(f32::sqrt);
    let f0 = (Vec3::ONE - 5.0 * root) / (Vec3::splat(5.0) - root);
    mix(specular_f0, f0 * f0, factor)
}

/// ClearCoatGGX (Brdf.azsli), with Kelemen's visibility 0.25 / HdotL^2.
pub fn clear_coat_ggx(n_dot_h: f32, h_dot_l: f32, n_dot_l: f32, roughness_a: f32, f: Vec3) -> Vec3 {
    let d = normal_distribution_ggx(n_dot_h, roughness_a * roughness_a).max(0.0);
    let g = (0.25 / (h_dot_l * h_dot_l).max(1e-4)).max(0.0);
    d * g * f * n_dot_l
}

/// ClearCoatSpecular (Brdf.azsli): the base specular through (1 - Fc), plus the coat's.
pub fn clear_coat_specular(dir_to_light: Vec3, dir_to_camera: Vec3, coat: &ClearCoat, specular: Vec3) -> Vec3 {
    let half_vector = (dir_to_light + dir_to_camera).normalize();
    let n_dot_h = saturate(coat.normal.dot(half_vector));
    let n_dot_l = saturate(coat.normal.dot(dir_to_light));
    let h_dot_l = saturate(half_vector.dot(dir_to_light));
    let clear_coat_f = fresnel_schlick(h_dot_l, Vec3::splat(0.04)) * coat.factor;
    let roughness = (coat.roughness * coat.roughness).max(0.0005);
    specular * (Vec3::ONE - clear_coat_f) + clear_coat_ggx(n_dot_h, h_dot_l, n_dot_l, roughness, clear_coat_f)
}

/// CalculateSpecularAA (SpecularAA.azsli; Tokuyoshi and Kaplanyan 2019):
/// roughnessA2 widened by the normal's screen-space derivatives.
pub fn calculate_specular_aa(dndx: Vec3, dndy: Vec3, roughness_a2: f32) -> f32 {
    let screen_variance = 0.25;
    let variance_thresh = 0.18;
    let variance = screen_variance * (dndx.dot(dndx) + dndy.dot(dndy));
    let kernel_roughness_a2 = (2.0 * variance).min(variance_thresh);
    saturate(roughness_a2 + kernel_roughness_a2)
}

impl Surface {
    /// CalculateRoughnessValues' specular AA (when the applySpecularAA
    /// property is on): roughnessA2 widened, then roughnessA and
    /// roughnessLinear recomputed from it.
    pub fn with_specular_aa(mut self, dndx: Vec3, dndy: Vec3) -> Surface {
        self.roughness_a2 = calculate_specular_aa(dndx, dndy, self.roughness_a2);
        self.roughness_a = self.roughness_a2.sqrt();
        self.roughness_linear = self.roughness_a.sqrt();
        self
    }

    pub fn new(position: Vec3, normal: Vec3, base_color: Vec3, metallic: f32, specular_f0_factor: f32, roughness: f32) -> Surface {
        let roughness_linear = roughness.max(MIN_ROUGHNESS_LINEAR);
        let roughness_a = roughness_linear * roughness_linear;
        let dielectric_specular_f0 = Vec3::splat(MAX_DIELECTRIC_SPECULAR_F0 * specular_f0_factor);
        Surface {
            position,
            normal,
            base_color,
            metallic,
            roughness_linear,
            roughness_a,
            roughness_a2: roughness_a * roughness_a,
            albedo: base_color * (1.0 - metallic),
            specular_f0: mix(dielectric_specular_f0, base_color, metallic),
            diffuse_ao: 1.0,
            specular_occlusion: 1.0,
            clear_coat: None,
            vertex_normal: normal,
            transmission: None,
        }
    }
}

pub fn fresnel_schlick_f90(h_dot_v: f32, f0: Vec3, f90: f32) -> Vec3 {
    f0 + (Vec3::splat(f90) - f0) * (1.0 - h_dot_v).powf(5.0)
}

pub fn fresnel_schlick(h_dot_v: f32, f0: Vec3) -> Vec3 {
    fresnel_schlick_f90(h_dot_v, f0, 1.0)
}

pub fn fresnel_schlick_with_roughness(n_dot_v: f32, specular_f0: Vec3, roughness: f32) -> Vec3 {
    let smoothness = 1.0 - roughness;
    specular_f0 + (Vec3::splat(smoothness).max(specular_f0) - specular_f0) * (1.0 - saturate(n_dot_v)).powf(5.0)
}

pub fn normal_distribution_ggx(n_dot_h: f32, roughness_a2: f32) -> f32 {
    let b = (n_dot_h * roughness_a2 - n_dot_h) * n_dot_h + 1.0;
    roughness_a2 / (PI * b * b)
}

pub fn geometric_shadowing_masking_ggx_correlated(n_dot_v: f32, n_dot_l: f32, roughness_a2: f32) -> f32 {
    let ggx_v = n_dot_l * ((-n_dot_v * roughness_a2 + n_dot_v) * n_dot_v + roughness_a2).sqrt();
    let ggx_l = n_dot_v * ((-n_dot_l * roughness_a2 + n_dot_l) * n_dot_l + roughness_a2).sqrt();
    0.5 / (ggx_v + ggx_l).max(GGX_EPSILON)
}

pub fn importance_sample_ggx(xi: Vec2, roughness_a: f32, normal_ws: Vec3) -> Vec3 {
    let roughness_a2 = roughness_a * roughness_a;
    let phi = 2.0 * PI * xi.x;
    let cos_theta = ((1.0 - xi.y) / (1.0 + (roughness_a2 - 1.0) * xi.y)).sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
    let micronormal_ts = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta);
    let up_vector = if normal_ws.z.abs() < 0.999 { Vec3::Z } else { Vec3::X };
    let tangent_ws = up_vector.cross(normal_ws).normalize();
    let bitangent_ws = normal_ws.cross(tangent_ws);
    tangent_ws * micronormal_ts.x + bitangent_ws * micronormal_ts.y + normal_ws * micronormal_ts.z
}

/// GetHammersleyPoint.
pub fn hammersley(point_index: u32, total_point_count: u32) -> Vec2 {
    Vec2::new(point_index as f32 / total_point_count as f32, point_index.reverse_bits() as f32 * 2.328_306_4e-10)
}

pub fn diffuse_lambertian(albedo: Vec3, normal: Vec3, dir_to_light: Vec3, diffuse_response: Vec3) -> Vec3 {
    let n_dot_l = saturate(normal.dot(dir_to_light));
    albedo * n_dot_l * INV_PI * diffuse_response
}

pub fn normalized_disney_diffuse(albedo: Vec3, normal: Vec3, dir_to_camera: Vec3, dir_to_light: Vec3, roughness_linear: f32) -> Vec3 {
    let half_vector = (dir_to_light + dir_to_camera).normalize();
    let n_dot_v = saturate(normal.dot(dir_to_camera));
    let n_dot_l = saturate(normal.dot(dir_to_light));
    let h_dot_v = saturate(half_vector.dot(dir_to_camera));
    let energy_bias = 0.5 * roughness_linear;
    let energy_factor = 1.0 + (1.0 / 1.51 - 1.0) * roughness_linear;
    let fd90 = energy_bias + 2.0 * h_dot_v * h_dot_v * roughness_linear;
    let light_scatter = fresnel_schlick_f90(n_dot_l, Vec3::ONE, fd90).x;
    let view_scatter = fresnel_schlick_f90(n_dot_v, Vec3::ONE, fd90).x;
    albedo * light_scatter * view_scatter * energy_factor * n_dot_l * INV_PI
}

/// The microfacet terms SpecularGGX multiplies, and the angles they read.
#[derive(Clone, Copy, Debug)]
pub struct GgxTerms {
    pub d: f32,
    /// The visibility G / (4 NdotL NdotV), as GeometricShadowingMaskingGGXCorrelated returns it.
    pub g: f32,
    pub f: Vec3,
    pub n_dot_l: f32,
    pub n_dot_h: f32,
    pub h_dot_v: f32,
    pub half_vector: Vec3,
}

pub fn ggx_terms(dir_to_camera: Vec3, dir_to_light: Vec3, normal: Vec3, specular_f0: Vec3, n_dot_v: f32, roughness_a2: f32) -> GgxTerms {
    let half_vector = (dir_to_light + dir_to_camera).normalize();
    let n_dot_h = saturate(normal.dot(half_vector));
    let n_dot_l = saturate(normal.dot(dir_to_light));
    let h_dot_v = saturate(half_vector.dot(dir_to_camera));
    let d = normal_distribution_ggx(n_dot_h, roughness_a2).max(0.0);
    let g = geometric_shadowing_masking_ggx_correlated(n_dot_v, n_dot_l, roughness_a2).max(0.0);
    GgxTerms { d, g, f: fresnel_schlick(h_dot_v, specular_f0), n_dot_l, n_dot_h, h_dot_v, half_vector }
}

pub fn specular_ggx(dir_to_camera: Vec3, dir_to_light: Vec3, normal: Vec3, specular_f0: Vec3, n_dot_v: f32, roughness_a2: f32, multi_scatter_compensation: Vec3) -> Vec3 {
    let t = ggx_terms(dir_to_camera, dir_to_light, normal, specular_f0, n_dot_v, roughness_a2);
    ((t.d * t.g) * t.n_dot_l * t.f) * multi_scatter_compensation
}

/// The BRDF texture BRDFTextureCS fills once at startup: (A, B) per texel,
/// row-major from the top row, NdotV across and smoothness (1 - roughness) up.
pub struct BrdfMap {
    pub texels: Vec<Vec2>,
}

/// BRDFTextureCS::MainCS for dispatch (x, y), which it stores at row size - 1 - y.
pub fn brdf_texel(x: u32, y: u32, texture_size: u32) -> Vec2 {
    let roughness = (y as f32 + 0.5) / texture_size as f32;
    let roughness_a = roughness * roughness;
    let roughness_a2 = roughness_a * roughness_a;
    let n_dot_v = (x as f32 + 0.5) / texture_size as f32;
    let v = Vec3::new((1.0 - n_dot_v * n_dot_v).sqrt(), 0.0, n_dot_v);
    let n = Vec3::Z;
    let (mut a, mut b) = (0.0f32, 0.0f32);
    for i in 0..BRDF_MAP_SAMPLES {
        let xi = hammersley(i, BRDF_MAP_SAMPLES);
        let h = importance_sample_ggx(xi, roughness_a, n);
        let l = (2.0 * v.dot(h) * h - v).normalize();
        let n_dot_l = l.z.max(0.0);
        let n_dot_h = h.z.max(0.0);
        let v_dot_h = v.dot(h).max(0.0);
        if n_dot_l > 0.0 {
            let g = geometric_shadowing_masking_ggx_correlated(n_dot_v, n_dot_l, roughness_a2);
            let g_vis = (g * 4.0 * v_dot_h * n_dot_l) / n_dot_h;
            let fc = (1.0 - v_dot_h).powf(5.0);
            a += (1.0 - fc) * g_vis;
            b += fc * g_vis;
        }
    }
    Vec2::new(a, b) / BRDF_MAP_SAMPLES as f32
}

impl BrdfMap {
    /// The whole texture, on every core.
    pub fn generate() -> BrdfMap {
        let size = BRDF_MAP_SIZE;
        let mut texels = vec![Vec2::ZERO; size * size];
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let rows_per = size.div_ceil(threads);
        std::thread::scope(|scope| {
            for (chunk, rows) in texels.chunks_mut(rows_per * size).enumerate() {
                scope.spawn(move || {
                    for (i, texel) in rows.iter_mut().enumerate() {
                        let row = chunk * rows_per + i / size;
                        let x = (i % size) as u32;
                        let y = (size - 1 - row) as u32;
                        *texel = brdf_texel(x, y, size as u32);
                    }
                });
            }
        });
        BrdfMap { texels }
    }

    /// Generated once per run.
    pub fn get() -> &'static BrdfMap {
        static MAP: OnceLock<BrdfMap> = OnceLock::new();
        MAP.get_or_init(BrdfMap::generate)
    }

    pub fn texel(&self, x: usize, row: usize) -> Vec2 {
        self.texels[row * BRDF_MAP_SIZE + x]
    }

    /// SampleBrdfMap: bilinear, clamped (the shader's sampleBrdfMap).
    pub fn sample(&self, uv: Vec2) -> Vec2 {
        let size = BRDF_MAP_SIZE as f32;
        let p = (uv * size - 0.5).clamp(Vec2::ZERO, Vec2::splat(size - 1.0));
        let (x0, y0) = (p.x.floor() as usize, p.y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(BRDF_MAP_SIZE - 1), (y0 + 1).min(BRDF_MAP_SIZE - 1));
        let w = p - p.floor();
        let lerp = |a: Vec2, b: Vec2, t: f32| a + (b - a) * t;
        lerp(lerp(self.texel(x0, y0), self.texel(x1, y0), w.x), lerp(self.texel(x0, y1), self.texel(x1, y1), w.x), w.y)
    }

    /// The shader's gShadingData.
    pub fn shading_data() -> Vec<[f32; 4]> {
        BrdfMap::get().texels.iter().map(|t| [t.x, t.y, 0.0, 0.0]).collect()
    }
}

pub fn multi_scatter_compensation(specular_f0: Vec3, brdf: Vec2, enabled: bool) -> Vec3 {
    if !enabled {
        return Vec3::ONE;
    }
    let compensation = specular_f0 * ((1.0 / (brdf.x + brdf.y)) - 1.0);
    compensation + 1.0
}

/// LightingData_BasePBR after InitializeLightingData_BasePBR.
#[derive(Clone, Copy, Debug)]
pub struct LightingData {
    pub dir_to_camera: Vec3,
    pub n_dot_v: f32,
    pub brdf: Vec2,
    pub specular_response: Vec3,
    pub diffuse_response: Vec3,
    pub multi_scatter_compensation: Vec3,
    pub diffuse_lighting: Vec3,
    pub specular_lighting: Vec3,
    pub translucent_back_lighting: Vec3,
}

impl LightingData {
    pub fn new(s: &Surface, view_position: Vec3, multiscatter: bool, map: &BrdfMap) -> LightingData {
        let dir_to_camera = (view_position - s.position).normalize();
        let n_dot_v = saturate(s.normal.dot(dir_to_camera));
        let brdf = map.sample(Vec2::new(n_dot_v, 1.0 - s.roughness_linear));
        let specular_response = fresnel_schlick_with_roughness(n_dot_v, s.specular_f0, s.roughness_linear);
        // InitializeLightingData_StandardPBR: the coat (IOR 1.5, F0 0.04) takes its share.
        let coat = s.clear_coat.map_or(Vec3::ONE, |c| Vec3::ONE - fresnel_schlick_with_roughness(n_dot_v, Vec3::splat(0.04), c.roughness) * c.factor);
        LightingData {
            dir_to_camera,
            n_dot_v,
            brdf,
            specular_response,
            diffuse_response: (Vec3::ONE - specular_response) * coat,
            multi_scatter_compensation: multi_scatter_compensation(s.specular_f0, brdf, multiscatter),
            diffuse_lighting: Vec3::ZERO,
            specular_lighting: Vec3::ZERO,
            translucent_back_lighting: Vec3::ZERO,
        }
    }
}

/// SimplePointLight (Lights/LightStructures.azsli), as the feature processor fills it.
#[derive(Clone, Copy, Debug)]
pub struct PointLight {
    pub position: Vec3,
    pub inv_attenuation_radius_squared: f32,
    pub rgb_intensity_candelas: Vec3,
    /// m_lightingChannelMask: the lighting channels it lights.
    pub lighting_channel_mask: u32,
}

/// The editor's lighting-channel default for lights and meshes: all five
/// channels (LightingChannelConfiguration).
pub const ALL_LIGHTING_CHANNELS: u32 = 0x1F;

impl PointLight {
    pub fn attenuation_radius(&self) -> f32 {
        1.0 / self.inv_attenuation_radius_squared.sqrt()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PointLightSample {
    pub dir_to_light: Vec3,
    pub distance_squared: f32,
    pub falloff: f32,
    pub radius_attenuation: f32,
    pub intensity: Vec3,
}

/// SimplePointLightUtil::Init and the first half of Apply.
pub fn simple_point_light(light: &PointLight, surface_position: Vec3) -> PointLightSample {
    let pos_to_light = light.position - surface_position;
    let distance_squared = pos_to_light.dot(pos_to_light);
    let falloff = distance_squared * light.inv_attenuation_radius_squared;
    let mut radius_attenuation = 1.0 - falloff * falloff;
    radius_attenuation *= radius_attenuation;
    PointLightSample {
        dir_to_light: pos_to_light.normalize(),
        distance_squared,
        falloff,
        radius_attenuation,
        intensity: light.rgb_intensity_candelas / (0.001f32 * 0.001).max(distance_squared) * radius_attenuation,
    }
}

/// The diffuse model (gShading.diffuseModel): a choice (`render-lab values`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, lab::Choice, serde::Serialize, serde::Deserialize)]
#[choice(package = "o3de_lighting")]
#[serde(rename_all = "camelCase")]
pub enum DiffuseModel {
    /// DiffuseLambertian, which StandardPBR uses.
    #[choice(label = "DiffuseLambertian")]
    Lambertian,
    /// NormalizedDisneyDiffuse, which StandardPBR does not call (EnhancedPBR does
    /// with subsurface scattering on, Skin always).
    #[choice(label = "NormalizedDisneyDiffuse")]
    NormalizedDisney,
}

/// GetDiffuseLighting_StandardPBR and GetSpecularLighting_StandardPBR for one light.
pub fn apply_light(s: &Surface, l: &mut LightingData, intensity: Vec3, dir_to_light: Vec3, model: DiffuseModel) {
    l.diffuse_lighting += diffuse_light(s, l, intensity, dir_to_light, model);
    l.specular_lighting += specular_light(s, l, intensity, dir_to_light);
}

/// GetDiffuseLighting_StandardPBR (and _EnhancedPBR without subsurface scattering).
pub fn diffuse_light(s: &Surface, l: &LightingData, intensity: Vec3, dir_to_light: Vec3, model: DiffuseModel) -> Vec3 {
    let mut diffuse = match model {
        DiffuseModel::Lambertian => diffuse_lambertian(s.albedo, s.normal, dir_to_light, l.diffuse_response),
        DiffuseModel::NormalizedDisney => normalized_disney_diffuse(s.albedo, s.normal, l.dir_to_camera, dir_to_light, s.roughness_linear),
    };
    if let (Some(c), DiffuseModel::Lambertian) = (s.clear_coat, model) {
        // The coat's Fresnel at the light's half vector, again.
        let h_dot_v = saturate((dir_to_light + l.dir_to_camera).normalize().dot(l.dir_to_camera));
        diffuse *= Vec3::ONE - fresnel_schlick(h_dot_v, Vec3::splat(0.04)) * c.factor;
    }
    diffuse * intensity
}

/// GetSpecularLighting_StandardPBR (and _EnhancedPBR without anisotropy).
pub fn specular_light(s: &Surface, l: &LightingData, intensity: Vec3, dir_to_light: Vec3) -> Vec3 {
    let mut specular = specular_ggx(l.dir_to_camera, dir_to_light, s.normal, s.specular_f0, l.n_dot_v, s.roughness_a2, l.multi_scatter_compensation);
    if let Some(c) = &s.clear_coat {
        specular = clear_coat_specular(dir_to_light, l.dir_to_camera, c, specular);
    }
    specular * intensity
}

/// ApplyIBL's specular in the forward pass, which clear coat switches on:
/// the global cubemap through the BRDF texture and the compensation, then the
/// coat's own IBL and the base layer through (1 - Fc)^2, times the exposure.
pub fn forward_ibl_specular(s: &Surface, l: &LightingData, exposure_stops: f32, map: &BrdfMap, cubes: &super::ibl::Ibl, probe: Option<&super::reflections::Probe>) -> Vec3 {
    let mut specular = ibl_specular(s.position, s.normal, s.specular_f0, s.roughness_linear, l.dir_to_camera, l.brdf, cubes, probe) * l.multi_scatter_compensation;
    if let Some(c) = s.clear_coat.filter(|c| c.factor > 0.0) {
        let n_dot_v = saturate(c.normal.dot(l.dir_to_camera)).max(0.01);
        let brdf = map.sample(Vec2::new(n_dot_v, 1.0 - c.roughness));
        let coat = ibl_specular(s.position, c.normal, Vec3::splat(0.04), c.roughness, l.dir_to_camera, brdf, cubes, probe) * c.factor;
        let response = fresnel_schlick_with_roughness(n_dot_v, Vec3::splat(0.04), c.roughness) * c.factor;
        specular = specular * (Vec3::ONE - response) * (Vec3::ONE - response) + coat;
    }
    // A probe's exposure replaces the skylight's.
    specular * 2.0f32.powf(probe.map_or(exposure_stops, |p| p.exposure))
}

/// GetIblSpecular: the global cubemap, blended toward the mesh's probe by its
/// inner/outer weight, with parallax correction when the probe has it.
#[allow(clippy::too_many_arguments)]
pub fn ibl_specular(position: Vec3, normal: Vec3, specular_f0: Vec3, roughness: f32, dir_to_camera: Vec3, brdf: Vec2, cubes: &super::ibl::Ibl, probe: Option<&super::reflections::Probe>) -> Vec3 {
    let reflect_dir = lab::scene::reflect(-dir_to_camera, normal);
    let split = specular_f0 * brdf.x + Vec3::splat(brdf.y);
    let global = cubes.sample_specular(reflect_dir, roughness * 5.0) * split;
    let Some(p) = probe else { return global };
    let dir = if p.parallax { p.parallax_correction(position, reflect_dir) } else { reflect_dir };
    let local = super::reflections::sample_probe(p, dir, roughness * 5.0) * split;
    let blend = if p.contains(p.inner_half, position) { 1.0 } else { p.lerp_inner_outer(position) };
    global.lerp(local, blend)
}

/// A material beyond what ObjectData holds (base colour, roughness).
#[derive(Clone, Copy, Debug, Default)]
pub struct Material {
    pub metallic: f32,
    pub specular_f0_factor: f32,
    /// The diffuse and specular occlusion factors when the material has the
    /// occlusion map (maps.rs), as `diffuseOcclusion.factor` and
    /// `specularOcclusion.factor`; none without it.
    pub diffuse_occlusion: Option<f32>,
    pub specular_occlusion: Option<f32>,
    /// clearCoat.factor and clearCoat.roughness when clearCoat.enable is on.
    pub clear_coat: Option<(f32, f32)>,
    /// The mesh's lighting-channel mask.
    pub lighting_channels: u32,
    /// EnhancedPBR's transmission, when its mode is not None.
    pub transmission: Option<super::transmission::Transmission>,
    /// The reflection probe forward-pass IBL uses (an index into Shading::probes).
    pub forward_probe: Option<usize>,
}

/// The shading the CPU mirror computes with (gShading holds its values and
/// what demos::shader_data derives from them; gShadingFrame the rest).
#[derive(Clone, Debug)]
pub struct Shading {
    /// The light the readout and views describe (light 0 of the frame's list).
    pub light: PointLight,
    pub light_on: bool,
    /// More simple point lights (the light-culling demonstration's).
    pub extra_lights: Vec<PointLight>,
    pub view: ViewMode,
    pub multiscatter: bool,
    pub diffuse_model: DiffuseModel,
    /// The object whose point the readout inspects.
    pub inspected: u32,
    /// Image-based lighting (the full-screen passes), its exposure in stops,
    /// and whether the forward targets quantize to 8 bits.
    pub ibl: bool,
    pub ibl_exposure: f32,
    pub quantize: bool,
    /// CalculateRoughnessValues' specular anti-aliasing (the material's applySpecularAA, true by default).
    pub specular_aa: bool,
    pub materials: [Material; 12],
    /// The view's decals (DecalTextureArrayFeatureProcessor's buffer).
    pub decals: Vec<super::decals::Decal>,
    /// O3DE point lights (sphere area lights), for transmission.
    pub sphere_lights: Vec<super::transmission::SphereLight>,
    /// Reflection probes, in the feature processor's order.
    pub probes: Vec<super::reflections::Probe>,
}

impl Shading {
    /// The per-object material records of gShadingFrame (from FRAME_MATERIALS):
    /// the clear coat's factor (-1 without it) and roughness.
    pub fn frame_materials(&self) -> Vec<[f32; 4]> {
        self.materials.iter().map(|m| {
            let (factor, roughness) = m.clear_coat.unwrap_or((-1.0, 0.0));
            [factor, roughness, m.lighting_channels as f32, m.forward_probe.map_or(-1.0, |i| i as f32)]
        }).collect()
    }

    /// Every simple point light, as the frame's list holds them: light 0 (off
    /// when `light_on` is false), then the extra ones.
    pub fn lights(&self) -> Vec<PointLight> {
        let main = PointLight { rgb_intensity_candelas: self.light.rgb_intensity_candelas * if self.light_on { 1.0 } else { 0.0 }, ..self.light };
        std::iter::once(main).chain(self.extra_lights.iter().copied()).collect()
    }

    /// The frame's simple point light records (gShadingFrame from FRAME_LIGHTS):
    /// the count, then (m_position, m_invAttenuationRadiusSquared),
    /// (m_rgbIntensityCandelas, m_lightingChannelMask) per light.
    pub fn frame_lights(&self) -> Vec<[f32; 4]> {
        let lights = self.lights();
        let mut out = vec![[lights.len() as f32, 0.0, 0.0, 0.0]];
        for l in &lights {
            out.push(l.position.extend(l.inv_attenuation_radius_squared).to_array());
            out.push(l.rgb_intensity_candelas.extend(l.lighting_channel_mask as f32).to_array());
        }
        out
    }

    pub fn material(&self, object: u32) -> Material {
        self.materials[(object as usize).min(self.materials.len() - 1)]
    }
}

/// One pixel, as o3dePixel computes it.
#[derive(Clone, Copy, Debug)]
pub struct Pixel {
    pub color: Vec3,
    pub surface: Surface,
    pub lighting: LightingData,
    pub light: PointLightSample,
    pub terms: GgxTerms,
    /// What the full-screen IBL passes add (computed even when they are off).
    pub ibl: Ibl,
}

impl Pixel {
    /// The point light's diffuse lighting.
    pub fn diffuse(&self) -> Vec3 {
        self.lighting.diffuse_lighting
    }

    pub fn specular(&self) -> Vec3 {
        self.lighting.specular_lighting
    }
}

/// What the full-screen IBL passes compute for a pixel (o3deIbl in the shader).
#[derive(Clone, Copy, Debug)]
pub struct Ibl {
    /// The forward pass's outputs, as the passes read them back.
    pub albedo: Vec3,
    pub specular_f0: Vec3,
    pub roughness: f32,
    /// DiffuseGlobalFullscreen.
    pub irradiance: Vec3,
    pub diffuse: Vec3,
    /// ReflectionGlobalFullscreen.
    pub reflect_dir: Vec3,
    pub mip: f32,
    pub prefiltered: Vec3,
    pub n_dot_v: f32,
    pub brdf: Vec2,
    pub compensation: Vec3,
    /// What ReflectionComposite adds (the global pass and the probes).
    pub specular: Vec3,
    /// The reflection passes at the point (stencil, blend weight, global weight).
    pub reflection: super::reflections::ReflectionSample,
}

/// What an R8G8B8A8_UNORM target stores. Ties round to even, as WGSL's
/// round() does (a GPU's conversion need only round to nearest).
pub fn unorm8(x: f32) -> f32 {
    (x.clamp(0.0, 1.0) * 255.0).round_ties_even() / 255.0
}

/// DiffuseGlobalFullscreen and ReflectionGlobalFullscreen for one pixel.
pub fn ibl(s: &Surface, l: &LightingData, eye: Vec3, shading: &Shading, map: &BrdfMap, cubes: &super::ibl::Ibl) -> Ibl {
    // The forward pass's albedo target: albedo times the diffuse response and
    // diffuse AO, specular occlusion in alpha.
    let mut albedo = s.albedo * l.diffuse_response * s.diffuse_ao;
    let mut specular_occlusion = s.specular_occlusion;
    let mut specular_f0 = s.specular_f0;
    let mut roughness = s.roughness_linear;
    if shading.quantize {
        albedo = albedo.map(unorm8);
        specular_occlusion = unorm8(specular_occlusion);
        specular_f0 = specular_f0.map(unorm8);
        roughness = unorm8(roughness);
    }
    let exposure = 2.0f32.powf(shading.ibl_exposure);
    let irradiance = cubes.sample_diffuse(s.normal);
    let dir_to_camera = (eye - s.position).normalize();
    let reflect_dir = lab::scene::reflect(-dir_to_camera, s.normal);
    let mip = roughness * 5.0;
    let prefiltered = cubes.sample_specular(reflect_dir, mip);
    let n_dot_v = saturate(s.normal.dot(dir_to_camera)).max(0.01);
    let brdf = map.sample(Vec2::new(saturate(n_dot_v), 1.0 - roughness));
    let compensation = multi_scatter_compensation(specular_f0, brdf, shading.multiscatter);
    // The reflection passes at this point; the forward pass's stencil keeps
    // forward-IBL (clear coat) materials out of them.
    let stencil = if s.clear_coat.is_some() { 0 } else { super::reflections::STENCIL_USE_IBL_SPECULAR_PASS } | super::reflections::STENCIL_USE_DIFFUSE_GI_PASS;
    let input = super::reflections::ReflectionInputs { position: s.position, normal: s.normal, enable_ibl: shading.ibl, multiscatter: shading.multiscatter, specular_f0, roughness, specular_occlusion, stencil };
    let reflection = super::reflections::reflection_sample(&shading.probes, &input, eye, shading.ibl_exposure, map);
    Ibl {
        albedo,
        specular_f0,
        roughness,
        irradiance,
        diffuse: albedo * irradiance * exposure,
        reflect_dir,
        mip,
        prefiltered,
        n_dot_v,
        brdf,
        compensation,
        specular: reflection.composited,
        reflection,
    }
}

/// The inverse of the lab's tonemapper (untonemap in the shader).
pub fn untonemap(display: Vec3, exposure: f32) -> Vec3 {
    let channel = |c: f32| {
        let lin = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        let y = lin.clamp(0.0, 0.98);
        let (a, b, cc) = (2.43 * y - 2.51, 0.59 * y - 0.03, 0.14 * y);
        (-b - (b * b - 4.0 * a * cc).max(0.0).sqrt()) / (2.0 * a)
    };
    Vec3::new(channel(display.x), channel(display.y), channel(display.z)) / exposure
}

/// The shader's dark-blue to yellow ramp.
pub fn heat(v: f32) -> Vec3 {
    let t = saturate(v);
    let (c0, c1, c2) = (Vec3::new(0.05, 0.08, 0.30), Vec3::new(0.10, 0.55, 0.65), Vec3::new(0.98, 0.90, 0.35));
    if t < 0.5 { mix(c0, c1, t * 2.0) } else { mix(c1, c2, (t - 0.5) * 2.0) }
}

/// o3dePixel: the forward pass for one pixel of `objects[object]` at `p`,
/// whose normal changes by `derivatives` (ddx_fine, ddy_fine) to the next
/// pixel across and down.
pub fn pixel(shading: &Shading, objects: &[ObjectData], p: Vec3, n: Vec3, eye: Vec3, object: u32, exposure: f32, map: &BrdfMap, derivatives: [Vec3; 2]) -> Pixel {
    let o = &objects[object as usize];
    let material = shading.material(object);
    let [r, g, b, roughness] = o.albedo_roughness;
    let mut s = Surface::new(p, n, Vec3::new(r, g, b), material.metallic, material.specular_f0_factor, roughness);
    if shading.specular_aa {
        s = s.with_specular_aa(derivatives[0], derivatives[1]);
    }
    // The occlusion maps, on the sphere's UVs.
    if material.diffuse_occlusion.is_some() || material.specular_occlusion.is_some() {
        let local = glam::Mat4::from_cols_array_2d(&o.world_to_local).transform_point3(p) - Vec3::from(o.local_center);
        let uv = super::maps::sphere_uv(local);
        s.diffuse_ao = super::maps::occlusion_input(uv, material.diffuse_occlusion);
        s.specular_occlusion = super::maps::occlusion_input(uv, material.specular_occlusion);
    }
    // GetClearCoatInputs without maps, then ApplyClearCoatToSpecularF0.
    if let Some((factor, roughness)) = material.clear_coat {
        s.clear_coat = Some(ClearCoat { factor, roughness: roughness.max(1e-3), normal: s.normal });
        s.specular_f0 = apply_clear_coat_to_specular_f0(s.specular_f0, factor);
    }
    s.transmission = material.transmission;
    // ApplyDecals_ForwardPass: every decal, in the tile lists' order (a decal
    // culled from the tile cannot contain the point).
    for i in super::decals::sorted(&shading.decals) {
        super::decals::apply_decal(&shading.decals[i], &mut s, n);
    }
    let mut l = LightingData::new(&s, eye, shading.multiscatter, map);
    // ApplySimplePointLight for each light, in index order: other lighting
    // channels and lights at or beyond their radius are skipped.
    for light in shading.lights() {
        if light.lighting_channel_mask & material.lighting_channels == 0 {
            continue;
        }
        let sample = simple_point_light(&light, p);
        if sample.falloff < 1.0 {
            apply_light(&s, &mut l, sample.intensity, sample.dir_to_light, shading.diffuse_model);
        }
    }
    // ApplyPointLights, after the punctual lights.
    for light in &shading.sphere_lights {
        super::transmission::apply_sphere_light(light, material.lighting_channels, &s, &mut l, shading.diffuse_model);
    }
    let sample = simple_point_light(&shading.lights()[0], p);
    // Image-based lighting in the forward pass, for clear coat.
    if s.clear_coat.is_some() && shading.ibl {
        let probe = material.forward_probe.map(|i| &shading.probes[i]);
        l.specular_lighting += forward_ibl_specular(&s, &l, shading.ibl_exposure, map, super::ibl::Ibl::get(), probe);
    }
    // LightingData_BasePBR::FinalizeLighting.
    l.specular_lighting *= s.specular_occlusion;
    l.diffuse_lighting *= s.diffuse_ao;
    // Transmitted light, after the occlusion.
    if let Some(t) = &s.transmission {
        l.diffuse_lighting += l.translucent_back_lighting * t.tint;
    }
    let terms = ggx_terms(l.dir_to_camera, sample.dir_to_light, s.normal, s.specular_f0, l.n_dot_v, s.roughness_a2);
    let ibl = ibl(&s, &l, eye, shading, map, super::ibl::Ibl::get());
    let ibl_on = if shading.ibl { 1.0 } else { 0.0 };
    let emissive = Vec3::new(o.emissive[0], o.emissive[1], o.emissive[2]) * o.emissive[3];
    let shown = |c: Vec3| untonemap(c.clamp(Vec3::ZERO, Vec3::ONE), exposure);
    let color = match shading.view {
        ViewMode::Diffuse => l.diffuse_lighting,
        ViewMode::Specular => l.specular_lighting,
        ViewMode::D => untonemap(heat(terms.d / (terms.d + 1.0)), exposure),
        ViewMode::G => untonemap(heat(terms.g * 4.0 * terms.n_dot_l * l.n_dot_v), exposure),
        ViewMode::F => shown(terms.f),
        ViewMode::Multiscatter => untonemap(heat((l.multi_scatter_compensation.y - 1.0) / 0.6), exposure),
        ViewMode::DiffuseResponse => shown(l.diffuse_response),
        ViewMode::Ndotl => untonemap(heat(terms.n_dot_l), exposure),
        ViewMode::Attenuation => untonemap(heat(if sample.falloff < 1.0 { sample.radius_attenuation } else { 0.0 }), exposure),
        ViewMode::IblDiffuse => ibl.diffuse,
        ViewMode::IblSpecular => ibl.specular,
        // The CPU mirror has no tiles, like the verification kernels.
        ViewMode::LightCount => untonemap(heat(0.0), exposure),
        ViewMode::ProbeStencil => untonemap(super::reflections::stencil_color(ibl.reflection.stencil & super::reflections::STENCIL_REFLECTION_MASK), exposure),
        ViewMode::ProbeWeight => untonemap(heat(ibl.reflection.blend_weight / 2.0), exposure),
        // The reflection passes test the IBL flag themselves; SSAO and the resolve are screen space.
        ViewMode::Final | ViewMode::Ssao | ViewMode::ResolveDifference => l.diffuse_lighting + l.specular_lighting + ibl.diffuse * ibl_on + ibl.specular + emissive,
    };
    Pixel { color, surface: s, lighting: l, light: sample, terms, ibl }
}

/// MSAAResolveCustom's luminance (CalcLuminance).
pub fn resolve_luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.299, 0.587, 0.114))
}

/// MSAAResolveCustom MainPS for one pixel, as MSAAResolveSpecularPass runs it:
/// each sample scaled down to at most `contrast` times the brightest
/// luminance of the diagonal neighbours' samples, then the mean taken in the
/// log domain (LOG_TONEMAPPING_STRATEGY).
pub fn msaa_resolve_custom(samples: &[Vec3], brightest_neighbor: f32, contrast: f32) -> Vec3 {
    let sum: Vec3 = samples
        .iter()
        .map(|&s| {
            let factor = contrast * brightest_neighbor / resolve_luminance(s);
            (s * 1.0f32.min(factor)).to_array().map(f32::ln).into()
        })
        .fold(Vec3::ZERO, |a, b: Vec3| a + b);
    (sum / samples.len() as f32).to_array().map(f32::exp).into()
}
