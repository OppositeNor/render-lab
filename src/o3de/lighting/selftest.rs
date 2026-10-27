//! The lighting investigation's headless verification (`selftest`): the WGSL
//! port against its CPU mirror (brdf.rs) term by term, the BRDF texture, the
//! rendered pixels, every demonstration's frame, and the source index
//! against the O3DE checkout. Every check records what it compared and the
//! worst difference.

use super::brdf::{self, BrdfMap, BRDF_MAP_SIZE};
use super::demos;
use super::shot::{self, Demo};
use crate::lab::camera::Projector;
use crate::lab::frame::Statics;
use crate::lab::gpu::{self, Gpu};
use crate::o3de::lighting::text::tr;
use crate::lab::investigation::SelftestOptions;
use crate::lab::layout;
use crate::lab::renderer::{self, FrameRequest, Renderer};
use crate::lab::scene::{self, ids, SHAPE_BOX, SHAPE_PLANE};
use crate::lab::shot::{BeatKind, ShotState, Timeline};
use crate::lab::ui::{Chrome, MIN_TEXT_PT};
use anyhow::{Context, Result};
use glam::{Vec2, Vec3};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

struct Report {
    checks: Vec<Value>,
}

impl Report {
    fn check(&mut self, name: &str, passed: bool, detail: Value) {
        eprintln!("{} {name}", if passed { "PASS" } else { "FAIL" });
        self.checks.push(json!({"name": name, "passed": passed, "detail": detail}));
    }
}

/// Deterministic pseudo-random numbers (xorshift) for test inputs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }
    fn unit(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0));
            if v.length_squared() > 0.01 && v.length_squared() <= 1.0 {
                return v.normalize();
            }
        }
    }
    /// A direction in the hemisphere around `n`.
    fn above(&mut self, n: Vec3) -> Vec3 {
        let v = self.unit();
        if v.dot(n) < 0.05 { (v - n * (v.dot(n) - 0.05 - self.next())).normalize() } else { v }
    }
}

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

fn still_timeline(width: u32, height: u32) -> Timeline {
    serde_json::from_value(json!({
        "fps": 30, "width": width, "height": height, "totalFrames": 1, "title": "selftest", "engineLabel": "",
        "chapters": [{"number": 1, "title": "Selftest", "startFrame": 0, "endFrame": 1}],
        "beats": [{"id": "still", "chapter": 1, "kind": "scene", "startFrame": 0, "frames": 1, "headline": "Selftest frame", "captions": []}],
        "keys": []
    }))
    .unwrap()
}

fn request<'a>(state: &'a ShotState, timeline: &'a Timeline) -> FrameRequest<'a> {
    FrameRequest {
        state,
        beat_kind: BeatKind::Scene,
        image: None,
        beat_sources: &[],
        chapter_number: 1,
        fade: 0.0,
        time: 0.0,
        raw_input: egui::RawInput { time: Some(0.0), ..Default::default() },
        chrome: Chrome::Recording { timeline, frame: 0 },
        stamp: None,
    }
}

