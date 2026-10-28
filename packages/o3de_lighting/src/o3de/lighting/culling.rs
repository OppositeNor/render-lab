//! The CPU mirror of O3DE's tiled light culling for simple point lights
//! (LightCulling.azsl and LightCullingRemap.azsl; NVLC.azsli), which the
//! selftest runs on the GPU's own tile data and compares with the GPU's
//! lists. Distances are positive view distances, as in the WGSL port.

use super::brdf::PointLight;
use super::decals::Decal;
use super::transmission::SphereLight;
use lab::layout::CameraData;
use glam::{Mat4, Vec2, Vec3};

pub const TILE_DIM: u32 = 16;
pub const MAX_LIGHTS_PER_BIN: u32 = 256;
pub const MAX_BINS: u32 = 8;
pub const BINS_MASK: u32 = 7;
pub const END_OF_LIST: u32 = 0xFFFF;
pub const END_OF_GROUP: u32 = 0xFFFE;
pub const ALL_BIN_BITS: u32 = 255;
/// Decals, simple point, simple spot, point, disk, capsule, and quad lights.
pub const LIGHT_GROUPS: u32 = 7;
pub const GROUP_DECALS: u32 = 0;
pub const GROUP_SIMPLE_POINT: u32 = 1;
pub const GROUP_POINT: u32 = 3;

/// NVLC.azsli: TileLightData, as Tile_UnpackData reads it.
#[derive(Clone, Copy, Debug)]
pub struct TileLightData {
    pub z_near: f32,
    pub z_far: f32,
    pub mask: u32,
    pub log_max_bins: u32,
}

impl TileLightData {
    /// The four words of a tile; the lost low bits are added back to zFar.
    pub fn unpack(words: &[u32]) -> TileLightData {
        TileLightData { z_near: f32::from_bits(words[0]), z_far: f32::from_bits(words[1] | BINS_MASK), mask: words[2], log_max_bins: words[1] & BINS_MASK }
    }

    /// NVLC_GetBin for a view distance.
    pub fn bin(&self, distance: f32) -> u32 {
        let f = ((distance - self.z_near) / (self.z_far - self.z_near)).clamp(0.0, 1.0);
        (f.min(0.999_999) * (1u32 << self.log_max_bins) as f32) as u32
    }
}

/// NVLC.azsli: Tile_UnitValueToBit.
fn unit_value_to_bit(x: f32) -> u32 {
    (x.clamp(0.0, 1.0) * f32::from_bits(32.0f32.to_bits() - 1)) as u32
}

/// NVLC.azsli: g_binMask0 and g_binMask1.
fn bin_mask(log_max_bins: u32, bin: u32) -> u32 {
    match log_max_bins {
        0 => if bin == 0 { u32::MAX } else { 0 },
        1 => if bin < 2 { 0xFFFF << (16 * bin) } else { 0 },
        2 => if bin < 4 { 0xFF << (8 * bin) } else { 0 },
        _ => 0xF << (4 * bin),
    }
}

/// NVLC.azsli: IsObjectInsideTile for a distance range; the bins it touches.
pub fn object_bins(data: &TileLightData, min_max: Vec2) -> Option<u32> {
    let normalized = (min_max - Vec2::splat(data.z_near)) / (data.z_far - data.z_near);
    let (ex, ey) = (1u32 << unit_value_to_bit(normalized.x), 1u32 << unit_value_to_bit(normalized.y));
    let object_mask = ((ey - 1) | ey) & !(ex - 1);
    let overlap = if normalized.x <= 1.0 && normalized.y >= 0.0 { data.mask & object_mask } else { 0 };
    let bins = (0..MAX_BINS).filter(|&b| overlap & bin_mask(data.log_max_bins, b) != 0).fold(0, |acc, b| acc | 1 << b);
    (overlap != 0).then_some(bins)
}

