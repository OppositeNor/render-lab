//! Frames of SDFGI on the CPU, in the order `RenderForwardClustered::_render_scene`
//! runs them (render_forward_clustered.cpp): the dirty regions are voxelized
//! and stored (`_update_sdfgi`, `SDFGI::render_region`), then the static
//! lights (`render_static_lights`), `update_light` (the dynamic pass on a
//! quarter of the cells, reading last frame's probe maps for the bounce),
//! `update_probes` (every probe of every cascade traces its rays into the
//! history slot `render_pass % history_size`), and `store_probes` (the maps).
//!
//! The lab runs whole frames for a camera that stays where SDFGI was created
//! (every cascade `DIRTY_ALL` on the first frame, clean after), as many as a
//! demonstration asks for.

use super::light::{self, Field, Light, LightTextures};
use super::model::{Cascades, Settings};
use super::probes::{self, History, Integrate, Maps, ProbeMaps, Sky, PROBES};
use super::voxels::{self, CascadeData};
use glam::Vec3;
use lab::layout::ObjectData;

/// `frames_to_update_lights`' table {1, 2, 4, 8, 16}, at its default index 2.
pub const FRAMES_TO_UPDATE_LIGHT: usize = 4;

pub struct World {
    pub cascades: Cascades,
    pub data: Vec<CascadeData>,
    pub light: Vec<LightTextures>,
    pub history: Vec<History>,
    pub maps: Vec<ProbeMaps>,
    /// The renderer's frame number (`get_frame_number`), from 0.
    pub frame: u32,
    /// `render_pass`: probe updates so far.
    pub render_pass: u32,
    /// `all_dynamic_lights_dirty`, per cascade.
    pub lights_dirty: Vec<bool>,
    pub integrate: Integrate,
    pub bounce_feedback: f32,
    /// A probe of cascade 0 whose mean coefficients are recorded after every frame.
    pub traced_probe: Option<usize>,
    pub trace: Vec<[Vec3; super::probes::SH_SIZE]>,
}

impl World {
    /// `SDFGI::create` and the first frame's regions: every cascade voxelized,
    /// flooded, and stored; its light textures cleared; its history zero.
    pub fn new(objects: &[ObjectData], settings: Settings, camera: Vec3) -> World {
        let cascades = Cascades::create(settings, camera);
        let n = cascades.cascades.len();
        let data = std::thread::scope(|s| {
            let handles: Vec<_> = (0..n).map(|i| {
                let cascades = &cascades;
                s.spawn(move || voxels::build_cascade(objects, cascades, i))
            }).collect();
            handles.into_iter().map(|h| h.join().expect("a cascade's voxelization")).collect()
        });
        let integrate = Integrate { y_mult: settings.y_mult(), ..Integrate::default() };
        World {
            cascades,
            data,
            light: (0..n).map(|_| LightTextures::cleared()).collect(),
            history: (0..n).map(|_| History::new(integrate.history_size)).collect(),
            maps: (0..n).map(|_| ProbeMaps::empty()).collect(),
            frame: 0,
            render_pass: 0,
            lights_dirty: vec![true; n],
            integrate,
            bounce_feedback: 0.5,
            traced_probe: None,
            trace: Vec::new(),
        }
    }

