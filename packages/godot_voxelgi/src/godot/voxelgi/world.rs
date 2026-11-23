//! The lab's VoxelGI: one node around the study room, baked once by the
//! engine's rules (bake.rs, sdf.rs) and uploaded (storage.rs); its lights
//! (the ceiling panel's area light, and the extra omni light when it is on)
//! go to the GPU's light passes with each frame (update.rs). The CPU mirror's
//! lit textures (light.rs) at each setting (another dynamic range,
//! propagation, bounce count, an extra light, a moved dynamic object), which
//! the demonstrations draw and the selftest compares, are computed on first
//! use and kept.
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
use lab::scene::{Scene, ids};
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
        LightSettings {
            dynamic_range: DYNAMIC_RANGE,
            propagation: PROPAGATION,
            two_bounces: true,
        }
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
    /// The room's own light: the panel.
    pub lights: Vec<Light>,
    pub bake_seconds: f32,
}

/// The box as `VoxelGI::bake` passes it: `AABB(-size / 2, size)` in the node's space.
pub fn bounds() -> Aabb {
    Aabb {
        position: -SIZE * 0.5,
        size: SIZE,
    }
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
        // Every mesh's triangles into the octree, then sorted; the sums are kept before end_bake averages them.
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
    Params {
        limits: probe.octree_size,
        propagation: s.propagation,
        dynamic_range: s.dynamic_range,
        cell_size: 1.0 / ps.max_element(),
    }
}

/// The ceiling panel as an AreaLight3D (the SDFGI investigation's choice):
/// its rectangle, its colour, energy so that its radiance matches the lab's
/// panel (`radiance / pi` per unit area), a 10 m range, attenuation 2, in the
/// probe's cell space.
pub fn panel_light(scene: &Scene, probe: &Probe) -> Light {
    // The room's panel as Godot's area light, in the probe's cells.
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
        inv_spot_attenuation: 1.0
            / (radius + glam::Vec2::new(width.length(), height.length()).length() / 2.0),
        direction: l.normal(),
        has_shadow: false,
        area_width: width * s,
        area_height: height * s,
    }
}

/// The extra omni light's range, metres.
pub const OMNI_RANGE: f32 = 5.0;

/// The extra omni light of the light demonstration (OmniLight3D, shadow on):
/// at `at` (world), range 5 m, attenuation 1, a warm white of energy 2.
pub fn omni_light(probe: &Probe, at: Vec3) -> Light {
    let s = probe.to_cell_scale;
    Light {
        kind: light::LIGHT_TYPE_OMNI,
        energy: 2.0,
        radius: OMNI_RANGE * s,
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
    World {
        scene,
        meshes,
        voxelizer,
        sums,
        probe,
        lights,
        bake_seconds,
    }
}

/// The lab's VoxelGI as the bake leaves it, built once.
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

/// The CPU mirror's lit texture for settings `s` (dynamic range and
/// propagation to two decimals) and the room's own light, computed on first use.
fn lit_with(s: &LightSettings) -> Arc<Lit> {
    static CACHE: OnceLock<Mutex<HashMap<VariantKey, Arc<Lit>>>> = OnceLock::new();
    let w = world();
    let key = VariantKey {
        dynamic_range: (s.dynamic_range * 100.0).round() as i32,
        propagation: (s.propagation * 100.0).round() as i32,
        two_bounces: s.two_bounces,
    };
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    // Each setting, rounded to a hundredth, is lit once and kept.
    if let Some(l) = cache.lock().unwrap().get(&key) {
        return l.clone();
    }
    let settings = LightSettings {
        dynamic_range: key.dynamic_range as f32 / 100.0,
        propagation: key.propagation as f32 / 100.0,
        two_bounces: key.two_bounces,
    };
    let lit = Arc::new(light::update(
        &w.probe,
        &params(&w.probe, &settings),
        &w.lights,
        settings.two_bounces,
    ));
    cache.lock().unwrap().insert(key, lit.clone());
    lit
}

/// The leaves' direct light with the extra omni light at `at` (to the
/// centimetre) beside the panel: `MODE_COMPUTE_LIGHT` alone, computed on first use.
pub fn direct_with_omni(at: Vec3) -> Arc<Vec<Vec3>> {
    /// By the light's position in centimetres.
    type Cache = HashMap<[i32; 3], Arc<Vec<Vec3>>>;
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    // The leaves' direct light with an extra omni light, kept by the light's position to a centimetre.
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
    let direct = light::par_map(leaves, |c| {
        light::compute_light(&w.probe, &params, &lights, c)
    });
    let mut all = vec![Vec3::ZERO; start];
    all.extend(direct);
    let all = Arc::new(all);
    cache.lock().unwrap().insert(key, all.clone());
    all
}

/// Which bake the room's VoxelGI holds: the room as it is, or the room with
/// the teapot's GI mode Dynamic (baked without it, `dynamic_world`), as the
/// dynamic demonstration has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bake {
    Room,
    DynamicTeapot,
}

pub const BAKES: [Bake; 2] = [Bake::Room, Bake::DynamicTeapot];

/// A bake's probe.
pub fn probe_of(bake: Bake) -> &'static Probe {
    match bake {
        Bake::Room => &world().probe,
        Bake::DynamicTeapot => &dynamic_world().probe,
    }
}

/// The bake a shot shows: the teapot is a Dynamic object in the dynamic demonstration.
pub fn bake_of(state: &lab::shot::ShotState) -> Bake {
    if super::shot::Demo::of(state.demo) == super::shot::Demo::Dynamic {
        Bake::DynamicTeapot
    } else {
        Bake::Room
    }
}