pub fn run(options: &SelftestOptions) -> Result<bool> {
    std::fs::create_dir_all(&options.work_dir)?;
    let mut report = Report { checks: Vec::new() };
    let gpu = Gpu::new(gpu::instance(None), None)?;
    let gpu_info = renderer::gpu_summary(&gpu);
    let mut renderer = Renderer::new(gpu, Statics::prepare()?, 1920, 1080)?;
    let still = still_timeline(1920, 1080);
    let map = BrdfMap::get();

    // ---- 1. Layouts and shader validation ---------------------------------
    let checks = layout::check_all(&renderer.statics.layouts);
    report.check("Rust struct layouts match the WGSL layouts computed by naga", checks.iter().all(|c| c.ok), serde_json::to_value(&checks)?);
    let manifest: Value = serde_json::from_str(crate::lab::shaders::MANIFEST)?;
    let modules = manifest["modules"].as_array().cloned().unwrap_or_default();
    let ported = modules.iter().any(|m| m["module"] == "standard_lighting" && m["validated"] == true);
    report.check("the WGSL port (scene module + standard_lighting.wgsl) validated by naga at build time", ported, json!(modules.iter().map(|m| &m["module"]).collect::<Vec<_>>()));

    // A frame uploads the objects and gShading the kernels read.
    renderer.render_frame(request(&shot::for_demo(Demo::Specular), &still), None);

    // ---- 2. The BRDF, term by term ----------------------------------------
    let mut rng = Rng(0x6f3d_e11b_4a5c_9e21);
    let count = 96;
    let mut tests: Vec<[f32; 4]> = Vec::new();
    let mut cases = Vec::new();
    for k in 0..count {
        let n = rng.unit();
        let v = rng.above(n);
        let l = rng.above(n);
        let base = Vec3::new(rng.next(), rng.next(), rng.next());
        let metallic = if k % 3 == 0 { rng.next() } else { (k % 2) as f32 };
        let roughness = if k % 8 == 0 { 0.0 } else { rng.next() };
        let factor = rng.next();
        let multiscatter = k % 5 != 0;
        // The normal's screen derivatives: zero, small, and large enough to reach the kernel's 0.18 cap.
        let spread = [0.0, 0.05, 0.6][k % 3];
        let (dndx, dndy) = (rng.unit() * spread * rng.next(), rng.unit() * spread * rng.next());
        tests.extend([base.extend(metallic).to_array(), n.extend(roughness).to_array(), v.extend(factor).to_array(), l.extend(if multiscatter { 1.0 } else { 0.0 }).to_array(), dndx.extend(0.0).to_array(), dndy.extend(0.0).to_array()]);
        cases.push((n, v, l, base, metallic, roughness, factor, multiscatter, dndx, dndy));
    }
    // The kernels read gShading from the frame above, with specular AA on (the default).
    let specular_aa = shot::for_demo(Demo::Specular).p("specularAA") > 0.5;
    let words = renderer.run_shading_verify("o3deBrdfTests", count as u32, &tests, 32 * count);
    let mut worst: Vec<(&str, f32)> = ["D", "G", "F", "SpecularGGX", "DiffuseLambertian", "NormalizedDisneyDiffuse", "diffuseResponse", "multiScatterCompensation", "brdf", "roughnessA2", "albedo", "specularF0"].iter().map(|n| (*n, 0.0f32)).collect();
    for (k, &(n, v, l, base, metallic, roughness, factor, multiscatter, dndx, dndy)) in cases.iter().enumerate() {
        let mut s = brdf::Surface::new(Vec3::ZERO, n, base, metallic, factor, roughness);
        if specular_aa {
            s = s.with_specular_aa(dndx, dndy);
        }
        // The kernel passes the direction to the camera as the camera's position from a surface at the origin.
        let ld = brdf::LightingData::new(&s, v, multiscatter, map);
        let t = brdf::ggx_terms(ld.dir_to_camera, l, n, s.specular_f0, ld.n_dot_v, s.roughness_a2);
        let spec = brdf::specular_ggx(ld.dir_to_camera, l, n, s.specular_f0, ld.n_dot_v, s.roughness_a2, ld.multi_scatter_compensation);
        let lambert = brdf::diffuse_lambertian(s.albedo, n, l, ld.diffuse_response);
        let disney = brdf::normalized_disney_diffuse(s.albedo, n, ld.dir_to_camera, l, s.roughness_linear);
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
            for value in values {
                worst[i].1 = worst[i].1.max(rel(f(&words, at), *value));
                at += 1;
            }
        }
    }
    let passed = worst.iter().all(|w| w.1 < 2e-3);
    report.check("every BRDF term on the GPU matches the CPU mirror (96 random surfaces, with and without specular AA)", passed && specular_aa, json!({"worstRelativeDifference": worst.iter().map(|(n, w)| (n.to_string(), json!(w))).collect::<serde_json::Map<_, _>>(), "tolerance": 2e-3}));

    // ---- 3. The BRDF texture ----------------------------------------------
    // The lab fills the texture on the CPU (the same on every GPU). The WGSL
    // port of BRDFTextureCS must match it once sinTheta's square root is
    // guarded; with O3DE's unguarded root, the GPU drops the samples whose
    // cosTheta rounds above 1, which is reported, not tested.
    let size = BRDF_MAP_SIZE as u32;
    let mut texels: Vec<(u32, u32)> = vec![(0, 0), (size - 1, 0), (0, size - 1), (size - 1, size - 1), (size / 2, size / 2)];
    texels.extend((0..8).map(|y| (size - 1, y)));
    while texels.len() < 64 {
        texels.push(((rng.next() * size as f32) as u32 % size, (rng.next() * size as f32) as u32 % size));
    }
    let run = |renderer: &mut Renderer, guarded: bool| {
        let tests: Vec<[f32; 4]> = texels.iter().map(|&(x, y)| [x as f32, y as f32, size as f32, if guarded { 1.0 } else { 0.0 }]).collect();
        renderer.run_shading_verify("o3deBrdfMapTests", tests.len() as u32, &tests, 2 * tests.len())
    };
    let (guarded, unguarded) = (run(&mut renderer, true), run(&mut renderer, false));
    let mut worst_map = 0.0f32;
    let mut worst_texel = (0, 0);
    let mut lost: Vec<Value> = Vec::new();
    for (i, &(x, y)) in texels.iter().enumerate() {
        let cpu = map.texel(x as usize, (size - 1 - y) as usize);
        let gpu = Vec2::new(f(&guarded, 2 * i), f(&guarded, 2 * i + 1));
        let d = (gpu - cpu).abs().max_element();
        if d > worst_map {
            (worst_map, worst_texel) = (d, (x, y));
        }
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

    // ---- 4. GGX's D is a distribution: the projected integral is 1 ---------
    let mut integrals = Vec::new();
    for a2 in [0.002f32, 0.02, 0.1, 0.4, 1.0] {
        // 2 pi * integral over cos(theta) of D(cos) * cos, in f64 on the f32 D.
        let steps = 400_000;
        let mut sum = 0.0f64;
        for i in 0..steps {
            let mu = (i as f64 + 0.5) / steps as f64;
            sum += brdf::normal_distribution_ggx(mu as f32, a2) as f64 * mu;
        }
        integrals.push((a2, sum * std::f64::consts::TAU / steps as f64));
    }
    report.check("NormalDistributionGGX integrates to 1 over projected solid angle", integrals.iter().all(|(_, v)| (v - 1.0).abs() < 5e-3), json!(integrals));

    // ---- 4b. Image-based lighting -----------------------------------------
    let cubes = super::ibl::Ibl::get();
    let mut tests = Vec::new();
    for k in 0..96 {
        // Face centres, edges, and corners, and random directions, at whole and fractional mips.
        let dir = if k < 6 { [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z][k] } else if k < 12 { Vec3::new(1.0, 1.0, if k % 2 == 0 { 1.0 } else { -1.0 }).normalize() } else { rng.unit() };
        tests.push(dir.extend(if k % 3 == 0 { (k % 6) as f32 } else { rng.range(0.0, 5.0) }).to_array());
    }
    let words = renderer.run_shading_verify("o3deIblTests", tests.len() as u32, &tests, 8 * tests.len());
    let mut worst_ibl = 0.0f32;
    for (k, t) in tests.iter().enumerate() {
        let dir = Vec3::new(t[0], t[1], t[2]);
        let rel3 = |a: Vec3, b: Vec3| (a - b).abs().max_element() / b.max_element().max(0.05);
        worst_ibl = worst_ibl.max(rel3(f3(&words, 8 * k), cubes.sample_specular(dir, t[3]))).max(rel3(f3(&words, 8 * k + 3), cubes.sample_diffuse(dir)));
    }
    report.check("the cubemap lookups on the GPU match the CPU (96 directions and mips)", worst_ibl < 1e-4, json!({"worstRelativeDifference": worst_ibl, "tolerance": 1e-4}));
    // A white furnace: both filters are normalised averages, so a constant
    // environment comes out constant in every mip and in the diffuse cubemap.
    let constant = Vec3::new(0.7, 0.5, 0.3);
    let furnace = super::ibl::Ibl::build(|_| constant, 32, 8, 64);
    let off = |c: &super::ibl::Cubemap| c.texels.iter().map(|t| (*t - constant).abs().max_element()).fold(0.0f32, f32::max);
    let worst_furnace = furnace.specular.iter().map(off).fold(off(&furnace.diffuse), f32::max);
    report.check("prefiltering keeps a constant environment constant (white furnace)", worst_furnace < 1e-5, json!({"worstDifference": worst_furnace, "mips": furnace.specular.len()}));
    let stats: Vec<Value> = cubes.specular.iter().enumerate().map(|(m, c)| json!({"mip": m, "size": c.size, "roughness": super::ibl::mip_roughness(m), "peak": c.texels.iter().map(|t| t.max_element()).fold(0.0f32, f32::max)})).collect();
    report.check("specular cubemap: six mips, the softbox spreading as roughness grows", cubes.specular.len() == 6 && cubes.specular[5].size == 8, json!(stats));

    // ---- 5. Whole pixels: o3dePixel on the GPU against brdf::pixel ---------
    let mut pixel_detail = Vec::new();
    let mut pixels_ok = true;
    for view in 0..brdf::view::COUNT {
        let mut state = shot::for_demo(Demo::Specular);
        state.set("viewMode", view as f32);
        // Every other view with the sphere's occlusion maps assigned.
        let occluded = view % 2 == 1;
        if occluded {
            state.set("sphereOcclusion", 1.0);
            state.set("diffuseOcclusionFactor", 1.6);
            state.set("specularOcclusionFactor", 0.7);
        }
        // Every third view with the sphere's clear coat.
        let coated = view % 3 == 2;
        if coated {
            state.set("sphereClearCoat", 1.0);
            state.set("clearCoatFactor", 0.8);
            state.set("clearCoatRoughness", 0.2);
        }
        let frame = renderer.render_frame(request(&state, &still), None);
        let content = frame.content.context("scene content")?;
        let shading = demos::shading(&state);
        let eye = Vec3::from(content.uniforms.camera.position);
        let projector = Projector::new(&content.uniforms.camera);
        let mut tests = Vec::new();
        let mut points = Vec::new();
        for j in 0..12 {
            for i in 0..16 {
                let pixel = Vec2::new((i as f32 + 0.5) / 16.0, (j as f32 + 0.5) / 12.0) * projector.viewport;
                let (o, d) = projector.ray(pixel);
                let hit = scene::trace_scene(&content.objects, o, d, 100.0, ids::PANEL as u32);
                if !hit.is_hit() {
                    continue;
                }
                let p = o + d * hit.t;
                let n = if hit.normal.dot(d) > 0.0 { -hit.normal } else { hit.normal };
                tests.extend([p.extend(hit.object as f32).to_array(), n.extend(0.0).to_array()]);
                points.push((p, n, hit.object));
            }
        }
        let words = renderer.run_shading_verify("o3dePixelTests", points.len() as u32, &tests, 8 * points.len());
        let mut worst = 0.0f32;
        for (k, &(p, n, object)) in points.iter().enumerate() {
            // The kernel has no screen, so no normal derivatives.
            let cpu = brdf::pixel(&shading, &content.objects, p, n, eye, object, state.p("exposure"), map, [Vec3::ZERO; 2]);
            let gpu = f3(&words, 8 * k);
            let d = (gpu - cpu.color).abs().max_element() / cpu.color.max_element().max(0.05);
            worst = worst.max(d);
        }
        pixels_ok &= worst < 3e-3;
        pixel_detail.push(json!({"view": view, "occlusionMaps": occluded, "clearCoat": coated, "points": points.len(), "worstRelativeDifference": worst}));
    }
    report.check("o3dePixel on the GPU matches brdf::pixel in every view (192 rays each; with occlusion maps and clear coat in turn)", pixels_ok, json!({"views": pixel_detail, "tolerance": 3e-3}));

    // ---- 5b. Clear coat at factor 0 changes no direct-lighting term -------
    {
        let mut worst = 0.0f32;
        for _ in 0..256 {
            let n = rng.unit();
            let (v, l) = (rng.above(n), rng.above(n));
            let base = Vec3::new(rng.next(), rng.next(), rng.next());
            let s = brdf::Surface::new(Vec3::ZERO, n, base, rng.next(), rng.next(), rng.next());
            let mut coated = s;
            coated.clear_coat = Some(brdf::ClearCoat { factor: 0.0, roughness: rng.next().max(1e-3), normal: n });
            coated.specular_f0 = brdf::apply_clear_coat_to_specular_f0(s.specular_f0, 0.0);
            let lit = |surface: &brdf::Surface| {
                let mut ld = brdf::LightingData::new(surface, v, true, map);
                brdf::apply_light(surface, &mut ld, Vec3::ONE, l, brdf::DiffuseModel::Lambertian);
                (ld.diffuse_lighting, ld.specular_lighting)
            };
            let ((d0, s0), (d1, s1)) = (lit(&s), lit(&coated));
            worst = worst.max((d0 - d1).abs().max_element()).max((s0 - s1).abs().max_element());
        }
        report.check("clear coat at factor 0 leaves the F0, the diffuse response, and every light's diffuse and specular unchanged", worst <= 1e-6, json!({"surfaces": 256, "worstDifference": worst}));
    }

    // ---- 5c. Decals: o3dePixel on the GPU against brdf::pixel inside them --
    {
        let mut detail = Vec::new();
        let mut ok = true;
        // The demonstration's start, the splat tilted under the grid at partial opacity, and the grid turned with weak attenuation.
        let configs: [&[(&str, f32)]; 3] = [&[], &[("decalTilt", 40.0), ("decalSplatSortKey", 15.0), ("decalOpacity", 0.6), ("decalNormalOpacity", 0.5)], &[("decalTurn", 70.0), ("decalAngleAttenuation", 0.3), ("decalTilt", -50.0)]];
        for config in configs {
            let mut state = shot::for_demo(Demo::Decals);
            for &(name, value) in config {
                state.set(name, value);
            }
            let frame = renderer.render_frame(request(&state, &still), None);
            let content = frame.content.context("scene content")?;
            let shading = demos::shading(&state);
            let eye = Vec3::from(content.uniforms.camera.position);
            let projector = Projector::new(&content.uniforms.camera);
            let (mut tests, mut points) = (Vec::new(), Vec::new());
            for j in 0..24 {
                for i in 0..32 {
                    let (o, d) = projector.ray(Vec2::new((i as f32 + 0.5) / 32.0, (j as f32 + 0.5) / 24.0) * projector.viewport);
                    let hit = scene::trace_scene(&content.objects, o, d, 100.0, ids::PANEL as u32);
                    let p = o + d * hit.t;
                    if !hit.is_hit() || !shading.decals.iter().any(|dc| super::decals::sample(dc, p, hit.normal).is_some()) {
                        continue;
                    }
                    let n = if hit.normal.dot(d) > 0.0 { -hit.normal } else { hit.normal };
                    tests.extend([p.extend(hit.object as f32).to_array(), n.extend(0.0).to_array()]);
                    points.push((p, n, hit.object));
                }
            }
            let words = renderer.run_shading_verify("o3dePixelTests", points.len() as u32, &tests, 8 * points.len());
            let mut worst = 0.0f32;
            for (k, &(p, n, object)) in points.iter().enumerate() {
                let cpu = brdf::pixel(&shading, &content.objects, p, n, eye, object, state.p("exposure"), map, [Vec3::ZERO; 2]);
                worst = worst.max((f3(&words, 8 * k) - cpu.color).abs().max_element() / cpu.color.max_element().max(0.05));
            }
            ok &= worst < 3e-3 && points.len() > 100;
            detail.push(json!({"settings": config.iter().map(|(n, v)| format!("{n}={v}")).collect::<Vec<_>>(), "pointsInDecals": points.len(), "worstRelativeDifference": worst}));
        }
        // ApplyDecal on a metallic surface: specularF0 moves by the opacity times metallic.
        let decal = super::decals::scene_decals(&shot::for_demo(Demo::Decals))[0];
        let mut metal = brdf::Surface::new(decal.position, Vec3::Y, Vec3::new(0.9, 0.6, 0.3), 0.75, 0.5, 0.4);
        let before = metal;
        super::decals::apply_decal(&decal, &mut metal, Vec3::Y);
        let k = super::decals::sample(&decal, decal.position, Vec3::Y).context("the splat's centre")?;
        let f0_ok = (metal.specular_f0 - before.specular_f0.lerp(k.blended, k.albedo_opacity * 0.75)).abs().max_element() < 1e-6 && k.albedo_opacity > 0.9;
        report.check(
            "decals: o3dePixel on the GPU matches brdf::pixel at points inside the decals (three settings), and ApplyDecal moves a metal's specularF0 by opacity × metallic",
            ok && f0_ok,
            json!({"settings": detail, "tolerance": 3e-3, "metallicF0": f0_ok}),
        );
    }

    // ---- 5d. Transmission and the sphere light: o3dePixel against brdf::pixel --
    {
        let mut detail = Vec::new();
        let mut ok = true;
        let configs: [&[(&str, f32)]; 3] = [&[], &[("transmissionPower", 2.0), ("transmissionDistortion", 0.5), ("transmissionThickness", 0.2), ("sphereLightBulb", 0.3)], &[("teapotTransmission", 2.0)]];
        for config in configs {
            let mut state = shot::for_demo(Demo::Transmission);
            for &(name, value) in config {
                state.set(name, value);
            }
            let frame = renderer.render_frame(request(&state, &still), None);
            let content = frame.content.context("scene content")?;
            let shading = demos::shading(&state);
            let eye = Vec3::from(content.uniforms.camera.position);
            let projector = Projector::new(&content.uniforms.camera);
            let (mut tests, mut points) = (Vec::new(), Vec::new());
            let mut lit = 0usize;
            for j in 0..24 {
                for i in 0..32 {
                    let (o, d) = projector.ray(Vec2::new((i as f32 + 0.5) / 32.0, (j as f32 + 0.5) / 24.0) * projector.viewport);
                    let hit = scene::trace_scene(&content.objects, o, d, 100.0, ids::PANEL as u32);
                    if !hit.is_hit() {
                        continue;
                    }
                    let p = o + d * hit.t;
                    let n = if hit.normal.dot(d) > 0.0 { -hit.normal } else { hit.normal };
                    tests.extend([p.extend(hit.object as f32).to_array(), n.extend(0.0).to_array()]);
                    points.push((p, n, hit.object));
                }
            }
            let words = renderer.run_shading_verify("o3dePixelTests", points.len() as u32, &tests, 8 * points.len());
            let mut worst = 0.0f32;
            for (k, &(p, n, object)) in points.iter().enumerate() {
                let cpu = brdf::pixel(&shading, &content.objects, p, n, eye, object, state.p("exposure"), map, [Vec3::ZERO; 2]);
                lit += usize::from(cpu.lighting.translucent_back_lighting.max_element() > 1e-3);
                worst = worst.max((f3(&words, 8 * k) - cpu.color).abs().max_element() / cpu.color.max_element().max(0.05));
            }
            ok &= worst < 3e-3 && lit > 20;
            detail.push(json!({"settings": config.iter().map(|(n, v)| format!("{n}={v}")).collect::<Vec<_>>(), "points": points.len(), "backLit": lit, "worstRelativeDifference": worst}));
        }
        report.check(
            "transmission: o3dePixel with the sphere light matches brdf::pixel, thick and thin, with back lighting on the teapot",
            ok,
            json!({"settings": detail, "tolerance": 3e-3}),
        );
    }

    // ---- 5e. Reflection probes: o3dePixel against brdf::pixel -------------
    {
        let mut detail = Vec::new();
        let mut ok = true;
        let mut grey = 0usize;
        // The last with a clear-coated sphere, whose forward-pass IBL blends in its own probe.
        let configs: [&[(&str, f32)]; 5] = [&[], &[("probeParallax", 0.0)], &[("probeInnerScale", 0.5), ("probeExposure", 1.0)], &[("ibl", 0.0)], &[("sphereClearCoat", 1.0), ("probeExposure", 0.5)]];
        for config in configs {
            let mut state = shot::for_demo(Demo::Reflections);
            for &(name, value) in config {
                state.set(name, value);
            }
            let frame = renderer.render_frame(request(&state, &still), None);
            let content = frame.content.context("scene content")?;
            let shading = demos::shading(&state);
            let eye = Vec3::from(content.uniforms.camera.position);
            let projector = Projector::new(&content.uniforms.camera);
            let (mut tests, mut points) = (Vec::new(), Vec::new());
            for j in 0..36 {
                for i in 0..48 {
                    let (o, d) = projector.ray(Vec2::new((i as f32 + 0.5) / 48.0, (j as f32 + 0.5) / 36.0) * projector.viewport);
                    let hit = scene::trace_scene(&content.objects, o, d, 100.0, ids::PANEL as u32);
                    if !hit.is_hit() {
                        continue;
                    }
                    let p = o + d * hit.t;
                    let n = if hit.normal.dot(d) > 0.0 { -hit.normal } else { hit.normal };
                    tests.extend([p.extend(hit.object as f32).to_array(), n.extend(0.0).to_array()]);
                    points.push((p, n, hit.object));
                }
            }
            let words = renderer.run_shading_verify("o3dePixelTests", points.len() as u32, &tests, 8 * points.len());
            let (mut worst, mut inner, mut blended) = (0.0f32, 0usize, 0usize);
            for (k, &(p, n, object)) in points.iter().enumerate() {
                let cpu = brdf::pixel(&shading, &content.objects, p, n, eye, object, state.p("exposure"), map, [Vec3::ZERO; 2]);
                let r = cpu.ibl.reflection;
                let masked = r.stencil & super::reflections::STENCIL_REFLECTION_MASK;
                inner += usize::from(masked > 3);
                blended += usize::from(masked == 3 && r.blend_weight > 0.0 && r.blend_weight < 1.0);
                // The IBL flag off: inside an inner volume the target's 0.4 grey reaches the composite.
                if !shading.ibl && masked > 3 && (r.composited - super::reflections::REFLECTION_CLEAR).abs().max_element() < 1e-6 {
                    grey += 1;
                }
                worst = worst.max((f3(&words, 8 * k) - cpu.color).abs().max_element() / cpu.color.max_element().max(0.05));
            }
            ok &= worst < 3e-3 && inner > 4 && blended > 10;
            detail.push(json!({"settings": config.iter().map(|(n, v)| format!("{n}={v}")).collect::<Vec<_>>(), "points": points.len(), "insideInner": inner, "blended": blended, "worstRelativeDifference": worst}));
        }
        report.check(
            "reflection probes: o3dePixel matches brdf::pixel (stencil, blend weights, global, outer, inner, composite; parallax on and off, IBL off; a clear-coated mesh's forward-pass probe)",
            ok,
            json!({"settings": detail, "tolerance": 3e-3}),
        );
        report.check(
            "finding: with the IBL flag off, surfaces inside an inner probe volume get the reflection target's 0.4 clear grey (reported, not required)",
            true,
            json!({"pointsWithGrey": grey}),
        );
    }

    // ---- 6. The scene pass draws with the port ----------------------------
    let mut state = shot::for_demo(Demo::Room);
    for name in ["labels", "vectors", "lobe", "lightOn"] {
        state.set(name, 0.0);
    }
    // With the light off only the image-based lighting remains; then with it on.
    let mut raster_detail = Vec::new();
    let mut raster_ok = true;
    for (light_on, probes) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)] {
        state.set("lightOn", light_on);
        state.set("probes", probes);
        let shading = demos::shading(&state);
        let frame = renderer.render_frame(request(&state, &still), None);
        let content = frame.content.context("scene content")?;
        let rgba = renderer.read_frame();
        // ModulateWithSsao: the GPU's upsampled AO, which section 6c checks against the CPU mirror.
        let (ao_words, ao_size) = renderer.read_chain_buffer("ssaoFull").context("ssao")?;
        let vp = renderer.frame_layout().viewport_px();
        let projector = Projector::new(&content.uniforms.camera);
        let eye = Vec3::from(content.uniforms.camera.position);
        let light_px = projector.project(shading.light.position).map(|(p, _)| p);
        let mut worst = 0u8;
        let mut compared = 0;
        for j in 0..24 {
            for i in 0..32 {
                let pixel = Vec2::new(((i as f32 + 0.5) / 32.0 * vp[2] as f32).floor() + 0.5, ((j as f32 + 0.5) / 24.0 * vp[3] as f32).floor() + 0.5);
                if light_px.is_some_and(|l| l.distance(pixel) < 40.0) {
                    continue;
                }
                // Flat surfaces only (the sphere and teapot are tessellated), away from edges.
                let hit_at = |px: Vec2| {
                    let (o, d) = projector.ray(px);
                    (o, d, scene::trace_scene(&content.objects, o, d, 100.0, ids::PANEL as u32))
                };
                let (o, d, hit) = hit_at(pixel);
                if !hit.is_hit() || ![SHAPE_PLANE, SHAPE_BOX].contains(&content.objects[hit.object as usize].shape_kind) {
                    continue;
                }
                if [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y].iter().any(|step| hit_at(pixel + *step * 3.0).2.object != hit.object) {
                    continue;
                }
                let p = o + d * hit.t;
                let n = if hit.normal.dot(d) > 0.0 { -hit.normal } else { hit.normal };
                // Flat surfaces: the normal does not change from pixel to pixel.
                let px = brdf::pixel(&shading, &content.objects, p, n, eye, hit.object, state.p("exposure"), map, [Vec3::ZERO; 2]);
                let ao = f32::from_bits(ao_words[(pixel.y as u32 * ao_size[0] + pixel.x as u32) as usize]);
                let diffuse = px.diffuse() + px.ibl.diffuse * if shading.ibl { 1.0 } else { 0.0 };
                let expected = scene::display_rgb(px.color - diffuse + diffuse * ao, state.p("exposure"));
                let (x, y) = (vp[0] + pixel.x as u32, vp[1] + pixel.y as u32);
                let at = ((y * renderer.width + x) * 4) as usize;
                let diff = (0..3).map(|c| rgba[at + c].abs_diff(expected[c])).max().unwrap_or(0);
                worst = worst.max(diff);
                compared += 1;
            }
        }
        raster_ok &= worst <= 3 && compared > 100;
        raster_detail.push(json!({"lightOn": light_on, "probes": probes, "pixels": compared, "worstLevels": worst}));
    }
    report.check("the scene pass's pixels on flat surfaces match the CPU shading (within 3 of 255, its diffuse times the GPU's SSAO), without and with reflection probes", raster_ok, json!(raster_detail));

    // ---- 6c. SSAO: each GPU stage against the CPU mirror of it ------------
    {
        use super::ssao::{self, Image};
        let mut detail = Vec::new();
        let mut ssao_ok = true;
        for blur in [1.0, 0.0] {
            let mut state = shot::for_demo(Demo::Room);
            state.set("labels", 0.0);
            state.set("ssaoBlur", blur);
            let frame = renderer.render_frame(request(&state, &still), None);
            let content = frame.content.context("scene content")?;
            let settings = ssao::Settings::from_records(&demos::ssao_records(&state));
            let projection = glam::Mat4::from_cols_array_2d(&content.uniforms.camera.view_to_clip);
            let tan = Vec2::new(1.0 / projection.x_axis.x, 1.0 / projection.y_axis.y);
            let read = |name: &str, half: bool| -> Result<Image> {
                let (words, size) = renderer.read_chain_buffer(name).context("ssao buffer")?;
                let size = if half { [size[0].div_ceil(2), size[1].div_ceil(2)] } else { size };
                Ok(Image { size, data: words[..(size[0] * size[1]) as usize].iter().map(|w| f32::from_bits(*w)).collect() })
            };
            let (linear, half, raw, blur_v, blur_h, full) = (read("linearDepth", false)?, read("halfDepth", true)?, read("ssao", true)?, read("ssaoBlurV", true)?, read("ssaoBlur", true)?, read("ssaoFull", false)?);
            // Each stage from the GPU's previous one, so differences do not pile up.
            let worst = |a: &Image, b: &Image| a.data.iter().zip(&b.data).map(|(x, y)| (x - y).abs()).fold(0.0f32, f32::max);
            let off = |a: &Image, b: &Image| a.data.iter().zip(&b.data).filter(|(x, y)| (*x - *y).abs() > 1.5 / 255.0).count();
            let stages = [
                ("DepthDownsample", worst(&half, &ssao::downsample(&linear)), 0usize),
                ("SsaoCompute", worst(&raw, &ssao::compute(&half, &settings, tan)), off(&raw, &ssao::compute(&half, &settings, tan))),
                ("blur, vertical", worst(&blur_v, &ssao::blur(&raw, &half, &settings, false)), off(&blur_v, &ssao::blur(&raw, &half, &settings, false))),
                ("blur, horizontal", worst(&blur_h, &ssao::blur(&blur_v, &half, &settings, true)), off(&blur_h, &ssao::blur(&blur_v, &half, &settings, true))),
                ("DepthUpsample", worst(&full, &ssao::upsample(&blur_h, &half, &linear)), off(&full, &ssao::upsample(&blur_h, &half, &linear))),
            ];
            // Off by one 8-bit level where float rounding lands on the other side of a step; anything more in at most 0.1% of pixels.
            let stage_ok = stages.iter().all(|(_, w, n)| *w <= 1.5 / 255.0 || *n * 1000 <= full.data.len());
            // The readout's SSAO, from ray-cast depths around the pixel, against the GPU's on flat surfaces.
            let projector = Projector::new(&content.uniforms.camera);
            let view_m = glam::Mat4::from_cols_array_2d(&content.uniforms.camera.world_to_view);
            let linear_depth = |x: i32, y: i32| -> f32 {
                let (o, d) = projector.ray(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                let hit = scene::trace_scene(&content.objects, o, d, 100.0, ids::PANEL as u32);
                if hit.is_hit() { -view_m.transform_point3(o + d * hit.t).z } else { content.uniforms.camera.far_plane }
            };
            let (mut probed, mut close) = (0usize, 0usize);
            for j in 0..4 {
                for i in 0..6 {
                    let (x, y) = ((full.size[0] as i32) * (2 * i + 1) / 12, (full.size[1] as i32) * (2 * j + 1) / 8);
                    let k = ssao::at_pixel(&linear_depth, full.size, &settings, tan, x, y);
                    probed += 1;
                    close += usize::from((k.upsampled - full.at(x, y)).abs() <= 2.5 / 255.0);
                }
            }
            ssao_ok &= close * 5 >= probed * 4;
            let darkened = full.data.iter().filter(|v| **v < 0.9).count();
            ssao_ok &= stage_ok && darkened > 100;
            detail.push(json!({"blur": blur, "stages": stages.iter().map(|(name, w, n)| json!({"stage": name, "worstDifference": w, "pixelsOffByMoreThanOneLevel": n})).collect::<Vec<_>>(), "pixelsDarkenedBelow0.9": darkened, "readoutPixels": probed, "readoutWithin2Levels": close}));
        }
        report.check("SSAO: the GPU's downsample, SsaoCompute, both blurs, and the upsample match the CPU mirror stage by stage (within one 8-bit level), with the blur on and off; the readout's ray-cast SSAO agrees within two levels at 80% of 24 pixels", ssao_ok, json!(detail));
    }

    // ---- 6b. Tiled light culling: the GPU's lists against the CPU mirror ---
    {
        use super::culling::{self, TileLightData};
        let mut state = shot::for_demo(Demo::Room);
        state.set("extraLights", 24.0);
        state.set("decals", 1.0);
        state.set("sphereLight", 1.0);
        let frame = renderer.render_frame(request(&state, &still), None);
        let content = frame.content.context("scene content")?;
        let shading = demos::shading(&state);
        let lights = shading.lights();
        let camera = content.uniforms.camera;
        let (tile_words, size) = renderer.read_chain_buffer("tileData").context("tile data")?;
        let (list_words, _) = renderer.read_chain_buffer("lightList").context("light list")?;
        let (remap_words, _) = renderer.read_chain_buffer("lightListRemapped").context("remapped list")?;
        let grid = [size[0].div_ceil(culling::TILE_DIM), size[1].div_ceil(culling::TILE_DIM)];
        let (mut tiles, mut list_mismatch, mut remap_mismatch, mut listed) = (0usize, 0usize, 0usize, 0usize);
        for ty in 0..grid[1] {
            for tx in 0..grid[0] {
                let t = (ty * grid[0] + tx) as usize;
                let data = TileLightData::unpack(&tile_words[4 * t..4 * t + 4]);
                let cpu = culling::cull_tile(&camera, &data, [tx, ty], &lights, &shading.decals, &shading.sphere_lights);
                let gpu = &list_words[t * 256..t * 256 + cpu.len()];
                tiles += 1;
                listed += cpu.len() - culling::LIGHT_GROUPS as usize;
                list_mismatch += usize::from(gpu != cpu.as_slice());
                // The remap from the GPU's own list is integer work: it must match exactly.
                let base = t * 256 * 8;
                for (bin, entries) in culling::remap(gpu).iter().enumerate() {
                    remap_mismatch += usize::from(&remap_words[base + bin * 256..base + bin * 256 + entries.len()] != entries.as_slice());
                }
            }
        }
        // Every light that reaches a surface point is in that pixel's bin.
        let projector = Projector::new(&camera);
        let view = glam::Mat4::from_cols_array_2d(&camera.world_to_view);
        let (mut points, mut missing) = (0usize, 0usize);
        for y in (4..size[1]).step_by(8) {
            for x in (4..size[0]).step_by(8) {
                let (o, d) = projector.ray(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                let hit = scene::trace_scene(&content.objects, o, d, 100.0, ids::PANEL as u32);
                if !hit.is_hit() {
                    continue;
                }
                let p = o + d * hit.t;
                let t = ((y / culling::TILE_DIM) * grid[0] + x / culling::TILE_DIM) as usize;
                let data = TileLightData::unpack(&tile_words[4 * t..4 * t + 4]);
                let distance = -view.transform_point3(p).z;
                // The raster and the analytic shapes differ by millimetres; skip points that close to a bin edge.
                let span = (data.z_far - data.z_near) / (1u32 << data.log_max_bins) as f32;
                let f = (distance - data.z_near) / span;
                if (f - f.round()).abs() * span < 0.005 {
                    continue;
                }
                let base = (t * 8 + data.bin(distance) as usize) * 256;
                let bin: Vec<u32> = remap_words[base..base + 256].iter().copied().take_while(|&v| v != culling::END_OF_LIST).collect();
                let group = bin.split(|&v| v == culling::END_OF_GROUP).nth(culling::GROUP_SIMPLE_POINT as usize).unwrap_or(&[]);
                points += 1;
                let decal_group = bin.split(|&v| v == culling::END_OF_GROUP).next().unwrap_or(&[]);
                for (i, decal) in shading.decals.iter().enumerate() {
                    if super::decals::sample(decal, p, hit.normal).is_some() && !decal_group.contains(&(i as u32)) {
                        missing += 1;
                    }
                }
                for (i, light) in lights.iter().enumerate() {
                    let reaches = (light.position - p).length_squared() * light.inv_attenuation_radius_squared < 1.0;
                    if reaches && !group.contains(&(i as u32)) {
                        missing += 1;
                    }
                }
            }
        }
        let ok = tiles > 0 && list_mismatch * 200 <= tiles && remap_mismatch == 0 && missing == 0 && points > 100 && listed > 0;
        report.check(
            "light culling: the GPU's tile lists match the CPU mirror (all but at most 0.5% of tiles, float rounding), the remap exactly, and every light that reaches a surface point (and every decal whose box holds it) is in its pixel's bin; with a sphere light",
            ok,
            json!({"tiles": tiles, "lightsListed": listed, "listMismatchTiles": list_mismatch, "remapMismatchBins": remap_mismatch, "points": points, "lightsMissing": missing, "lights": lights.len()}),
        );
    }

    // ---- 7. Every demonstration renders, with readable text ---------------
    let mut demo_detail = Vec::new();
    let mut demos_ok = true;
    for demo in Demo::ALL {
        let mut state = shot::for_demo(demo);
        let result = renderer.render_frame(request(&state, &still), None);
        let values = result.content.as_ref().and_then(|c| c.readout.as_ref()).map_or(0, |r| r.rows.len());
        // Its formula above the readout, at full size, with every row below it.
        let formula = result.content.as_ref().and_then(|c| c.readout.as_ref()?.formula);
        let formula_ok = formula == Some(demo.key()) && result.report.formula_scale == Some(1.0) && result.report.readout_overflow == 0;
        let ok = result.report.min_text_pt >= MIN_TEXT_PT && values > 0 && formula_ok;
        let png = options.work_dir.join(format!("o3de-{}.png", demo.key()));
        crate::lab::record::write_png(&png, renderer.width, renderer.height, &renderer.read_frame())?;
        // The Code tab's panel.
        state.panel = crate::lab::shot::PanelMode::Code;
        state.code = Some(demo.listing().to_string());
        let code = renderer.render_frame(request(&state, &still), None);
        let listing_ok = code.report.panel_code.as_deref() == Some(demo.listing()) && code.report.code_overflow == 0;
        demos_ok &= ok && listing_ok;
        demo_detail.push(json!({"demo": demo.key(), "minTextPt": result.report.min_text_pt, "readoutRows": values, "formula": formula, "formulaScale": result.report.formula_scale, "rowsBelowItCut": result.report.readout_overflow, "listing": demo.listing(), "listingFits": listing_ok, "snapshot": png}));
    }
    report.check("every demonstration renders its formula at full size, its readout, and its listing with text of at least 24 px", demos_ok, json!(demo_detail));

    // ---- 7a. Numbers the narration states, measured at the demonstrations' points ----
    {
        let row = |demo: Demo, prefix: &str| -> Option<f32> {
            let content = demos::build_frame(&shot::for_demo(demo), &renderer.statics, [1280.0, 720.0], 1.0, 0.0);
            let (_, value, _) = content.readout?.rows.into_iter().find(|(name, _, _)| name.starts_with(prefix))?;
            value.trim_start_matches('(').split([',', ' ']).find(|v| !v.is_empty())?.parse().ok()
        };
        // "At roughness zero point eight and this view angle, that is only about fifty seven percent."
        let energy = row(Demo::Multiscatter, "A + B");
        // "Six candelas at just under two metres give about one point seven lux, before the window."
        let distance = row(Demo::PointLight, tr("common.distance_d"));
        let lux = row(Demo::PointLight, "I / max");
        let ok = energy.is_some_and(|e| (0.565..0.575).contains(&e)) && distance.is_some_and(|d| (1.85..2.0).contains(&d)) && lux.is_some_and(|l| (1.65..1.75).contains(&l));
        report.check("the narration's measured numbers: A + B about 57% at the multiscatter point; about 1.7 lux from 6 cd just under 2 m", ok, json!({"singleScatteringEnergy": energy, "distanceMetres": distance, "luxBeforeWindow": lux}));
    }

    // ---- 7b. The formula images: what formulas.py made, each term lit ----
    {
        let manifest: Value = serde_json::from_str(include_str!("formulas/manifest.json"))?;
        let mut made: Vec<String> = manifest["formulas"].as_object().into_iter().flatten().flat_map(|(_, f)| f["variants"].as_object().into_iter().flatten().map(|(k, _)| k.clone())).collect();
        let mut embedded: Vec<String> = super::formulas::IMAGES.iter().map(|(k, _)| k.to_string()).collect();
        made.sort();
        embedded.sort();
        let mut images_ok = made == embedded;
        let mut detail = Vec::new();
        for (key, bytes) in super::formulas::IMAGES {
            let image = crate::lab::record::decode_png(std::io::Cursor::new(*bytes));
            let size = image.as_ref().map(|i| i.size).unwrap_or_default();
            let recorded = manifest["formulas"].as_object().into_iter().flatten().find_map(|(_, f)| f["variants"].get(*key).cloned()).unwrap_or_default();
            // Within the readout panel's inner width, and at least 24 px glyphs.
            let ok = image.is_ok() && size[0] <= 546 && recorded["width"] == json!(size[0]) && recorded["height"] == json!(size[1]) && recorded["minFontPx"].as_f64().is_some_and(|px| px >= MIN_TEXT_PT as f64);
            images_ok &= ok;
            detail.push(json!({"key": key, "size": size, "minFontPx": recorded["minFontPx"], "ok": ok}));
        }
        for demo in Demo::ALL {
            let terms = super::formulas::terms(demo);
            images_ok &= terms > 0 && (1..=terms).all(|n| super::formulas::key(demo, n as f32).is_some_and(|k| k == format!("{}-{n}", demo.key()))) && super::formulas::key(demo, -1.0).is_none();
        }
        report.check("formula images: one per entry of the manifest, decoded, inside the panel, glyphs >= 24 px; every term of each demonstration's formula has its image", images_ok && made == embedded, json!({"images": detail, "manifestMatchesEmbedded": made == embedded}));
    }

    // ---- 8. Listings and the source index ---------------------------------
    let index = super::sources::index();
    let listings = super::pseudocode::all();
    let listing_problems: Vec<String> = listings
        .iter()
        .filter(|l| !index.entries.contains_key(&l.source) || l.demo.as_deref().is_none_or(|d| Demo::ALL.iter().all(|x| x.key() != d)))
        .map(|l| l.name.to_string())
        .collect();
    let demo_problems: Vec<&str> = Demo::ALL.iter().filter(|d| !listings.iter().any(|l| l.name == d.listing()) || !index.demos.contains_key(d.key())).map(|d| d.key()).collect();
    report.check("every listing cites a source entry and a demonstration; every demonstration has a listing and sources", listing_problems.is_empty() && demo_problems.is_empty(), json!({"listings": listing_problems, "demos": demo_problems}));
    report.check("source index", true, json!({"engine": index.engine.name, "version": index.engine.version, "commit": index.engine.commit, "entries": index.entries.len()}));
    let checkout = std::env::var_os("O3DE_SOURCE_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../o3de"));
    if checkout.join("engine.json").is_file() {
        let index_json: Value = serde_json::from_str(super::sources::INDEX_JSON)?;
        let mut mismatches = Vec::new();
        let mut cited = 0;
        for (key, entry) in index_json["entries"].as_object().into_iter().flatten() {
            let path = entry["path"].as_str().unwrap_or_default();
            let text = std::fs::read_to_string(checkout.join(path)).unwrap_or_default();
            let lines: Vec<&str> = text.lines().collect();
            let mut lines_cited = vec![(entry["line"].as_u64().unwrap_or(0), entry["lineHash"].as_str().unwrap_or_default().to_string())];
            for d in entry["details"].as_array().into_iter().flatten() {
                lines_cited.push((d["line"].as_u64().unwrap_or(0), d["lineHash"].as_str().unwrap_or_default().to_string()));
            }
            for (line, hash) in lines_cited {
                cited += 1;
                let actual = lines.get(line.saturating_sub(1) as usize).map(|l| Sha256::digest(l.trim().as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect::<String>());
                if actual.as_deref() != Some(hash.as_str()) {
                    mismatches.push(format!("{key}: {path}:{line}"));
                }
            }
        }
        report.check("every cited O3DE line is unchanged in the checkout", mismatches.is_empty(), json!({"checkout": checkout, "linesCited": cited, "mismatches": mismatches}));
    } else {
        report.check("every cited O3DE line is unchanged in the checkout (skipped: no checkout)", true, json!({"checkout": checkout}));
    }

    // ---- 9. The narrated timeline: every scene beat readable ---------------
    if let Some(path) = &options.timeline {
        let timeline = Timeline::load(path)?;
        let mut problems = Vec::new();
        let mut beats = 0;
        for beat in timeline.beats.iter().filter(|b| b.kind == BeatKind::Scene) {
            let frame = beat.start_frame + beat.frames / 2;
            let state = timeline.state_at(frame);
            let mut req = request(&state, &timeline);
            req.chrome = Chrome::Recording { timeline: &timeline, frame };
            req.chapter_number = beat.chapter;
            let r = renderer.render_frame(req, None).report;
            beats += 1;
            if r.min_text_pt < MIN_TEXT_PT || r.label_overlaps > 0 || r.labels_outside > 0 || r.code_overflow > 0 || r.readout_overflow > 0 || r.formula_scale.is_some_and(|s| s < 1.0) {
                problems.push(json!({"beat": beat.id, "frame": frame, "minTextPt": r.min_text_pt, "labelOverlaps": r.label_overlaps, "labelsOutside": r.labels_outside, "codeOverflow": r.code_overflow, "readoutOverflow": r.readout_overflow, "formulaScale": r.formula_scale}));
            }
        }
        report.check("every scene beat of the timeline: text >= 24 px, labels inside and apart, listings, formulas, and readouts fit", problems.is_empty() && beats > 0, json!({"timeline": path, "sceneBeats": beats, "problems": problems}));
    }

    let passed = report.checks.iter().all(|c| c["passed"] == true);
    std::fs::write(&options.report, serde_json::to_string_pretty(&json!({"investigation": "o3de/lighting", "passed": passed, "gpu": gpu_info, "checks": report.checks}))?)?;
    Ok(passed)
}