    pub fn field(&self) -> Field<'_> {
        Field { cascades: &self.cascades, sdf: self.data.iter().map(|d| d.stored.sdf.as_slice()).collect() }
    }

    /// One frame: the dynamic light pass, the probes, and their maps.
    pub fn step(&mut self, lights_per_cascade: &[Vec<Light>], sky: &(dyn Fn(Vec3) -> Vec3 + Sync)) {
        self.step_sky(lights_per_cascade, sky, None)
    }

    /// `step` with a flat sky colour in place of the sky's radiance, when given.
    pub fn step_sky(&mut self, lights_per_cascade: &[Vec<Light>], sky: &(dyn Fn(Vec3) -> Vec3 + Sync), color: Option<Vec3>) {
        let n = self.cascades.cascades.len();
        // update_light: all cells of a freshly built cascade, else every fourth.
        let maps_before: Vec<ProbeMaps> = self.maps.clone();
        {
            let field = self.field();
            let maps = Maps(maps_before.iter().collect());
            let mut new_light = self.light.clone();
            std::thread::scope(|s| {
                for (i, textures) in new_light.iter_mut().enumerate() {
                    let field = &field;
                    let maps = &maps;
                    let cells = &self.data[i].stored.cells;
                    let lights = &lights_per_cascade[i];
                    let (offset, increment) = if self.lights_dirty[i] { (0, 1) } else { (self.frame as usize % FRAMES_TO_UPDATE_LIGHT, FRAMES_TO_UPDATE_LIGHT) };
                    let bounce = self.bounce_feedback;
                    s.spawn(move || light::dynamic_pass(field, i, cells, lights, bounce, Some(maps), offset, increment, textures));
                }
            });
            self.light = new_light;
        }
        self.lights_dirty = vec![false; n];
        // update_probes.
        let history_index = self.render_pass % self.integrate.history_size;
        {
            let field = Field { cascades: &self.cascades, sdf: self.data.iter().map(|d| d.stored.sdf.as_slice()).collect() };
            let light_refs: Vec<&LightTextures> = self.light.iter().collect();
            let integrate = self.integrate;
            let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).max(1);
            for c in 0..n {
                let values: Vec<_> = std::thread::scope(|s| {
                    let handles: Vec<_> = (0..threads).map(|t| {
                        let field = &field;
                        let light_refs = &light_refs;
                        let sky_fn = |d: Vec3| sky(d);
                        s.spawn(move || {
                            let sky = match color { Some(c) => Sky::Color(c), None => Sky::Radiance(&sky_fn) };
                            (t..PROBES).step_by(threads).map(|p| {
                                let rays = probes::trace_rays(field, light_refs, c, probes::probe_of(p), history_index, &integrate, &sky);
                                (p, probes::frame_coefficients(&rays, integrate.ray_count))
                            }).collect::<Vec<_>>()
                        })
                    }).collect();
                    handles.into_iter().flat_map(|h| h.join().expect("a probe thread")).collect()
                });
                for (p, v) in values {
                    self.history[c].push(p, history_index, v);
                }
            }
        }
        self.render_pass += 1;
        // store_probes.
        self.maps = self.history.iter().map(probes::store).collect();
        if let Some(p) = self.traced_probe {
            self.trace.push(self.history[0].mean(p));
        }
        self.frame += 1;
    }
}

/// The room's light as Godot would hold it: the ceiling panel as an
/// AreaLight3D of the panel's size (normalize_energy on, attenuation 2, a 10 m
/// range), dynamic, its energy chosen so that Godot's forward pass would light
/// a surface as the lab's panel does (`energy = radiance * area / pi`, from
/// the forward pass's `pi * energy * color / area * I / (2 pi)`), as
/// `pre_process_gi` packs it: position y times y_mult, the rectangle's edges
/// and its direction unscaled, energy divided by the area.
pub fn panel_light(scene: &lab::scene::Scene, y_mult: f32) -> Light {
    let l = &scene.lighting.light;
    let width = l.axis_u * 2.0;
    let height = l.axis_v * 2.0;
    let area = width.length() * height.length();
    let energy = l.radiance * area / std::f32::consts::PI;
    let radius = 10.0;
    Light {
        kind: light::LIGHT_TYPE_AREA,
        color: l.color,
        energy: energy / area,
        direction: l.normal(),
        position: Vec3::new(l.center.x, l.center.y * y_mult, l.center.z),
        attenuation: 2.0,
        radius,
        cos_spot_angle: 0.0,
        inv_spot_attenuation: 1.0 / (radius + glam::Vec2::new(width.length(), height.length()).length() / 2.0),
        area_width: width,
        area_height: height,
    }
}

/// The room's objects SDFGI voxelizes: everything but the ceiling panel, whose
/// light is the area light (in Godot, a fixture mesh with GI mode Disabled;
/// voxelized, it would shadow its own light).
pub fn voxelized_objects(objects: &[ObjectData]) -> Vec<ObjectData> {
    objects.iter().enumerate().filter(|(i, _)| *i != lab::scene::ids::PANEL).map(|(_, o)| *o).collect()
}

/// Frames the lab runs before it shows SDFGI's light: two histories, so the
/// ring is full and the bounce has gone round the probes a few times.
pub const CONVERGED_FRAMES: u32 = 60;

/// The room's SDFGI after `CONVERGED_FRAMES`, at the default settings, with
/// the panel's area light and the lab's sky; built once, on first use (the
/// loading step, when the scene shading's data is first asked for).
pub fn converged() -> &'static World {
    static WORLD: std::sync::OnceLock<World> = std::sync::OnceLock::new();
    WORLD.get_or_init(|| converged_with(0.5))
}

/// The room's world after `CONVERGED_FRAMES` with a given bounce feedback.
pub fn converged_with(bounce_feedback: f32) -> World {
    {
        let scene = lab::scene::Scene::study_room();
        let objects = voxelized_objects(&scene.records());
        let settings = Settings::default();
        let mut world = World::new(&objects, settings, super::demos::ORIGIN);
        world.bounce_feedback = bounce_feedback;
        world.traced_probe = Some(super::probes::probe_index(super::demos::FOLLOWED_PROBE));
        let panel = panel_light(&scene, settings.y_mult());
        let lights: Vec<Vec<Light>> = (0..world.cascades.cascades.len()).map(|_| vec![panel]).collect();
        let lighting = scene.lighting;
        let sky = move |d: Vec3| lab::scene::environment_radiance(&lighting, d);
        for _ in 0..CONVERGED_FRAMES {
            world.step(&lights, &sky);
        }
        world
    }
}

