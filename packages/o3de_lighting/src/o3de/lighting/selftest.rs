//! The lighting investigation's headless verification (`selftest`): the WGSL
//! port against its CPU mirror (brdf.rs) term by term, the BRDF texture, the
//! rendered pixels, every demonstration's frame, and the source index
//! against the O3DE checkout. Every check records what it compared and the
//! worst difference.

mod pixels;
mod scene_pass;
use pixels::*;
use scene_pass::*;

use super::brdf::{self, BRDF_MAP_SIZE, BrdfMap};
use super::demos;
use super::shot::{self, Demo};
use crate::o3de::lighting::text::tr;
use anyhow::{Context, Result};
use glam::{Vec2, Vec3};
use lab::camera::Projector;
use lab::frame::Statics;
use lab::gpu::{self, Gpu};
use lab::investigation::SelftestOptions;
use lab::layout;
use lab::renderer::{self, Renderer};
use lab::scene::{self, SHAPE_BOX, SHAPE_PLANE, ids};
use lab::shot::{SceneType, Timeline};
use lab::ui::{Chrome, MIN_TEXT_PT};
use lab::utility::selftest::{Report, Rng, request, still_timeline};
use lab::values::Choice;
use serde_json::{Value, json};

/// The relative difference, against a floor so values near zero compare absolutely.
fn rel(a: f32, b: f32) -> f32 {
    (a - b).abs() / b.abs().max(1e-3)
}

fn f(words: &[u32], i: usize) -> f32 {
    f32::from_bits(words[i])
}

fn f3(words: &[u32], i: usize) -> Vec3 {
    Vec3::new(f(words, i), f(words, i + 1), f(words, i + 2))
}

pub fn run(options: &SelftestOptions) -> Result<bool> {
    std::fs::create_dir_all(&options.work_dir)?;
    let mut report = Report::new();
    let gpu = Gpu::new(gpu::instance(None), None)?;
    let gpu_info = renderer::gpu_summary(&gpu);
    let mut renderer = Renderer::new(
        gpu,
        Statics::prepare(lab::scene::SceneId::STUDY_ROOM)?,
        1920,
        1080,
    )?;
    let still = still_timeline(1920, 1080);
    let mut rng = Rng(0x6f3d_e11b_4a5c_9e21);

    check_layouts(&mut report, &mut renderer)?;
    check_pick(&mut report, &mut renderer);
    check_animations(&mut report);
    // A frame uploads the objects and gShading the kernels read.
    renderer.render_frame(request(&shot::for_demo(Demo::Specular), &still), None);
    check_brdf_terms(&mut report, &mut renderer, &mut rng);
    check_brdf_texture(&mut report, &mut renderer, &mut rng);
    check_ggx_integral(&mut report);
    check_ibl(&mut report, &mut renderer, &mut rng);
    check_resolve(&mut report, &mut renderer, &mut rng);
    check_cascades(&mut report);
    check_furnace(&mut report);
    check_pixels(&mut report, &mut renderer, &still)?;
    check_clear_coat(&mut report, &mut rng);
    check_decals(&mut report, &mut renderer, &still)?;
    check_transmission(&mut report, &mut renderer, &still)?;
    check_probe_camera(&mut report);
    check_probes(&mut report, &mut renderer, &still)?;
    check_sun_shadows(&mut report, &mut renderer, &still);
    check_scene_pass(&mut report, &mut renderer, &still)?;
    check_ssao(&mut report, &mut renderer, &still)?;
    check_tiled_lights(&mut report, &mut renderer, &still)?;
    check_demos(&mut report, &mut renderer, &still, options)?;
    check_free_camera(&mut report, &mut renderer, &still)?;
    check_numbers(&mut report, &mut renderer, &still)?;
    check_formulas(&mut report);
    check_listings(&mut report);
    check_timeline(&mut report, &mut renderer, options)?;

    let passed = report.passed();
    std::fs::write(
        &options.report,
        serde_json::to_string_pretty(
            &json!({"investigation": "o3de/lighting", "passed": passed, "gpu": gpu_info, "checks": report.checks}),
        )?,
    )?;
    Ok(passed)
}

/// Section 1: the layouts, the shaders' validation, and the values' typed structs.
fn check_layouts(report: &mut Report, renderer: &mut Renderer) -> Result<()> {
    // ---- 1. Layouts and shader validation ---------------------------------
    let checks = layout::check_all(&renderer.statics.layouts);
    report.check(
        "Rust struct layouts match the WGSL layouts computed by naga",
        checks.iter().all(|c| c.ok),
        serde_json::to_value(&checks)?,
    );
    let manifest: Value = lab::shaders::manifest();
    let modules = manifest["modules"].as_array().cloned().unwrap_or_default();
    let module =
        lab::investigation::Investigation::scene_shading(&super::LIGHTING).map_or("", |s| s.module);
    let ported = modules
        .iter()
        .any(|m| m["module"] == module && m["validated"] == true);
    report.check(
        "the WGSL port (scene module + standard_lighting.wgsl) validated by naga",
        ported,
        json!(modules.iter().map(|m| &m["module"]).collect::<Vec<_>>()),
    );

    // The values (values.rs): each demonstration's, read through the typed struct and stored back, are the same.
    {
        let mut detail = Vec::new();
        for demo in Demo::ALL {
            // A demonstration's values must survive the round trip, and each field must read as its value in the state.
            let state = shot::for_demo(demo);
            let typed = state.view::<super::values::LightingValues>();
            let fields = serde_json::to_value(&typed)?;
            let mut stored = state.clone();
            stored.store(&typed);
            let same = stored == state
                && fields
                    .as_object()
                    .is_some_and(|f| f.iter().all(|(k, v)| v.as_f64() == Some(state.p(k) as f64)));
            if !same {
                detail.push(demo.key());
            }
        }
        report.check("every demonstration's values read the same through the typed struct (view) and back (store)", detail.is_empty(), json!({"differing": detail}));
    }
    Ok(())
}