/// LightCulling BuildAabb: the view-space box through the tile's corners at
/// its near and far distances (centre, half extents).
pub fn tile_aabb(camera: &CameraData, data: &TileLightData, tile: [u32; 2]) -> (Vec3, Vec3) {
    let projection = Mat4::from_cols_array_2d(&camera.view_to_clip);
    let grid = Vec2::splat(TILE_DIM as f32) / Vec2::from(camera.viewport_size);
    let corner = |uv: Vec2, distance: f32| {
        let ndc = Vec2::new(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
        Vec3::new(ndc.x * distance / projection.x_axis.x, ndc.y * distance / projection.y_axis.y, -distance)
    };
    let uv_min = Vec2::new(tile[0] as f32, tile[1] as f32) * grid;
    let uv_max = uv_min + grid;
    let points = [corner(uv_min, data.z_near), corner(uv_min, data.z_far), corner(uv_max, data.z_near), corner(uv_max, data.z_far)];
    let lo = points.iter().fold(Vec3::splat(f32::MAX), |a, p| a.min(*p));
    let hi = points.iter().fold(Vec3::splat(f32::MIN), |a, p| a.max(*p));
    let center = (lo + hi) * 0.5;
    (center, center - lo)
}

/// LightCulling MainCS for one tile: the packed list, each group closed by
/// END_OF_GROUP; decals in SortDecals' order (sort key, then index), lights
/// in light-index order (as the lab's port appends them).
pub fn cull_tile(camera: &CameraData, data: &TileLightData, tile: [u32; 2], lights: &[PointLight], decals: &[Decal], sphere_lights: &[SphereLight]) -> Vec<u32> {
    let view = Mat4::from_cols_array_2d(&camera.world_to_view);
    let (center, half) = tile_aabb(camera, data, tile);
    let mut list = Vec::new();
    for group in 0..LIGHT_GROUPS {
        if group == GROUP_DECALS {
            for i in super::decals::sorted(decals) {
                let d = &decals[i];
                let c = view.transform_point3(d.position);
                // CullDecals: the draw distance, then a sphere around the box.
                if d.max_draw_distance > 0.0 && c.dot(c) > d.max_draw_distance * d.max_draw_distance {
                    continue;
                }
                let radius_squared = d.half_size.dot(d.half_size);
                let delta = ((center - c).abs() - half).max(Vec3::ZERO);
                if delta.dot(delta) < radius_squared {
                    let radius = radius_squared.sqrt();
                    if let Some(bins) = object_bins(data, Vec2::new(-c.z - radius, -c.z + radius)) {
                        list.push(((i as u32) << 16) | bins);
                    }
                }
            }
        }
        // CullPointLight for simple point lights and for point (sphere) lights.
        let spheres: Vec<(Vec3, f32)> = match group {
            GROUP_SIMPLE_POINT => lights.iter().map(|l| (l.position, l.inv_attenuation_radius_squared)).collect(),
            GROUP_POINT => sphere_lights.iter().map(|l| (l.position, l.inv_attenuation_radius_squared)).collect(),
            _ => Vec::new(),
        };
        for (i, &(position, inv_attenuation_radius_squared)) in spheres.iter().enumerate() {
            let c = view.transform_point3(position);
            // TestSphereVsAabbInvSqrt.
            let delta = ((center - c).abs() - half).max(Vec3::ZERO);
            if delta.dot(delta) * inv_attenuation_radius_squared < 1.0 {
                let radius = 1.0 / inv_attenuation_radius_squared.sqrt();
                if let Some(bins) = object_bins(data, Vec2::new(-c.z - radius, -c.z + radius)) {
                    list.push(((i as u32) << 16) | bins);
                }
            }
        }
        list.push((END_OF_GROUP << 16) | ALL_BIN_BITS);
    }
    list
}

/// The standard 4x MSAA sample positions, in pixels from the pixel's centre.
const MSAA4: [[f32; 2]; 4] = [[-0.125, -0.375], [0.375, -0.125], [-0.375, 0.125], [0.125, 0.375]];

/// A tile's data from ray casts at its pixels' 4x samples, as
/// LightCullingTilePrepare builds it from the depth buffer: the nearest
/// sample, the farthest one short of the sky (the sky counts as the near
/// plane), and one mask bit per sample. The raster's tessellated meshes can
/// differ from these analytic hits by millimetres.
pub fn cpu_tile(camera: &CameraData, objects: &[lab::layout::ObjectData], tile: [u32; 2]) -> TileLightData {
    let projector = lab::camera::Projector::new(camera);
    let view = Mat4::from_cols_array_2d(&camera.world_to_view);
    let size = Vec2::from(camera.viewport_size);
    let mut samples: Vec<Option<f32>> = Vec::new();
    for y in 0..TILE_DIM {
        for x in 0..TILE_DIM {
            let pixel = Vec2::new((tile[0] * TILE_DIM + x) as f32, (tile[1] * TILE_DIM + y) as f32);
            if pixel.x >= size.x || pixel.y >= size.y {
                continue;
            }
            for s in MSAA4 {
                let (o, d) = projector.ray(pixel + Vec2::splat(0.5) + Vec2::from(s));
                let hit = lab::scene::trace_scene(objects, o, d, 100.0, lab::scene::ids::PANEL as u32);
                samples.push(hit.is_hit().then(|| -view.transform_point3(o + d * hit.t).z));
            }
        }
    }
    let near_plane = camera.near_plane;
    let z_near = samples.iter().map(|z| z.unwrap_or(camera.far_plane)).fold(f32::MAX, f32::min);
    let z_far = f32::from_bits(samples.iter().map(|z| z.unwrap_or(near_plane)).fold(near_plane, f32::max).to_bits() + 1);
    let mask = samples.iter().flatten().fold(0, |m, z| m | 1 << unit_value_to_bit((z - z_near) / (z_far - z_near)));
    let log_max_bins = ((z_far - z_near).abs().log2().clamp(0.0, 3.0) + 0.5) as u32;
    // Packed and unpacked, so zFar keeps only its high bits, as on the GPU.
    TileLightData::unpack(&[z_near.to_bits(), (z_far.to_bits() & !BINS_MASK) | log_max_bins, mask, 0])
}

/// LightCullingRemap for one tile: per bin, the indices of the entries that
/// touch it, in list order, ending with END_OF_LIST.
pub fn remap(list: &[u32]) -> Vec<Vec<u32>> {
    let total = list.len().min(MAX_LIGHTS_PER_BIN as usize - 1);
    (0..MAX_BINS).map(|bin| list[..total].iter().filter(|e| *e & (1 << bin) != 0).map(|e| e >> 16).chain(std::iter::once(END_OF_LIST)).collect()).collect()
}
