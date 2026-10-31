//! The lab's VoxelGI: one node around the study room, baked once by the
//! engine's rules (bake.rs, sdf.rs), uploaded (storage.rs), and lit by the
//! ceiling panel's area light (light.rs), at the default settings. The
//! demonstrations' variants (another dynamic range, propagation, bounce
//! count, an extra light, a moved dynamic object) are computed on first use
//! and kept.
//!
//! The node: centred at (0.0125, 1.5125, 0.0125), 6.4 x 3.2 x 6.4 m,
//! subdivision 128 (the default), so 128 x 64 x 128 cells of 5 cm in 8
//! levels. The quarter-cell offset keeps every wall and the floor off a cell
//! boundary: a plane exactly on one can fall between the two cells it
//! touches when the separating-axis test rounds (the room's back and left
//! walls did, at (0, 1.5, 0)), and a floor on the box's bottom face would get
//! no light from the screen pass (its edge fade is 0 there).

use super::bake::{self, Aabb, BakeMesh, Cell, Voxelizer};
use super::cone::Instance;
use super::light::{self, Light, Lit, Params};
use super::sdf;
use super::storage::Probe;
use glam::{Mat4, Vec3};
use lab::scene::{ids, Scene};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// The VoxelGI node's position and size (`VoxelGI::size`), and `SUBDIV_128`'s depth.
pub const NODE: Vec3 = Vec3::new(0.0125, 1.5125, 0.0125);
pub const SIZE: Vec3 = Vec3::new(6.4, 3.2, 6.4);
pub const SUBDIV: i32 = 7;

/// VoxelGIData's defaults (scene/3d/voxel_gi.h).
pub const DYNAMIC_RANGE: f32 = 2.0;
pub const ENERGY: f32 = 1.0;
pub const BIAS: f32 = 1.5;
pub const NORMAL_BIAS: f32 = 0.0;
pub const PROPAGATION: f32 = 0.5;

/// The settings a lit texture depends on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightSettings {
    pub dynamic_range: f32,
    pub propagation: f32,
    pub two_bounces: bool,
}

impl Default for LightSettings {
    fn default() -> LightSettings {
        LightSettings { dynamic_range: DYNAMIC_RANGE, propagation: PROPAGATION, two_bounces: true }
    }
}

pub struct World {
    pub scene: Scene,
    pub meshes: Vec<BakeMesh>,
    /// The voxelizer after `end_bake`.
    pub voxelizer: Voxelizer,
    /// The cells after `_sort`, before `_fixup_plot` (the leaves' raw sums).
    pub sums: Vec<Cell>,
    pub probe: Probe,
    pub lights: Vec<Light>,
    pub lit: Lit,
    pub bake_seconds: f32,
    pub light_seconds: f32,
}

/// The box as `VoxelGI::bake` passes it: `AABB(-size / 2, size)` in the node's space.
pub fn bounds() -> Aabb {
    Aabb { position: -SIZE * 0.5, size: SIZE }
}

/// The node's transform (no rotation or scale: `VoxelGI` disables scale).
pub fn node_transform() -> Mat4 {
    Mat4::from_translation(NODE)
}

/// The objects the bake leaves out: the ceiling panel (the area light's
/// fixture; in Godot, a mesh with GI mode Disabled), and `extra` (a dynamic object).
pub fn bake(scene: &Scene, extra: &[usize]) -> (Voxelizer, Vec<Cell>, Vec<BakeMesh>) {
    let meshes = lab::scene::build_meshes(scene);
    let mut skip = vec![ids::PANEL];
    skip.extend_from_slice(extra);
    let bake_meshes = bake::room_meshes(scene, &meshes, node_transform(), &skip);
    let mut v = Voxelizer::begin_bake(SUBDIV, bounds());
    for m in &bake_meshes {
        v.plot_mesh(m);
    }
    v.sort();
    let sums = v.cells.clone();
    v.end_bake();
    (v, sums, bake_meshes)
}

/// `ps = octree_size / bounds.size`, `cell_size = 1 / max(ps)`: metres per cell.
pub fn params(probe: &Probe, s: &LightSettings) -> Params {
    let ps = probe.octree_size.as_vec3() / SIZE;
    Params { limits: probe.octree_size, propagation: s.propagation, dynamic_range: s.dynamic_range, cell_size: 1.0 / ps.max_element() }
}