/// A pick round-trips (script/picks.lam, `pickPixel`): the pixel picked projects back to the click.
fn check_pick(report: &mut Report, renderer: &mut Renderer) {
    // A pick round-trips (script/picks.lam, `pickPixel`): a click at a pixel of the scene, as
    // a click on a chain-target view at the same place sets it, picks that
    // pixel, and the point it inspects projects back to it.
    let mut worst = 0.0f32;
    let mut picked = 0;
    let content = demos::build_frame(
        &shot::for_demo(Demo::Room),
        &renderer.statics,
        [1280.0, 720.0],
        1.0,
        0.0,
    );
    let projector = Projector::new(&content.uniforms.camera);
    for (u, v) in [
        (0.30, 0.70),
        (0.42, 0.62),
        (0.55, 0.55),
        (0.70, 0.80),
        (0.50, 0.40),
    ] {
        // Each click picks the pixel under it; one that picks nothing is skipped, and the count shows it.
        let shown = shot::for_demo(Demo::Room);
        let mut state = shot::for_demo(Demo::Room);
        let mut frame = lab::actions::ClickFrame {
            shown: &shown,
            state: &mut state,
            uv: Vec2::new(u, v),
            content: &content,
            statics: &renderer.statics,
        };
        if lab::script::runtime::click("pickPixel", &mut frame) != Some(true) {
            continue;
        }
        picked += 1;
        let p = Vec3::new(
            state.p("inspectX"),
            state.p("inspectY"),
            state.p("inspectZ"),
        );
        let back = projector
            .project(p)
            .map(|(px, _)| px)
            .unwrap_or(Vec2::splat(f32::MAX));
        let at = Vec2::new(u, v) * Vec2::new(1280.0, 720.0);
        worst = worst.max(back.distance(at));
    }
    report.check("a pick round-trips: the picked pixel's surface point projects back to it (5 pixels, within 1 px)", picked == 5 && worst < 1.0 && shot::for_demo(Demo::Room).p("pickShown") == 0.0, json!({"picked": picked, "worstPixels": worst}));
}

/// The scripts' Play animations (script/animations.lam) are the waves they describe.
fn check_animations(report: &mut Report) {
    // The scripts' Play animations (script/animations.lam) are the waves they describe, and say what they set.
    use lab::animation::{Clip, Motion, Wave};
    let mut worst = 0.0f32;
    let mut samples = 0;
    let cases: [(Demo, Vec<Motion>); 2] = [
        (
            Demo::Surface,
            vec![
                Motion::Wave(Wave::new("sphereMetallic", 0.5, 0.5, 8.0)),
                Motion::Wave(Wave::new("sphereRoughness", 0.45, 0.35, 13.0)),
            ],
        ),
        (
            Demo::Ibl,
            vec![Motion::Wave(
                Wave::new("sphereRoughness", 0.5, -0.5, 12.0).cosine(),
            )],
        ),
    ];
    for (demo, motions) in cases {
        // Each demonstration's script and its waves are sampled at the same 400 times.
        let clip = Clip(motions);
        for k in 0..400 {
            let t = k as f32 * 0.137;
            let mut state = shot::for_demo(demo);
            let keys = lab::script::runtime::animate(state.demo, t, &mut state).unwrap_or_default();
            for (param, value) in clip.sample(t, |_| 0.0) {
                // Every key a wave moves must match it, and be among the keys the script says it set.
                samples += 1;
                worst = worst.max((state.p(&param) - value).abs());
                if !keys.contains(&param) {
                    // A key the script moves without saying so fails the check.
                    worst = f32::MAX;
                }
            }
        }
    }
    report.check("the scripts' Play animations (surface, ibl) are their waves within 1e-5, and set what they move", worst < 1e-5, json!({"samples": samples, "worst": worst}));
}

/// A surface the BRDF terms are checked on: its normal, the directions to the camera and the light, base
/// colour, metallic, roughness, specular factor, multiscatter, and the normal's screen derivatives.
type BrdfCase = (Vec3, Vec3, Vec3, Vec3, f32, f32, f32, bool, Vec3, Vec3);

