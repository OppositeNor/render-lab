//! The directional light's cascaded shadow maps, on the CPU: how
//! DirectionalLightFeatureProcessor splits a camera's frustum into cascades,
//! fits each with a sphere and a light-space box snapped to texels, and how
//! DirectionalLightShadowCalculator picks a pixel's cascade. The lab has no
//! directional light and renders no shadow maps; this module follows the
//! engine's arithmetic for an observer camera in the room, so the
//! demonstration can draw it (schematic, and said so on screen).
//!
//! Code ported from the Open 3D Engine (Gems/Atom/Feature/Common/Code/Source/
//! CoreLights/DirectionalLightFeatureProcessor.cpp; Assets/ShaderLib/Atom/
//! Features/Shadow/DirectionalLightShadowCalculator.azsli) is
//! Copyright (c) Contributors to the Open 3D Engine Project, Apache-2.0 OR MIT.

use glam::{Mat4, Vec2, Vec3, Vec4};

/// The component's defaults (DirectionalLightComponentConfig.h): four
/// cascades of a 1024-texel shadow map, split automatically at ratio 0.9.
pub const CASCADES: usize = 4;
pub const SHADOWMAP_SIZE: f32 = 1024.0;
/// DirectionalLightShadowCalculator.azsli: PIXEL_MARGIN.
pub const PIXEL_MARGIN: f32 = 1.5;
/// NormalOffsetShadows.azsli with the component's normal bias 2.5: the
/// world-space offset along the normal, 2.5 * 2 / size.
pub const NORMAL_OFFSET: f32 = 2.5 * 2.0 / SHADOWMAP_SIZE;

/// The camera whose frustum the cascades cover (the shadow code takes these
/// from the active camera; the lab's observer uses a 60 degree vertical field
/// of view, 16:9, and a 0.1 m near plane).
#[derive(Clone, Copy, Debug)]
pub struct Observer {
    pub position: Vec3,
    pub forward: Vec3,
    pub fov_y: f32,
    pub aspect: f32,
    pub near: f32,
    /// min(the camera's far plane, the light's shadow far).
    pub far: f32,
}

impl Observer {
    pub fn right(&self) -> Vec3 {
        self.forward.cross(Vec3::Y).normalize()
    }
    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward)
    }
    /// CascadeShadowCameraConfiguration::SetDepthCenterRatio:
    /// 1 + tan^2(fov/2) (1 + ar^2).
    pub fn depth_center_ratio(&self) -> f32 {
        let t = (self.fov_y / 2.0).tan();
        1.0 + t * t * (1.0 + self.aspect * self.aspect)
    }
    /// The bounding sphere of the frustum's slice from depth `dn` to `df`
    /// (GetWorldCenterPosition, GetRadius): its centre depth and radius.
    pub fn slice_sphere(&self, dn: f32, df: f32) -> (f32, f32) {
        let k = self.depth_center_ratio();
        let dc = (dn + df) / 2.0 * k;
        if dc < df {
            (dc, ((df - dc) * (df - dc) + df * df * (k - 1.0)).sqrt())
        } else {
            (df, df * (self.fov_y / 2.0).tan() * (1.0 + self.aspect * self.aspect).sqrt())
        }
    }
    /// The four corners of the frustum's cross-section at depth `d`.
    pub fn corners(&self, d: f32) -> [Vec3; 4] {
        let h = d * (self.fov_y / 2.0).tan();
        let w = h * self.aspect;
        let c = self.position + self.forward * d;
        let (r, u) = (self.right(), self.up());
        [c - r * w - u * h, c + r * w - u * h, c + r * w + u * h, c - r * w + u * h]
    }
}

/// UpdateBorderDepthsForSegments: each cascade's far depth, a blend of the
/// logarithmic and the uniform split by `ratio`; the last is the far plane.
pub fn split_depths(near: f32, far: f32, ratio: f32) -> [f32; CASCADES] {
    let mut depths = [far; CASCADES];
    for (i, d) in depths.iter_mut().enumerate().take(CASCADES - 1) {
        let t = (i + 1) as f32 / CASCADES as f32;
        let uni = near + (far - near) * t;
        let log = near * (far / near).powf(t);
        *d = ratio * log + (1.0 - ratio) * uni;
    }
    depths
}

/// The light's frame (Matrix3x4::CreateLookAt from the origin along the
/// light's direction: +Y is the direction; the lab's +Y-up stands for O3DE's
/// +Z-up).
#[derive(Clone, Copy, Debug)]
pub struct LightFrame {
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
}

impl LightFrame {
    pub fn new(direction: Vec3) -> LightFrame {
        let y = direction.normalize();
        let x = y.cross(Vec3::Y).normalize();
        let z = x.cross(y);
        LightFrame { x, y, z }
    }
    pub fn to_light(&self, p: Vec3) -> Vec3 {
        Vec3::new(p.dot(self.x), p.dot(self.y), p.dot(self.z))
    }
    pub fn to_world(&self, p: Vec3) -> Vec3 {
        self.x * p.x + self.y * p.y + self.z * p.z
    }
}

