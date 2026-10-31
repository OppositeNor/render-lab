//! The CPU mirror of O3DE's SSAO (SsaoParent.pass): DepthDownsample,
//! SsaoCompute, the FastDepthAwareBlur passes, and DepthUpsample, run on the
//! GPU's own linear depth so the selftest can compare every stage.

use glam::{Vec2, Vec3};

/// The SSAO constants (demos::ssao_records).
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub strength: f32,
    pub sampling_radius: f32,
    pub blur: bool,
    pub const_falloff: f32,
    pub depth_falloff_threshold: f32,
    pub depth_falloff_strength: f32,
}

impl Settings {
    pub fn from_records(r: &[[f32; 4]]) -> Settings {
        Settings { strength: r[0][0], sampling_radius: r[0][1], blur: r[0][2] > 0.5, const_falloff: r[1][0], depth_falloff_threshold: r[1][1], depth_falloff_strength: r[1][2] }
    }
}

/// A one-channel image, read with clamped coordinates as the point samplers do.
#[derive(Clone, Debug)]
pub struct Image {
    pub size: [u32; 2],
    pub data: Vec<f32>,
}

impl Image {
    pub fn at(&self, x: i32, y: i32) -> f32 {
        let cx = x.clamp(0, self.size[0] as i32 - 1) as usize;
        let cy = y.clamp(0, self.size[1] as i32 - 1) as usize;
        self.data[cy * self.size[0] as usize + cx]
    }

    fn from_fn(size: [u32; 2], f: impl Fn(i32, i32) -> f32) -> Image {
        let mut data = Vec::with_capacity((size[0] * size[1]) as usize);
        for y in 0..size[1] as i32 {
            for x in 0..size[0] as i32 {
                data.push(f(x, y));
            }
        }
        Image { size, data }
    }
}

/// An R8_UNORM store.
pub fn unorm8(x: f32) -> f32 {
    (x.clamp(0.0, 1.0) * 255.0).round() / 255.0
}

/// DepthDownsample: the upper-left pixel of each 2 x 2.
pub fn downsample(full: &Image) -> Image {
    Image::from_fn([full.size[0].div_ceil(2), full.size[1].div_ceil(2)], |x, y| full.at(2 * x, 2 * y))
}

const LOCAL_SAMPLES: [[i32; 2]; 12] = [[0, -4], [-4, 0], [4, 0], [0, 4], [-4, -4], [4, -4], [-4, 4], [4, 4], [0, -8], [-8, 0], [8, 0], [0, 8]];
const OUTER_SAMPLE_ANGLE: f32 = 2.0 * 3.14159265 / 10.0;

/// GetViewSpacePosition with m_unprojectionConstants from the tangents of half the fields of view.
fn view_position(uv: Vec2, depth: f32, tan: Vec2) -> Vec3 {
    let ray = uv * Vec2::new(2.0 * tan.x, -2.0 * tan.y) + Vec2::new(-tan.x, tan.y);
    (ray * depth).extend(depth)
}

/// AccumulateOcclusion, option 4.
fn accumulate(position: Vec3, normal: Vec3, occluder: Vec3, occlusion: &mut f32, total: &mut f32) {
    let diff = occluder - position;
    let distance_sq = diff.dot(diff);
    let direction = diff * (1.0 / distance_sq.sqrt());
    let cosine = direction.dot(normal);
    let weight = 1.0 / (distance_sq + 0.07);
    *occlusion += cosine.clamp(0.0, 1.0) * weight;
    *total += weight;
}

/// SsaoCompute for every half-res pixel; `tan` is (tan of half the horizontal
/// field of view, of the vertical one).
pub fn compute(half: &Image, s: &Settings, tan: Vec2) -> Image {
    Image::from_fn(half.size, |x, y| compute_at(&|x, y| half.at(x, y), half.size, s, tan, x, y))
}

/// SsaoCompute at one half-res pixel, reading the clamped half-res depth through `half`.
pub fn compute_at(half: &dyn Fn(i32, i32) -> f32, half_size: [u32; 2], s: &Settings, tan: Vec2, x: i32, y: i32) -> f32 {
    let samples = samples_at(half, half_size, s, tan, x, y);
    let (mut occlusion, mut total) = (0.0, 0.0);
    for o in &samples.occluders {
        accumulate(samples.position, samples.normal, *o, &mut occlusion, &mut total);
    }
    let ao = occlusion / f32::max(total, 10e-5);
    unorm8((1.0 - ao * s.strength).powf(2.2))
}

/// One half-res pixel's SSAO inputs: its view-space position and the normal
/// rebuilt from its neighbours' depths, and its 52 occluders in the order
/// SsaoCompute accumulates them (12 local taps from the depth tile, then 10
/// rounds of 4 spiral samples shared within its 4 x 4 subgroup), in the view
/// space GetViewSpacePosition gives (x right, y up, z the distance forward).
pub struct PixelSamples {
    pub position: Vec3,
    pub normal: Vec3,
    pub occluders: Vec<Vec3>,
}

