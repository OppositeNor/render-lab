//! Transmission (PBR/BackLighting.azsli, EnhancedPBR's
//! SubsurfaceAndTransmissionPropertyGroup) and the O3DE point light it needs,
//! the sphere area light (Lights/PointLight.azsli,
//! Pipeline/Forward/ForwardPassPointLights.azsli): simple point lights do no
//! back lighting. The CPU mirror of the shader's getBackLighting and
//! applySphereLight.

use super::brdf::{saturate, DiffuseModel, LightingData, Surface};
use glam::{Vec3, Vec4};

/// o_transmission_mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransmissionMode {
    ThickObject = 1,
    ThinObject = 2,
}

/// TransmissionSurfaceData, as SubsurfaceTransmissionParameterFunctor fills it.
#[derive(Clone, Copy, Debug)]
pub struct Transmission {
    pub mode: TransmissionMode,
    pub tint: Vec3,
    pub thickness: f32,
    /// Thick: (attenuation coefficient, power, distortion, scale); thin:
    /// (shrink factor, NdotL bias, distance attenuation, scale).
    pub params: Vec4,
    /// scatterColor times scatterDistance.
    pub scatter_distance: Vec3,
}

/// The frame's transmission record (gShadingFrame from FRAME_TRANSMISSION) for `object`.
pub fn frame_record(t: Option<&Transmission>, object: usize) -> [[f32; 4]; 4] {
    match t {
        Some(t) => [[t.mode as u32 as f32, t.thickness, object as f32, 0.0], t.params.to_array(), t.tint.extend(0.0).to_array(), t.scatter_distance.extend(0.0).to_array()],
        None => [[0.0; 4]; 4],
    }
}

/// PointLight (Lights/LightStructures.azsli): the fields the lab sets.
#[derive(Clone, Copy, Debug)]
pub struct SphereLight {
    pub position: Vec3,
    pub inv_attenuation_radius_squared: f32,
    pub rgb_intensity_candelas: Vec3,
    pub bulb_radius: f32,
    pub lighting_channel_mask: u32,
}

/// The sphere lights' records (gShadingFrame from FRAME_SPHERE_LIGHTS).
pub fn frame_lights(lights: &[SphereLight]) -> Vec<[f32; 4]> {
    let mut out = vec![[lights.len() as f32, 0.0, 0.0, 0.0]];
    for l in lights {
        out.push(l.position.extend(l.inv_attenuation_radius_squared).to_array());
        out.push(l.rgb_intensity_candelas.extend(l.lighting_channel_mask as f32).to_array());
        out.push([l.bulb_radius, 0.0, 0.0, 0.0]);
    }
    out
}

/// TransmissionKernel.
pub fn transmission_kernel(t: f32, s: Vec3) -> Vec3 {
    let exponent = s * t;
    0.25 * (Vec3::ONE / exponent.exp() + 3.0 / (exponent / 3.0).exp())
}

/// What GetBackLighting's thick-object branch computes.
#[derive(Clone, Copy, Debug)]
pub struct ThickTerms {
    /// saturate(V . -normalize(L + N distortion)).
    pub v_dot: f32,
    pub transmittance: f32,
    pub lamber_attenuation: f32,
}

pub fn thick_terms(t: &Transmission, vertex_normal: Vec3, dir_to_camera: Vec3, dir_to_light: Vec3, transmission_distance: f32) -> ThickTerms {
    let thickness = transmission_distance.max(t.thickness);
    let v_dot = saturate(dir_to_camera.dot(-(dir_to_light + vertex_normal * t.params.z).normalize()));
    ThickTerms { v_dot, transmittance: v_dot.powf(t.params.y) * t.params.w, lamber_attenuation: (-thickness * t.params.x).exp() * saturate(1.0 - thickness) }
}

/// GetBackLighting_Default (USE_HUMAN_SKIN_PROFILE off).
pub fn back_lighting(s: &Surface, l: &LightingData, light_intensity: Vec3, dir_to_light: Vec3, transmission_distance: f32, attenuation_distance: f32) -> Vec3 {
    let Some(t) = &s.transmission else { return Vec3::ZERO };
    match t.mode {
        TransmissionMode::ThickObject => {
            let k = thick_terms(t, s.vertex_normal, l.dir_to_camera, dir_to_light, transmission_distance);
            k.transmittance * k.lamber_attenuation * light_intensity
        }
        TransmissionMode::ThinObject => {
            // A negative distance means no shadow data.
            if transmission_distance < 0.0 {
                return Vec3::ZERO;
            }
            let e = s.albedo * saturate(t.params.y + (-s.vertex_normal).dot(dir_to_light));
            let distance = transmission_distance * 300.0 * t.thickness;
            let inv_scattering = Vec3::ONE / t.scatter_distance.max(Vec3::splat(0.00001));
            transmission_kernel(distance, inv_scattering) * light_intensity * e * t.params.w / (attenuation_distance * attenuation_distance * t.params.z).max(1.0)
        }
    }
}

/// GetIntensityAdjustedByRadiusAndRoughness.
pub fn intensity_adjusted_by_radius_and_roughness(roughness_a: f32, radius: f32, distance2: f32) -> f32 {
    let roughness_adjusted = saturate(roughness_a + radius / (3.0 * distance2.sqrt()));
    let normalization = roughness_a.max(0.001) / roughness_adjusted;
    normalization * normalization
}

/// ApplyPointLight and PointLightUtil_PBR::Apply, litRatio 1: the shadow branch
/// runs (its shader options default to true) and a light without shadows
/// gives a transmission distance of 0 (GetThickness).
pub fn apply_sphere_light(light: &SphereLight, surface_channels: u32, s: &Surface, l: &mut LightingData, model: DiffuseModel) {
    if light.lighting_channel_mask & surface_channels == 0 {
        return;
    }
    let surface_to_light = light.position - s.position;
    let dir = surface_to_light.normalize();
    let mut distance_squared = surface_to_light.dot(surface_to_light);
    let falloff = distance_squared * light.inv_attenuation_radius_squared;
    if falloff >= 1.0 {
        return;
    }
    let mut radius_attenuation = 1.0 - falloff * falloff;
    radius_attenuation *= radius_attenuation;
    distance_squared = distance_squared.max(0.001 * 0.001);
    let intensity = light.rgb_intensity_candelas / distance_squared * radius_attenuation;
    l.diffuse_lighting += super::brdf::diffuse_light(s, l, intensity, dir, model);
    l.translucent_back_lighting += back_lighting(s, l, intensity, dir, 0.0, surface_to_light.length());
    let reflection = lab::scene::reflect(-l.dir_to_camera, s.normal);
    let reflection_to_light = surface_to_light - surface_to_light.dot(reflection) * reflection;
    let adjusted = surface_to_light - reflection_to_light * saturate(light.bulb_radius / reflection_to_light.length());
    let normalization = intensity_adjusted_by_radius_and_roughness(s.roughness_a, light.bulb_radius, distance_squared);
    l.specular_lighting += normalization * super::brdf::specular_light(s, l, intensity, adjusted.normalize());
}