/// `count` random surfaces for the BRDF terms: the kernel's inputs (six vec4s each), and the cases.
fn brdf_cases(rng: &mut Rng, count: usize) -> (Vec<[f32; 4]>, Vec<BrdfCase>) {
    let mut tests: Vec<[f32; 4]> = Vec::new();
    let mut cases = Vec::new();
    for k in 0..count {
        // A third of the surfaces are partly metal, the rest fully metal or not; every eighth is perfectly smooth.
        let n = rng.unit();
        let v = rng.above(n);
        let l = rng.above(n);
        let base = Vec3::new(rng.next(), rng.next(), rng.next());
        let metallic = if k % 3 == 0 {
            rng.next()
        } else {
            (k % 2) as f32
        };
        let roughness = if k % 8 == 0 {
            0.0
        } else {
            rng.next()
        };
        let factor = rng.next();
        let multiscatter = k % 5 != 0;
        // The normal's screen derivatives: zero, small, and large enough to reach the kernel's 0.18 cap.
        let spread = [0.0, 0.05, 0.6][k % 3];
        let (dndx, dndy) = (
            rng.unit() * spread * rng.next(),
            rng.unit() * spread * rng.next(),
        );
        tests.extend([
            base.extend(metallic).to_array(),
            n.extend(roughness).to_array(),
            v.extend(factor).to_array(),
            l.extend(if multiscatter {
                1.0
            } else {
                0.0
            })
            .to_array(),
            dndx.extend(0.0).to_array(),
            dndy.extend(0.0).to_array(),
        ]);
        cases.push((
            n,
            v,
            l,
            base,
            metallic,
            roughness,
            factor,
            multiscatter,
            dndx,
            dndy,
        ));
    }
    (tests, cases)
}

/// Section 2: the BRDF, term by term, on the GPU against brdf.rs.
fn check_brdf_terms(report: &mut Report, renderer: &mut Renderer, rng: &mut Rng) {
    let map = BrdfMap::get();
    // ---- 2. The BRDF, term by term ----------------------------------------
    let count = 96;
    let (tests, cases) = brdf_cases(rng, count);
    // The kernels read gShading from the frame above, with specular AA on (the default).
    let specular_aa = shot::for_demo(Demo::Specular).p("specularAA") > 0.5;
    let words = renderer.run_shading_verify("o3deBrdfTests", count as u32, &tests, 32 * count);
    let mut worst: Vec<(&str, f32)> = [
        "D",
        "G",
        "F",
        "SpecularGGX",
        "DiffuseLambertian",
        "NormalizedDisneyDiffuse",
        "diffuseResponse",
        "multiScatterCompensation",
        "brdf",
        "roughnessA2",
        "albedo",
        "specularF0",
    ]
    .iter()
    .map(|n| (*n, 0.0f32))
    .collect();
    for (k, &(n, v, l, base, metallic, roughness, factor, multiscatter, dndx, dndy)) in
        cases.iter().enumerate()
    {
        let mut s = brdf::Surface::new(Vec3::ZERO, n, base, metallic, factor, roughness);
        if specular_aa {
            s = s.with_specular_aa(dndx, dndy);
        }
        // The kernel passes the direction to the camera as the camera's position from a surface at the origin.
        let ld = brdf::LightingData::new(&s, v, multiscatter, map);
        let t = brdf::ggx_terms(
            ld.dir_to_camera,
            l,
            n,
            s.specular_f0,
            ld.n_dot_v,
            s.roughness_a2,
        );
        let spec = brdf::specular_ggx(
            ld.dir_to_camera,
            l,
            n,
            s.specular_f0,
            ld.n_dot_v,
            s.roughness_a2,
            ld.multi_scatter_compensation,
        );
        let lambert = brdf::diffuse_lambertian(s.albedo, n, l, ld.diffuse_response);
        let disney =
            brdf::normalized_disney_diffuse(s.albedo, n, ld.dir_to_camera, l, s.roughness_linear);
        // The kernel writes 32 words per case, the terms in the order of `cpu` below.
        let base_word = 32 * k;
        let cpu: [Vec<f32>; 12] = [
            vec![t.d],
            vec![t.g],
            t.f.to_array().to_vec(),
            spec.to_array().to_vec(),
            lambert.to_array().to_vec(),
            disney.to_array().to_vec(),
            ld.diffuse_response.to_array().to_vec(),
            ld.multi_scatter_compensation.to_array().to_vec(),
            ld.brdf.to_array().to_vec(),
            vec![s.roughness_a2],
            s.albedo.to_array().to_vec(),
            s.specular_f0.to_array().to_vec(),
        ];
        let mut at = base_word;
        for (i, values) in cpu.iter().enumerate() {
            // Each term keeps its worst relative difference over every case and component.
            for value in values {
                worst[i].1 = worst[i].1.max(rel(f(&words, at), *value));
                at += 1;
            }
        }
    }
    let passed = worst.iter().all(|w| w.1 < 2e-3);
    report.check("every BRDF term on the GPU matches the CPU mirror (96 random surfaces, with and without specular AA)", passed && specular_aa, json!({"worstRelativeDifference": worst.iter().map(|(n, w)| (n.to_string(), json!(w))).collect::<serde_json::Map<_, _>>(), "tolerance": 2e-3}));
}

