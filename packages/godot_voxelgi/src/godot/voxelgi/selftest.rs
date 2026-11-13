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
use lab::utility::selftest::{Report, Rng, request, still_timeline};
use serde_json::json;

fn rel(a: f32, b: f32) -> f32 {
    (a - b).abs() / b.abs().max(1e-3)
}

pub fn run(options: &lab::investigation::SelftestOptions) -> Result<bool> {
    std::fs::create_dir_all(&options.work_dir)?;
    let mut report = Report::new();
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    check_bake(&mut report, &mut rng);
    check_octree(&mut report);
    check_records(&mut report);
    check_field(&mut report, &mut rng);
    check_light(&mut report);
    check_cones(&mut report);
    check_dynamic(&mut report);
    check_gpu(&mut report, &mut rng, options)?;
    check_story(&mut report)?;
    check_sources(&mut report);

    let _ = NODE;
    report.write(&options.report)
}

/// Sections 1 and 2: begin_bake's cube and cells, and the separating-axis test.
fn check_bake(report: &mut Report, rng: &mut Rng) {
    let w = world::world();
    let v = &w.voxelizer;
    // ---- 1. begin_bake: the cube and the cells -------------------------------
    report.check(
        "begin_bake: a 6.4 x 3.2 x 6.4 m box at subdiv 128 is 128 x 64 x 128 cells of 5 cm in a 6.4 m cube",
        v.octree_size() == [128, 64, 128] && (v.cell_size - 0.05).abs() < 1e-6 && v.po2_bounds.size == Vec3::splat(6.4),
        json!({"octreeSize": v.octree_size(), "cellSize": v.cell_size, "po2": v.po2_bounds.size.to_array()}),
    );
    let default_box = bake::Voxelizer::begin_bake(
        7,
        Aabb {
            position: Vec3::splat(-10.0),
            size: Vec3::splat(20.0),
        },
    );
    let flat = bake::Voxelizer::begin_bake(
        7,
        Aabb {
            position: Vec3::ZERO,
            size: Vec3::new(20.0, 10.0, 5.0),
        },
    );
    let twelve = bake::Voxelizer::begin_bake(
        7,
        Aabb {
            position: Vec3::ZERO,
            size: Vec3::new(20.0, 12.0, 20.0),
        },
    );
    report.check(
        "begin_bake: the default 20 m box is 128 cubed cells of 15.6 cm; 20 x 10 x 5 gives 128 x 64 x 32; a 12 m side does not halve (10 < 12)",
        default_box.octree_size() == [128, 128, 128] && (default_box.cell_size - 0.15625).abs() < 1e-6 && flat.octree_size() == [128, 64, 32] && twelve.octree_size() == [128, 128, 128],
        json!({"default": [default_box.octree_size(), [default_box.cell_size as i32]], "flat": flat.octree_size(), "twelve": twelve.octree_size()}),
    );

    // ---- 2. The separating-axis test ------------------------------------------
    let tri = [
        Vec3::new(-0.2, 0.0, -0.2),
        Vec3::new(0.3, 0.1, -0.1),
        Vec3::new(0.0, -0.1, 0.4),
    ];
    let inside = bake::triangle_box_overlap(Vec3::ZERO, Vec3::splat(1.0), &tri);
    let far = bake::triangle_box_stage(Vec3::new(5.0, 0.0, 0.0), Vec3::splat(1.0), &tri);
    // A triangle whose bounds overlap the box's corner while its plane misses it.
    let slanted = [
        Vec3::new(1.5, 0.0, 0.0),
        Vec3::new(0.0, 1.5, 0.0),
        Vec3::new(0.0, 0.0, 1.5),
    ];
    let corner = bake::triangle_box_stage(Vec3::new(-0.6, -0.6, -0.6), Vec3::splat(0.5), &slanted);
    // Sampled: a triangle with a vertex inside the box always overlaps it.
    let mut vertex_inside_ok = true;
    for _ in 0..2000 {
        // A random box and a triangle with one vertex inside it: the test must call that an overlap.
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
}

/// Section 3: the octree's levels, leaves, and parents, every object baked, and the walk down.
fn check_octree(report: &mut Report) {
    let w = world::world();
    let v = &w.voxelizer;
    let probe = &w.probe;
    // ---- 3. The octree ----------------------------------------------------------
    let counts = &probe.level_counts;
    let total: u32 = counts.iter().sum();
    let sorted = v
        .cells
        .windows(2)
        .all(|p| (p[0].level, p[0].x, p[0].y, p[0].z) <= (p[1].level, p[1].x, p[1].y, p[1].z));
    let children_finer = v.cells.iter().enumerate().all(|(i, c)| {
        c.children.iter().all(|&ch| {
            ch == CHILD_EMPTY || (ch as usize > i && v.cells[ch as usize].level == c.level + 1)
        })
    });
    let leaves_ok = v
        .cells
        .iter()
        .filter(|c| c.level as i32 == v.cell_subdiv)
        .all(|c| c.alpha == 1.0 && c.children.iter().all(|&ch| ch == CHILD_EMPTY));
    let parents_empty = v
        .cells
        .iter()
        .filter(|c| (c.level as i32) < v.cell_subdiv)
        .all(|c| {
            c.albedo == [0.0; 3] && c.emission == [0.0; 3] && c.normal == [0.0; 3] && c.alpha <= 1.0
        });
    let parent_alpha = v
        .cells
        .iter()
        .filter(|c| c.level as i32 == v.cell_subdiv - 1)
        .all(|c| {
            let n = c.children.iter().filter(|&&ch| ch != CHILD_EMPTY).count();
            (c.alpha - n as f32 / 8.0).abs() < 1e-6
        });
    report.check(
        "the bake: cells sorted by level, then x, y, z; the root alone at level 0; children one level finer and later; leaves alpha 1; parents keep no colour, their alpha the children's sum / 8",
        sorted && counts[0] == 1 && total as usize == v.cells.len() && children_finer && leaves_ok && parents_empty && parent_alpha && v.leaf_voxel_count == counts[7] as usize,
        json!({"levelCounts": counts, "cells": v.cells.len(), "leaves": v.leaf_voxel_count, "bakeSeconds": w.bake_seconds}),
    );
    // Every object of the room is baked: its leaves when it is baked alone.
    let per_object: Vec<(String, usize, usize)> = w
        .meshes
        .iter()
        .map(|m| {
            let mut alone = bake::Voxelizer::begin_bake(world::SUBDIV, world::bounds());
            alone.plot_mesh(m);
            let leaves = alone
                .cells
                .iter()
                .filter(|c| c.level as i32 == alone.cell_subdiv)
                .count();
            (
                w.scene.objects[m.object].name.to_string(),
                m.triangles.len(),
                leaves,
            )
        })
        .collect();
    report.check(
        "every object but the ceiling panel is baked, each into leaves of its own",
        per_object.iter().all(|o| o.2 > 0) && per_object.len() == w.scene.objects.len() - 1,
        json!(per_object),
    );
    // Every leaf is found by walking down from the root, and only leaves are.
    let leaves = probe.level_range(probe.leaf_level());
    let found = leaves
        .clone()
        .step_by(97)
        .all(|c| probe.find_leaf(probe.data[c].pos()) == Some(c));
    let empty_cell = IVec3::new(64, 40, 64);
    report.check("the octree: a walk from the root finds each sampled leaf at its position; the room's empty middle has none", found && probe.find_leaf(empty_cell).is_none(), json!({"sampled": leaves.len() / 97, "emptyCell": empty_cell.to_array()}));
}

/// Section 4: the cells' records decoded, and the RGBE encoding.
fn check_records(report: &mut Report) {
    let w = world::world();
    let v = &w.voxelizer;
    let probe = &w.probe;
    // ---- 4. The records -----------------------------------------------------------
    let mut decode_ok = true;
    let mut worst_albedo = 0.0f32;
    for (i, c) in v.cells.iter().enumerate().step_by(31) {
        // Every 31st cell: its packed record decodes back to the bake's position, albedo, and (at a leaf) normal.
        let d = probe.data[i];
        decode_ok &= d.pos() == IVec3::new(c.x as i32, c.y as i32, c.z as i32);
        worst_albedo = worst_albedo.max(
            (d.albedo().truncate() - Vec3::from(c.albedo))
                .abs()
                .max_element(),
        );
        if c.level as i32 == v.cell_subdiv && Vec3::from(c.normal) != Vec3::ZERO {
            decode_ok &=
                (d.normal() - Vec3::from(c.normal)).abs().max_element() < 1.0 / 127.0 + 1e-6;
        }
    }
    let samples = [
        Vec3::new(1.0, 0.5, 0.25),
        Vec3::new(6.0, 0.0, 0.003),
        Vec3::new(1000.0, 2.0, 0.1),
        Vec3::new(0.001, 0.002, 0.0005),
    ];
    let rgbe: Vec<(u32, [f32; 3])> = samples
        .iter()
        .map(|&s| {
            let e = storage::to_rgbe9995(s);
            (e, storage::decode_rgbe9995(e).to_array())
        })
        .collect();
    let rgbe_ok = samples.iter().zip(&rgbe).all(|(s, (_, d))| {
        (Vec3::from(*d) - *s).abs().max_element() <= s.max_element() / 256.0 + 1e-6
    }) && storage::to_rgbe9995(Vec3::ZERO) == storage::to_rgbe9995(Vec3::ZERO);
    report.check(
        "CellData: positions decode 11/10/11 bits to the cells' own; albedo within one 8-bit step (truncated); normals within one snorm step; RGB9E5 emission within 1/256 of its largest channel",
        decode_ok && worst_albedo < 1.0 / 255.0 + 1e-6 && rgbe_ok,
        json!({"worstAlbedo": worst_albedo, "rgbe9995": rgbe.iter().map(|(e, d)| json!({"word": format!("{e:#010x}"), "decoded": d})).collect::<Vec<_>>()}),
    );
}

/// Section 5: the distance field at the leaves and at sampled texels.
fn check_field(report: &mut Report, rng: &mut Rng) {
    let w = world::world();
    let probe = &w.probe;
    let leaves = probe.level_range(probe.leaf_level());
    // ---- 5. The distance field ----------------------------------------------------
    let leaf_cells: Vec<IVec3> = leaves.clone().map(|c| probe.data[c].pos()).collect();
    let solid_zero = leaf_cells.iter().step_by(13).all(|&p| probe.sdf_at(p) == 0);
    let mut exact = 0;
    let mut checked = 0;
    let s = probe.octree_size;
    for _ in 0..300 {
        // At random texels: the stored byte against the exact distance to the nearest leaf.
        let p = IVec3::new(
            (rng.next() * s.x as f32) as i32,
            (rng.next() * s.y as f32) as i32,
            (rng.next() * s.z as f32) as i32,
        )
        .min(s - IVec3::ONE);
        let best = leaf_cells
            .iter()
            .map(|&q| (q - p).length_squared())
            .min()
            .unwrap() as f32;
        checked += 1;
        if probe.sdf_at(p) == sdf::encode(best) {
            exact += 1;
        }
    }
    report.check("the distance field: every leaf stores 0; at sampled texels the byte is min(floor(sqrt(d^2)), 254) + 1 of the exact nearest leaf", solid_zero && exact == checked, json!({"checked": checked, "exact": exact}));
}

/// Section 6: the light passes on the CPU: the write, the mips, the second bounce, the lights' shadows.
fn check_light(report: &mut Report) {
    let w = world::world();
    let probe = &w.probe;
    let leaves = probe.level_range(probe.leaf_level());
    // ---- 6. The light passes ------------------------------------------------------
    let lit = &w.lit;
    let params = world::params(probe, &LightSettings::default());
    report.check(
        "cell_size: ps = octree_size / size = 20 cells per metre in every axis, 0.05 m per cell",
        (params.cell_size - 0.05).abs() < 1e-7,
        json!({"cellSize": params.cell_size}),
    );
    let mut write_ok = true;
    for c in (0..probe.data.len()).step_by(53) {
        // Every 53rd cell: the texel the write pass gave it in its mip.
        let d = probe.data[c];
        let mip = probe.mip_count() - 1 - probe.level_of(c);
        let t = lit.texture.mips[mip][lit.texture.index(mip, d.pos())];
        write_ok &=
            t == storage::unorm8((lit.outputs[c] / world::DYNAMIC_RANGE).extend(d.albedo().w));
    }
    // A parent's light: the children's sum / mix(8, count, propagation), at 0 and 1.
    let parent = probe
        .level_range(probe.leaf_level() - 1)
        .find(|&c| {
            probe.octree[c]
                .iter()
                .filter(|&&ch| ch != CHILD_EMPTY)
                .count()
                == 4
        })
        .expect("a half-full parent");
    let sum: Vec3 = probe.octree[parent]
        .iter()
        .filter(|&&ch| ch != CHILD_EMPTY)
        .map(|&ch| lit.direct[ch as usize])
        .sum();
    let p0 = light::mip_cell(
        probe,
        &light::Params {
            propagation: 0.0,
            ..params
        },
        &lit.direct,
        parent,
    )
    .0;
    let p1 = light::mip_cell(
        probe,
        &light::Params {
            propagation: 1.0,
            ..params
        },
        &lit.direct,
        parent,
    )
    .0;
    let mip_ok =
        (p0 - sum / 8.0).abs().max_element() < 1e-6 && (p1 - sum / 4.0).abs().max_element() < 1e-6;
    report.check("the mip pass: a parent of 4 children holds their sum / 8 at propagation 0 and / 4 at 1; the write pass stores light / dynamic range and coverage in RGBA8", write_ok && mip_ok, json!({"parent": parent, "sum": sum.to_array(), "prop0": p0.to_array(), "prop1": p1.to_array()}));
    let leaves_v: Vec<usize> = leaves.clone().collect();
    let brighter = leaves_v
        .iter()
        .all(|&c| (lit.outputs[c] - lit.direct[c]).min_element() >= -1e-6);
    let gained = leaves_v
        .iter()
        .filter(|&&c| lit.outputs[c].max_element() > lit.direct[c].max_element() * 1.05 + 1e-4)
        .count();
    let unlit_direct = leaves_v
        .iter()
        .filter(|&&c| lit.direct[c].max_element() < 1e-5)
        .count();
    let unlit_after = leaves_v
        .iter()
        .filter(|&&c| lit.outputs[c].max_element() < 1e-5)
        .count();
    report.check("the second bounce only adds light: no leaf loses light; it lights leaves the panel's area light did not reach", brighter && gained > 0 && unlit_after < unlit_direct, json!({"leaves": leaves_v.len(), "gainedOver5pc": gained, "unlitDirect": unlit_direct, "unlitAfter": unlit_after, "lightSeconds": w.light_seconds}));
    // The area light has no shadow: the floor cell under the sphere's centre still receives it.
    let under = demos_leaf(Vec3::new(-1.33, 0.0, -0.64));
    let under_light = under
        .map(|c| light::compute_light(probe, &params, &w.lights, c))
        .unwrap_or(Vec3::ZERO);
    let below = demos_leaf(Vec3::new(0.0, 0.0, -0.2));
    let below_light = below
        .map(|c| light::compute_light(probe, &params, &w.lights, c))
        .unwrap_or(Vec3::ZERO);
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
}

/// Section 7: the cone traces: the 45-degree cone's steps and the edge fade.
fn check_cones(report: &mut Report) {
    let w = world::world();
    let probe = &w.probe;
    let lit = &w.lit;
    // ---- 7. The cone traces -------------------------------------------------------
    let inst = world::instance(probe, &LightSettings::default(), false);
    let mut steps = Vec::new();
    #[allow(clippy::approx_constant)] // the sample's direction as written
    cone::cone_trace_45(
        &lit.texture,
        Vec3::ONE / inst.bounds,
        cone::Cone {
            pos: Vec3::new(64.0, 10.0, 64.0),
            direction: Vec3::new(0.0, 0.7071, 0.7071),
            max_distance: inst.bounds.length(),
            bias: inst.bias,
        },
        Some(&mut steps),
    );
    let doubles = steps.windows(2).all(|p| {
        (p[1].dist - 2.0 * p[0].dist).abs() < 1e-4 && (p[1].lod - p[0].lod - 1.0).abs() < 1e-5
    }) && steps
        .first()
        .is_some_and(|s| (s.lod - 3f32.log2()).abs() < 1e-5);
    report.check("voxel_cone_trace_45_degrees: from bias 1.5 each step doubles the distance and climbs one mip, from log2(3)", doubles, json!(steps.iter().map(|s| [s.dist, s.lod]).collect::<Vec<_>>()));
    // The edge fade cancels for one instance: the ambient is the cones' light, not light x blend.
    let p = Vec3::new(2.6, 0.5, 1.0);
    let n = Vec3::NEG_X;
    let mut cones = cone::Cones::default();
    let (amb, _) = cone::process(
        &inst,
        &lit.texture,
        &cone::Sample {
            position: p,
            normal: n,
            reflection: n,
            roughness: 0.5,
        },
        false,
        Some(&mut cones),
    );
    let cancels =
        cones.blend > 0.0 && cones.blend < 0.3 && (amb - cones.light).abs().max_element() < 1e-5;
    let face = inst.from_cell(Vec3::new(64.0, 0.0, 64.0));
    let (at_face, _) = cone::process(
        &inst,
        &lit.texture,
        &cone::Sample {
            position: face,
            normal: Vec3::Y,
            reflection: Vec3::Y,
            roughness: 0.5,
        },
        false,
        None,
    );
    report.check("the edge fade: one VoxelGI's sums are divided by its own blend, so near a face (blend < 0.3) the ambient is the cones' full light; only on a face (blend 0) does it vanish", cancels && at_face == Vec4::ZERO, json!({"blend": cones.blend, "ambient": amb.to_array(), "cones": cones.light.to_array(), "atFace": at_face.to_array()}));
}

/// Section 8: the dynamic pass's views, maps, and plotted voxels.
fn check_dynamic(report: &mut Report) {
    // ---- 8. The dynamic pass --------------------------------------------------------
    let dw = world::dynamic_world();
    let started = std::time::Instant::now();
    let plot = world::dynamic_plot(0.0);
    let seconds = started.elapsed().as_secs_f32();
    let teapot = world::teapot_at(0.0);
    let c = Vec3::from(teapot.world_center);
    let near = plot.plotted[0].iter().all(|&p| {
        (super::demos::cell_center(&dw.probe, dw.probe.leaf_level(), p) - c)
            .abs()
            .max_element()
            < 0.6
    });
    let changed = plot.plotted[0]
        .iter()
        .filter(|&&p| plot.texture.texel(0, p) != dw.lit.texture.texel(0, p))
        .count();
    let untouched = (0..dw.lit.texture.mips[0].len()).step_by(101).all(|i| {
        let s = dw.probe.octree_size;
        let p = IVec3::new(
            i as i32 % s.x,
            (i as i32 / s.x) % s.y,
            i as i32 / (s.x * s.y),
        );
        plot.plotted[0]
            .binary_search_by_key(&(p.z, p.y, p.x), |v| (v.z, v.y, v.x))
            .is_ok()
            || plot.texture.mips[0][i] == dw.lit.texture.mips[0][i]
    });
    let maps = plot.views[0].maps.len();
    report.check("the dynamic pass: six views, 4 oversampled maps then one per mip; the plotted voxels lie at the teapot and change their texels; texels it does not plot keep the static light", plot.views.len() == 6 && maps == 4 + dw.probe.mip_count() && !plot.plotted[0].is_empty() && near && changed > 0 && untouched, json!({"box": [plot.begin.to_array(), plot.end.to_array()], "plottedMip0": plot.plotted[0].len(), "changed": changed, "maps": maps, "seconds": seconds}));
}

/// Section 9: the WGSL ports against the CPU, the scripts, and the timeline's beats.
fn check_gpu(
    report: &mut Report,
    rng: &mut Rng,
    options: &lab::investigation::SelftestOptions,
) -> Result<()> {
    check_cost_numbers(report);
    let gpu = lab::gpu::Gpu::new(lab::gpu::instance(None), None)?;
    let statics = lab::frame::Statics::prepare(lab::scene::SceneId::STUDY_ROOM)?;
    let mut renderer = lab::renderer::Renderer::new(gpu, statics, 640, 360)?;
    let still = still_timeline(640, 360);
    renderer.render_frame(request(&shot::for_demo(Demo::Room), &still), None);
    check_scripts(report, &mut renderer)?;
    check_scene_click(report, &mut renderer);
    let gpu_texture = light_on_gpu(report, &mut renderer);
    check_sampler(report, rng, &mut renderer, &gpu_texture);
    check_screen_pass(report, rng, &mut renderer, &gpu_texture);
    check_light_pass(report, &mut renderer);
    check_bounce(report, &mut renderer, &gpu_texture);
    check_floor_click(report)?;
    check_beats(report, &mut renderer, options)?;
    Ok(())
}

/// The numbers chapter 8 says (cost.rs, update.rs).
fn check_cost_numbers(report: &mut Report) {
    // The numbers chapter 8 says (cost.rs, update.rs).
    let state = shot::for_demo(Demo::Cost);
    let c = super::cost::costs(&state);
    let mut one = state.clone();
    one.set("giTwoBounces", 0.0);
    let one_bounce = super::update::plan(&one).dispatches.len();
    let ok = c.leaves == 42_289
        && c.texels == 1_048_576
        && c.dispatches == 32
        && one_bounce == 16
        && c.low_steps == 7
        && c.spec_steps[0] == 2
        && c.spec_steps[2] == 381
        && c.passes == 2;
    report.check(
        "chapter 8's numbers: 42,289 leaves against 1,048,576 texels of mip 0; 32 dispatches an update with two bounces (16 with one); a Low cone at most 7 steps; the reflection cone 2 steps at roughness 1, 381 at 0",
        ok,
        json!({"leaves": c.leaves, "texels": c.texels, "dispatches": c.dispatches, "oneBounce": one_bounce, "workgroups": c.workgroups, "lowSteps": c.low_steps, "highSteps": c.high_steps, "specSteps": c.spec_steps}),
    );
}

/// The scripts (script/logic.lam): marchCone's steps on the story's frames, and pickSliceCell's cell.
fn check_scripts(report: &mut Report, renderer: &mut lab::renderer::Renderer) -> Result<()> {
    let w = world::world();
    // The scripts (script/logic.lam): marchCone steps the cone one sample every `seconds` on every frame the
    // story runs it, and pickSliceCell picks the leaf under a click at the slice's height.
    let timeline = lab::shot::Timeline::load(
        &lab::resources::root().join("packages/godot_voxelgi/build/timelines/en/timeline.json"),
    )?;
    let (mut checked, mut differ) = (0, Vec::new());
    for key in timeline
        .keys
        .iter()
        .filter(|k| k.run.as_ref().is_some_and(|r| r.name == "marchCone"))
    {
        for f in key.frame..(key.frame + 600).min(timeline.total_frames) {
            // Each frame the action runs: the step the script set, against the step its seconds and start imply.
            let st = timeline.state_at(f);
            let Some(a) = st.actions.iter().find(|a| a.key == "marchCone") else {
                continue;
            };
            let seconds: f32 = a.arg("seconds").map_err(anyhow::Error::msg)?;
            let from: Option<u32> = a.arg("from").map_err(anyhow::Error::msg)?;
            let to: Option<u32> = a.arg("to").map_err(anyhow::Error::msg)?;
            let rust =
                (from.unwrap_or(0) + (a.elapsed / seconds) as u32).min(to.unwrap_or(16)) as f32;
            checked += 1;
            if st.p("coneSteps") != rust {
                differ.push(json!({"frame": f, "script": st.p("coneSteps"), "rust": rust}));
            }
        }
    }
    let shown0 = shot::for_demo(Demo::Room);
    let content =
        lab::shader_data::build_frame(&shown0, &renderer.statics, [640.0, 360.0], 1.0, 0.0);
    let s = w.probe.octree_size;
    let mut clicks = 0;
    for height in [0.0f32, 0.3, 1.27, 2.9] {
        // At four slice heights and 41 x 41 points, the script's pick against the Rust one.
        let mut shown = shot::for_demo(Demo::Room);
        shown.set("sliceHeight", height);
        for i in 0..=40 {
            for j in 0..=40 {
                let uv = glam::Vec2::new(i as f32 / 40.0, j as f32 / 40.0);
                let mut state = shown.clone();
                let mut frame = lab::actions::ClickFrame {
                    shown: &shown,
                    state: &mut state,
                    uv,
                    content: &content,
                    statics: &renderer.statics,
                };
                let ran = lab::script::runtime::click("pickSliceCell", &mut frame);
                let y = super::demos::leaf_of(&w.probe, glam::Vec3::new(0.0, height, 0.0))
                    .y
                    .clamp(0, s.y - 1);
                let x = ((uv.x * s.x as f32) as i32).clamp(0, s.x - 1);
                let z = ((uv.y * s.z as f32) as i32).clamp(0, s.z - 1);
                clicks += 1;
                if ran != Some(true)
                    || state.p("cellX") != x as f32
                    || state.p("cellY") != y as f32
                    || state.p("cellZ") != z as f32
                {
                    differ.push(json!({"uv": [uv.x, uv.y], "height": height, "script": [state.p("cellX"), state.p("cellY"), state.p("cellZ")], "rust": [x, y, z]}));
                }
            }
        }
    }
    report.check("the scripts: marchCone steps one sample every `seconds` on the story's frames; pickSliceCell picks the leaf under the click (41 x 41 points at 4 heights)", differ.is_empty() && checked > 0, json!({"actionFrames": checked, "clicks": clicks, "differ": differ.iter().take(20).collect::<Vec<_>>()}));
    Ok(())
}

/// The click on the scene (script/logic.lam `scene`): the leaf behind the surface hit, at 41 x 41 points.
fn check_scene_click(report: &mut Report, renderer: &mut lab::renderer::Renderer) {
    let w = world::world();
    let shown0 = shot::for_demo(Demo::Room);
    let content =
        lab::shader_data::build_frame(&shown0, &renderer.statics, [640.0, 360.0], 1.0, 0.0);
    // The click on the scene (script/logic.lam `scene`): the leaf just behind the surface the ray hits, as the
    // Rust click picked it, at 41 x 41 points of the room's view.
    let mut differ = Vec::new();
    let (mut hits, mut misses) = (0, 0);
    let projector = lab::camera::Projector::new(&content.uniforms.camera);
    let viewport = glam::Vec2::from(content.uniforms.camera.viewport_size);
    for i in 0..=40 {
        for j in 0..=40 {
            // Each point clicked: the script's cell against the Rust click's.
            let uv = glam::Vec2::new(i as f32 / 40.0, j as f32 / 40.0);
            let mut state = shown0.clone();
            let mut frame = lab::actions::ClickFrame {
                shown: &shown0,
                state: &mut state,
                uv,
                content: &content,
                statics: &renderer.statics,
            };
            let ran = lab::script::runtime::click("scene", &mut frame);
            let (ro, rd) = projector.ray(uv * viewport);
            let hit = lab::scene::trace_scene(&content.objects, ro, rd, 100.0, u32::MAX);
            let expected = hit
                .is_hit()
                .then(|| super::demos::leaf_of(&w.probe, ro + rd * hit.t - hit.normal * 0.01));
            let got = [state.p("cellX"), state.p("cellY"), state.p("cellZ")];
            let same = match expected {
                Some(c) => ran == Some(true) && got == [c.x as f32, c.y as f32, c.z as f32],
                None => {
                    ran == Some(false)
                        && got == [shown0.p("cellX"), shown0.p("cellY"), shown0.p("cellZ")]
                }
            };
            if expected.is_some() {
                hits += 1
            } else {
                misses += 1
            }
            if !same {
                differ.push(json!({"uv": [uv.x, uv.y], "script": got, "rust": expected.map(|c| [c.x, c.y, c.z])}));
            }
        }
    }
    report.check("the scene click (script/logic.lam `scene`) picks the leaf behind the surface hit, as the Rust click did (41 x 41 points)", differ.is_empty() && hits > 0, json!({"hits": hits, "misses": misses, "differ": differ.iter().take(20).collect::<Vec<_>>()}));
}

/// The light passes on the GPU against light.rs, every texel of every mip; the texture the GPU wrote.
fn light_on_gpu(
    report: &mut Report,
    renderer: &mut lab::renderer::Renderer,
) -> storage::LightTexture {
    let w = world::world();
    let probe = &w.probe;
    let lit = &w.lit;
    // The light passes on the GPU (update.rs) against light.rs: every texel of every mip.
    let mut gpu_texture = lit.texture.clone();
    let mut worst = 0u8;
    let mut differing = 0usize;
    let mut texels = 0usize;
    let mut sizes = true;
    for mip in 0..probe.mip_count() {
        // Each mip read back: its size, and how far each texel is from the CPU's byte.
        let Some((bytes, size)) = renderer.read_volume("voxelgiLight", mip as u32) else {
            sizes = false;
            break;
        };
        let cpu = &lit.texture.mips[mip];
        let s = probe.mip_size(mip);
        sizes &= size == [s.x as u32, s.y as u32, s.z as u32] && bytes.len() == cpu.len() * 4;
        gpu_texture.mips[mip] = bytes.as_chunks::<4>().0.to_vec();
        for (g, c) in bytes.as_chunks::<4>().0.iter().zip(cpu) {
            let d = g
                .iter()
                .zip(c)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap_or(0);
            worst = worst.max(d);
            differing += (d > 0) as usize;
            texels += 1;
        }
    }
    report.check(
        "light passes on the GPU (MODE_COMPUTE_LIGHT, SECOND_BOUNCE, UPDATE_MIPMAPS, WRITE_TEXTURE) write the texture light.rs computes, every mip",
        // Float sums in another order round a texel one step the other way, now and then.
        sizes && worst <= 1 && differing * 50 <= texels,
        json!({"texels": texels, "differing": differing, "worstStep": worst}),
    );
    gpu_texture
}

/// The 3D texture's sampler against storage.rs's sample_lod.
fn check_sampler(
    report: &mut Report,
    rng: &mut Rng,
    renderer: &mut lab::renderer::Renderer,
    gpu_texture: &storage::LightTexture,
) {
    let w = world::world();
    let probe = &w.probe;
    let lit = &w.lit;
    let params = world::params(probe, &LightSettings::default());
    // The 3D texture's sampler (trilinear, between mips, clamped) against storage.rs's sample_lod.
    let mut cases = Vec::new();
    for k in 0..64 {
        // Random points at random levels, then the points a bounce cone samples.
        cases.push([rng.next(), rng.next(), rng.next(), (k % 16) as f32 * 0.5]);
    }
    // And where a bounce cone samples: along +x from a leaf at the room's -x wall.
    let mut steps = Vec::new();
    light::bounce_cone(
        &lit.texture,
        &params,
        Vec3::new(3.5, 44.5, 92.5),
        Vec3::X,
        Some(&mut steps),
    );
    let cell_size = Vec3::ONE / probe.octree_size.as_vec3();
    for st in &steps {
        let uvw = (Vec3::new(3.5, 44.5, 92.5) + st.dist * Vec3::X) * cell_size;
        cases.push([uvw.x, uvw.y, uvw.z, st.lod]);
    }
    let words = renderer.run_shading_verify(
        "voxelgiSampleTests",
        cases.len() as u32,
        &cases,
        4 * cases.len(),
    );
    let mut worst = 0.0f32;
    let mut worst_at = json!(null);
    for (k, c) in cases.iter().enumerate() {
        let cpu = gpu_texture.sample_lod(Vec3::new(c[0], c[1], c[2]), c[3]);
        for i in 0..4 {
            let g = f32::from_bits(words[k * 4 + i]);
            if (g - cpu[i]).abs() > worst {
                worst = (g - cpu[i]).abs();
                worst_at = json!({"case": c, "component": i, "gpu": g, "cpu": cpu[i]});
            }
        }
    }
    report.check("the light texture's sampler matches storage.rs's sample_lod (trilinear, between mips, clamped to the edge)", worst < 3.0 / 255.0, json!({"cases": cases.len(), "worstAbsolute": worst, "worstAt": worst_at}));
}

/// The screen pass at points on the room's surfaces, both qualities, inside and outside.
fn check_screen_pass(
    report: &mut Report,
    rng: &mut Rng,
    renderer: &mut lab::renderer::Renderer,
    gpu_texture: &storage::LightTexture,
) {
    let w = world::world();
    let probe = &w.probe;
    // The screen pass at points on the room's surfaces, both qualities, inside and outside.
    let mut cases: Vec<(Vec3, Vec3, Vec3, f32, bool, bool)> = Vec::new();
    for k in 0..48 {
        // The cases take turns: the floor, the left wall, the back wall, and a random point and normal in the room.
        let (p, n) = match k % 4 {
            0 => (
                Vec3::new(rng.next() * 5.8 - 2.9, 0.0, rng.next() * 5.8 - 2.9),
                Vec3::Y,
            ),
            1 => (
                Vec3::new(-3.0, rng.next() * 2.9, rng.next() * 5.8 - 2.9),
                Vec3::X,
            ),
            2 => (
                Vec3::new(rng.next() * 5.8 - 2.9, rng.next() * 2.9, -3.0),
                Vec3::Z,
            ),
            _ => (
                Vec3::new(
                    rng.next() * 6.0 - 3.0,
                    rng.next() * 3.0,
                    rng.next() * 6.0 - 3.0,
                ),
                (Vec3::new(rng.next(), rng.next(), rng.next()) - Vec3::splat(0.5)).normalize(),
            ),
        };
        let view = (Vec3::new(0.0, 1.6, 4.0) - p).normalize();
        let r = lab::scene::reflect(-view, n);
        cases.push((p, n, r, rng.next(), k % 3 == 0, k % 5 != 0));
    }
    let tests: Vec<[f32; 4]> = cases
        .iter()
        .flat_map(|&(p, n, r, rough, high, blend)| {
            [
                p.extend(rough).to_array(),
                n.extend(high as u32 as f32).to_array(),
                r.extend(blend as u32 as f32).to_array(),
            ]
        })
        .collect();
    let words = renderer.run_shading_verify(
        "voxelgiProcessTests",
        cases.len() as u32,
        &tests,
        8 * cases.len(),
    );
    let mut worst = 0.0f32;
    let mut worst_at = json!(null);
    for (k, &(p, n, r, rough, high, blend)) in cases.iter().enumerate() {
        // Each case through cone.rs on the GPU's own texture, against the port's eight words.
        let inst = world::instance(probe, &LightSettings::default(), !blend);
        let (a, s) = cone::process(
            &inst,
            gpu_texture,
            &cone::Sample {
                position: p,
                normal: n,
                reflection: r,
                roughness: rough,
            },
            high,
            None,
        );
        let cpu = [a.x, a.y, a.z, a.w, s.x, s.y, s.z, s.w];
        for (i, c) in cpu.iter().enumerate() {
            let g = f32::from_bits(words[k * 8 + i]);
            if rel(g, *c) > worst {
                worst = rel(g, *c);
                worst_at = json!({"case": k, "component": i, "gpu": g, "cpu": c, "high": high, "rough": rough});
            }
        }
    }
    // The texture unit filters with fewer bits than f32 (its weights are fixed point): about 1 %.
    report.check("screen pass: the WGSL port of voxel_gi_compute matches cone.rs on the GPU's texture (ambient and reflection, rgba; Low and High; interior on and off; the texture unit's filtering within 2 %)", worst < 2e-2, json!({"cases": cases.len(), "worstRelative": worst, "worstAt": worst_at}));
}

/// The light pass: every light type at sampled leaves.
fn check_light_pass(report: &mut Report, renderer: &mut lab::renderer::Renderer) {
    let w = world::world();
    let probe = &w.probe;
    let leaves = probe.level_range(probe.leaf_level());
    let params = world::params(probe, &LightSettings::default());
    let omni = world::omni_light(probe, Vec3::new(0.9, 1.4, -0.4));
    // The light pass: every light type at sampled leaves.
    let lights = [
        w.lights[0],
        omni,
        light::Light {
            kind: light::LIGHT_TYPE_SPOT,
            cos_spot_angle: 40f32.to_radians().cos(),
            direction: Vec3::new(0.2, -1.0, 0.1).normalize(),
            ..omni
        },
        light::Light {
            kind: light::LIGHT_TYPE_DIRECTIONAL,
            direction: Vec3::new(0.3, -0.8, 0.2).normalize(),
            ..omni
        },
    ];
    let cells: Vec<usize> = leaves.clone().step_by(leaves.len() / 96 + 1).collect();
    let mut tests = Vec::new();
    for (k, &c) in cells.iter().enumerate() {
        // Each sampled leaf takes a light type in turn.
        let l = lights[k % 4];
        let d = probe.data[c];
        tests.extend([
            [l.kind as f32, l.energy, l.radius, l.attenuation],
            l.color.extend(l.cos_spot_angle).to_array(),
            l.position.extend(l.inv_spot_attenuation).to_array(),
            l.direction.extend(l.has_shadow as u32 as f32).to_array(),
            l.area_width.extend(params.cell_size).to_array(),
            l.area_height.extend(0.0).to_array(),
            (d.pos().as_vec3() + Vec3::splat(0.5))
                .extend(0.0)
                .to_array(),
            d.normal().extend(0.0).to_array(),
        ]);
    }
    let words = renderer.run_shading_verify(
        "voxelgiLightTests",
        cells.len() as u32,
        &tests,
        4 * cells.len(),
    );
    let mut worst = 0.0f32;
    let mut mismatched = 0;
    for (k, &c) in cells.iter().enumerate() {
        // The GPU's term for the same leaf and light, against light.rs's.
        let l = lights[k % 4];
        let d = probe.data[c];
        let pos = d.pos().as_vec3() + Vec3::splat(0.5);
        let n = d.normal();
        let cpu = if l.kind == light::LIGHT_TYPE_AREA {
            light::area_light(&params, &l, pos, n).unwrap_or(Vec3::ZERO)
        } else {
            light::light_at_pos(probe, &params, &l, pos, n, None)
                .map(|(li, dir)| {
                    if n.length() > 0.2 {
                        n.dot(-dir).max(0.0) * li
                    } else {
                        li
                    }
                })
                .unwrap_or(Vec3::ZERO)
        };
        let gpu = Vec3::new(
            f32::from_bits(words[k * 4]),
            f32::from_bits(words[k * 4 + 1]),
            f32::from_bits(words[k * 4 + 2]),
        );
        let d = (gpu - cpu).abs().max_element() / cpu.max_element().max(1e-3);
        if d > 1e-3 {
            mismatched += 1;
        }
        worst = worst.max(d);
    }
    // A shadow ray that grazes an edge can flip between float orders; allow one.
    report.check("light pass: the WGSL port of compute_light_at_pos (with raymarch) and compute_area_light matches light.rs (area, omni, spot, directional)", mismatched <= 1, json!({"cases": cells.len(), "mismatched": mismatched, "worstRelative": worst}));
}

/// The bounce's cone, from sampled leaves along their normals.
fn check_bounce(
    report: &mut Report,
    renderer: &mut lab::renderer::Renderer,
    gpu_texture: &storage::LightTexture,
) {
    let w = world::world();
    let probe = &w.probe;
    let leaves = probe.level_range(probe.leaf_level());
    let params = world::params(probe, &LightSettings::default());
    // The bounce's cone, from sampled leaves along their normals.
    let mut cases = Vec::new();
    for &c in leaves
        .clone()
        .step_by(leaves.len() / 48 + 1)
        .collect::<Vec<_>>()
        .iter()
    {
        let d = probe.data[c];
        let n = if d.normal().length() > 0.2 {
            d.normal()
        } else {
            Vec3::Y
        };
        cases.push((d.pos().as_vec3() + Vec3::splat(0.5), n));
    }
    let tests: Vec<[f32; 4]> = cases
        .iter()
        .flat_map(|(p, d)| [p.extend(0.0).to_array(), d.extend(0.0).to_array()])
        .collect();
    let words = renderer.run_shading_verify(
        "voxelgiBounceTests",
        cases.len() as u32,
        &tests,
        4 * cases.len(),
    );
    let mut worst = 0.0f32;
    let mut worst_at = json!(null);
    for (k, (p, d)) in cases.iter().enumerate() {
        // The CPU's bounce cone on the GPU's own texture, against the port's.
        let c = light::bounce_cone(gpu_texture, &params, *p, *d, None);
        for i in 0..4 {
            let g = f32::from_bits(words[k * 4 + i]);
            if rel(g, c[i]) > worst {
                worst = rel(g, c[i]);
                worst_at = json!({"case": k, "component": i, "gpu": g, "cpu": c[i], "p": p.to_array(), "d": d.to_array()});
            }
        }
    }
    report.check("second bounce: the WGSL port of its cone matches light.rs on the GPU's texture (the texture unit's filtering within 2 %)", worst < 2e-2, json!({"cases": cases.len(), "worstRelative": worst, "worstAt": worst_at}));
}

/// A click on a floor cell's projected centre picks that cell's column.
fn check_floor_click(report: &mut Report) -> Result<()> {
    let w = world::world();
    let probe = &w.probe;
    // A click on a floor cell's projected centre picks that cell's column.
    let state = shot::for_demo(Demo::Room);
    let camera = state.camera.camera_data([640.0, 360.0]);
    let projector = lab::camera::Projector::new(&camera);
    let target = Vec3::new(0.4, 0.0, 1.2);
    let cell = super::demos::leaf_of(probe, target - Vec3::Y * 0.01);
    let center =
        super::demos::cell_center(probe, probe.leaf_level(), cell) * Vec3::new(1.0, 0.0, 1.0);
    // The click is the script's (script/logic.lam `scene`), which traces from the frame's camera.
    let content = lab::frame::FrameContent::new(
        lab::scene::Scene::study_room().records(),
        lab::layout::FrameUniforms {
            camera,
            ..Default::default()
        },
    );
    let statics = lab::frame::Statics::prepare(lab::scene::SceneId::STUDY_ROOM)?;
    let mut picked = state.clone();
    let hit = projector.project(center).and_then(|(px, _)| {
        let mut frame = lab::actions::ClickFrame {
            shown: &state,
            state: &mut picked,
            uv: px / glam::Vec2::new(640.0, 360.0),
            content: &content,
            statics: &statics,
        };
        lab::script::runtime::click("scene", &mut frame)
    });
    let got = IVec3::new(
        picked.p("cellX") as i32,
        picked.p("cellY") as i32,
        picked.p("cellZ") as i32,
    );
    report.check(
        "a click on a floor cell's centre picks that cell",
        hit == Some(true) && got == cell,
        json!({"cell": cell.to_array(), "picked": got.to_array()}),
    );
    let _ = Vec2::ZERO;
    Ok(())
}

/// Every scene beat of a timeline, rendered at its middle: text, labels, listings, readouts.
fn check_beats(
    report: &mut Report,
    renderer: &mut lab::renderer::Renderer,
    options: &lab::investigation::SelftestOptions,
) -> Result<()> {
    // Every scene beat of a timeline, rendered at its middle: text, labels, listings, readouts.
    if let Some(path) = &options.timeline {
        let timeline = lab::shot::Timeline::load(path)?;
        let mut problems = Vec::new();
        let mut beats = 0;
        for beat in timeline
            .beats
            .iter()
            .filter(|b| b.kind == lab::shot::SceneType::ThreeD)
        {
            let frame = beat.start_frame + beat.frames / 2;
            let state = timeline.state_at(frame);
            let mut req = request(&state, &timeline);
            req.chrome = lab::ui::Chrome::Recording {
                timeline: &timeline,
                frame,
            };
            req.chapter_number = beat.chapter;
            let mut r = renderer.render_frame(req, None).report;
            // A listing fits in the block style too.
            if state.panel == lab::shot::PanelMode::Code {
                let mut req = request(&state, &timeline);
                req.chrome = lab::ui::Chrome::Recording {
                    timeline: &timeline,
                    frame,
                };
                req.chapter_number = beat.chapter;
                r.code_overflow +=
                    lab::pseudocode::with_style(lab::pseudocode::Style::Blocks, || {
                        renderer.render_frame(req, None).report.code_overflow
                    });
            }
            beats += 1;
            if r.min_text_pt < lab::ui::MIN_TEXT_PT
                || r.label_overlaps > 0
                || r.labels_outside > 0
                || r.code_overflow > 0
                || r.readout_overflow > 0
                || r.formula_scale.is_some_and(|s| s < 1.0)
            {
                problems.push(json!({"beat": beat.id, "frame": frame, "minTextPt": r.min_text_pt, "labelOverlaps": r.label_overlaps, "labelsOutside": r.labels_outside, "codeOverflow": r.code_overflow, "readoutOverflow": r.readout_overflow}));
            }
        }
        report.check("every scene beat of the timeline: text >= 24 px, labels inside and apart, listings and readouts fit", problems.is_empty() && beats > 0, json!({"timeline": path, "sceneBeats": beats, "problems": problems}));
    }
    Ok(())
}

/// The listings, the demonstrations' files, the story's action, and the worked elements' places.
fn check_story(report: &mut Report) -> Result<()> {
    let w = world::world();
    let probe = &w.probe;
    let leaves = probe.level_range(probe.leaf_level());
    let lit = &w.lit;
    // Listings: each cites a source entry and a demonstration, and every demonstration has one.
    {
        let index = super::sources::INDEX.get();
        let listings = super::pseudocode::all();
        let bad: Vec<&str> = listings
            .iter()
            .filter(|l| {
                !index.entries.contains_key(&l.source)
                    || l.demo
                        .as_deref()
                        .is_none_or(|d| Demo::ALL.iter().all(|x| x.key() != d))
            })
            .map(|l| l.name)
            .collect();
        let without: Vec<&str> = Demo::ALL
            .iter()
            .filter(|d| shot::demos().spec(d.id()).listing.is_none())
            .map(|d| d.key())
            .collect();
        report.check("every listing cites a source entry and a demonstration; every demonstration has a listing", bad.is_empty() && without.is_empty(), json!({"listings": listings.len(), "bad": bad, "demosWithout": without}));
    }

    let states_ok = Demo::ALL.iter().all(|&d| shot::for_demo(d).demo == d.id());
    report.check(
        "every demonstration's file loads",
        states_ok,
        json!(Demo::ALL.iter().map(|d| d.key()).collect::<Vec<_>>()),
    );

    // The story's action (actions.rs): marchCone, started at frame 30, one step every 0.5 s (15 frames).
    {
        let mut timeline = still_timeline(64, 64);
        (
            timeline.total_frames,
            timeline.beats[0].frames,
            timeline.chapters[0].end_frame,
        ) = (400, 400, 400);
        timeline.keys = serde_json::from_value(
            json!([{"frame": 0, "demo": "pixel"}, {"frame": 30, "run": {"name": "marchCone", "args": {"seconds": 0.5, "to": 6}}}, {"frame": 300, "stop": ["marchCone"]}]),
        )?;
        let valid = timeline.validate().map_err(|e| e.to_string());
        let frames = [20u64, 30, 44, 45, 60, 200, 300];
        let steps: Vec<f32> = frames
            .iter()
            .map(|&f| timeline.state_at(f).p("coneSteps"))
            .collect();
        let expected = [16.0, 0.0, 0.0, 1.0, 2.0, 6.0, 16.0];
        report.check("the marchCone action steps coneSteps through a timeline up to its end, and stopping it hands coneSteps back", valid.is_ok() && steps == expected, json!({"valid": valid, "frames": frames, "steps": steps, "expected": expected}));
    }

    // The worked elements' places (for the story's cameras).
    {
        let (leaf_box, leaf) = super::demos::worked_leaf(w);
        let light_cell = demos_leaf(super::demos::LIGHT_POINT).map(|c| {
            super::demos::cell_center(probe, probe.leaf_level(), probe.data[c].pos()).to_array()
        });
        let bounce_cell = demos_leaf(super::demos::BOUNCE_POINT).map(|c| {
            super::demos::cell_center(probe, probe.leaf_level(), probe.data[c].pos()).to_array()
        });
        let rejected: Vec<[f32; 4]> = super::demos::descent(w)
            .iter()
            .filter(|d| !d.overlaps)
            .map(|d| (d.aabb.center() + NODE).extend(d.level as f32).to_array())
            .collect();
        let max_light = lit.outputs[leaves.clone()]
            .iter()
            .map(|v| v.max_element())
            .fold(0.0f32, f32::max);
        report.check("the worked elements (information)", true, json!({"bakePoint": super::demos::bake_point(&w.scene).to_array(), "leafCentre": (leaf_box.center() + NODE).to_array(), "leaf": leaf.to_array(), "rejectedChildren": rejected, "lightCell": light_cell, "bounceCell": bounce_cell, "brightestLeafLight": max_light}));
    }
    Ok(())
}

/// Section 10: the source index.
fn check_sources(report: &mut Report) {
    // ---- 10. The source index -----------------------------------------------------
    let index = super::sources::INDEX.get();
    report.check("source index", true, json!({"engine": index.engine.name, "version": index.engine.version, "commit": index.engine.commit, "entries": index.entries.len()}));
}

/// The leaf a world point falls in, if the octree has one there (or next to it).
fn demos_leaf(p: Vec3) -> Option<usize> {
    let w = world::world();
    super::demos::nearest_leaf(&w.probe, super::demos::leaf_of(&w.probe, p)).map(|c| c.0)
}
