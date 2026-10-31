//! The VoxelGI investigation's verification suite: the CPU model against the
//! engine's rules read from Godot's source, the WGSL ports against the CPU
//! model, and the source index against the Godot checkout. Every check
//! records what it compared and the result in the JSON report.

use super::bake::{self, Aabb, CHILD_EMPTY};
use super::cone;
use super::light;
use super::sdf;
use super::shot::{self, Demo};
use super::storage;
use super::world::{self, LightSettings, NODE};
use anyhow::Result;
use glam::{IVec3, Vec2, Vec3, Vec4};
use lab::utility::selftest::{request, still_timeline, Report};
use serde_json::json;
use std::path::PathBuf;

/// A small xorshift for sampled checks.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn rel(a: f32, b: f32) -> f32 {
    (a - b).abs() / b.abs().max(1e-3)
}

pub fn run(options: &lab::investigation::SelftestOptions) -> Result<bool> {
    std::fs::create_dir_all(&options.work_dir)?;
    let mut report = Report::new();
    let w = world::world();
    let v = &w.voxelizer;
    let probe = &w.probe;

    // ---- 1. begin_bake: the cube and the cells -------------------------------
    report.check(
        "begin_bake: a 6.4 x 3.2 x 6.4 m box at subdiv 128 is 128 x 64 x 128 cells of 5 cm in a 6.4 m cube",
        v.octree_size() == [128, 64, 128] && (v.cell_size - 0.05).abs() < 1e-6 && v.po2_bounds.size == Vec3::splat(6.4),
        json!({"octreeSize": v.octree_size(), "cellSize": v.cell_size, "po2": v.po2_bounds.size.to_array()}),
    );
    let default_box = bake::Voxelizer::begin_bake(7, Aabb { position: Vec3::splat(-10.0), size: Vec3::splat(20.0) });
    let flat = bake::Voxelizer::begin_bake(7, Aabb { position: Vec3::ZERO, size: Vec3::new(20.0, 10.0, 5.0) });
    let twelve = bake::Voxelizer::begin_bake(7, Aabb { position: Vec3::ZERO, size: Vec3::new(20.0, 12.0, 20.0) });
    report.check(
        "begin_bake: the default 20 m box is 128 cubed cells of 15.6 cm; 20 x 10 x 5 gives 128 x 64 x 32; a 12 m side does not halve (10 < 12)",
        default_box.octree_size() == [128, 128, 128] && (default_box.cell_size - 0.15625).abs() < 1e-6 && flat.octree_size() == [128, 64, 32] && twelve.octree_size() == [128, 128, 128],
        json!({"default": [default_box.octree_size(), [default_box.cell_size as i32]], "flat": flat.octree_size(), "twelve": twelve.octree_size()}),
    );

    // ---- 2. The separating-axis test ------------------------------------------
    let tri = [Vec3::new(-0.2, 0.0, -0.2), Vec3::new(0.3, 0.1, -0.1), Vec3::new(0.0, -0.1, 0.4)];
    let inside = bake::triangle_box_overlap(Vec3::ZERO, Vec3::splat(1.0), &tri);
    let far = bake::triangle_box_stage(Vec3::new(5.0, 0.0, 0.0), Vec3::splat(1.0), &tri);
    // A triangle whose bounds overlap the box's corner while its plane misses it.
    let slanted = [Vec3::new(1.5, 0.0, 0.0), Vec3::new(0.0, 1.5, 0.0), Vec3::new(0.0, 0.0, 1.5)];
    let corner = bake::triangle_box_stage(Vec3::new(-0.6, -0.6, -0.6), Vec3::splat(0.5), &slanted);
    // Sampled: a triangle with a vertex inside the box always overlaps it.
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let mut vertex_inside_ok = true;
    for _ in 0..2000 {
        let c = Vec3::new(rng.next(), rng.next(), rng.next()) * 2.0 - Vec3::ONE;
        let h = Vec3::new(rng.next(), rng.next(), rng.next()) * 0.5 + Vec3::splat(0.05);
        let a = c + (Vec3::new(rng.next(), rng.next(), rng.next()) * 2.0 - Vec3::ONE) * h * 0.99;
        let b = a + (Vec3::new(rng.next(), rng.next(), rng.next()) * 2.0 - Vec3::ONE);
        let d = a + (Vec3::new(rng.next(), rng.next(), rng.next()) * 2.0 - Vec3::ONE);
        vertex_inside_ok &= bake::triangle_box_overlap(c, h, &[a, b, d]);
    }
    report.check(
        "triangle_box_overlap: a triangle inside overlaps; one far along x and one beside a corner are separated (by the axis the report names, edge axes first, as the engine tests them); any vertex inside means overlap (2000 samples)",
        inside && far != bake::SatStage::Overlap && corner != bake::SatStage::Overlap && vertex_inside_ok,
        json!({"far": format!("{far:?}"), "corner": format!("{corner:?}")}),
    );

    // ---- 3. The octree ----------------------------------------------------------
    let counts = &probe.level_counts;
    let total: u32 = counts.iter().sum();
    let sorted = v.cells.windows(2).all(|p| (p[0].level, p[0].x, p[0].y, p[0].z) <= (p[1].level, p[1].x, p[1].y, p[1].z));
    let children_finer = v.cells.iter().enumerate().all(|(i, c)| c.children.iter().all(|&ch| ch == CHILD_EMPTY || (ch as usize > i && v.cells[ch as usize].level == c.level + 1)));
    let leaves_ok = v.cells.iter().filter(|c| c.level as i32 == v.cell_subdiv).all(|c| c.alpha == 1.0 && c.children.iter().all(|&ch| ch == CHILD_EMPTY));
    let parents_empty = v.cells.iter().filter(|c| (c.level as i32) < v.cell_subdiv).all(|c| c.albedo == [0.0; 3] && c.emission == [0.0; 3] && c.normal == [0.0; 3] && c.alpha <= 1.0);
    let parent_alpha = v.cells.iter().filter(|c| c.level as i32 == v.cell_subdiv - 1).all(|c| {
        let n = c.children.iter().filter(|&&ch| ch != CHILD_EMPTY).count();
        (c.alpha - n as f32 / 8.0).abs() < 1e-6
    });
    report.check(
        "the bake: cells sorted by level, then x, y, z; the root alone at level 0; children one level finer and later; leaves alpha 1; parents keep no colour, their alpha the children's sum / 8",
        sorted && counts[0] == 1 && total as usize == v.cells.len() && children_finer && leaves_ok && parents_empty && parent_alpha && v.leaf_voxel_count == counts[7] as usize,
        json!({"levelCounts": counts, "cells": v.cells.len(), "leaves": v.leaf_voxel_count, "bakeSeconds": w.bake_seconds}),
    );
    // Every object of the room is baked: its leaves when it is baked alone.
    let per_object: Vec<(String, usize, usize)> = w.meshes.iter().map(|m| {
        let mut alone = bake::Voxelizer::begin_bake(world::SUBDIV, world::bounds());
        alone.plot_mesh(m);
        let leaves = alone.cells.iter().filter(|c| c.level as i32 == alone.cell_subdiv).count();
        (w.scene.objects[m.object].name.to_string(), m.triangles.len(), leaves)
    }).collect();
    report.check("every object but the ceiling panel is baked, each into leaves of its own", per_object.iter().all(|o| o.2 > 0) && per_object.len() == w.scene.objects.len() - 1, json!(per_object));
    // Every leaf is found by walking down from the root, and only leaves are.
    let leaves = probe.level_range(probe.leaf_level());
    let found = leaves.clone().step_by(97).all(|c| probe.find_leaf(probe.data[c].pos()) == Some(c));
    let empty_cell = IVec3::new(64, 40, 64);
    report.check("the octree: a walk from the root finds each sampled leaf at its position; the room's empty middle has none", found && probe.find_leaf(empty_cell).is_none(), json!({"sampled": leaves.len() / 97, "emptyCell": empty_cell.to_array()}));

    // ---- 4. The records -----------------------------------------------------------
    let mut decode_ok = true;
    let mut worst_albedo = 0.0f32;
    for (i, c) in v.cells.iter().enumerate().step_by(31) {
        let d = probe.data[i];
        decode_ok &= d.pos() == IVec3::new(c.x as i32, c.y as i32, c.z as i32);
        worst_albedo = worst_albedo.max((d.albedo().truncate() - Vec3::from(c.albedo)).abs().max_element());
        if c.level as i32 == v.cell_subdiv && Vec3::from(c.normal) != Vec3::ZERO {
            decode_ok &= (d.normal() - Vec3::from(c.normal)).abs().max_element() < 1.0 / 127.0 + 1e-6;
        }
    }
    let samples = [Vec3::new(1.0, 0.5, 0.25), Vec3::new(6.0, 0.0, 0.003), Vec3::new(1000.0, 2.0, 0.1), Vec3::new(0.001, 0.002, 0.0005)];
    let rgbe: Vec<(u32, [f32; 3])> = samples.iter().map(|&s| {
        let e = storage::to_rgbe9995(s);
        (e, storage::decode_rgbe9995(e).to_array())
    }).collect();
    let rgbe_ok = samples.iter().zip(&rgbe).all(|(s, (_, d))| (Vec3::from(*d) - *s).abs().max_element() <= s.max_element() / 256.0 + 1e-6) && storage::to_rgbe9995(Vec3::ZERO) == storage::to_rgbe9995(Vec3::ZERO);
    report.check(
        "CellData: positions decode 11/10/11 bits to the cells' own; albedo within one 8-bit step (truncated); normals within one snorm step; RGB9E5 emission within 1/256 of its largest channel",
        decode_ok && worst_albedo < 1.0 / 255.0 + 1e-6 && rgbe_ok,
        json!({"worstAlbedo": worst_albedo, "rgbe9995": rgbe.iter().map(|(e, d)| json!({"word": format!("{e:#010x}"), "decoded": d})).collect::<Vec<_>>()}),
    );

    // ---- 5. The distance field ----------------------------------------------------
    let leaf_cells: Vec<IVec3> = leaves.clone().map(|c| probe.data[c].pos()).collect();
    let solid_zero = leaf_cells.iter().step_by(13).all(|&p| probe.sdf_at(p) == 0);
    let mut exact = 0;
    let mut checked = 0;
    let s = probe.octree_size;
    for _ in 0..300 {
        let p = IVec3::new((rng.next() * s.x as f32) as i32, (rng.next() * s.y as f32) as i32, (rng.next() * s.z as f32) as i32).min(s - IVec3::ONE);
        let best = leaf_cells.iter().map(|&q| (q - p).length_squared()).min().unwrap() as f32;
        checked += 1;
        if probe.sdf_at(p) == sdf::encode(best) {
            exact += 1;
        }
    }
    report.check("the distance field: every leaf stores 0; at sampled texels the byte is min(floor(sqrt(d^2)), 254) + 1 of the exact nearest leaf", solid_zero && exact == checked, json!({"checked": checked, "exact": exact}));

    // ---- 6. The light passes ------------------------------------------------------
    let lit = &w.lit;
    let params = world::params(probe, &LightSettings::default());
    report.check("cell_size: ps = octree_size / size = 20 cells per metre in every axis, 0.05 m per cell", (params.cell_size - 0.05).abs() < 1e-7, json!({"cellSize": params.cell_size}));
    let mut write_ok = true;
    for c in (0..probe.data.len()).step_by(53) {
        let d = probe.data[c];
        let mip = probe.mip_count() - 1 - probe.level_of(c);
        let t = lit.texture.mips[mip][lit.texture.index(mip, d.pos())];
        write_ok &= t == storage::unorm8((lit.outputs[c] / world::DYNAMIC_RANGE).extend(d.albedo().w));
    }
    // A parent's light: the children's sum / mix(8, count, propagation), at 0 and 1.
    let parent = probe.level_range(probe.leaf_level() - 1).find(|&c| probe.octree[c].iter().filter(|&&ch| ch != CHILD_EMPTY).count() == 4).expect("a half-full parent");
    let sum: Vec3 = probe.octree[parent].iter().filter(|&&ch| ch != CHILD_EMPTY).map(|&ch| lit.direct[ch as usize]).sum();
    let p0 = light::mip_cell(probe, &light::Params { propagation: 0.0, ..params }, &lit.direct, parent).0;
    let p1 = light::mip_cell(probe, &light::Params { propagation: 1.0, ..params }, &lit.direct, parent).0;
    let mip_ok = (p0 - sum / 8.0).abs().max_element() < 1e-6 && (p1 - sum / 4.0).abs().max_element() < 1e-6;
    report.check("the mip pass: a parent of 4 children holds their sum / 8 at propagation 0 and / 4 at 1; the write pass stores light / dynamic range and coverage in RGBA8", write_ok && mip_ok, json!({"parent": parent, "sum": sum.to_array(), "prop0": p0.to_array(), "prop1": p1.to_array()}));
    let leaves_v: Vec<usize> = leaves.clone().collect();
    let brighter = leaves_v.iter().all(|&c| (lit.outputs[c] - lit.direct[c]).min_element() >= -1e-6);
    let gained = leaves_v.iter().filter(|&&c| lit.outputs[c].max_element() > lit.direct[c].max_element() * 1.05 + 1e-4).count();
    let unlit_direct = leaves_v.iter().filter(|&&c| lit.direct[c].max_element() < 1e-5).count();
    let unlit_after = leaves_v.iter().filter(|&&c| lit.outputs[c].max_element() < 1e-5).count();
    report.check("the second bounce only adds light: no leaf loses light; it lights leaves the panel's area light did not reach", brighter && gained > 0 && unlit_after < unlit_direct, json!({"leaves": leaves_v.len(), "gainedOver5pc": gained, "unlitDirect": unlit_direct, "unlitAfter": unlit_after, "lightSeconds": w.light_seconds}));
    // The area light has no shadow: the floor cell under the sphere's centre still receives it.
    let under = demos_leaf(Vec3::new(-1.33, 0.0, -0.64));
    let under_light = under.map(|c| light::compute_light(probe, &params, &w.lights, c)).unwrap_or(Vec3::ZERO);
    let below = demos_leaf(Vec3::new(0.0, 0.0, -0.2));
    let below_light = below.map(|c| light::compute_light(probe, &params, &w.lights, c)).unwrap_or(Vec3::ZERO);
    let normal_under = under.map(|c| probe.data[c].normal().to_array());
    report.check("compute_area_light has no shadow march: a floor cell in the sphere's shadow from the panel still receives the panel's light", under_light.max_element() > 0.0, json!({"cell": under, "normal": normal_under, "light": under_light.to_array(), "underPanel": below_light.to_array(), "panel": format!("{:?}", w.lights[0])}));
    // The omni light's shadow: the light demonstration's worked cell is in the sphere's shadow.
    let omni = world::omni_light(probe, Vec3::new(0.9, 1.4, -0.4));
    let shadowed = demos_leaf(super::demos::LIGHT_POINT).map(|c| {
        let d = probe.data[c];
        let pos = d.pos().as_vec3() + Vec3::splat(0.5);
        let mut t = light::LightTrace::default();
        let r = light::light_at_pos(probe, &params, &omni, pos, d.normal(), Some(&mut t));
        (r.is_none(), t.steps.len(), t.occlusion)
    });
    report.check("raymarch: the omni light's shadow ray from the worked floor cell meets the sphere's solid cells (occlusion 0)", shadowed.is_some_and(|s| s.0 && s.2 == 0.0), json!(format!("{shadowed:?}")));

    // ---- 7. The cone traces -------------------------------------------------------
    let inst = world::instance(probe, &LightSettings::default(), false);
    let mut steps = Vec::new();
    cone::cone_trace_45(&lit.texture, Vec3::ONE / inst.bounds, Vec3::new(64.0, 10.0, 64.0), Vec3::new(0.0, 0.7071, 0.7071), inst.bounds.length(), inst.bias, Some(&mut steps));
    let doubles = steps.windows(2).all(|p| (p[1].dist - 2.0 * p[0].dist).abs() < 1e-4 && (p[1].lod - p[0].lod - 1.0).abs() < 1e-5) && steps.first().is_some_and(|s| (s.lod - 3f32.log2()).abs() < 1e-5);
    report.check("voxel_cone_trace_45_degrees: from bias 1.5 each step doubles the distance and climbs one mip, from log2(3)", doubles, json!(steps.iter().map(|s| [s.dist, s.lod]).collect::<Vec<_>>()));
    // The edge fade cancels for one instance: the ambient is the cones' light, not light x blend.
    let p = Vec3::new(2.6, 0.5, 1.0);
    let n = Vec3::NEG_X;
    let mut cones = cone::Cones::default();
    let (amb, _) = cone::process(&inst, &lit.texture, p, n, n, 0.5, false, Some(&mut cones));
    let cancels = cones.blend > 0.0 && cones.blend < 0.3 && (amb - cones.light).abs().max_element() < 1e-5;
    let face = inst.from_cell(Vec3::new(64.0, 0.0, 64.0));
    let (at_face, _) = cone::process(&inst, &lit.texture, face, Vec3::Y, Vec3::Y, 0.5, false, None);
    report.check("the edge fade: one VoxelGI's sums are divided by its own blend, so near a face (blend < 0.3) the ambient is the cones' full light; only on a face (blend 0) does it vanish", cancels && at_face == Vec4::ZERO, json!({"blend": cones.blend, "ambient": amb.to_array(), "cones": cones.light.to_array(), "atFace": at_face.to_array()}));

    // ---- 8. The dynamic pass --------------------------------------------------------
    {
        let dw = world::dynamic_world();
        let started = std::time::Instant::now();
        let plot = world::dynamic_plot(0.0);
        let seconds = started.elapsed().as_secs_f32();
        let teapot = world::teapot_at(0.0);
        let c = Vec3::from(teapot.world_center);
        let near = plot.plotted[0].iter().all(|&p| (super::demos::cell_center(&dw.probe, dw.probe.leaf_level(), p) - c).abs().max_element() < 0.6);
        let changed = plot.plotted[0].iter().filter(|&&p| plot.texture.texel(0, p) != dw.lit.texture.texel(0, p)).count();
        let untouched = (0..dw.lit.texture.mips[0].len()).step_by(101).all(|i| {
            let s = dw.probe.octree_size;
            let p = IVec3::new(i as i32 % s.x, (i as i32 / s.x) % s.y, i as i32 / (s.x * s.y));
            plot.plotted[0].binary_search_by_key(&(p.z, p.y, p.x), |v| (v.z, v.y, v.x)).is_ok() || plot.texture.mips[0][i] == dw.lit.texture.mips[0][i]
        });
        let maps = plot.views[0].maps.len();
        report.check("the dynamic pass: six views, 4 oversampled maps then one per mip; the plotted voxels lie at the teapot and change their texels; texels it does not plot keep the static light", plot.views.len() == 6 && maps == 4 + dw.probe.mip_count() && !plot.plotted[0].is_empty() && near && changed > 0 && untouched, json!({"box": [plot.begin.to_array(), plot.end.to_array()], "plottedMip0": plot.plotted[0].len(), "changed": changed, "maps": maps, "seconds": seconds}));
    }

    // ---- 9. The WGSL ports against the CPU -----------------------------------------
    {
        let gpu = lab::gpu::Gpu::new(lab::gpu::instance(None), None)?;
        let statics = lab::frame::Statics::prepare(lab::scene::SceneId::STUDY_ROOM)?;
        let mut renderer = lab::renderer::Renderer::new(gpu, statics, 640, 360)?;
        let still = still_timeline(640, 360);
        renderer.render_frame(request(&shot::for_demo(Demo::Room), &still), None);
        // The screen pass at points on the room's surfaces, both qualities, inside and outside.
        let mut cases: Vec<(Vec3, Vec3, Vec3, f32, bool, bool)> = Vec::new();
        for k in 0..48 {
            let (p, n) = match k % 4 {
                0 => (Vec3::new(rng.next() * 5.8 - 2.9, 0.0, rng.next() * 5.8 - 2.9), Vec3::Y),
                1 => (Vec3::new(-3.0, rng.next() * 2.9, rng.next() * 5.8 - 2.9), Vec3::X),
                2 => (Vec3::new(rng.next() * 5.8 - 2.9, rng.next() * 2.9, -3.0), Vec3::Z),
                _ => (Vec3::new(rng.next() * 6.0 - 3.0, rng.next() * 3.0, rng.next() * 6.0 - 3.0), (Vec3::new(rng.next(), rng.next(), rng.next()) - Vec3::splat(0.5)).normalize()),
            };
            let view = (Vec3::new(0.0, 1.6, 4.0) - p).normalize();
            let r = lab::scene::reflect(-view, n);
            cases.push((p, n, r, rng.next(), k % 3 == 0, k % 5 != 0));
        }
        let tests: Vec<[f32; 4]> = cases.iter().flat_map(|&(p, n, r, rough, high, blend)| [p.extend(rough).to_array(), n.extend(high as u32 as f32).to_array(), r.extend(blend as u32 as f32).to_array()]).collect();
        let words = renderer.run_shading_verify("voxelgiProcessTests", cases.len() as u32, &tests, 8 * cases.len());
        let mut worst = 0.0f32;
        for (k, &(p, n, r, rough, high, blend)) in cases.iter().enumerate() {
            let inst = world::instance(probe, &LightSettings::default(), !blend);
            let (a, s) = cone::process(&inst, &lit.texture, p, n, r, rough, high, None);
            let cpu = [a.x, a.y, a.z, a.w, s.x, s.y, s.z, s.w];
            for (i, c) in cpu.iter().enumerate() {
                worst = worst.max(rel(f32::from_bits(words[k * 8 + i]), *c));
            }
        }
        report.check("screen pass: the WGSL port of voxel_gi_compute matches cone.rs (ambient and reflection, rgba; Low and High; interior on and off)", worst < 2e-3, json!({"cases": cases.len(), "worstRelative": worst}));

        // The light pass: every light type at sampled leaves.
        let lights = [
            w.lights[0],
            omni,
            light::Light { kind: light::LIGHT_TYPE_SPOT, cos_spot_angle: 40f32.to_radians().cos(), direction: Vec3::new(0.2, -1.0, 0.1).normalize(), ..omni },
            light::Light { kind: light::LIGHT_TYPE_DIRECTIONAL, direction: Vec3::new(0.3, -0.8, 0.2).normalize(), ..omni },
        ];
        let cells: Vec<usize> = leaves.clone().step_by(leaves.len() / 96 + 1).collect();
        let mut tests = Vec::new();
        for (k, &c) in cells.iter().enumerate() {
            let l = lights[k % 4];
            let d = probe.data[c];
            tests.extend([[l.kind as f32, l.energy, l.radius, l.attenuation], l.color.extend(l.cos_spot_angle).to_array(), l.position.extend(l.inv_spot_attenuation).to_array(), l.direction.extend(l.has_shadow as u32 as f32).to_array(), l.area_width.extend(params.cell_size).to_array(), l.area_height.extend(0.0).to_array(), (d.pos().as_vec3() + Vec3::splat(0.5)).extend(0.0).to_array(), d.normal().extend(0.0).to_array()]);
        }
        let words = renderer.run_shading_verify("voxelgiLightTests", cells.len() as u32, &tests, 4 * cells.len());
        let mut worst = 0.0f32;
        let mut mismatched = 0;
        for (k, &c) in cells.iter().enumerate() {
            let l = lights[k % 4];
            let d = probe.data[c];
            let pos = d.pos().as_vec3() + Vec3::splat(0.5);
            let n = d.normal();
            let cpu = if l.kind == light::LIGHT_TYPE_AREA {
                light::area_light(&params, &l, pos, n).unwrap_or(Vec3::ZERO)
            } else {
                light::light_at_pos(probe, &params, &l, pos, n, None).map(|(li, dir)| if n.length() > 0.2 { n.dot(-dir).max(0.0) * li } else { li }).unwrap_or(Vec3::ZERO)
            };
            let gpu = Vec3::new(f32::from_bits(words[k * 4]), f32::from_bits(words[k * 4 + 1]), f32::from_bits(words[k * 4 + 2]));
            let d = (gpu - cpu).abs().max_element() / cpu.max_element().max(1e-3);
            if d > 1e-3 {
                mismatched += 1;
            }
            worst = worst.max(d);
        }
        // A shadow ray that grazes an edge can flip between float orders; allow one.
        report.check("light pass: the WGSL port of compute_light_at_pos (with raymarch) and compute_area_light matches light.rs (area, omni, spot, directional)", mismatched <= 1, json!({"cases": cells.len(), "mismatched": mismatched, "worstRelative": worst}));

        // The bounce's cone, from sampled leaves along their normals.
        let mut cases = Vec::new();
        for &c in leaves.clone().step_by(leaves.len() / 48 + 1).collect::<Vec<_>>().iter() {
            let d = probe.data[c];
            let n = if d.normal().length() > 0.2 { d.normal() } else { Vec3::Y };
            cases.push((d.pos().as_vec3() + Vec3::splat(0.5), n));
        }
        let tests: Vec<[f32; 4]> = cases.iter().flat_map(|(p, d)| [p.extend(0.0).to_array(), d.extend(0.0).to_array()]).collect();
        let words = renderer.run_shading_verify("voxelgiBounceTests", cases.len() as u32, &tests, 4 * cases.len());
        let mut worst = 0.0f32;
        for (k, (p, d)) in cases.iter().enumerate() {
            let c = light::bounce_cone(&lit.texture, &params, *p, *d, None);
            for i in 0..4 {
                worst = worst.max(rel(f32::from_bits(words[k * 4 + i]), c[i]));
            }
        }
        report.check("second bounce: the WGSL port of its cone matches light.rs", worst < 2e-3, json!({"cases": cases.len(), "worstRelative": worst}));

        // A click on a floor cell's projected centre picks that cell's column.
        {
            let state = shot::for_demo(Demo::Room);
            let camera = state.camera.camera_data([640.0, 360.0]);
            let projector = lab::camera::Projector::new(&camera);
            let target = Vec3::new(0.4, 0.0, 1.2);
            let cell = super::demos::leaf_of(probe, target - Vec3::Y * 0.01);
            let center = super::demos::cell_center(probe, probe.leaf_level(), cell) * Vec3::new(1.0, 0.0, 1.0);
            let content = lab::frame::FrameContent::new(lab::scene::Scene::study_room().records(), lab::layout::FrameUniforms::default());
            let statics = lab::frame::Statics::prepare(lab::scene::SceneId::STUDY_ROOM)?;
            let mut picked = state.clone();
            let hit = projector.project(center).map(|(px, _)| {
                use lab::investigation::Investigation;
                super::VOXELGI.click(&state, &mut picked, &content, &statics, &projector, px)
            });
            let got = IVec3::new(picked.p("cellX") as i32, picked.p("cellY") as i32, picked.p("cellZ") as i32);
            report.check("a click on a floor cell's centre picks that cell", hit == Some(true) && got == cell, json!({"cell": cell.to_array(), "picked": got.to_array()}));
            let _ = Vec2::ZERO;
        }

        // Every scene beat of a timeline, rendered at its middle: text, labels, listings, readouts.
        if let Some(path) = &options.timeline {
            let timeline = lab::shot::Timeline::load(path)?;
            let mut problems = Vec::new();
            let mut beats = 0;
            for beat in timeline.beats.iter().filter(|b| b.kind == lab::shot::SceneType::ThreeD) {
                let frame = beat.start_frame + beat.frames / 2;
                let state = timeline.state_at(frame);
                let mut req = request(&state, &timeline);
                req.chrome = lab::ui::Chrome::Recording { timeline: &timeline, frame };
                req.chapter_number = beat.chapter;
                let r = renderer.render_frame(req, None).report;
                beats += 1;
                if r.min_text_pt < lab::ui::MIN_TEXT_PT || r.label_overlaps > 0 || r.labels_outside > 0 || r.code_overflow > 0 || r.readout_overflow > 0 || r.formula_scale.is_some_and(|s| s < 1.0) {
                    problems.push(json!({"beat": beat.id, "frame": frame, "minTextPt": r.min_text_pt, "labelOverlaps": r.label_overlaps, "labelsOutside": r.labels_outside, "codeOverflow": r.code_overflow, "readoutOverflow": r.readout_overflow}));
                }
            }
            report.check("every scene beat of the timeline: text >= 24 px, labels inside and apart, listings and readouts fit", problems.is_empty() && beats > 0, json!({"timeline": path, "sceneBeats": beats, "problems": problems}));
        }
    }

    // Listings: each cites a source entry and a demonstration, and every demonstration has one.
    {
        let index = super::sources::INDEX.get();
        let listings = super::pseudocode::all();
        let bad: Vec<&str> = listings.iter().filter(|l| !index.entries.contains_key(&l.source) || l.demo.as_deref().is_none_or(|d| Demo::ALL.iter().all(|x| x.key() != d))).map(|l| l.name).collect();
        let without: Vec<&str> = Demo::ALL.iter().filter(|d| shot::demos().spec(d.id()).listing.is_none()).map(|d| d.key()).collect();
        report.check("every listing cites a source entry and a demonstration; every demonstration has a listing", bad.is_empty() && without.is_empty(), json!({"listings": listings.len(), "bad": bad, "demosWithout": without}));
    }

    let states_ok = Demo::ALL.iter().all(|&d| shot::for_demo(d).demo == d.id());
    report.check("every demonstration's file loads", states_ok, json!(Demo::ALL.iter().map(|d| d.key()).collect::<Vec<_>>()));

    // The story's action (actions.rs): marchCone, started at frame 30, one step every 0.5 s (15 frames).
    {
        let mut timeline = still_timeline(64, 64);
        (timeline.total_frames, timeline.beats[0].frames, timeline.chapters[0].end_frame) = (400, 400, 400);
        timeline.keys = serde_json::from_value(json!([{"frame": 0, "demo": "pixel"}, {"frame": 30, "run": {"name": "marchCone", "args": {"seconds": 0.5, "to": 6}}}, {"frame": 300, "stop": ["marchCone"]}]))?;
        let valid = timeline.validate().map_err(|e| e.to_string());
        let frames = [20u64, 30, 44, 45, 60, 200, 300];
        let steps: Vec<f32> = frames.iter().map(|&f| timeline.state_at(f).p("coneSteps")).collect();
        let expected = [16.0, 0.0, 0.0, 1.0, 2.0, 6.0, 16.0];
        report.check("the marchCone action steps coneSteps through a timeline up to its end, and stopping it hands coneSteps back", valid.is_ok() && steps == expected, json!({"valid": valid, "frames": frames, "steps": steps, "expected": expected}));
    }

    // The worked elements' places (for the story's cameras).
    {
        let (leaf_box, leaf) = super::demos::worked_leaf(w);
        let light_cell = demos_leaf(super::demos::LIGHT_POINT).map(|c| super::demos::cell_center(probe, probe.leaf_level(), probe.data[c].pos()).to_array());
        let bounce_cell = demos_leaf(super::demos::BOUNCE_POINT).map(|c| super::demos::cell_center(probe, probe.leaf_level(), probe.data[c].pos()).to_array());
        let rejected: Vec<[f32; 4]> = super::demos::descent(w).iter().filter(|d| !d.overlaps).map(|d| (d.aabb.center() + NODE).extend(d.level as f32).to_array()).collect();
        let max_light = lit.outputs[leaves.clone()].iter().map(|v| v.max_element()).fold(0.0f32, f32::max);
        report.check("the worked elements (information)", true, json!({"bakePoint": super::demos::bake_point(&w.scene).to_array(), "leafCentre": (leaf_box.center() + NODE).to_array(), "leaf": leaf.to_array(), "rejectedChildren": rejected, "lightCell": light_cell, "bounceCell": bounce_cell, "brightestLeafLight": max_light}));
    }

    // ---- 10. The source index against the checkout ------------------------------
    let index = super::sources::INDEX.get();
    report.check("source index", true, json!({"engine": index.engine.name, "version": index.engine.version, "commit": index.engine.commit, "entries": index.entries.len()}));
    let checkout = std::env::var_os("GODOT_SOURCE_DIR").map(PathBuf::from).unwrap_or_else(|| lab::resources::root().join("../../godot"));
    report.cited_lines("Godot", &super::sources::INDEX, &checkout, "version.py")?;

    let passed = report.passed();
    std::fs::write(&options.report, serde_json::to_string_pretty(&json!({"passed": passed, "checks": report.checks}))?)?;
    let _ = NODE;
    Ok(passed)
}

/// The leaf a world point falls in, if the octree has one there (or next to it).
fn demos_leaf(p: Vec3) -> Option<usize> {
    let w = world::world();
    super::demos::nearest_leaf(&w.probe, super::demos::leaf_of(&w.probe, p)).map(|c| c.0)
}