/// Section 3: the BRDF texture, the WGSL port of BRDFTextureCS against the CPU's.
fn check_brdf_texture(report: &mut Report, renderer: &mut Renderer, rng: &mut Rng) {
    let map = BrdfMap::get();
    // ---- 3. The BRDF texture ----------------------------------------------
    // The lab fills the texture on the CPU (the same on every GPU). The WGSL
    // port of BRDFTextureCS must match it once sinTheta's square root is
    // guarded; with O3DE's unguarded root, the GPU drops the samples whose
    // cosTheta rounds above 1, which is reported, not tested.
    let size = BRDF_MAP_SIZE as u32;
    let mut texels: Vec<(u32, u32)> = vec![
        (0, 0),
        (size - 1, 0),
        (0, size - 1),
        (size - 1, size - 1),
        (size / 2, size / 2),
    ];
    // Then the last column's first eight texels, at the lowest roughness, and random texels up to 64.
    texels.extend((0..8).map(|y| (size - 1, y)));
    while texels.len() < 64 {
        texels.push((
            (rng.next() * size as f32) as u32 % size,
            (rng.next() * size as f32) as u32 % size,
        ));
    }
    let run = |renderer: &mut Renderer, guarded: bool| {
        let tests: Vec<[f32; 4]> = texels
            .iter()
            .map(|&(x, y)| {
                [
                    x as f32,
                    y as f32,
                    size as f32,
                    if guarded {
                        1.0
                    } else {
                        0.0
                    },
                ]
            })
            .collect();
        renderer.run_shading_verify(
            "o3deBrdfMapTests",
            tests.len() as u32,
            &tests,
            2 * tests.len(),
        )
    };
    let (guarded, unguarded) = (run(renderer, true), run(renderer, false));
    let mut worst_map = 0.0f32;
    let mut worst_texel = (0, 0);
    let mut lost: Vec<Value> = Vec::new();
    for (i, &(x, y)) in texels.iter().enumerate() {
        // The CPU texture's rows run the other way: texel y sits on row size - 1 - y.
        let cpu = map.texel(x as usize, (size - 1 - y) as usize);
        let gpu = Vec2::new(f(&guarded, 2 * i), f(&guarded, 2 * i + 1));
        let d = (gpu - cpu).abs().max_element();
        if d > worst_map {
            (worst_map, worst_texel) = (d, (x, y));
        }
        // Without the guard, the GPU's A + B falls short of the CPU's where it drops samples.
        let raw = Vec2::new(f(&unguarded, 2 * i), f(&unguarded, 2 * i + 1));
        let loss = (cpu.x + cpu.y) - (raw.x + raw.y);
        if loss > 2e-3 {
            lost.push(json!({"texel": [x, y], "roughness": (y as f32 + 0.5) / size as f32, "cpuAplusB": cpu.x + cpu.y, "gpuAplusB": raw.x + raw.y}));
        }
    }
    report.check(
        "BRDFTextureCS's WGSL port on the GPU (square root guarded) matches the CPU texture (64 texels)",
        worst_map < 2e-3,
        json!({"worstAbsoluteDifference": worst_map, "worstTexel": [worst_texel.0, worst_texel.1], "tolerance": 2e-3}),
    );
    report.check(
        "finding: with O3DE's unguarded sinTheta, this GPU loses energy at low roughness (reported, not tested)",
        true,
        json!({"texelsLosingEnergy": lost.len(), "of": texels.len(), "texels": lost}),
    );
    // Row r of the texture holds y = size - 1 - r: smoothness grows downwards.
    let max_energy = map.texels.iter().map(|t| t.x + t.y).fold(0.0f32, f32::max);
    let smooth_head_on = map.texel(BRDF_MAP_SIZE - 1, BRDF_MAP_SIZE - 6);
    let rough_grazing = map.texel(0, 0);
    report.check(
        "the BRDF texture conserves energy: A + B <= 1, and is ~1 for a smooth surface seen head on",
        max_energy <= 1.002 && (smooth_head_on.x + smooth_head_on.y) > 0.98 && smooth_head_on.y < 0.01,
        json!({"maxAplusB": max_energy, "smoothHeadOn": smooth_head_on.to_array(), "roughGrazing": rough_grazing.to_array()}),
    );
}

/// Section 4: GGX's D is a distribution: its projected integral is 1.
fn check_ggx_integral(report: &mut Report) {
    // ---- 4. GGX's D is a distribution: the projected integral is 1 ---------
    let mut integrals = Vec::new();
    for a2 in [0.002f32, 0.02, 0.1, 0.4, 1.0] {
        // 2 pi * integral over cos(theta) of D(cos) * cos, in f64 on the f32 D.
        let steps = 400_000;
        let mut sum = 0.0f64;
        for i in 0..steps {
            // A midpoint sum over mu = cos(theta), from 0 to 1.
            let mu = (i as f64 + 0.5) / steps as f64;
            sum += brdf::normal_distribution_ggx(mu as f32, a2) as f64 * mu;
        }
        integrals.push((a2, sum * std::f64::consts::TAU / steps as f64));
    }
    report.check(
        "NormalDistributionGGX integrates to 1 over projected solid angle",
        integrals.iter().all(|(_, v)| (v - 1.0).abs() < 5e-3),
        json!(integrals),
    );
}

