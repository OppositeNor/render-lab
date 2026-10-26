//! The study room, its ObjectData records, and CPU mirrors of the scene
//! math in shaders/wgsl/common.wgsl. The mirrors drive picking, the debug
//! ray, probe tracing, and the prepared Surface Cache texels; `selftest`
//! compares them with the GPU versions.
//!
//! Coordinates: right-handed, +Y up, metres. Some engines are left-handed, +Z up,
//! and use centimetres.

use crate::lab::layout::ObjectData;
use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};

pub const SHAPE_PLANE: u32 = 0;
pub const SHAPE_BOX: u32 = 1;
pub const SHAPE_SPHERE: u32 = 2;
pub const SHAPE_PANEL: u32 = 3;
/// The Utah teapot's triangles (src/lab/mesh.rs); local bounds are its box.
pub const SHAPE_MESH: u32 = 4;
pub const NO_HIT: u32 = u32::MAX;
pub const PI: f32 = std::f32::consts::PI;

pub mod ids {
    pub const FLOOR: usize = 0;
    pub const BACK_WALL: usize = 1;
    pub const LEFT_WALL: usize = 2;
    pub const RIGHT_WALL: usize = 3;
    pub const PANEL: usize = 4;
    pub const SPHERE: usize = 5;
    pub const BOX: usize = 6;
    pub const SMALL_BOX: usize = 7;
    pub const TEAPOT: usize = 8;
}

#[derive(Clone, Debug)]
pub struct SceneObject {
    pub name: &'static str,
    pub shape: u32,
    pub local_center: Vec3,
    pub local_extent: Vec3,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub albedo: Vec3,
    pub roughness: f32,
    pub emissive: Vec3,
    pub emissive_strength: f32,
}

