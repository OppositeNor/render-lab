//! Decals: the records O3DE's decal feature processor uploads (Decal in
//! Decals/DecalStructures.azsli), the two the lab places on the floor, and
//! the CPU mirror of ApplyDecal (PBR/Decals.azsli). The forward pass applies
//! a pixel's decals, from its tile's first group, after the surface and
//! before InitializeLightingData.

use super::brdf::Surface;
use super::maps;
use crate::lab::shot::ShotState;
use glam::{Quat, Vec2, Vec3};

/// Decal (DecalStructures.azsli), with the decal feature processor's defaults
/// for what the lab does not set.
#[derive(Clone, Copy, Debug)]
pub struct Decal {
    pub position: Vec3,
    pub opacity: f32,
    pub rotation: Quat,
    pub half_size: Vec3,
    pub angle_attenuation: f32,
    pub normal_map_opacity: f32,
    /// m_sortKeyPacked & 0xFF: larger keys are applied later, on top.
    pub sort_key: u32,
    /// The lab's texture (m_textureArrayIndex and m_textureIndex in O3DE).
    pub texture: usize,
    pub color: Vec3,
    pub color_factor: f32,
    /// 0: no limit.
    pub max_draw_distance: f32,
}

impl Decal {
    /// The decal's local axes in the world: x and y span the texture, z is the projection axis.
    pub fn axes(&self) -> [Vec3; 3] {
        [self.rotation * Vec3::X, self.rotation * Vec3::Y, self.rotation * Vec3::Z]
    }
}

/// The scene's decals when `decals` is on: a paint splat and a panel grid on
/// the floor beside the sphere, overlapping, projecting straight down. Their
/// half sizes are what a decal entity's non-uniform scale gives
/// (SetDecalHalfSize); the grid keeps the default sort key.
pub fn scene_decals(state: &ShotState) -> Vec<Decal> {
    if state.p("decals") < 0.5 {
        return Vec::new();
    }
    // Local z to the world's up.
    let down = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let turn = Quat::from_rotation_y(state.p("decalTurn").to_radians());
    let tilt = Quat::from_rotation_x(state.p("decalTilt").to_radians());
    let base = Decal {
        position: Vec3::ZERO,
        opacity: state.p("decalOpacity"),
        rotation: down,
        half_size: Vec3::new(0.45, 0.45, 0.25),
        angle_attenuation: state.p("decalAngleAttenuation"),
        normal_map_opacity: state.p("decalNormalOpacity"),
        // DecalComponentConfig::DefaultDecalSortKey.
        sort_key: 16,
        texture: 0,
        color: Vec3::ONE,
        color_factor: 1.0,
        max_draw_distance: 0.0,
    };
    vec![
        Decal { position: Vec3::new(-0.35, 0.0, 0.45), rotation: tilt * down, sort_key: state.p("decalSplatSortKey").round().clamp(0.0, 255.0) as u32, texture: 0, ..base },
        Decal { position: Vec3::new(-0.95, 0.0, 0.7), rotation: turn * down, half_size: Vec3::new(0.55, 0.55, 0.25), texture: 1, ..base },
    ]
}

/// LightCulling SortDecals' order: by sort key, then by index.
pub fn sorted(decals: &[Decal]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..decals.len()).collect();
    order.sort_by_key(|&i| (decals[i].sort_key, i));
    order
}

/// The decals gShadingFrame has room for.
pub const MAX_DECALS: usize = 8;

/// The frame's decal records (gShadingFrame from FRAME_DECALS): the count,
/// two vectors of the sorted order, then five vectors per decal.
pub fn frame_records(decals: &[Decal]) -> Vec<[f32; 4]> {
    assert!(decals.len() <= MAX_DECALS);
    let order = sorted(decals);
    let at = |k: usize| order.get(k).map_or(0.0, |&i| i as f32);
    let mut out = vec![[decals.len() as f32, 0.0, 0.0, 0.0], [at(0), at(1), at(2), at(3)], [at(4), at(5), at(6), at(7)]];
    for d in decals {
        out.push(d.position.extend(d.opacity).to_array());
        out.push(d.rotation.to_array());
        out.push(d.half_size.extend(d.angle_attenuation).to_array());
        out.push([d.normal_map_opacity, d.sort_key as f32, d.texture as f32, d.max_draw_distance]);
        out.push(d.color.extend(d.color_factor).to_array());
    }
    out
}

/// GetDecalAttenuation: none facing the decal's axis, fading out to 0.55.
pub fn decal_attenuation(surface_normal: Vec3, decal_up: Vec3, angle_attenuation: f32) -> f32 {
    let clamp_angle = 0.55;
    let d = surface_normal.dot(decal_up).clamp(clamp_angle, 1.0);
    let t = (d - clamp_angle) / (1.0 - clamp_angle);
    let smooth = t * t * (3.0 - 2.0 * t);
    1.0 + (smooth - 1.0) * angle_attenuation
}

/// What ApplyDecal computes for a point inside a decal's box.
#[derive(Clone, Copy, Debug)]
pub struct DecalSample {
    /// decalUVW before its y flip, from -1 to 1 inside the box.
    pub uvw: Vec3,
    pub base: [f32; 4],
    pub attenuation: f32,
    pub albedo_opacity: f32,
    pub normal_opacity: f32,
    /// BlendBaseColor's result, and the normal map's normal in the world.
    pub blended: Vec3,
    pub normal: Vec3,
}

/// The decal at `position`, or none outside its box.
pub fn sample(d: &Decal, position: Vec3, vertex_normal: Vec3) -> Option<DecalSample> {
    let [x, y, z] = d.axes();
    let local = position - d.position;
    let uvw = Vec3::new(x.dot(local), y.dot(local), z.dot(local)) / d.half_size;
    if uvw.abs().max_element() > 1.0 {
        return None;
    }
    let uv = Vec2::new(uvw.x, -uvw.y) * 0.5 + Vec2::splat(0.5);
    let base = maps::sample_decal(d.texture, false, uv);
    let normal = maps::sample_decal(d.texture, true, uv);
    let attenuation = decal_attenuation(vertex_normal, z, d.angle_attenuation);
    // BlendBaseColor, TextureBlendMode::Multiply.
    let blended = d.color_factor * d.color * Vec3::new(base[0], base[1], base[2]);
    // GetTangentSpaceNormal_Unnormalized swaps the channels and rebuilds z.
    let ts = Vec3::new(normal[1], normal[0], (1.0 - normal[0] * normal[0] - normal[1] * normal[1]).max(0.0).sqrt().max(0.01));
    Some(DecalSample {
        uvw,
        base,
        attenuation,
        albedo_opacity: base[3] * d.opacity * attenuation,
        normal_opacity: base[3] * d.normal_map_opacity * attenuation,
        blended,
        normal: (x * ts.x + y * ts.y + z * ts.z).normalize(),
    })
}

/// ApplyDecal: inside the decal's box, the base map multiplied by the decal
/// colour replaces the albedo by its opacity (and F0, scaled by metallic),
/// and the normal map bends the normal.
pub fn apply_decal(d: &Decal, s: &mut Surface, vertex_normal: Vec3) {
    let Some(k) = sample(d, s.position, vertex_normal) else { return };
    s.albedo = s.albedo.lerp(k.blended, k.albedo_opacity);
    s.specular_f0 = s.specular_f0.lerp(k.blended, k.albedo_opacity * s.metallic);
    s.normal = s.normal.lerp(k.normal, k.normal_opacity).normalize();
}