/// Section 4b: image-based lighting on the GPU against the CPU's cubemaps.
fn check_ibl(report: &mut Report, renderer: &mut Renderer, rng: &mut Rng) {
    // ---- 4b. Image-based lighting -----------------------------------------
    let cubes = super::ibl::Ibl::get();
    let mut tests = Vec::new();
    for k in 0..96 {
        // Face centres, edges, and corners, and random directions, at whole and fractional mips.
        let dir = if k < 6 {
            [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z][k]
        } else if k < 12 {
            Vec3::new(
                1.0,
                1.0,
                if k % 2 == 0 {
                    1.0
                } else {
                    -1.0
                },
            )
            .normalize()
        } else {
            rng.unit()
        };
        // Every third case reads whole mip 0 or 3; the others read a random fractional mip up to 5.
        tests.push(
            dir.extend(if k % 3 == 0 {
                (k % 6) as f32
            } else {
                rng.range(0.0, 5.0)
            })
            .to_array(),
        );
    }
    let words =
        renderer.run_shading_verify("o3deIblTests", tests.len() as u32, &tests, 8 * tests.len());
    let mut worst_ibl = 0.0f32;
    for (k, t) in tests.iter().enumerate() {
        // Each case's words hold the specular lookup at 8k, and the diffuse one at 8k + 3.
        let dir = Vec3::new(t[0], t[1], t[2]);
        let rel3 = |a: Vec3, b: Vec3| (a - b).abs().max_element() / b.max_element().max(0.05);
        worst_ibl = worst_ibl
            .max(rel3(f3(&words, 8 * k), cubes.sample_specular(dir, t[3])))
            .max(rel3(f3(&words, 8 * k + 3), cubes.sample_diffuse(dir)));
    }
    report.check(
        "the cubemap lookups on the GPU match the CPU (96 directions and mips)",
        worst_ibl < 1e-4,
        json!({"worstRelativeDifference": worst_ibl, "tolerance": 1e-4}),
    );
}

/// MSAAResolveCustom on the GPU and the CPU.
fn check_resolve(report: &mut Report, renderer: &mut Renderer, rng: &mut Rng) {
    // MSAAResolveCustom: four samples clamped against the brightest
    // neighbour and averaged in the log domain, on the GPU and the CPU.
    let mut tests: Vec<[f32; 4]> = Vec::new();
    let mut cases = Vec::new();
    for k in 0..64 {
        // Four random samples per pixel, every fourth pixel eight times brighter; half resolve 2 samples, half 4.
        let samples: Vec<Vec3> = (0..4)
            .map(|_| {
                Vec3::new(rng.next(), rng.next(), rng.next())
                    * if k % 4 == 0 {
                        8.0
                    } else {
                        1.0
                    }
                    + Vec3::splat(1e-3)
            })
            .collect();
        let brightest = rng.next() * 2.0;
        let count = [2, 4][k % 2];
        tests.extend(samples.iter().map(|s| s.extend(0.0).to_array()));
        tests.push([brightest, 1.5, count as f32, 0.0]);
        cases.push((samples, brightest, count));
    }
    let words = renderer.run_shading_verify(
        "o3deResolveTests",
        cases.len() as u32,
        &tests,
        4 * cases.len(),
    );
    let mut worst_resolve = 0.0f32;
    let mut clamped = 0;
    for (k, (samples, brightest, count)) in cases.iter().enumerate() {
        let cpu = brdf::msaa_resolve_custom(&samples[..*count], *brightest, 1.5);
        let gpu = f3(&words, 4 * k);
        worst_resolve =
            worst_resolve.max((gpu - cpu).abs().max_element() / cpu.max_element().max(0.05));
        // The samples brighter than 1.5 times the brightest neighbour are counted, so the clamp is known to be tested.
        clamped += samples[..*count]
            .iter()
            .filter(|s| brdf::resolve_luminance(**s) > 1.5 * brightest)
            .count();
    }
    report.check("MSAAResolveCustom (neighbour clamp, log-domain mean) on the GPU matches the CPU (64 pixels)", worst_resolve < 1e-4 && clamped > 0, json!({"worstRelativeDifference": worst_resolve, "clampedSamples": clamped, "tolerance": 1e-4}));
}