/// Where cascade 0's distance field starts in the shading data, in vec4s
/// (sdfgi_passes.wgsl's SDFGI_SDF_OFFSET).
pub const SDF_OFFSET: usize = 4 * 2 * (super::probes::MAP_WIDTH * super::probes::MAP_HEIGHT) as usize / 4;

/// The scene shading's static data (sdfgi_shading.wgsl, gShadingData): each
/// cascade's irradiance then radiance layer, as E5B9G9R9 words, four per vec4;
/// then cascade 0's distance field, four bytes per word (sdfgi_passes.wgsl).
pub fn shading_data() -> Vec<[f32; 4]> {
    let world = converged();
    let mut words: Vec<u32> = Vec::new();
    for maps in &world.maps {
        words.extend_from_slice(&maps.irradiance);
        words.extend_from_slice(&maps.radiance);
    }
    debug_assert_eq!(words.len() / 4, SDF_OFFSET);
    words.extend(world.data[0].stored.sdf.chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])));
    words.chunks(4).map(|c| {
        let mut v = [0.0f32; 4];
        for (k, w) in c.iter().enumerate() {
            v[k] = f32::from_bits(*w);
        }
        v
    }).collect()
}

/// gShadingFrame: two vectors per cascade (SDFGIData::ProbeCascadeData):
/// (position relative to SDFGI's camera, to_probe), (probe_world_offset, to_cell).
pub fn frame_cascades(cascades: &Cascades, camera: Vec3) -> Vec<[f32; 4]> {
    let y_mult = cascades.settings.y_mult();
    let cam = Vec3::new(camera.x, camera.y * y_mult, camera.z);
    let mut out = Vec::new();
    for c in &cascades.cascades {
        let pos = (glam::IVec3::splat(-(super::model::CASCADE_SIZE >> 1)) + c.position).as_vec3() * c.cell_size - cam;
        let to_probe = 1.0 / (super::model::CASCADE_SIZE as f32 * c.cell_size / (super::model::PROBE_AXIS_COUNT - 1) as f32);
        let probe = c.position / super::model::PROBE_CELLS;
        out.push([pos.x, pos.y, pos.z, to_probe]);
        out.push([probe.x as f32, probe.y as f32, probe.z as f32, 1.0 / c.cell_size]);
    }
    out
}

/// The screen pass's inputs for a world, with the default bias and energy.
pub fn screen_inputs(world: &World) -> super::screen::Inputs<'_> {
    let frame = frame_cascades(&world.cascades, super::demos::ORIGIN);
    let cascades = frame.chunks(2).map(|c| super::screen::ProbeCascade { position: Vec3::new(c[0][0], c[0][1], c[0][2]), to_probe: c[0][3] }).collect();
    super::screen::Inputs { cascades, maps: world.maps.iter().collect(), y_mult: world.cascades.settings.y_mult(), normal_bias: 1.1, energy: 1.0 }
}

impl World {
    /// The camera moves to `to`: `SDFGI::update`, then for each cascade that
    /// moved, `render_region` (its voxels scrolled or rebuilt, its probe
    /// history scrolled, its maps stored again when the bounce reads them),
    /// as `_update_sdfgi` runs before the frame's light and probe passes.
    pub fn walk(&mut self, objects: &[ObjectData], to: Vec3) {
        let before = self.cascades.clone();
        // The cascade UBO the scroll shader reads: the previous frame's offsets.
        let ubo: Vec<(Vec3, f32)> = before.cascades.iter().map(|c| ((glam::IVec3::splat(-(super::model::CASCADE_SIZE >> 1)) + c.position).as_vec3() * c.cell_size, 1.0 / c.cell_size)).collect();
        self.cascades.update(to);
        let n = self.cascades.cascades.len();
        let old_history = self.history.clone();
        for i in 0..n {
            match self.cascades.cascades[i].dirty {
                super::model::Dirty::All => {
                    self.data[i] = voxels::build_cascade(objects, &self.cascades, i);
                    self.lights_dirty[i] = true;
                }
                super::model::Dirty::Regions(d) if d != glam::IVec3::ZERO => {
                    self.data[i] = voxels::scroll_cascade(objects, &self.cascades, i, &self.data[i].stored.cells, d);
                    let parent = if i + 1 < n { Some(&old_history[i + 1]) } else { None };
                    self.history[i] = probes::scroll_history(&old_history[i], parent, d / super::model::PROBE_CELLS, &ubo, i);
                    if self.bounce_feedback > 0.0 {
                        self.maps[i] = probes::store(&self.history[i]);
                    }
                    self.lights_dirty[i] = true;
                }
                _ => {}
            }
        }
    }
}