impl PixelSamples {
    /// Each occluder's saturate(cos) against the normal and its weight 1 / (d^2 + 0.07).
    pub fn terms(&self) -> Vec<(f32, f32)> {
        self.occluders.iter().map(|o| {
            let diff = *o - self.position;
            let d2 = diff.dot(diff);
            ((diff / d2.sqrt()).dot(self.normal).clamp(0.0, 1.0), 1.0 / (d2 + 0.07))
        }).collect()
    }
}

pub fn samples_at(half: &dyn Fn(i32, i32) -> f32, half_size: [u32; 2], s: &Settings, tan: Vec2, x: i32, y: i32) -> PixelSamples {
    let size = Vec2::new(half_size[0] as f32, half_size[1] as f32);
    let pixel_size = Vec2::ONE / size;
    let at = |x: i32, y: i32| half(x.clamp(0, half_size[0] as i32 - 1), y.clamp(0, half_size[1] as i32 - 1));
    let position_at = |x: i32, y: i32| view_position((Vec2::new(x as f32, y as f32) + 0.5) * pixel_size, at(x, y), tan);
    let (group, local) = ([x as u32 / 16, y as u32 / 16], [x as u32 % 16, y as u32 % 16]);
    let subgroup = [local[0] / 4, local[1] / 4];
    let sub = [local[0] % 4, local[1] % 4];
    let linear = (sub[0] & 1) | ((sub[1] & 1) << 1) | ((sub[0] >> 1) << 2) | ((sub[1] >> 1) << 3) | (subgroup[0] << 4) | (subgroup[1] << 6);
    let p = position_at(x, y);
    let (l, r, u, d) = (position_at(x - 1, y), position_at(x + 1, y), position_at(x, y - 1), position_at(x, y + 1));
    let (dl, dr) = (p - l, r - p);
    let dx = if dl.z.abs() < dr.z.abs() { dl } else { dr };
    let (du, dd) = (p - u, d - p);
    let dy = if du.z.abs() < dd.z.abs() { du } else { dd };
    let n = dx.cross(dy).normalize();
    let mut occluders = Vec::with_capacity(52);
    for [ox, oy] in LOCAL_SAMPLES {
        occluders.push(position_at(x + ox, y + oy));
    }
    let group_offset = Vec2::new(group[0] as f32, group[1] as f32) * 16.0 + 2.0;
    let subgroup_offset = Vec2::new(subgroup[0] as f32, subgroup[1] as f32) * 4.0 + group_offset;
    let outer = |id: u32, i: u32| {
        let angle_offset = ((id >> 4) & 0x0F) as f32 * (OUTER_SAMPLE_ANGLE / 16.0) + i as f32 * OUTER_SAMPLE_ANGLE;
        let factor = ((id & 0x0F) as f32 + 1.0) / 16.0;
        let radius = factor.sqrt() * (size.y * s.sampling_radius);
        let angle = factor * (OUTER_SAMPLE_ANGLE / 16.0) + angle_offset;
        let pixel = (Vec2::new(angle.cos(), angle.sin()) * radius + subgroup_offset).floor();
        let uv = pixel * pixel_size + 0.5 * pixel_size;
        let texel = (uv * size).floor();
        view_position(uv, at(texel.x as i32, texel.y as i32), tan)
    };
    for i in 0..10 {
        let mut other = linear;
        occluders.push(outer(other, i));
        for _ in 1..4 {
            other = (other & 0xF0) + ((other + 4) & 0x0F);
            occluders.push(outer(other, i));
        }
    }
    PixelSamples { position: p, normal: n, occluders }
}

/// FastDepthAwareBlur along x (`horizontal`) or y: each output is half the
/// forward chain and half the backward one, as the three-pixel threads compute them.
pub fn blur(values: &Image, half: &Image, s: &Settings, horizontal: bool) -> Image {
    Image::from_fn(values.size, |x, y| blur_at(&|x, y| values.at(x, y), &|x, y| half.at(x, y), s, horizontal, x, y))
}

/// The blur at one half-res pixel; `values` and `half` read clamped.
pub fn blur_at(values: &dyn Fn(i32, i32) -> f32, half: &dyn Fn(i32, i32) -> f32, s: &Settings, horizontal: bool, x: i32, y: i32) -> f32 {
    if !s.blur {
        return values(x, y);
    }
    let falloff = |previous: f32, current: f32| ((previous - current).abs() - s.depth_falloff_threshold).mul_add(-s.depth_falloff_strength, 1.0).clamp(0.0, 1.0);
    let axis = if horizontal { [1, 0] } else { [0, 1] };
    let chain = |start: [i32; 2], dir: [i32; 2], steps: i32| {
        let mut p = start;
        let mut previous_depth = half(p[0], p[1]);
        let mut previous_value = values(p[0], p[1]);
        p = [p[0] + dir[0], p[1] + dir[1]];
        let mut current_depth = half(p[0], p[1]);
        let mut current_value = values(p[0], p[1]);
        let mut previous_slope = current_depth - previous_depth;
        let mut current_slope = previous_slope;
        current_value = previous_value + (current_value - previous_value) * s.const_falloff;
        for _ in 0..steps {
            previous_value = current_value;
            previous_depth = current_depth;
            previous_slope = current_slope;
            p = [p[0] + dir[0], p[1] + dir[1]];
            current_depth = half(p[0], p[1]);
            current_value = values(p[0], p[1]);
            current_slope = current_depth - previous_depth;
            let f = falloff(previous_slope, current_slope) * s.const_falloff;
            current_value = current_value + (previous_value - current_value) * f;
        }
        current_value
    };
    {
        let coordinate = if horizontal { x } else { y };
        let k = coordinate % 3;
        let base = [x - axis[0] * k, y - axis[1] * k];
        let start = [base[0] - axis[0] * 8, base[1] - axis[1] * 8];
        let forwards = chain(start, axis, 6 + k + 1);
        let back_start = [start[0] + axis[0] * 18, start[1] + axis[1] * 18];
        let backwards = chain(back_start, [-axis[0], -axis[1]], 6 + (2 - k) + 1);
        unorm8(forwards * 0.5 + backwards * 0.5)
    }
}