/// The ceiling panel as an AreaLight3D (the SDFGI investigation's choice):
/// its rectangle, its colour, energy so that its radiance matches the lab's
/// panel (`radiance / pi` per unit area), a 10 m range, attenuation 2, in the
/// probe's cell space.
pub fn panel_light(scene: &Scene, probe: &Probe) -> Light {
    let l = &scene.lighting.light;
    let width = l.axis_u * 2.0;
    let height = l.axis_v * 2.0;
    let area = width.length() * height.length();
    let energy = l.radiance * area / std::f32::consts::PI;
    let range = 10.0;
    let s = probe.to_cell_scale;
    let radius = range * s;
    Light {
        kind: light::LIGHT_TYPE_AREA,
        energy: energy / area,
        radius,
        attenuation: 2.0,
        color: l.color,
        cos_spot_angle: 0.0,
        position: probe.to_cell(l.center - NODE),
        inv_spot_attenuation: 1.0 / (radius + glam::Vec2::new(width.length(), height.length()).length() / 2.0),
        direction: l.normal(),
        has_shadow: false,
        area_width: width * s,
        area_height: height * s,
    }
}

/// The extra omni light of the light demonstration (OmniLight3D, shadow on):
/// at `at` (world), range 5 m, attenuation 1, a warm white of energy 2.
pub fn omni_light(probe: &Probe, at: Vec3) -> Light {
    let s = probe.to_cell_scale;
    Light {
        kind: light::LIGHT_TYPE_OMNI,
        energy: 2.0,
        radius: 5.0 * s,
        attenuation: 1.0,
        color: Vec3::new(1.0, 0.85, 0.65),
        cos_spot_angle: 45f32.to_radians().cos(),
        position: probe.to_cell(at - NODE),
        inv_spot_attenuation: 1.0,
        direction: Vec3::NEG_Y,
        has_shadow: true,
        area_width: Vec3::ZERO,
        area_height: Vec3::ZERO,
    }
}

/// The screen pass's `VoxelGIData` for the node (world positions to cells).
pub fn instance(probe: &Probe, s: &LightSettings, interior: bool) -> Instance {
    Instance {
        to_cell_scale: probe.to_cell_scale,
        to_cell_origin: probe.to_cell_origin + NODE,
        bounds: probe.octree_size.as_vec3(),
        dynamic_range: s.dynamic_range * ENERGY,
        bias: BIAS,
        normal_bias: NORMAL_BIAS,
        blend_ambient: !interior,
        exposure_normalization: 1.0,
    }
}

fn build() -> World {
    let scene = Scene::study_room();
    let started = std::time::Instant::now();
    let (voxelizer, sums, meshes) = bake(&scene, &[]);
    let field = sdf::sdf_image(&voxelizer);
    let probe = Probe::from_bake(&voxelizer, field);
    let bake_seconds = started.elapsed().as_secs_f32();
    let lights = vec![panel_light(&scene, &probe)];
    let started = std::time::Instant::now();
    let lit = light::update(&probe, &params(&probe, &LightSettings::default()), &lights, true);
    let light_seconds = started.elapsed().as_secs_f32();
    World { scene, meshes, voxelizer, sums, probe, lights, lit, bake_seconds, light_seconds }
}

/// The lab's VoxelGI at the defaults, built once.
pub fn world() -> &'static World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD.get_or_init(build)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct VariantKey {
    dynamic_range: i32,
    propagation: i32,
    two_bounces: bool,
}

/// The lit texture for other settings (dynamic range and propagation to two
/// decimals), computed on first use.
pub fn lit_with(s: &LightSettings) -> Arc<Lit> {
    static CACHE: OnceLock<Mutex<HashMap<VariantKey, Arc<Lit>>>> = OnceLock::new();
    let w = world();
    let key = VariantKey { dynamic_range: (s.dynamic_range * 100.0).round() as i32, propagation: (s.propagation * 100.0).round() as i32, two_bounces: s.two_bounces };
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(l) = cache.lock().unwrap().get(&key) {
        return l.clone();
    }
    let settings = LightSettings { dynamic_range: key.dynamic_range as f32 / 100.0, propagation: key.propagation as f32 / 100.0, two_bounces: key.two_bounces };
    let lit = Arc::new(light::update(&w.probe, &params(&w.probe, &settings), &w.lights, settings.two_bounces));
    cache.lock().unwrap().insert(key, lit.clone());
    lit
}