impl SceneObject {
    pub fn local_to_world(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub center: Vec3,
    pub axis_u: Vec3,
    pub axis_v: Vec3,
    pub color: Vec3,
    pub radiance: f32,
}

impl Light {
    pub fn normal(&self) -> Vec3 {
        self.axis_v.cross(self.axis_u).normalize()
    }
}

/// Lighting constants shared with the shaders through FrameUniforms.
#[derive(Clone, Copy, Debug)]
pub struct Lighting {
    pub light: Light,
    pub ambient: Vec3,
    pub f0: f32,
    pub reflection_strength: f32,
    /// GGX reflection rays per floor pixel.
    pub ggx_samples: u32,
    pub exposure: f32,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub objects: Vec<SceneObject>,
    pub lighting: Lighting,
}

fn deg(v: f32) -> f32 {
    v.to_radians()
}

impl Scene {
    pub fn study_room() -> Scene {
        let plane = |name, center: Vec3, rotation: Quat, ex: f32, ez: f32, albedo: Vec3| SceneObject {
            name,
            shape: SHAPE_PLANE,
            local_center: Vec3::ZERO,
            local_extent: Vec3::new(ex, 0.0, ez),
            translation: center,
            rotation,
            scale: Vec3::ONE,
            albedo,
            roughness: 1.0,
            emissive: Vec3::ZERO,
            emissive_strength: 0.0,
        };
        let light_color = Vec3::new(1.0, 0.88, 0.72);
        let light_radiance = 6.0;
        // A ceiling light: the panel's local +Y normal turned to face down.
        let panel_rotation = Quat::from_rotation_x(deg(180.0));
        let panel_center = Vec3::new(0.0, 2.95, -0.2);
        let panel_extent = Vec3::new(1.2, 0.0, 0.6);
        let mut objects = vec![
            plane("floor", Vec3::ZERO, Quat::IDENTITY, 3.0, 3.0, Vec3::new(0.16, 0.17, 0.19)),
            plane("back wall", Vec3::new(0.0, 1.5, -3.0), Quat::from_rotation_x(deg(90.0)), 3.0, 1.5, Vec3::new(0.72, 0.70, 0.66)),
            plane("left wall", Vec3::new(-3.0, 1.5, 0.0), Quat::from_rotation_z(deg(-90.0)), 1.5, 3.0, Vec3::new(0.70, 0.24, 0.18)),
            plane("right wall", Vec3::new(3.0, 1.5, 0.0), Quat::from_rotation_z(deg(90.0)), 1.5, 3.0, Vec3::new(0.16, 0.55, 0.50)),
            SceneObject {
                name: "emissive panel",
                shape: SHAPE_PANEL,
                local_center: Vec3::ZERO,
                local_extent: panel_extent,
                translation: panel_center,
                rotation: panel_rotation,
                scale: Vec3::ONE,
                albedo: Vec3::splat(0.8),
                roughness: 1.0,
                emissive: light_color,
                emissive_strength: light_radiance,
            },
            SceneObject {
                name: "sphere",
                shape: SHAPE_SPHERE,
                local_center: Vec3::ZERO,
                local_extent: Vec3::splat(0.62),
                translation: Vec3::new(-1.05, 0.62, -0.55),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                albedo: Vec3::new(0.86, 0.82, 0.74),
                roughness: 0.5,
                emissive: Vec3::ZERO,
                emissive_strength: 0.0,
            },
            SceneObject {
                name: "rotated box",
                shape: SHAPE_BOX,
                local_center: Vec3::ZERO,
                local_extent: Vec3::new(0.5, 0.45, 0.38),
                translation: Vec3::new(1.05, 0.45, 0.15),
                rotation: Quat::from_rotation_y(deg(30.0)),
                scale: Vec3::ONE,
                albedo: Vec3::new(0.22, 0.42, 0.85),
                roughness: 0.6,
                emissive: Vec3::ZERO,
                emissive_strength: 0.0,
            },
            SceneObject {
                name: "small box",
                shape: SHAPE_BOX,
                local_center: Vec3::ZERO,
                local_extent: Vec3::splat(0.24),
                translation: Vec3::new(1.95, 0.24, 1.45),
                rotation: Quat::from_rotation_y(deg(20.0)),
                scale: Vec3::ONE,
                albedo: Vec3::new(0.92, 0.55, 0.22),
                roughness: 0.6,
                emissive: Vec3::ZERO,
                emissive_strength: 0.0,
            },
            // The bounds demonstration's object: the spout reaches farther
            // than the handle, so the local bounds' centre is off the pivot.
            SceneObject {
                name: "teapot",
                shape: SHAPE_MESH,
                local_center: crate::lab::mesh::teapot().local_center(),
                local_extent: crate::lab::mesh::teapot().local_extent(),
                translation: Vec3::new(-0.25, 0.0, -1.75),
                rotation: Quat::from_rotation_y(deg(30.0)),
                scale: Vec3::ONE,
                albedo: Vec3::new(0.50, 0.66, 0.56),
                roughness: 0.35,
                emissive: Vec3::ZERO,
                emissive_strength: 0.0,
            },
        ];
        objects.shrink_to_fit();
        let light = Light {
            center: panel_center,
            axis_u: panel_rotation * Vec3::new(panel_extent.x, 0.0, 0.0),
            axis_v: panel_rotation * Vec3::new(0.0, 0.0, panel_extent.z),
            color: light_color,
            radiance: light_radiance,
        };
        Scene {
            objects,
            lighting: Lighting {
                light,
                ambient: Vec3::new(0.060, 0.070, 0.090),
                f0: 0.04,
                reflection_strength: 1.0,
                ggx_samples: 16,
                exposure: 1.0,
            },
        }
    }