/// A static light (Light3D bake mode Static) as the culling sees it: its
/// packed record, its world box (real metres, `light_get_aabb`), and
/// `max_sdfgi_cascade` (2 by default).
#[derive(Clone, Copy, Debug)]
pub struct StaticLight {
    pub light: Light,
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
    pub max_sdfgi_cascade: usize,
}

fn overlaps(a_min: Vec3, a_max: Vec3, b_min: Vec3, b_max: Vec3) -> bool {
    a_min.cmplt(b_max).all() && b_min.cmplt(a_max).all()
}

/// What one frame's culling hands `render_static_lights`: the dirty cascades
/// in region order (`cascade_light_index`) and the light lists, stored by
/// cascade number (`sdfgi_cascade_lights[cascade]`, renderer_scene_cull.cpp).
pub struct StaticCull {
    pub cascade_light_index: Vec<usize>,
    pub lists: Vec<Vec<usize>>,
}

pub fn cull_static(cascades: &Cascades, lights: &[StaticLight]) -> StaticCull {
    let regions = cascades.pending_regions();
    let mut cascade_light_index: Vec<usize> = Vec::new();
    for r in &regions {
        if cascade_light_index.last() != Some(&r.cascade) {
            cascade_light_index.push(r.cascade);
        }
    }
    let mut lists = vec![Vec::new(); 8];
    for (i, l) in lights.iter().enumerate() {
        let mut last: Option<(usize, usize)> = None;
        for r in &regions {
            if overlaps(l.aabb_min, l.aabb_max, r.bounds_min, r.bounds_min + r.bounds_size) && r.cascade <= l.max_sdfgi_cascade && last != Some((i, r.cascade)) {
                last = Some((i, r.cascade));
                lists[r.cascade].push(i);
            }
        }
    }
    StaticCull { cascade_light_index, lists }
}

/// `render_static_lights` as it reads the cull: dirty cascade `i` of the
/// list takes the lights stored under index `i` (not under its own cascade
/// number), keeps those within `max_sdfgi_cascade` whose box meets the
/// cascade's box (squashed y on one side, real y on the other), and is
/// lit by them. Returns, per cascade, the lights it was given.
pub fn static_pass_lights(cascades: &Cascades, cull: &StaticCull, lights: &[StaticLight]) -> Vec<(usize, Vec<Light>)> {
    let any = cull.cascade_light_index.iter().enumerate().any(|(i, _)| !cull.lists[i].is_empty());
    if !any {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, &c) in cull.cascade_light_index.iter().enumerate() {
        let cc = &cascades.cascades[c];
        let cmin = (glam::IVec3::splat(-(super::model::CASCADE_SIZE >> 1)) + cc.position).as_vec3() * cc.cell_size;
        let cmax = cmin + Vec3::splat(super::model::CASCADE_SIZE as f32 * cc.cell_size);
        let given: Vec<Light> = cull.lists[i].iter().map(|&k| &lights[k]).filter(|l| c <= l.max_sdfgi_cascade && overlaps(cmin, cmax, l.aabb_min, l.aabb_max)).take(1024).map(|l| l.light).collect();
        out.push((c, given));
    }
    out
}

impl World {
    /// The frame's static lights, culled against its dirty regions and
    /// written into the cells (`render_static_lights`), before `step`.
    pub fn static_frame(&mut self, lights: &[StaticLight]) -> Vec<(usize, Vec<Light>)> {
        let cull = cull_static(&self.cascades, lights);
        let given = static_pass_lights(&self.cascades, &cull, lights);
        let field_cascades = self.cascades.clone();
        let sdf: Vec<Vec<u8>> = self.data.iter().map(|d| d.stored.sdf.clone()).collect();
        let field = Field { cascades: &field_cascades, sdf: sdf.iter().map(|s| s.as_slice()).collect() };
        for (c, ls) in &given {
            if !ls.is_empty() {
                light::static_pass(&field, *c, &mut self.data[*c].stored.cells, ls);
            }
        }
        given
    }
}

/// An omni light as `pre_process_gi` packs it (y times y_mult, Light3D's
/// defaults of attenuation 1 and energy 1 unless given), and its static box.
pub fn omni(position: Vec3, range: f32, energy: f32, y_mult: f32) -> StaticLight {
    StaticLight {
        light: Light { kind: light::LIGHT_TYPE_OMNI, color: Vec3::ONE, energy, direction: Vec3::NEG_Z, position: Vec3::new(position.x, position.y * y_mult, position.z), attenuation: 1.0, radius: range, cos_spot_angle: 0.0, inv_spot_attenuation: 1.0, area_width: Vec3::ZERO, area_height: Vec3::ZERO },
        aabb_min: position - Vec3::splat(range),
        aabb_max: position + Vec3::splat(range),
        max_sdfgi_cascade: 2,
    }
}