/// The directional light's cascades (cascades.rs): the worked example's split and boxes.
fn check_cascades(report: &mut Report) {
    // The directional light's cascades (cascades.rs): the split of research.md
    // §6's worked example, boxes floored to whole texels, and the first cascade
    // holding a point at the camera's focus.
    use super::cascades;
    // Ratio 0.9 makes the splits mostly logarithmic; ratio 0 spaces them evenly (quarters of 0.1 to 100 m).
    let splits = cascades::split_depths(0.1, 100.0, 0.9);
    let splits_ok = splits
        .iter()
        .zip([3.014, 7.851, 23.507, 100.0])
        .all(|(a, b)| (a - b).abs() < 2e-3);
    let uniform_ok = cascades::split_depths(0.1, 100.0, 0.0)
        .iter()
        .zip([25.075, 50.05, 75.025, 100.0])
        .all(|(a, b)| (a - b).abs() < 1e-3);
    let observer = cascades::Observer {
        position: Vec3::new(1.0, 1.5, 2.0),
        forward: Vec3::new(-0.4, -0.2, -1.0).normalize(),
        fov_y: 60f32.to_radians(),
        aspect: 16.0 / 9.0,
        near: 0.1,
        far: 12.0,
    };
    let set = cascades::cascades(observer, Vec3::new(0.3, -1.0, 0.2), 0.9);
    // A snapped box starts on a whole texel, and a texel spans the sphere's diameter over the map's size.
    let snapped = set.cascades.iter().all(|k| {
        let cells = k.snapped_min / k.snap;
        (cells - cells.round()).abs().max_element() < 1e-3
            && (k.texel() - 2.0 * k.radius / cascades::SHADOWMAP_SIZE).abs() < 1e-6
    });
    // A point just ahead of the camera falls in the first cascade.
    let first = set.select(observer.position + observer.forward * 0.3, Vec3::Y);
    report.check(
        "the directional light's cascades follow DirectionalLightFeatureProcessor: log/uniform splits, snapped boxes, selection by bounds",
        splits_ok && uniform_ok && snapped && first == Some(0),
        json!({"splits": splits, "radii": set.cascades.iter().map(|k| k.radius).collect::<Vec<_>>(), "focusCascade": first}),
    );
}

/// A white furnace: a constant environment comes out constant in every mip and the diffuse cubemap.
fn check_furnace(report: &mut Report) {
    let cubes = super::ibl::Ibl::get();
    // A white furnace: both filters are normalised averages, so a constant
    // environment comes out constant in every mip and in the diffuse cubemap.
    let constant = Vec3::new(0.7, 0.5, 0.3);
    let furnace = super::ibl::Ibl::build(|_| constant, 32, 8, 64);
    let off = |c: &super::ibl::Cubemap| {
        c.texels
            .iter()
            .map(|t| (*t - constant).abs().max_element())
            .fold(0.0f32, f32::max)
    };
    let worst_furnace = furnace
        .specular
        .iter()
        .map(off)
        .fold(off(&furnace.diffuse), f32::max);
    report.check(
        "prefiltering keeps a constant environment constant (white furnace)",
        worst_furnace < 1e-5,
        json!({"worstDifference": worst_furnace, "mips": furnace.specular.len()}),
    );
    let stats: Vec<Value> = cubes.specular.iter().enumerate().map(|(m, c)| json!({"mip": m, "size": c.size, "roughness": super::ibl::mip_roughness(m), "peak": c.texels.iter().map(|t| t.max_element()).fold(0.0f32, f32::max)})).collect();
    report.check(
        "specular cubemap: six mips, the sky spreading as roughness grows",
        cubes.specular.len() == 6 && cubes.specular[5].size == 8,
        json!(stats),
    );
}

/// Section 7: every demonstration renders, with readable text.
fn check_demos(
    report: &mut Report,
    renderer: &mut Renderer,
    still: &Timeline,
    options: &SelftestOptions,
) -> Result<()> {
    // ---- 7. Every demonstration renders, with readable text ---------------
    let mut demo_detail = Vec::new();
    let mut demos_ok = true;
    for demo in Demo::ALL {
        // Each demonstration renders once with its readout, then once per listing style with its Code tab open.
        let mut state = shot::for_demo(demo);
        let result = renderer.render_frame(request(&state, still), None);
        let values = result
            .content
            .as_ref()
            .and_then(|c| c.readout.as_ref())
            .map_or(0, |r| r.rows.len());
        // Its formula above the readout, at full size, with every row below it.
        let formula = result
            .content
            .as_ref()
            .and_then(|c| c.readout.as_ref()?.formula.clone());
        let formula_ok = formula.as_deref() == Some(super::formulas::text(demo))
            && result.report.formula_scale == Some(1.0)
            && result.report.readout_overflow == 0;
        let ok = result.report.min_text_pt >= MIN_TEXT_PT && values > 0 && formula_ok;
        let png = options.work_dir.join(format!("o3de-{}.png", demo.key()));
        lab::record::write_png(
            &png,
            renderer.width,
            renderer.height,
            &renderer.read_frame(),
        )?;
        // The Code tab's panel.
        state.panel = lab::shot::PanelMode::Code;
        let listing = shot::demos().spec(demo.id()).listing.unwrap_or_default();
        state.code = Some(listing.to_string());
        // In each listing style.
        let mut listing_ok = true;
        let mut overflow = serde_json::Map::new();
        for style in lab::pseudocode::Style::ALL {
            let code = lab::pseudocode::with_style(style, || {
                renderer.render_frame(request(&state, still), None).report
            });
            listing_ok &= code.panel_code.as_deref() == Some(listing) && code.code_overflow == 0;
            overflow.insert(style.key().into(), json!(code.code_overflow));
        }
        demos_ok &= ok && listing_ok;
        demo_detail.push(json!({"demo": demo.key(), "minTextPt": result.report.min_text_pt, "readoutRows": values, "formula": formula, "formulaScale": result.report.formula_scale, "rowsBelowItCut": result.report.readout_overflow, "listing": listing, "listingFits": listing_ok, "linesCut": overflow, "snapshot": png}));
    }
    report.check("every demonstration renders its formula at full size, its readout, and its listing with text of at least 24 px", demos_ok, json!(demo_detail));
    Ok(())
}