/// The leaves' direct light with the extra omni light at `at` (to the
/// centimetre) beside the panel: `MODE_COMPUTE_LIGHT` alone, computed on first use.
pub fn direct_with_omni(at: Vec3) -> Arc<Vec<Vec3>> {
    static CACHE: OnceLock<Mutex<HashMap<[i32; 3], Arc<Vec<Vec3>>>>> = OnceLock::new();
    let key = (at * 100.0).round().as_ivec3().to_array();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(l) = cache.lock().unwrap().get(&key) {
        return l.clone();
    }
    let w = world();
    let at = glam::IVec3::from_array(key).as_vec3() / 100.0;
    let lights = [w.lights[0], omni_light(&w.probe, at)];
    let params = params(&w.probe, &LightSettings::default());
    let leaves = w.probe.level_range(w.probe.leaf_level());
    let start = leaves.start;
    let direct = light::par_map(leaves, |c| light::compute_light(&w.probe, &params, &lights, c));
    let mut all = vec![Vec3::ZERO; start];
    all.extend(direct);
    let all = Arc::new(all);
    cache.lock().unwrap().insert(key, all.clone());
    all
}

/// The scene shading's data buffer (gShadingData): every mip of the lit
/// texture, one RGBA8 word per texel, x fastest, mip 0 first; then the
/// distance field, four bytes a word (for the light kernels); four words a vec4.
pub fn shading_data() -> Vec<[f32; 4]> {
    let w = world();
    let mut words = w.lit.texture.words();
    debug_assert_eq!(words.len() as u32, texture_words(&w.probe));
    words.extend(w.probe.sdf.chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])));
    while words.len() % 4 != 0 {
        words.push(0);
    }
    words.chunks(4).map(|c| [f32::from_bits(c[0]), f32::from_bits(c[1]), f32::from_bits(c[2]), f32::from_bits(c[3])]).collect()
}

/// Words of the texture's mips in the data buffer: where the field starts.
pub fn texture_words(probe: &Probe) -> u32 {
    (0..probe.mip_count()).map(|m| {
        let s = probe.mip_size(m);
        (s.x * s.y * s.z) as u32
    }).sum()
}

/// The lit texture for `s`: the world's own at the defaults, else `lit_with`.
pub fn lit_for(s: &LightSettings) -> Arc<Lit> {
    if *s == LightSettings::default() {
        static DEFAULT: OnceLock<Arc<Lit>> = OnceLock::new();
        return DEFAULT.get_or_init(|| Arc::new(world().lit.clone())).clone();
    }
    lit_with(s)
}

/// The room with the teapot's GI mode Dynamic: baked without it, lit at the
/// defaults (its float buffer and texture are what each frame rewrites
/// before the plot).
pub struct DynamicWorld {
    pub probe: Probe,
    pub lit: Lit,
}

pub fn dynamic_world() -> &'static DynamicWorld {
    static WORLD: OnceLock<DynamicWorld> = OnceLock::new();
    WORLD.get_or_init(|| {
        let scene = Scene::study_room();
        let (voxelizer, _, _) = bake(&scene, &[ids::TEAPOT]);
        let probe = Probe::from_bake(&voxelizer, sdf::sdf_image(&voxelizer));
        let lights = vec![panel_light(&scene, &probe)];
        let lit = light::update(&probe, &params(&probe, &LightSettings::default()), &lights, true);
        DynamicWorld { probe, lit }
    })
}

/// The teapot's record moved `dx` metres along x.
pub fn teapot_at(dx: f32) -> lab::layout::ObjectData {
    let scene = Scene::study_room();
    let mut o = scene.objects[ids::TEAPOT].clone();
    o.translation.x += dx;
    lab::scene::object_record(&o, ids::TEAPOT as u32)
}

/// The dynamic pass for the teapot moved `dx` (to the centimetre), computed on first use.
pub fn dynamic_plot(dx: f32) -> Arc<super::dynamic::Plot> {
    static CACHE: OnceLock<Mutex<HashMap<i32, Arc<super::dynamic::Plot>>>> = OnceLock::new();
    let key = (dx * 100.0).round() as i32;
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(p) = cache.lock().unwrap().get(&key) {
        return p.clone();
    }
    let w = dynamic_world();
    let scene = Scene::study_room();
    let lights = [panel_light(&scene, &w.probe)];
    let plot = Arc::new(super::dynamic::plot(&w.probe, &params(&w.probe, &LightSettings::default()), &lights, &teapot_at(key as f32 / 100.0), &w.lit.texture, NODE));
    cache.lock().unwrap().insert(key, plot.clone());
    plot
}