    pub fn records(&self) -> Vec<ObjectData> {
        self.objects.iter().enumerate().map(|(i, o)| object_record(o, i as u32)).collect()
    }
}

/// The standard bound of a transformed box (Arvo, "Transforming Axis-Aligned
/// Bounding Boxes", Graphics Gems, 1990): transform the centre, and build the
/// new extent from absolute matrix terms. With column vectors the extent's
/// component i is sum_j |M[i][j]| * extent_j.
pub fn transform_box(m: Mat4, center: Vec3, extent: Vec3) -> (Vec3, Vec3) {
    let a = Mat3::from_mat4(m);
    let world_center = m.transform_point3(center);
    let abs = Mat3::from_cols(a.x_axis.abs(), a.y_axis.abs(), a.z_axis.abs());
    (world_center, abs * extent)
}

/// Brute-force reference: transform the eight corners and take min/max.
pub fn transform_box_corners(m: Mat4, center: Vec3, extent: Vec3) -> (Vec3, Vec3) {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for i in 0..8 {
        let s = Vec3::new(
            if i & 1 != 0 { 1.0 } else { -1.0 },
            if i & 2 != 0 { 1.0 } else { -1.0 },
            if i & 4 != 0 { 1.0 } else { -1.0 },
        );
        let p = m.transform_point3(center + s * extent);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    ((lo + hi) * 0.5, (hi - lo) * 0.5)
}

pub fn object_record(o: &SceneObject, index: u32) -> ObjectData {
    let m = o.local_to_world();
    let inv = m.inverse();
    let (world_center, world_extent) = transform_box(m, o.local_center, o.local_extent);
    let local_radius = if o.shape == SHAPE_SPHERE { o.local_extent.x } else { o.local_extent.length() };
    // The bounding sphere's radius scales by the longest basis vector, then is
    // clamped to the new box's half diagonal.
    let a = Mat3::from_mat4(m);
    let max_axis = a.x_axis.length().max(a.y_axis.length()).max(a.z_axis.length());
    let radius = (local_radius * max_axis).min(world_extent.length());
    ObjectData {
        local_to_world: m.to_cols_array_2d(),
        world_to_local: inv.to_cols_array_2d(),
        local_center: o.local_center.to_array(),
        bounding_radius: radius,
        local_extent: o.local_extent.to_array(),
        object_index: index,
        world_center: world_center.to_array(),
        shape_kind: o.shape,
        world_extent: world_extent.to_array(),
        _pad0: 0.0,
        albedo_roughness: [o.albedo.x, o.albedo.y, o.albedo.z, o.roughness],
        emissive: [o.emissive.x, o.emissive.y, o.emissive.z, o.emissive_strength],
    }
}

// ---------------------------------------------------------------------------
// CPU mirrors of common.wgsl

fn m4(o: &[[f32; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array_2d(o)
}

pub fn normal_to_world(o: &ObjectData, n_local: Vec3) -> Vec3 {
    (Mat3::from_mat4(m4(&o.world_to_local)).transpose() * n_local).normalize()
}

pub fn occludes_light(o: &ObjectData) -> bool {
    o.shape_kind == SHAPE_BOX || o.shape_kind == SHAPE_SPHERE || o.shape_kind == SHAPE_MESH
}

/// Distance from `p` to an object's surface, and the closest surface point,
/// in world space. Closed shapes (sphere, boxes) are signed, negative
/// inside; planes, the panel, and the open teapot mesh are unsigned.
pub fn object_distance(o: &ObjectData, p: Vec3) -> (f32, Vec3) {
    let l2w = m4(&o.local_to_world);
    let lp = m4(&o.world_to_local).transform_point3(p);
    let c = Vec3::from(o.local_center);
    let e = Vec3::from(o.local_extent);
    let (q, inside) = match o.shape_kind {
        SHAPE_SPHERE => {
            let r = lp - c;
            (c + r.normalize_or(Vec3::X) * e.x, r.length() < e.x)
        }
        SHAPE_BOX => {
            let r = lp - c;
            let inside = r.abs().cmple(e).all();
            let q = if inside {
                // Push out through the nearest face.
                let gap = e - r.abs();
                let mut q = r;
                let axis = if gap.x <= gap.y && gap.x <= gap.z { 0 } else if gap.y <= gap.z { 1 } else { 2 };
                q[axis] = e[axis] * r[axis].signum();
                q
            } else {
                r.clamp(-e, e)
            };
            (c + q, inside)
        }
        SHAPE_MESH => (crate::lab::mesh::teapot().closest_point(lp).1, false),
        _ => (Vec3::new(lp.x.clamp(c.x - e.x, c.x + e.x), c.y, lp.z.clamp(c.z - e.z, c.z + e.z)), false),
    };
    let world = l2w.transform_point3(q);
    let d = (world - p).length();
    (if inside { -d } else { d }, world)
}

pub fn intersect_object(o: &ObjectData, ro: Vec3, rd: Vec3, t_max: f32) -> Option<(f32, Vec3)> {
    let w2l = m4(&o.world_to_local);
    let lo = w2l.transform_point3(ro);
    let ld = w2l.transform_vector3(rd);
    let c = Vec3::from(o.local_center);
    let e = Vec3::from(o.local_extent);
    match o.shape_kind {
        SHAPE_SPHERE => {
            let oc = lo - c;
            let a = ld.dot(ld);
            let b = oc.dot(ld);
            let k = oc.dot(oc) - e.x * e.x;
            let disc = b * b - a * k;
            if disc < 0.0 {
                return None;
            }
            let s = disc.sqrt();
            let mut t = (-b - s) / a;
            if t < 1e-4 {
                t = (-b + s) / a;
            }
            if t < 1e-4 || t >= t_max {
                return None;
            }
            Some((t, normal_to_world(o, (lo + ld * t - c) / e.x)))
        }
        SHAPE_BOX => {
            let safe = Vec3::select(ld.abs().cmplt(Vec3::splat(1e-9)), Vec3::splat(1e-9), ld);
            let inv = Vec3::ONE / safe;
            let t0 = (c - e - lo) * inv;
            let t1 = (c + e - lo) * inv;
            let tn3 = t0.min(t1);
            let tf3 = t0.max(t1);
            let tn = tn3.x.max(tn3.y).max(tn3.z);
            let tf = tf3.x.min(tf3.y).min(tf3.z);
            if tf < tn {
                return None;
            }
            let t = if tn > 1e-4 { tn } else { tf };
            if t < 1e-4 || t >= t_max {
                return None;
            }
            let p = (lo + ld * t - c) / e;
            let a = p.abs();
            let n_local = if a.x >= a.y && a.x >= a.z {
                Vec3::new(p.x.signum(), 0.0, 0.0)
            } else if a.y >= a.z {
                Vec3::new(0.0, p.y.signum(), 0.0)
            } else {
                Vec3::new(0.0, 0.0, p.z.signum())
            };
            Some((t, normal_to_world(o, n_local)))
        }
        SHAPE_MESH => {
            let (t, n_local) = crate::lab::mesh::teapot().intersect(lo, ld, t_max)?;
            // The mesh is open (lid gap, spout): face the normal toward the ray.
            let n = normal_to_world(o, n_local);
            Some((t, if n.dot(rd) > 0.0 { -n } else { n }))
        }
        _ => {
            if ld.y.abs() < 1e-9 {
                return None;
            }
            let t = (c.y - lo.y) / ld.y;
            if t < 1e-4 || t >= t_max {
                return None;
            }
            let p = lo + ld * t;
            if (p.x - c.x).abs() > e.x || (p.z - c.z).abs() > e.z {
                return None;
            }
            let mut n = normal_to_world(o, Vec3::Y);
            if n.dot(rd) > 0.0 {
                n = -n;
            }
            Some((t, n))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub t: f32,
    pub object: u32,
    pub normal: Vec3,
}

impl Hit {
    pub fn is_hit(&self) -> bool {
        self.object != NO_HIT
    }
}

pub fn trace_scene(objects: &[ObjectData], ro: Vec3, rd: Vec3, t_max: f32, skip: u32) -> Hit {
    let mut hit = Hit { t: t_max, object: NO_HIT, normal: Vec3::Y };
    for (i, o) in objects.iter().enumerate() {
        if i as u32 == skip {
            continue;
        }
        if let Some((t, n)) = intersect_object(o, ro, rd, hit.t) {
            hit = Hit { t, object: i as u32, normal: n };
        }
    }
    hit
}

pub fn trace_one(objects: &[ObjectData], index: usize, ro: Vec3, rd: Vec3, t_max: f32) -> Option<(f32, Vec3)> {
    intersect_object(&objects[index], ro, rd, t_max)
}

fn shadowed(objects: &[ObjectData], p: Vec3, dir: Vec3, dist: f32, self_object: u32) -> bool {
    objects.iter().enumerate().any(|(i, o)| {
        if i as u32 == self_object || !occludes_light(o) {
            return false;
        }
        if o.shape_kind == SHAPE_MESH {
            // Any triangle in range blocks the light, as in shadowed() in common.wgsl.
            let w2l = m4(&o.world_to_local);
            return crate::lab::mesh::teapot().occludes(w2l.transform_point3(p), w2l.transform_vector3(dir), dist);
        }
        intersect_object(o, p, dir, dist).is_some()
    })
}

/// The 16 points of the ceiling panel that direct_irradiance samples (a 4×4 grid).
pub fn light_samples(lighting: &Lighting) -> Vec<Vec3> {
    let l = &lighting.light;
    (0..16).map(|k| l.center + l.axis_u * (((k % 4) as f32 + 0.5) / 2.0 - 1.0) + l.axis_v * (((k / 4) as f32 + 0.5) / 2.0 - 1.0)).collect()
}

/// The nearest light blocker along a shadow ray, where `shadowed` only asks whether there is one.
pub fn shadow_blocker(objects: &[ObjectData], p: Vec3, dir: Vec3, dist: f32, self_object: u32) -> Option<f32> {
    objects
        .iter()
        .enumerate()
        .filter(|(i, o)| *i as u32 != self_object && occludes_light(o))
        .filter_map(|(_, o)| intersect_object(o, p, dir, dist).map(|h| h.0))
        .min_by(f32::total_cmp)
}

pub fn direct_irradiance(objects: &[ObjectData], lighting: &Lighting, p: Vec3, n: Vec3, self_object: u32) -> Vec3 {
    let l = &lighting.light;
    let nl = l.normal();
    let area = 4.0 * l.axis_u.length() * l.axis_v.length();
    let origin = p + n * 2e-3;
    let mut sum = 0.0f32;
    for j in 0..4 {
        for i in 0..4 {
            let s = l.center + l.axis_u * ((i as f32 + 0.5) / 2.0 - 1.0) + l.axis_v * ((j as f32 + 0.5) / 2.0 - 1.0);
            let mut w = s - origin;
            let d2 = w.dot(w);
            let d = d2.sqrt();
            w /= d;
            let cos_s = n.dot(w);
            let cos_l = nl.dot(-w);
            if cos_s <= 0.0 || cos_l <= 0.0 {
                continue;
            }
            if shadowed(objects, origin, w, d - 1e-3, self_object) {
                continue;
            }
            sum += cos_s * cos_l / d2;
        }
    }
    l.color * l.radiance * sum * area / 16.0
}

pub fn environment_radiance(lighting: &Lighting, dir: Vec3) -> Vec3 {
    lighting.ambient * (0.35 + 0.25 * dir.y.clamp(0.0, 1.0))
}

/// WGSL reflect(e1, e2) = e1 - 2 * dot(e2, e1) * e2 (same as HLSL).
pub fn reflect(i: Vec3, n: Vec3) -> Vec3 {
    i - 2.0 * n.dot(i) * n
}

// Reflections (the CPU side of common.wgsl's reflection section). The parts
// that follow an engine's code are in src/commercial/lab/reflection_rays.rs,
// mirroring src/commercial/shaders/surface_lookup.wgsl.
#[cfg(commercial)]
pub use crate::lab::reflection_rays::*;

/// A visible GGX normal by spherical caps over the bounded range, tangent space
/// (Dupuy & Benyoub 2023; Eto & Tokuyoshi 2023), as importanceSampleVisibleGGX in common.wgsl.
pub fn importance_sample_visible_ggx(e: Vec2, alpha: f32, v: Vec3) -> Vec3 {
    let vh = Vec3::new(alpha * v.x, alpha * v.y, v.z).normalize();
    let phi = 2.0 * PI * e.x;
    let a = alpha.clamp(0.0, 1.0);
    let s = 1.0 + Vec2::new(v.x, v.y).length();
    let (a2, s2) = (a * a, s * s);
    let k = (s2 - a2 * s2) / (s2 + a2 * v.z * v.z);
    let z = 1.0 + (-k * vh.z - 1.0) * e.y;
    let sin_theta = (1.0 - z * z).clamp(0.0, 1.0).sqrt();
    let h = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), z) + vh;
    Vec3::new(alpha * h.x, alpha * h.y, h.z.max(0.0)).normalize()
}

pub fn tangent_basis(n: Vec3) -> [Vec3; 3] {
    let up = if n.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
    let t = up.cross(n).normalize();
    [t, n.cross(t), n]
}

/// The k-th of `count` Hammersley points (sampleSequence without the per-pixel rotation).
pub fn hammersley(k: u32, count: u32) -> Vec2 {
    Vec2::new((k as f32 + 0.5) / count as f32, k.reverse_bits() as f32 * 2.328_306_4e-10)
}

/// Karis's analytic environment BRDF fit ("Physically Based Shading on Mobile", 2014), with a grey F0.
pub fn env_brdf_approx(f0: f32, roughness: f32, no_v: f32) -> f32 {
    let c0 = Vec4::new(-1.0, -0.0275, -0.572, 0.022);
    let c1 = Vec4::new(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = (r.x * r.x).min((-9.28 * no_v).exp2()) * r.x + r.y;
    let ab = Vec2::new(-1.04, 1.04) * a004 + Vec2::new(r.z, r.w);
    f0 * ab.x + (50.0 * f0).clamp(0.0, 1.0) * ab.y
}

pub fn tonemap(x: Vec3, exposure: f32) -> Vec3 {
    let x = x * exposure;
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(Vec3::ZERO, Vec3::ONE)
}

pub fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// Display colour (sRGB bytes) of an HDR radiance, as the composite pass computes it.
pub fn display_rgb(radiance: Vec3, exposure: f32) -> [u8; 3] {
    let t = tonemap(radiance, exposure);
    [t.x, t.y, t.z].map(|v| (linear_to_srgb(v) * 255.0 + 0.5) as u8)
}

// ---------------------------------------------------------------------------
// Raster meshes, in each object's local space.

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

pub struct Meshes {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// Index range of each object's mesh.
    pub ranges: Vec<std::ops::Range<u32>>,
}

pub fn build_meshes(scene: &Scene) -> Meshes {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut ranges = Vec::new();
    for o in &scene.objects {
        let first = indices.len() as u32;
        let base = vertices.len() as u32;
        let c = o.local_center;
        let e = o.local_extent;
        match o.shape {
            SHAPE_SPHERE => {
                let (rings, segments) = (40u32, 80u32);
                for r in 0..=rings {
                    let v = r as f32 / rings as f32;
                    let theta = v * PI;
                    for s in 0..=segments {
                        let u = s as f32 / segments as f32;
                        let phi = u * 2.0 * PI;
                        let n = Vec3::new(theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin());
                        vertices.push(Vertex { position: (c + n * e.x).to_array(), normal: n.to_array() });
                    }
                }
                for r in 0..rings {
                    for s in 0..segments {
                        let a = base + r * (segments + 1) + s;
                        let b = a + segments + 1;
                        indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
                    }
                }
            }
            SHAPE_BOX => {
                for axis in 0..3 {
                    for side in [-1.0f32, 1.0] {
                        let mut n = Vec3::ZERO;
                        n[axis] = side;
                        let t = if axis == 0 { Vec3::Y } else { Vec3::X };
                        let b = n.cross(t);
                        let start = vertices.len() as u32;
                        for (du, dv) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                            let p = c + (n + t * du + b * dv) * e;
                            vertices.push(Vertex { position: p.to_array(), normal: n.to_array() });
                        }
                        indices.extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
                    }
                }
            }
            SHAPE_MESH => {
                for tri in &crate::lab::mesh::teapot().triangles {
                    for (p, n) in [(tri.p0, tri.n0), (tri.p1, tri.n1), (tri.p2, tri.n2)] {
                        indices.push(vertices.len() as u32);
                        vertices.push(Vertex { position: p, normal: n });
                    }
                }
            }
            _ => {
                for (dx, dz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    let p = c + Vec3::new(dx * e.x, 0.0, dz * e.z);
                    vertices.push(Vertex { position: p.to_array(), normal: [0.0, 1.0, 0.0] });
                }
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
        ranges.push(first..indices.len() as u32);
    }
    Meshes { vertices, indices, ranges }
}