/// DepthUpsample: the four nearest half-res pixels, 3/4 near and 1/4 far on each axis, by depth.
pub fn upsample(values: &Image, half: &Image, full: &Image) -> Image {
    Image::from_fn(full.size, |x, y| upsample_at(&|x, y| values.at(x, y), &|x, y| half.at(x, y), full.at(x, y), x, y))
}

/// DepthUpsample at one full-res pixel of linear depth `depth`.
pub fn upsample_at(values: &dyn Fn(i32, i32) -> f32, half: &dyn Fn(i32, i32) -> f32, depth: f32, x: i32, y: i32) -> f32 {
    {
        let near = [x / 2, y / 2];
        let far = [if x & 1 == 1 { near[0] + 1 } else { near[0] - 1 }, if y & 1 == 1 { near[1] + 1 } else { near[1] - 1 }];
        let taps = [(near, 0.75 * 0.75), ([far[0], near[1]], 0.25 * 0.75), ([near[0], far[1]], 0.75 * 0.25), (far, 0.25 * 0.25)];
        let (mut value, mut total) = (0.0, 0.0);
        for (t, w) in taps {
            let d = (depth - half(t[0], t[1])).abs() + 0.00001;
            let weight = w / (d * d);
            value += values(t[0], t[1]) * weight;
            total += weight;
        }
        unorm8(value / total)
    }
}

/// Every stage at one full-res pixel, from ray-cast linear depths around it
/// (the readout's view of the SSAO chain).
pub struct PixelStages {
    pub half_pixel: [i32; 2],
    pub linear_depth: f32,
    pub raw: f32,
    pub blur_vertical: f32,
    pub blurred: f32,
    pub upsampled: f32,
}

pub fn at_pixel(linear_depth: &dyn Fn(i32, i32) -> f32, full_size: [u32; 2], s: &Settings, tan: Vec2, x: i32, y: i32) -> PixelStages {
    use std::cell::RefCell;
    use std::collections::HashMap;
    let half_size = [full_size[0].div_ceil(2), full_size[1].div_ceil(2)];
    let clamp_half = |x: i32, y: i32| (x.clamp(0, half_size[0] as i32 - 1), y.clamp(0, half_size[1] as i32 - 1));
    let depths: RefCell<HashMap<(i32, i32), f32>> = RefCell::new(HashMap::new());
    let half = |x: i32, y: i32| -> f32 {
        let key = clamp_half(x, y);
        if let Some(d) = depths.borrow().get(&key) {
            return *d;
        }
        let d = linear_depth(2 * key.0, 2 * key.1);
        depths.borrow_mut().insert(key, d);
        d
    };
    let raws: RefCell<HashMap<(i32, i32), f32>> = RefCell::new(HashMap::new());
    let raw = |x: i32, y: i32| -> f32 {
        let key = clamp_half(x, y);
        if let Some(v) = raws.borrow().get(&key) {
            return *v;
        }
        let v = compute_at(&half, half_size, s, tan, key.0, key.1);
        raws.borrow_mut().insert(key, v);
        v
    };
    let verticals: RefCell<HashMap<(i32, i32), f32>> = RefCell::new(HashMap::new());
    let vertical = |x: i32, y: i32| -> f32 {
        let key = clamp_half(x, y);
        if let Some(v) = verticals.borrow().get(&key) {
            return *v;
        }
        let v = blur_at(&raw, &half, s, false, key.0, key.1);
        verticals.borrow_mut().insert(key, v);
        v
    };
    let horizontal = |x: i32, y: i32| -> f32 {
        let key = clamp_half(x, y);
        blur_at(&vertical, &half, s, true, key.0, key.1)
    };
    let depth = linear_depth(x, y);
    let h = [x / 2, y / 2];
    PixelStages { half_pixel: h, linear_depth: depth, raw: raw(h[0], h[1]), blur_vertical: vertical(h[0], h[1]), blurred: horizontal(h[0], h[1]), upsampled: upsample_at(&horizontal, &half, depth, x, y) }
}