/// The free camera changes only what is seen.
fn check_free_camera(report: &mut Report, renderer: &mut Renderer, still: &Timeline) -> Result<()> {
    // ---- 7b. The free camera changes only what is seen ----------------------
    // With the free camera on, the main camera's render (its image, the buffers its image kinds show) and the
    // readout are what they are with it off; only the frame on screen, drawn from the free camera, differs.
    let mut detail = Vec::new();
    let mut ok = true;
    for demo in [Demo::Room, Demo::Ssao, Demo::Specular] {
        // Each demonstration renders twice with the free camera off (to see it repeat), then with it on, turned 70°.
        let off = shot::for_demo(demo);
        let mut on = off.clone();
        on.free_camera.on = true;
        on.free_camera.yaw = off.camera.yaw + 70.0;
        let mut seen = Vec::new();
        for state in [&off, &off, &on] {
            let result = renderer.render_frame(request(state, still), None);
            let rows: Vec<(String, String)> = result
                .content
                .as_ref()
                .and_then(|c| c.readout.as_ref())
                .map(|r| {
                    r.rows
                        .iter()
                        .map(|(n, v, _)| (n.clone(), v.clone()))
                        .collect()
                })
                .unwrap_or_default();
            let (_, linear) = renderer
                .read_view(renderer::ViewKind::Live)
                .context("the main view")?;
            let buffers: Vec<Vec<u32>> = ["ssaoFull", "tileData", "lightList"]
                .iter()
                .map(|b| {
                    renderer
                        .read_chain_buffer(b)
                        .map(|(w, _)| w)
                        .unwrap_or_default()
                })
                .collect();
            seen.push((rows, linear, buffers, renderer.read_frame()));
        }
        // The render with the free camera on must match the second one off in all but the frame on screen.
        let (rows, image, buffers) = (
            seen[2].0 == seen[1].0,
            seen[2].1 == seen[1].1,
            seen[2].2 == seen[1].2,
        );
        let stable = seen[0].1 == seen[1].1 && seen[0].2 == seen[1].2;
        let screen_differs = seen[2].3 != seen[1].3;
        ok &= rows && image && buffers && screen_differs && !seen[1].0.is_empty();
        detail.push(json!({"demo": demo.key(), "readoutRowsEqual": rows, "readoutRows": seen[1].0.len(), "mainImageEqual": image, "buffersEqual": buffers, "offRendersRepeat": stable, "screenDiffers": screen_differs}));
    }
    report.check("the free camera: the main camera's image, its image kinds' buffers (SSAO, tiles, light lists), and the readout match those with it off; the screen differs", ok, json!(detail));
    Ok(())
}

/// Numbers the narration states, measured at the demonstrations' points.
fn check_numbers(report: &mut Report, renderer: &mut Renderer, still: &Timeline) -> Result<()> {
    // ---- 7a. Numbers the narration states, measured at the demonstrations' points ----
    // Each demonstration's scene is loaded by a frame of it first (the last one rendered may be another scene).
    let mut row = |demo: Demo, prefix: &str| -> Option<f32> {
        let state = shot::for_demo(demo);
        renderer.render_frame(request(&state, still), None);
        // The lab's frame: the demonstration's readout rows include its readout script's (script/readout.lam).
        let content =
            lab::shader_data::build_frame(&state, &renderer.statics, [1280.0, 720.0], 1.0, 0.0);
        let (_, value, _) = content
            .readout?
            .rows
            .into_iter()
            .find(|(name, _, _)| name.starts_with(prefix))?;
        // A row's value may be a vector; its first number is read.
        value
            .trim_start_matches('(')
            .split([',', ' '])
            .find(|v| !v.is_empty())?
            .parse()
            .ok()
    };
    // "At roughness zero point eight and this view angle, that is only about fifty seven percent."
    let energy = row(Demo::Multiscatter, "A + B");
    // "Six candelas at just under two metres give about one point seven lux, before the window."
    let distance = row(Demo::PointLight, tr("common-distance_d"));
    let lux = row(Demo::PointLight, "I / max");
    let ok = energy.is_some_and(|e| (0.565..0.575).contains(&e))
        && distance.is_some_and(|d| (1.85..2.0).contains(&d))
        && lux.is_some_and(|l| (1.65..1.75).contains(&l));
    report.check("the narration's measured numbers: A + B about 57% at the multiscatter point; about 1.7 lux from 6 cd just under 2 m", ok, json!({"singleScatteringEnergy": energy, "distanceMetres": distance, "luxBeforeWindow": lux}));
    Ok(())
}

