//! The SDFGI investigation's verification suite: the CPU model against the
//! worked examples read from Godot's source, and the source index against the
//! Godot checkout. Every check records what it compared and the result in the
//! JSON report.

use super::model::{Cascades, Dirty, Settings};
use super::voxels;
use super::occlusion;
use super::{light, probes, screen, sim};
use super::shot::{self, Demo};
use anyhow::Result;
use glam::{IVec3, Vec3};
use serde_json::{json, Value};
use lab::utility::selftest::{request, still_timeline, Report};
use std::path::PathBuf;

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).abs().max_element() < 1e-4
}

pub fn run(options: &lab::investigation::SelftestOptions) -> Result<bool> {
    std::fs::create_dir_all(&options.work_dir)?;
    let mut report = Report::new();

    // ---- 1. The cascades (gi.cpp create, update, get_pending_region_data) ----
    let settings = Settings::default();
    let mut cascades = Cascades::create(settings, Vec3::ZERO);
    let created = cascades.cascades.iter().map(|c| (c.cell_size, c.position, c.dirty)).collect::<Vec<_>>();
    let sizes_ok = created.iter().enumerate().all(|(i, c)| (c.0 - 0.2 * 2f32.powi(i as i32)).abs() < 1e-6 && c.1 == IVec3::ZERO && c.2 == Dirty::All);
    report.check("create: four cascades of 0.2, 0.4, 0.8, 1.6 m cells at the origin, all dirty", sizes_ok, json!(format!("{created:?}")));

    // The worked scroll (research.md 1.A): the camera walks 1.0 m along +x.
    cascades.update(Vec3::new(1.0, 0.0, 0.0));
    let c0 = cascades.cascades[0];
    let c1 = cascades.cascades[1];
    let regions = cascades.pending_regions();
    let r = regions.first().copied();
    let scroll_ok = c0.position == IVec3::new(8, 0, 0)
        && c0.dirty == Dirty::Regions(IVec3::new(-8, 0, 0))
        && c1.position == IVec3::ZERO
        && c1.dirty.is_clean()
        && regions.len() == 1
        && r.is_some_and(|r| r.cascade == 0 && r.local_offset == IVec3::new(120, 0, 0) && r.local_size == IVec3::new(8, 128, 128) && close(r.bounds_min, Vec3::new(12.8, -64.0 * 0.2 / 1.5, -12.8)) && close(r.bounds_size, Vec3::new(1.6, 128.0 * 0.2 / 1.5, 25.6)));
    report.check("update: a 1.0 m walk moves cascade 0 by 8 cells and dirties the 8-cell slab at x 120..128; cascade 1 stays", scroll_ok, json!({"cascade0": format!("{c0:?}"), "cascade1": format!("{c1:?}"), "regions": format!("{regions:?}")}));

    // A diagonal move: two regions, the corner given to x only.
    let mut diagonal = Cascades::create(settings, Vec3::ZERO);
    diagonal.update(Vec3::new(1.0, 0.0, 1.0));
    let d = diagonal.pending_regions();
    let diagonal_ok = d.len() == 2 && d[0].local_offset == IVec3::new(120, 0, 0) && d[0].local_size == IVec3::new(8, 128, 128) && d[1].local_offset == IVec3::new(0, 0, 120) && d[1].local_size == IVec3::new(120, 128, 8);
    report.check("update: a diagonal move gives an x slab and a z slab chipped by it", diagonal_ok, json!(format!("{d:?}")));

    // A long move: 48 cells (six steps) is more than half of the kept volume.
    let mut long = Cascades::create(settings, Vec3::ZERO);
    long.update(Vec3::new(10.0, 0.0, 0.0));
    let long_ok = long.cascades[0].position == IVec3::new(48, 0, 0) && long.cascades[0].dirty == Dirty::All;
    let mut forty = Cascades::create(settings, Vec3::ZERO);
    forty.update(Vec3::new(8.6, 0.0, 0.0));
    let forty_ok = forty.cascades[0].dirty == Dirty::Regions(IVec3::new(-40, 0, 0));
    report.check("update: 48 cells redraws the whole cascade (3d > 128); 40 cells stays partial", long_ok && forty_ok, json!({"long": format!("{:?}", long.cascades[0]), "forty": format!("{:?}", forty.cascades[0])}));

    // The y scale: 75 % is y_mult 1.5, so cascade 0 is 17.07 m tall.
    let (_, size) = Cascades::create(settings, Vec3::ZERO).bounds(0);
    report.check("the 75 % y scale squashes cascade 0 to 25.6 x 17.07 x 25.6 m", close(size, Vec3::new(25.6, 25.6 / 1.5, 25.6)), json!(size.to_array()));

    // ---- 2. Voxelization, the jump flood, and STORE (cascade 0 of the room) --
    let statics = lab::frame::Statics::prepare(lab::scene::SceneId::STUDY_ROOM)?;
    let objects = statics.scene.records();
    let voxelized = sim::voxelized_objects(&objects);
    let room = Cascades::create(settings, super::demos::ORIGIN);
    let started = std::time::Instant::now();
    let data = voxels::build_cascade(&voxelized, &room, 0);
    let seconds = started.elapsed().as_secs_f32();
    let solid: Vec<IVec3> = data.stored.cells.iter().map(|c| c.cell()).collect();
    let all_zero = solid.iter().all(|&p| data.stored.sdf[voxels::index(p)] == 0);
    report.check("STORE: every solid voxel stores 0 and is in the solid-cell list", all_zero && !solid.is_empty() && solid.len() <= (voxels::CELLS / 4), json!({"solidCells": solid.len(), "capacity": voxels::CELLS / 4, "seconds": seconds}));
    // A face neighbour of a solid voxel (not itself solid) stores 1 + 1 = 2.
    let face = solid.iter().find_map(|&p| [IVec3::X, IVec3::Y, IVec3::Z, IVec3::NEG_X, IVec3::NEG_Y, IVec3::NEG_Z].into_iter().map(|d| p + d).find(|&q| voxels::inside(q) && !data.voxels.solid(q) && data.stored.sdf[voxels::index(q)] == 2));
    report.check("STORE: an empty face neighbour of a solid voxel stores 2 (1 + its distance of one cell)", face.is_some(), json!(face.map(|p| p.to_array())));
    // The jump flood against the exact nearest solid voxel, at sampled cells.
    let mut rng = 0x2545_f491_4f6c_dd1du64;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng >> 33) as i32
    };
    let (mut exact, mut worst, mut samples) = (0, 0.0f32, 0);
    for _ in 0..600 {
        let p = IVec3::new(next() % 128, next() % 128, next() % 128);
        let best = solid.iter().map(|&q| (q - p).as_vec3().length()).fold(f32::MAX, f32::min);
        let stored = data.stored.sdf[voxels::index(p)] as f32;
        let expected = if best == 0.0 { 0.0 } else { (1.0 + best).min(255.0).round() };
        samples += 1;
        if stored == expected {
            exact += 1;
        }
        worst = worst.max(stored - expected);
    }
    report.check("the half-size jump flood finds the nearest solid voxel at most sampled cells (never nearer than exact)", exact * 10 >= samples * 9 && worst >= 0.0 - 1e-6, json!({"samples": samples, "exact": exact, "worstExcessCells": worst}));
    // Neighbour bits: bit n is set exactly when that neighbour's seed is this cell.
    let neighbours_ok = data.stored.cells.iter().take(2000).all(|c| {
        let pos = c.cell();
        let mut bit = 0;
        let mut expected = 0u32;
        for i in -1..=1 {
            for j in -1..=1 {
                for k in -1..=1 {
                    if (i, j, k) == (0, 0, 0) {
                        continue;
                    }
                    let q = pos + IVec3::new(i, j, k);
                    if voxels::inside(q) && !data.voxels.solid(q) && data.stored.sdf[voxels::index(q)] != 0 {
                        let s = data.flood.seeds[voxels::index(q)];
                        if IVec3::new(s[0] as i32, s[1] as i32, s[2] as i32) == pos {
                            expected |= 1 << bit;
                        }
                    }
                    bit += 1;
                }
            }
        }
        c.neighbours() == expected
    });
    report.check("STORE: the 26 neighbour bits, split over the cell's four words, mark the neighbours whose seed is the cell", neighbours_ok, json!({"cellsChecked": data.stored.cells.len().min(2000)}));
    // Occlusion: around a probe inside the room, the cells next to it are fully visible (15 of 15 in its
    // class); some free cells are dimmed below 1 (behind surfaces seen from it).
    {
        let probe = IVec3::new(64, 56, 64);
        let class = |p: IVec3| -> usize {
            let probe_cell = (p + IVec3::splat(4)).div_euclid(IVec3::splat(8));
            let world = room.cascades[0].position / 8 + probe_cell;
            let bits = world & IVec3::ONE;
            (bits.x | (bits.y << 1) | (bits.z << 2)) as usize
        };
        let next = probe + IVec3::new(0, 0, 0);
        let near = occlusion::class_value(data.occlusion[voxels::index(next)], class(probe));
        let free: Vec<f32> = (0..16).flat_map(|i| (0..16).map(move |j| probe + IVec3::new(i - 8, -4, j - 8))).filter(|&p| voxels::inside(p) && !data.voxels.solid(p)).map(|p| occlusion::class_value(data.occlusion[voxels::index(p)], class(probe))).collect();
        let dimmed = free.iter().filter(|&&v| v < 1.0).count();
        report.check("occlusion: the cell at a probe is fully visible to it; free cells around it range below 1", near == 1.0 && dimmed > 0, json!({"probeCell": probe.to_array(), "atProbe": near, "freeCells": free.len(), "dimmed": dimmed}));
    }

    // The RGBE8985 encode round-trips the worked example (research.md 6.A): 0.22971 -> 0.22949 in G.
    let rgbe = voxels::encode_rgbe8985(Vec3::splat(0.22971));
    let back = voxels::decode_rgbe8985(rgbe);
    report.check("RGBE8985: 0.22971 decodes to about 0.2295 (G keeps 9 bits, R and B 8)", (back.y - 0.22949).abs() < 2e-4, json!({"word": format!("{rgbe:#010x}"), "decoded": back.to_array()}));

    // ---- 4. Light in the voxels, and the probes ------------------------------
    // The panel as an AreaLight3D: one dynamic pass, then the floor cell under
    // it, at the default y scale (75 %) and at 100 %.
    let lighting = statics.scene.lighting;
    let sky = move |d: Vec3| lab::scene::environment_radiance(&lighting, d);
    let floor_ratio = |settings: Settings| -> (f32, Value) {
        let started = std::time::Instant::now();
        let mut world = sim::World::new(&voxelized, settings, super::demos::ORIGIN);
        let built = started.elapsed().as_secs_f32();
        let panel = sim::panel_light(&statics.scene, settings.y_mult());
        let lights: Vec<Vec<light::Light>> = (0..world.cascades.cascades.len()).map(|_| vec![panel]).collect();
        let started = std::time::Instant::now();
        world.step(&lights, &sky);
        let frame_seconds = started.elapsed().as_secs_f32();
        let below = Vec3::new(lighting.light.center.x, 0.0, lighting.light.center.z);
        let field = world.field();
        let unsquash = |c: &voxels::SolidCell| {
            let p = (c.cell().as_vec3() + Vec3::splat(0.5)) / field.to_cell(0) + field.offset(0);
            Vec3::new(p.x, p.y / settings.y_mult(), p.z)
        };
        let Some(cell) = world.data[0].stored.cells.iter().filter(|c| c.facing() & 0b10 != 0).min_by(|a, b| unsquash(a).distance(below).total_cmp(&unsquash(b).distance(below))).copied() else {
            return (0.0, json!("no floor cell"));
        };
        let world_p = unsquash(&cell);
        let voxel = world.light[0].light_at(cell.cell());
        let surface = Vec3::new(world_p.x, 0.0, world_p.z);
        let lab_lo = cell.albedo_rgb() * lab::scene::direct_irradiance(&objects, &lighting, surface, Vec3::Y, lab::scene::ids::FLOOR as u32) / std::f32::consts::PI;
        let ratio = voxel.y / lab_lo.y.max(1e-6);
        (ratio, json!({"yMult": settings.y_mult(), "cell": cell.cell().to_array(), "world": world_p.to_array(), "voxelLight": voxel.to_array(), "forwardLo": lab_lo.to_array(), "ratio": ratio, "buildSeconds": built, "frameSeconds": frame_seconds}))
    };
    let (flat_ratio, flat_detail) = floor_ratio(Settings { y_scale_mode: 2, ..settings });
    report.check("area light (F26): at y scale 100 %, a floor cell under the panel holds about twice the forward pass's outgoing radiance", (1.6..2.2).contains(&flat_ratio), flat_detail);
    let (squashed_ratio, squashed_detail) = floor_ratio(settings);
    report.check("area light at 75 % (F27): the squashed distance to the panel makes the injected light weaker than at 100 %", squashed_ratio < flat_ratio * 0.7, squashed_detail);

    // A uniform sky of radiance 1 and no geometry, y scale 100 %: the probes' irradiance (F4).
    let flat = Settings { y_scale_mode: 2, ..settings };
    let mut empty = sim::World::new(&[], flat, Vec3::ZERO);
    let one = |_: Vec3| Vec3::ONE;
    let no_lights: Vec<Vec<light::Light>> = vec![Vec::new(); empty.cascades.cascades.len()];
    for _ in 0..empty.integrate.history_size {
        empty.step_sky(&no_lights, &one, Some(Vec3::ONE));
    }
    let centre = probes::probe_index(IVec3::splat(8));
    let mean = empty.history[0].mean(centre);
    let dirs = [(Vec3::X, "+x"), (Vec3::Y, "+y"), (Vec3::Z, "+z"), (Vec3::NEG_Z, "-z")];
    let values: Vec<(String, f32)> = dirs.iter().map(|(d, name)| {
        let c = probes::sh_basis(*d);
        let e: f32 = (0..probes::SH_SIZE).map(|i| mean[i].x * c[i] * 4.0 * probes::L_MULT[i]).sum();
        (name.to_string(), e)
    }).collect();
    let average = values.iter().map(|v| v.1).sum::<f32>() / values.len() as f32;
    report.check("uniform sky (F4): a probe's irradiance under a sky of radiance 1 is about 4/pi = 1.27, not 1", (1.15..1.40).contains(&average), json!({"irradiance": values, "mean": average, "expected": 4.0 / std::f32::consts::PI}));

    // ---- 5. The screen pass: the WGSL port against screen.rs -----------------
    {
        let gpu = lab::gpu::Gpu::new(lab::gpu::instance(None), None)?;
        let mut renderer = lab::renderer::Renderer::new(gpu, lab::frame::Statics::prepare(lab::scene::SceneId::STUDY_ROOM)?, 640, 360)?;
        let still = still_timeline(640, 360);
        renderer.render_frame(request(&shot::for_demo(Demo::Room), &still), None);
        let converged = sim::converged();
        let inputs = sim::screen_inputs(converged);
        // Points on the room's surfaces (floor, walls, sphere top), near and far from cascade 0's edge.
        let mut cases: Vec<(Vec3, Vec3, Vec3, f32)> = Vec::new();
        let mut rng = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng >> 40) as f32 / (1u64 << 24) as f32
        };
        for k in 0..64 {
            let (p, n) = match k % 4 {
                0 => (Vec3::new(next() * 5.8 - 2.9, 0.0, next() * 5.8 - 2.9), Vec3::Y),
                1 => (Vec3::new(-3.0, next() * 2.9, next() * 5.8 - 2.9), Vec3::X),
                2 => (Vec3::new(next() * 5.8 - 2.9, next() * 2.9, -3.0), Vec3::Z),
                _ => (Vec3::new(next() * 30.0 - 15.0, next() * 12.0 - 2.0, next() * 30.0 - 15.0), Vec3::new(next() - 0.5, next() - 0.5, next() - 0.5).normalize()),
            };
            let v = (Vec3::new(0.0, 1.6, 4.0) - p).normalize();
            let r = (-v) - 2.0 * (-v).dot(n) * n;
            cases.push((p - super::demos::ORIGIN, n, r, next()));
        }
        let tests: Vec<[f32; 4]> = cases.iter().flat_map(|&(p, n, r, rough)| [p.extend(rough).to_array(), n.extend(0.0).to_array(), r.extend(0.0).to_array()]).collect();
        let words = renderer.run_shading_verify("sdfgiProcessTests", cases.len() as u32, &tests, 8 * cases.len());
        let mut worst = 0.0f32;
        let mut worst_case = 0;
        for (k, &(p, n, r, rough)) in cases.iter().enumerate() {
            let cpu = screen::process(&inputs, p, n, r, rough);
            let gpu_vals: Vec<f32> = (0..8).map(|i| f32::from_bits(words[k * 8 + i])).collect();
            let cpu_vals = [cpu.ambient.x, cpu.ambient.y, cpu.ambient.z, cpu.ambient.w, cpu.reflection.x, cpu.reflection.y, cpu.reflection.z, cpu.reflection.w];
            let d = gpu_vals.iter().zip(cpu_vals.iter()).map(|(g, c)| (g - c).abs() / c.abs().max(1e-3)).fold(0.0, f32::max);
            if d > worst {
                worst = d;
                worst_case = k;
            }
        }
        report.check("screen pass: the WGSL port of sdfgi_process matches screen.rs (ambient and reflection, rgba)", worst < 1e-3, json!({"cases": cases.len(), "worstRelative": worst, "worstCase": worst_case}));

        // The light pass: lights at cells of cascade 0, with the shadow march in cascade 0 alone.
        let one = Cascades { settings: converged.cascades.settings, cascades: vec![converged.cascades.cascades[0]] };
        let field = light::Field { cascades: &one, sdf: vec![converged.data[0].stored.sdf.as_slice()] };
        let offset = field.offset(0);
        let to_cell = field.to_cell(0);
        let panel = sim::panel_light(&lab::scene::Scene::study_room(), settings.y_mult());
        let omni = light::Light { kind: light::LIGHT_TYPE_OMNI, color: Vec3::ONE, energy: 1.0, direction: Vec3::NEG_Y, position: Vec3::new(1.0, 2.0 * 1.5, 0.5), attenuation: 1.0, radius: 5.0, cos_spot_angle: 0.0, inv_spot_attenuation: 1.0, area_width: Vec3::ZERO, area_height: Vec3::ZERO };
        let spot = light::Light { kind: light::LIGHT_TYPE_SPOT, cos_spot_angle: 45f32.to_radians().cos(), position: Vec3::new(-1.5, 2.5 * 1.5, -1.0), ..omni };
        let sun = light::Light { kind: light::LIGHT_TYPE_DIRECTIONAL, direction: Vec3::new(0.3, -0.8, 0.2).normalize(), ..omni };
        let cells: Vec<voxels::SolidCell> = converged.data[0].stored.cells.iter().step_by(converged.data[0].stored.cells.len() / 64 + 1).copied().collect();
        let mut tests = Vec::new();
        let mut cases = Vec::new();
        for (k, cell) in cells.iter().enumerate() {
            let l = [panel, omni, spot, sun][k % 4];
            let position = (cell.cell().as_vec3() + Vec3::splat(0.5)) / to_cell + offset;
            tests.extend([position.extend(l.kind as f32).to_array(), l.position.extend(l.radius).to_array(), l.direction.extend(l.attenuation).to_array(), l.area_width.extend(l.inv_spot_attenuation).to_array(), l.area_height.extend(l.cos_spot_angle).to_array(), offset.extend(to_cell).to_array(), [0.0; 4]]);
            cases.push((position, l));
        }
        let words = renderer.run_shading_verify("sdfgiLightTests", cases.len() as u32, &tests, 8 * cases.len());
        let (mut worst_att, mut hit_mismatch, mut capped) = (0.0f32, 0, 0);
        let mut worst_light = json!(null);
        for (k, (position, l)) in cases.iter().enumerate() {
            let gpu_att = f32::from_bits(words[k * 8]);
            let gpu_valid = words[k * 8 + 5] != 0;
            let gpu_hit = words[k * 8 + 6] != 0;
            capped += (words[k * 8 + 7] >= 4096) as usize;
            let cpu = light::light_at(l, *position);
            let cpu_att = cpu.map(|c| c.2).unwrap_or(0.0);
            if cpu.is_some() != gpu_valid {
                hit_mismatch += 1;
                continue;
            }
            if !gpu_valid {
                continue;
            }
            let d = (gpu_att - cpu_att).abs() / cpu_att.abs().max(1e-3);
            if d > worst_att {
                worst_att = d;
                worst_light = json!({"kind": l.kind, "position": position.to_array(), "gpu": gpu_att, "cpu": cpu_att, "gpuDirection": [f32::from_bits(words[k * 8 + 1]), f32::from_bits(words[k * 8 + 2]), f32::from_bits(words[k * 8 + 3])], "gpuDistance": f32::from_bits(words[k * 8 + 4])});
            }
            if let Some((dir, dist, _)) = cpu {
                let cpu_hit = light::shadow_ray(&field, 0, *position, dir, dist, None);
                hit_mismatch += (cpu_hit != gpu_hit) as usize;
            }
        }
        report.check("light pass: the WGSL port's attenuation (omni, spot, directional, area LTC) and shadow march match light.rs", worst_att < 1e-3 && hit_mismatch <= 1 && capped == 0, json!({"cases": cases.len(), "worstAttenuationRelative": worst_att, "worst": worst_light, "visibilityMismatches": hit_mismatch, "capped": capped}));

        // The probe rays: directions, the hash's rotation, and the march in cascade 0.
        let integrate = converged.integrate;
        let world_offset = converged.cascades.cascades[0].position / 8;
        let mut tests = Vec::new();
        let mut cases = Vec::new();
        for k in 0..96u32 {
            let probe = probes::probe_of(((k * 2654435761) % probes::PROBES as u32) as usize);
            let index = (k * 7) % (integrate.history_size * integrate.ray_count);
            let cpu_offset = probes::ray_offset(world_offset + probe);
            let pos = probes::probe_position(&field, 0, probe);
            tests.extend([pos.extend(index as f32).to_array(), offset.extend(to_cell).to_array(), [cpu_offset, (integrate.history_size * integrate.ray_count) as f32, integrate.y_mult, integrate.ray_bias], (world_offset + probe).as_vec3().extend(0.0).to_array()]);
            cases.push((pos, index, cpu_offset, probe));
        }
        let words = renderer.run_shading_verify("sdfgiProbeTests", cases.len() as u32, &tests, 8 * cases.len());
        let (mut worst_dir, mut worst_advance, mut hit_mismatch, mut worst_hash, mut capped) = (0.0f32, 0.0f32, 0, 0.0f32, 0);
        for (k, &(pos, index, cpu_offset, _)) in cases.iter().enumerate() {
            let mut dir = probes::vogel(index, integrate.history_size * integrate.ray_count, cpu_offset);
            dir.y *= integrate.y_mult;
            dir = dir.normalize();
            let gpu_dir = Vec3::new(f32::from_bits(words[k * 8]), f32::from_bits(words[k * 8 + 1]), f32::from_bits(words[k * 8 + 2]));
            worst_dir = worst_dir.max((gpu_dir - dir).abs().max_element());
            let start = pos + dir / dir.abs().max_element() * integrate.ray_bias / to_cell;
            let m = probes::march_cascade(&converged.data[0].stored.sdf, (start - offset) * to_cell, dir);
            let gpu_hit = words[k * 8 + 3] != 0;
            capped += (words[k * 8 + 4] >= 4096 || m.steps >= 4096) as usize;
            if gpu_hit != m.hit {
                hit_mismatch += 1;
            } else if m.hit {
                worst_advance = worst_advance.max((f32::from_bits(words[k * 8 + 5]) - m.advance).abs());
            }
            worst_hash = worst_hash.max((f32::from_bits(words[k * 8 + 6]) - cpu_offset).abs());
        }
        report.check("probe rays: the WGSL port's Vogel directions and march match probes.rs (the same rotation given)", worst_dir < 2e-4 && hit_mismatch <= 1 && worst_advance < 0.05 && capped == 0, json!({"cases": cases.len(), "worstDirection": worst_dir, "hitMismatches": hit_mismatch, "worstAdvanceCells": worst_advance, "capped": capped}));
        report.check("probe rays: the GPU's and the CPU's hashf3 rotation (sin of a large argument) recorded", true, json!({"worstDifference": worst_hash, "note": "hashf3 takes sin of arguments near 1e7; the shader's result depends on the GPU's sin precision"}));

        // STORE: SH means to the octahedral texels and their E5B9G9R9 words.
        let mut tests = Vec::new();
        let mut cases = Vec::new();
        for k in 0..36usize {
            let p = (k * 97) % probes::PROBES;
            let mean = converged.history[0].mean(p);
            let local = glam::IVec2::new((k % 6) as i32, (k / 6) as i32);
            tests.extend(mean.iter().map(|m| m.extend(0.0).to_array()));
            tests.push([local.x as f32, local.y as f32, 0.0, 0.0]);
            cases.push((mean, local));
        }
        let words = renderer.run_shading_verify("sdfgiStoreTests", cases.len() as u32, &tests, 8 * cases.len());
        let (mut worst, mut word_mismatch) = (0.0f32, 0);
        for (k, (mean, local)) in cases.iter().enumerate() {
            let (irr, rad) = probes::store_texel(mean, *local);
            for (i, v) in [irr.x, irr.y, irr.z, rad.x, rad.y, rad.z].iter().enumerate() {
                worst = worst.max((f32::from_bits(words[k * 8 + i]) - v).abs() / v.abs().max(1e-3));
            }
            word_mismatch += (words[k * 8 + 6] != light::encode_e5b9g9r9(irr)) as usize + (words[k * 8 + 7] != light::encode_e5b9g9r9(rad)) as usize;
        }
        report.check("STORE: the WGSL port's octahedral texels and E5B9G9R9 words match probes.rs", worst < 1e-4 && word_mismatch <= 2, json!({"cases": cases.len(), "worstRelative": worst, "wordMismatches": word_mismatch}));

        // Every scene beat of a timeline, rendered at its middle: text, labels, listings, readouts.
        if let Some(path) = &options.timeline {
            let timeline = lab::shot::Timeline::load(path)?;
            let mut problems = Vec::new();
            let mut beats = 0;
            for beat in timeline.beats.iter().filter(|b| b.kind == lab::shot::BeatKind::Scene) {
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

    // ---- 6. Static lights after a scroll (F1) and their lists (F2) ----------
    {
        let objects = sim::voxelized_objects(&lab::scene::Scene::study_room().records());
        let static_light = sim::omni(Vec3::new(1.5, 1.2, 1.0), 15.0, 1.0, settings.y_mult());
        let mut world = sim::World::new(&objects, settings, super::demos::ORIGIN);
        world.static_frame(&[static_light]);
        let field = world.field();
        // A floor cell near the light, kept by a 1.0 m walk (it moves 8 cells down in x).
        let keep = world.data[0].stored.cells.iter().filter(|c| c.facing() & 0b10 != 0).min_by(|a, b| {
            let pa = (a.cell().as_vec3() + Vec3::splat(0.5)) / field.to_cell(0) + field.offset(0);
            let pb = (b.cell().as_vec3() + Vec3::splat(0.5)) / field.to_cell(0) + field.offset(0);
            pa.distance(Vec3::new(1.5, 0.0, 1.0)).total_cmp(&pb.distance(Vec3::new(1.5, 0.0, 1.0)))
        }).copied().expect("a floor cell");
        let before = voxels::decode_rgbe8985(keep.light & 0x3FFF_FFFF);
        let mut after = Vec::new();
        let start_position = world.cascades.cascades[0].position;
        let mut steps_detail = Vec::new();
        for step in 0..3 {
            // Each walk moves cascade 0 by 8 cells (2.6 m would not: int(12.99) = 12 is not past 8 + 4).
            world.walk(&objects, super::demos::ORIGIN + Vec3::X * [1.0, 2.7, 4.3][step]);
            let dirty: Vec<String> = world.cascades.cascades.iter().map(|c| format!("{:?}", c.dirty)).collect();
            let given = world.static_frame(&[static_light]);
            steps_detail.push(json!({"dirty": dirty, "given": given.iter().map(|(c, l)| (c, l.len())).collect::<Vec<_>>(), "position": world.cascades.cascades[0].position.to_array()}));
            let walked = keep.cell() - (world.cascades.cascades[0].position - start_position);
            let cell = world.data[0].stored.cells.iter().find(|c| c.cell() == walked).copied();
            after.push(cell.map(|c| voxels::decode_rgbe8985(c.light & 0x3FFF_FFFF).y).unwrap_or(-1.0));
        }
        // Each scroll adds the light once more: 1x, 2x, 3x, 4x the first bake.
        let grows = after.iter().enumerate().all(|(k, &v)| (v / before.y - (k + 2) as f32).abs() < 0.1);
        report.check("static light after a scroll (F1): a kept cell holds the static light once more after every scroll the light overlaps (2x, 3x, 4x)", grows, json!({"cell": keep.cell().to_array(), "initial": before.y, "afterScrolls": after, "steps": steps_detail}));

        // F2: walk until a frame's dirty cascades are not 0, 1, ...; their lists then come from the wrong slots.
        let mut found = json!(null);
        let mut probe = sim::World::new(&[], settings, super::demos::ORIGIN);
        for k in 1..400 {
            let to = super::demos::ORIGIN + Vec3::new(0.37 * k as f32, 0.0, 0.0);
            probe.cascades.update(to);
            let wide = sim::omni(Vec3::new(1.5, 1.2, 1.0), 60.0, 1.0, settings.y_mult());
            let cull = sim::cull_static(&probe.cascades, &[wide]);
            if cull.cascade_light_index.first().is_some_and(|&c| c != 0) && cull.lists.iter().any(|l| !l.is_empty()) {
                let given = sim::static_pass_lights(&probe.cascades, &cull, &[wide]);
                found = json!({"walk": 0.37 * k as f32, "dirtyCascades": cull.cascade_light_index, "storedUnder": (0..8).filter(|&c| !cull.lists[c].is_empty()).collect::<Vec<_>>(), "given": given.iter().map(|(c, l)| (c, l.len())).collect::<Vec<_>>()});
                break;
            }
        }
        let dropped = found["given"].as_array().is_some_and(|g| g.iter().all(|e| e[1] == 0));
        report.check("static-light lists (F2): a frame whose only dirty cascade is not 0 reads its lights from slot 0 and gets none", dropped, found);
    }

    // Listings: each cites a source entry and a demonstration, and every demonstration has one.
    {
        let index = super::sources::INDEX.get();
        let listings = super::pseudocode::all();
        let bad: Vec<&str> = listings.iter().filter(|l| !index.entries.contains_key(&l.source) || l.demo.as_deref().is_none_or(|d| Demo::ALL.iter().all(|x| x.key() != d))).map(|l| l.name).collect();
        let without: Vec<&str> = Demo::ALL.iter().filter(|d| shot::demos().spec(d.id()).listing.is_none()).map(|d| d.key()).collect();
        report.check("every listing cites a source entry and a demonstration; every demonstration has a listing", bad.is_empty() && without.is_empty(), json!({"listings": listings.len(), "bad": bad, "demosWithout": without}));
    }

    // Every demonstration builds its state.
    let states_ok = Demo::ALL.iter().all(|&d| shot::for_demo(d).demo == d.id());
    report.check("every demonstration's file loads", states_ok, json!(Demo::ALL.iter().map(|d| d.key()).collect::<Vec<_>>()));

    // The story's actions (actions.rs): jumpFlood, started at frame 30, steps the flood every 0.5 s (15 frames).
    {
        let mut timeline = still_timeline(64, 64);
        (timeline.total_frames, timeline.beats[0].frames, timeline.chapters[0].end_frame) = (400, 400, 400);
        timeline.keys = serde_json::from_value(json!([{"frame": 0, "demo": "field"}, {"frame": 30, "run": {"name": "jumpFlood", "args": {"seconds": 0.5}}}, {"frame": 120, "stop": ["jumpFlood"]}]))?;
        let valid = timeline.validate().map_err(|e| e.to_string());
        let frames = [20u64, 30, 44, 45, 60, 119, 120];
        let steps: Vec<f32> = frames.iter().map(|&f| timeline.state_at(f).p("floodStep")).collect();
        // Before it, the demonstration's own step (the full-size pass); then 0, 0, 1, 2, 5; once stopped, the demonstration's again.
        let expected = [8.0, 0.0, 0.0, 1.0, 2.0, 5.0, 8.0];
        report.check("the jumpFlood action steps floodStep through a timeline, and stopping it hands floodStep back", valid.is_ok() && steps == expected, json!({"valid": valid, "frames": frames, "steps": steps, "expected": expected}));
    }

    // The image kind sdfgi_views.wgsl defines (lab/src/views.rs), and a story's instances of it.
    {
        let kinds = lab::views::kinds("godot_sdfgi").map_err(|e| anyhow::anyhow!("{e:#}"))?;
        let kind = kinds.iter().find(|k| k.key == "probeMap");
        let params: Vec<(&str, Vec<f32>)> = kind.map(|k| k.params.iter().map(|p| (p.key.as_str(), p.default.clone())).collect()).unwrap_or_default();
        let declared = kind.is_some_and(|k| k.size == [208, 104] && k.entry == "sdfgiProbeMapView") && params == [("probe", vec![7.0, 7.0, 8.0]), ("cascade", vec![0.0]), ("gain", vec![3.0])];
        let mut timeline = still_timeline(64, 64);
        timeline.keys = serde_json::from_value(json!([{"frame": 0, "image": "maps", "imageFrom": "probeMap", "imageParams": {"probe": [9, 7, 8]}}]))?;
        let valid = timeline.validate().map_err(|e| e.to_string());
        let mut wrong = still_timeline(64, 64);
        wrong.keys = serde_json::from_value(json!([{"frame": 0, "image": "maps", "imageFrom": "probeMap", "imageParams": {"probe": [9, 7]}}]))?;
        let refused = wrong.validate().map_err(|e| e.to_string()).err();
        report.check("the probeMap image kind: its size, entry point, and parameters; a story's instance of it loads, and a wrong parameter does not", declared && valid.is_ok() && refused.is_some(), json!({"params": format!("{params:?}"), "valid": valid, "refused": refused}));
    }

    // ---- 3. The source index against the checkout ----------------------------
    let index = super::sources::INDEX.get();
    report.check("source index", true, json!({"engine": index.engine.name, "version": index.engine.version, "commit": index.engine.commit, "entries": index.entries.len()}));
    let checkout = std::env::var_os("GODOT_SOURCE_DIR").map(PathBuf::from).unwrap_or_else(|| lab::resources::root().join("../../godot"));
    report.cited_lines("Godot", &super::sources::INDEX, &checkout, "version.py")?;

    let passed = report.passed();
    std::fs::write(&options.report, serde_json::to_string_pretty(&json!({"passed": passed, "checks": report.checks}))?)?;
    Ok(passed)
}