/// Where a bake's records start in the scene shading's data buffer
/// (`shading_data`), in vec4s: its distance field's bytes, sixteen a vec4;
/// its cells' `CellData`, one each; their `CellChildren`, two each.
#[derive(Clone, Copy, Debug)]
pub struct DataLayout {
    pub sdf: u32,
    pub cells: u32,
    pub children: u32,
}

/// The vec4s a bake's records take.
fn data_len(probe: &Probe) -> u32 {
    (probe.sdf.len() as u32).div_ceil(16) + 3 * probe.data.len() as u32
}

pub fn data_layout(bake: Bake) -> DataLayout {
    let sdf = match bake {
        Bake::Room => 0,
        Bake::DynamicTeapot => data_len(probe_of(Bake::Room)),
    };
    let probe = probe_of(bake);
    let cells = sdf + (probe.sdf.len() as u32).div_ceil(16);
    DataLayout {
        sdf,
        cells,
        children: cells + probe.data.len() as u32,
    }
}

/// One bake's records, as `voxel_gi_allocate_data` uploads them (gi.cpp:66-190):
/// the distance field's bytes, four a word (padded to a vec4), then each
/// cell's `CellData` and its `CellChildren`.
fn bake_words(probe: &Probe) -> Vec<u32> {
    let mut words: Vec<u32> = probe
        .sdf
        .chunks(4)
        .map(|b| {
            let mut q = [0u8; 4];
            q[..b.len()].copy_from_slice(b);
            u32::from_le_bytes(q)
        })
        .collect();
    while !words.len().is_multiple_of(4) {
        words.push(0);
    }
    for d in &probe.data {
        words.extend([d.position, d.albedo, d.emission, d.normal]);
    }
    for c in &probe.octree {
        words.extend(c);
    }
    words
}

/// The scene shading's data buffer (gShadingData): each bake's records
/// (`bake_words`), at `data_layout`. The plan copies the shown bake's field
/// into the R8 texture and lights its cells; the lights go up with each frame
/// (`light_words`), and the light texture is the program's (update.rs).
pub fn shading_data() -> Vec<[f32; 4]> {
    let mut words = Vec::new();
    for bake in BAKES {
        // Each bake's field, cells, and children, where data_layout says they start.
        debug_assert_eq!(words.len() as u32 / 4, data_layout(bake).sdf);
        words.extend(bake_words(probe_of(bake)));
    }
    words
        .chunks(4)
        .map(|c| {
            [
                f32::from_bits(c[0]),
                f32::from_bits(c[1]),
                f32::from_bits(c[2]),
                f32::from_bits(c[3]),
            ]
        })
        .collect()
}

/// The lights as the light passes read them (`VoxelGILight`, gi.h:164-182,
/// as six vec4s each), at most `voxel_gi_max_lights` of them (gi.cpp:2971).
pub fn light_words(lights: &[Light]) -> Vec<u32> {
    lights
        .iter()
        .take(light::MAX_LIGHTS)
        .flat_map(|l| {
            [
                [l.kind as f32, l.energy, l.radius, l.attenuation],
                l.color.extend(l.cos_spot_angle).to_array(),
                l.position.extend(l.inv_spot_attenuation).to_array(),
                l.direction.extend(l.has_shadow as u32 as f32).to_array(),
                l.area_width.extend(0.0).to_array(),
                l.area_height.extend(0.0).to_array(),
            ]
        })
        .flatten()
        .map(f32::to_bits)
        .collect()
}

/// The scene node's light settings in a shot (giDynamicRange, giPropagation, giTwoBounces).
pub fn scene_settings(state: &lab::shot::ShotState) -> LightSettings {
    LightSettings {
        dynamic_range: state.p("giDynamicRange"),
        propagation: state.p("giPropagation"),
        two_bounces: state.p("giTwoBounces") > 0.5,
    }
}

/// The CPU mirror's lit texture for `s` and the room's own light, computed on first use.
pub fn lit_for(s: &LightSettings) -> Arc<Lit> {
    lit_with(s)
}

/// The room with the teapot's GI mode Dynamic: baked without it.
pub struct DynamicWorld {
    pub probe: Probe,
}

pub fn dynamic_world() -> &'static DynamicWorld {
    static WORLD: OnceLock<DynamicWorld> = OnceLock::new();
    WORLD.get_or_init(|| {
        let scene = Scene::study_room();
        let (voxelizer, _, _) = bake(&scene, &[ids::TEAPOT]);
        let probe = Probe::from_bake(&voxelizer, sdf::sdf_image(&voxelizer));
        DynamicWorld { probe }
    })
}

/// The CPU mirror's light of the room without the teapot, at the defaults (its
/// float buffer and texture are what each frame rewrites before the plot),
/// computed on first use.
pub fn dynamic_lit() -> &'static Lit {
    static LIT: OnceLock<Lit> = OnceLock::new();
    LIT.get_or_init(|| {
        let probe = &dynamic_world().probe;
        let lights = vec![panel_light(&Scene::study_room(), probe)];
        light::update(
            probe,
            &params(probe, &LightSettings::default()),
            &lights,
            true,
        )
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
    let plot = Arc::new(super::dynamic::plot(
        &w.probe,
        &params(&w.probe, &LightSettings::default()),
        &lights,
        &teapot_at(key as f32 / 100.0),
        &dynamic_lit().texture,
        NODE,
    ));
    cache.lock().unwrap().insert(key, plot.clone());
    plot
}