/// The formulas: each typesets within the lab's limits, and each term lights.
fn check_formulas(report: &mut Report) {
    // ---- 7b. The formulas: each typesets within the lab's limits, and each term lights ----
    let mut formulas_ok = true;
    let mut detail = Vec::new();
    for demo in Demo::ALL {
        let tag = lab::rich_text::first_formula(super::formulas::text(demo));
        let checked = tag
            .as_ref()
            .ok_or_else(|| "no formula tag".to_string())
            .and_then(|(latex, display, _)| lab::formula::check(latex, *display));
        let terms = super::formulas::terms(demo);
        // One colour per \lit term, each term with some ink of its own.
        let (ok, size, glyph, lit) = match &checked {
            Ok(f) => (
                f.terms.len() == terms && f.terms.iter().all(|t| t.iter().any(|&c| c > 128)),
                [f.width, f.height],
                f.min_glyph_px,
                f.terms.len(),
            ),
            Err(_) => (false, [0, 0], 0.0, 0),
        };
        // `formula` -1 hides the formula, and the term count lights the last term at full strength.
        let hidden = super::formulas::readout(demo, -1.0).0.is_none()
            && super::formulas::readout(demo, terms as f32).1 == Some((terms, 1.0));
        formulas_ok &= ok && hidden && terms > 0;
        detail.push(json!({"demo": demo.key(), "size": size, "minGlyphPx": glyph, "terms": terms, "litMasks": lit, "error": checked.err(), "ok": ok}));
    }
    report.check("formulas: each demonstration's typesets with glyphs >= 24 px inside the panel, one colour and one lit mask per term", formulas_ok, json!(detail));
}

/// Section 8: the listings and the source index.
fn check_listings(report: &mut Report) {
    // ---- 8. Listings and the source index ---------------------------------
    let index = super::sources::INDEX.get();
    let listings = super::pseudocode::all();
    // A listing must cite an indexed source and a demonstration that exists.
    let listing_problems: Vec<String> = listings
        .iter()
        .filter(|l| {
            !index.entries.contains_key(&l.source)
                || l.demo
                    .as_deref()
                    .is_none_or(|d| Demo::ALL.iter().all(|x| x.key() != d))
        })
        .map(|l| l.name.to_string())
        .collect();
    let demo_problems: Vec<&str> = Demo::ALL
        .iter()
        .filter(|d| {
            !listings
                .iter()
                .any(|l| Some(l.name) == shot::demos().spec(d.id()).listing)
                || !index.demos.contains_key(d.key())
        })
        .map(|d| d.key())
        .collect();
    report.check("every listing cites a source entry and a demonstration; every demonstration has a listing and sources", listing_problems.is_empty() && demo_problems.is_empty(), json!({"listings": listing_problems, "demos": demo_problems}));
    report.check("source index", true, json!({"engine": index.engine.name, "version": index.engine.version, "commit": index.engine.commit, "entries": index.entries.len()}));
}

/// Section 9: the narrated timeline, every scene beat readable.
fn check_timeline(
    report: &mut Report,
    renderer: &mut Renderer,
    options: &SelftestOptions,
) -> Result<()> {
    // ---- 9. The narrated timeline: every scene beat readable ---------------
    if let Some(path) = &options.timeline {
        let timeline = Timeline::load(path)?;
        let mut problems = Vec::new();
        let mut beats = 0;
        for beat in timeline
            .beats
            .iter()
            .filter(|b| b.kind == SceneType::ThreeD)
        {
            // Each 3D beat is checked at its middle frame, as the recording draws it.
            let frame = beat.start_frame + beat.frames / 2;
            let state = timeline.state_at(frame);
            let mut req = request(&state, &timeline);
            req.chrome = Chrome::Recording {
                timeline: &timeline,
                frame,
            };
            req.chapter_number = beat.chapter;
            let mut r = renderer.render_frame(req, None).report;
            // A listing fits in the block style too.
            if state.panel == lab::shot::PanelMode::Code {
                let mut req = request(&state, &timeline);
                req.chrome = Chrome::Recording {
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
            if r.min_text_pt < MIN_TEXT_PT
                || r.label_overlaps > 0
                || r.labels_outside > 0
                || r.code_overflow > 0
                || r.readout_overflow > 0
                || r.formula_scale.is_some_and(|s| s < 1.0)
            {
                problems.push(json!({"beat": beat.id, "frame": frame, "minTextPt": r.min_text_pt, "labelOverlaps": r.label_overlaps, "labelsOutside": r.labels_outside, "codeOverflow": r.code_overflow, "readoutOverflow": r.readout_overflow, "formulaScale": r.formula_scale}));
            }
        }
        report.check("every scene beat of the timeline: text >= 24 px, labels inside and apart, listings, formulas, and readouts fit", problems.is_empty() && beats > 0, json!({"timeline": path, "sceneBeats": beats, "problems": problems}));
    }
    Ok(())
}