/// One cascade: its slice of the camera's depth, its bounding sphere, and
/// its light-space box before and after snapping.
#[derive(Clone, Copy, Debug)]
pub struct Cascade {
    pub near: f32,
    pub far: f32,
    pub center: Vec3,
    pub radius: f32,
    /// SnapAabbToPixelIncrements' step: CalculateShadowViewAabb's box (the
    /// sphere's cube in the light's frame, its near side along the light
    /// pulled back to cover the casters between the light and the whole
    /// frustum, and behind the camera) over the map's size.
    pub snap: Vec3,
    /// That box's min and max floored to whole steps.
    pub snapped_min: Vec3,
    pub snapped_max: Vec3,
    /// The box's extent along the light before snapping: the orthographic
    /// projection's near and far (cascadeNear and cascadeFar, from
    /// aabb.min.y and aabb.max.y; the snapped y is not used).
    pub min_y: f32,
    pub max_y: f32,
}

impl Cascade {
    /// One shadow-map texel across the map, metres (2R / size).
    pub fn texel(&self) -> f32 {
        self.snap.x
    }
    /// A light-space point's texel coordinates in this cascade's map (x and
    /// z across the orthographic projection).
    pub fn texel_coord(&self, light: Vec3) -> Vec2 {
        let (lo, hi) = (self.snapped_min, self.snapped_max);
        Vec2::new((light.x - lo.x) / (hi.x - lo.x), (hi.z - light.z) / (hi.z - lo.z)) * SHADOWMAP_SIZE
    }
}

pub struct Cascades {
    pub frame: LightFrame,
    pub cascades: [Cascade; CASCADES],
}

/// Every cascade of `observer` for a light shining along `direction`, split at `ratio`.
pub fn cascades(observer: Observer, direction: Vec3, ratio: f32) -> Cascades {
    let frame = LightFrame::new(direction);
    let far = split_depths(observer.near, observer.far, ratio);
    // The whole frustum's sphere, and the camera, bound the near side.
    let (entire_depth, entire_radius) = observer.slice_sphere(observer.near, observer.far);
    let entire_y = frame.to_light(observer.position + observer.forward * entire_depth).y;
    let behind_y = frame.to_light(observer.position).y - observer.far;
    let cascades = std::array::from_fn(|i| {
        let near = if i == 0 { observer.near } else { far[i - 1] };
        let (depth, radius) = observer.slice_sphere(near, far[i]);
        let center = observer.position + observer.forward * depth;
        let c = frame.to_light(center);
        let mut min = c - Vec3::splat(radius);
        let max = c + Vec3::splat(radius);
        min.y = min.y.min(entire_y - entire_radius).min(behind_y);
        let texel = (max - min) / SHADOWMAP_SIZE;
        let snapped_min = (min / texel).floor() * texel;
        let snapped_max = (max / texel).floor() * texel;
        Cascade { near, far: far[i], center, radius, snap: texel, snapped_min, snapped_max, min_y: min.y, max_y: max.y }
    });
    Cascades { frame, cascades }
}

impl Cascades {
    /// Cascade `i`'s world to shadow-map transform, as
    /// m_lightViewToShadowmapMatrices[i] * m_worldToLightViewMatrices[i]
    /// give it: x, y the map's texture coordinates (y down, the
    /// ClipToShadowmapTextureMatrix flip), z the depth from the light, 0 at
    /// the box's near side and 1 at its far side (O3DE's shadow maps are not
    /// reversed: D32_FLOAT, cleared to 1, LessEqual).
    pub fn world_to_shadowmap(&self, i: usize) -> Mat4 {
        let c = &self.cascades[i];
        let f = &self.frame;
        let (lo, hi) = (c.snapped_min, c.snapped_max);
        let (w, d, h) = (hi.x - lo.x, c.max_y - c.min_y, hi.z - lo.z);
        let rows = [
            Vec4::new(f.x.x / w, f.x.y / w, f.x.z / w, -lo.x / w),
            Vec4::new(-f.z.x / h, -f.z.y / h, -f.z.z / h, hi.z / h),
            Vec4::new(f.y.x / d, f.y.y / d, f.y.z / d, -c.min_y / d),
            Vec4::W,
        ];
        Mat4::from_cols(rows[0], rows[1], rows[2], rows[3]).transpose()
    }

    /// GetShadowCascadeRange's first cascade: the first whose texel
    /// coordinates of the point, offset along its normal, lie inside the map
    /// by PIXEL_MARGIN on both axes (by bounds, not by depth); None outside
    /// every cascade (lit).
    pub fn select(&self, p: Vec3, n: Vec3) -> Option<usize> {
        let q = self.frame.to_light(p + n * NORMAL_OFFSET);
        self.cascades.iter().position(|c| {
            let t = c.texel_coord(q);
            t.min_element() >= PIXEL_MARGIN && t.max_element() <= SHADOWMAP_SIZE - PIXEL_MARGIN
        })
    }
    /// The point's texel coordinates in cascade `i`.
    pub fn texel_of(&self, i: usize, p: Vec3, n: Vec3) -> Vec2 {
        self.cascades[i].texel_coord(self.frame.to_light(p + n * NORMAL_OFFSET))
    }
}
