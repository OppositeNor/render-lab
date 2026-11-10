//! Section 5: whole pixels of o3dePixel against brdf::pixel (clear coat, decals, transmission, reflection probes), and the sun's shadow maps.

use super::*;

/// Section 5: whole pixels, o3dePixel on the GPU against brdf::pixel, in every view.
pub(super) fn check_pixels(
    report: &mut Report,
    renderer: &mut Renderer,
    still: &Timeline,
) -> Result<()> {
    let map = BrdfMap::get();
    // ---- 5. Whole pixels: o3dePixel on the GPU against brdf::pixel ---------
    let mut pixel_detail = Vec::new();
    let mut pixels_ok = true;
    for view in 0..brdf::ViewMode::choices().options.len() {
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
        let frame = renderer.render_frame(request(&state, still), None);
        let content = frame.content.context("scene content")?;
        let shading = demos::shading(&state);
        let eye = Vec3::from(content.uniforms.camera.position);
        let projector = Projector::new(&content.uniforms.camera);
        let mut tests = Vec::new();
        let mut points = Vec::new();
        for j in 0..12 {
            for i in 0..16 {
                // Each ray through a grid cell's centre finds the surface the kernel shades there.
                let pixel = Vec2::new((i as f32 + 0.5) / 16.0, (j as f32 + 0.5) / 12.0)
                    * projector.viewport;
                let (o, d) = projector.ray(pixel);
                let hit = scene::trace_scene(&content.objects, o, d, 100.0, ids::PANEL as u32);
                if !hit.is_hit() {
                    continue;
                }
                let p = o + d * hit.t;
                // A back face's normal is flipped to face the ray.
                let n = if hit.normal.dot(d) > 0.0 {
                    -hit.normal
                } else {
                    hit.normal
                };
                tests.extend([
                    p.extend(hit.object as f32).to_array(),
                    n.extend(0.0).to_array(),
                ]);
                points.push((p, n, hit.object));
            }
        }
        let words = renderer.run_shading_verify(
            "o3dePixelTests",
            points.len() as u32,
            &tests,
            8 * points.len(),
        );
        let mut worst = 0.0f32;
        for (k, &(p, n, object)) in points.iter().enumerate() {
            // The kernel has no screen, so no normal derivatives.
            let cpu = brdf::pixel(
                &shading,
                &content.objects,
                &brdf::Fragment::at(object, p, n),
                eye,
                state.p("exposure"),
                map,
            );
            let gpu = f3(&words, 8 * k);
            // The difference relative to the CPU's brightest channel, floored at 0.05 for dark pixels.
            let d = (gpu - cpu.color).abs().max_element() / cpu.color.max_element().max(0.05);
            worst = worst.max(d);
        }
        pixels_ok &= worst < 3e-3;
        pixel_detail.push(json!({"view": view, "occlusionMaps": occluded, "clearCoat": coated, "points": points.len(), "worstRelativeDifference": worst}));
    }
    report.check("o3dePixel on the GPU matches brdf::pixel in every view (192 rays each; with occlusion maps and clear coat in turn)", pixels_ok, json!({"views": pixel_detail, "tolerance": 3e-3}));
    Ok(())
}

/// Section 5b: clear coat at factor 0 changes no direct-lighting term.
pub(super) fn check_clear_coat(report: &mut Report, rng: &mut Rng) {
    let map = BrdfMap::get();
    // ---- 5b. Clear coat at factor 0 changes no direct-lighting term -------
    let mut worst = 0.0f32;
    for _ in 0..256 {
        let n = rng.unit();
        let (v, l) = (rng.above(n), rng.above(n));
        let base = Vec3::new(rng.next(), rng.next(), rng.next());
        let s = brdf::Surface::new(Vec3::ZERO, n, base, rng.next(), rng.next(), rng.next());
        let mut coated = s;
        coated.clear_coat = Some(brdf::ClearCoat {
            factor: 0.0,
            roughness: rng.next().max(1e-3),
            normal: n,
        });
        coated.specular_f0 = brdf::apply_clear_coat_to_specular_f0(s.specular_f0, 0.0);
        let lit = |surface: &brdf::Surface| {
            let mut ld = brdf::LightingData::new(surface, v, true, map);
            brdf::apply_light(
                surface,
                &mut ld,
                Vec3::ONE,
                l,
                brdf::DiffuseModel::Lambertian,
            );
            (ld.diffuse_lighting, ld.specular_lighting)
        };
        let ((d0, s0), (d1, s1)) = (lit(&s), lit(&coated));
        worst = worst
            .max((d0 - d1).abs().max_element())
            .max((s0 - s1).abs().max_element());
    }
    report.check("clear coat at factor 0 leaves the F0, the diffuse response, and every light's diffuse and specular unchanged", worst <= 1e-6, json!({"surfaces": 256, "worstDifference": worst}));
}

/// Section 5c: decals, o3dePixel on the GPU against brdf::pixel inside them.
pub(super) fn check_decals(
    report: &mut Report,
    renderer: &mut Renderer,
    still: &Timeline,
) -> Result<()> {
    let map = BrdfMap::get();
    // ---- 5c. Decals: o3dePixel on the GPU against brdf::pixel inside them --
    let mut detail = Vec::new();
    let mut ok = true;
    // The demonstration's start, the splat tilted under the grid at partial opacity, and the grid turned with weak attenuation.
    let configs: [&[(&str, f32)]; 3] = [
        &[],
        &[
            ("decalTilt", 40.0),
            ("decalSplatSortKey", 15.0),
            ("decalOpacity", 0.6),
            ("decalNormalOpacity", 0.5),
        ],
        &[
            ("decalTurn", 70.0),
            ("decalAngleAttenuation", 0.3),
            ("decalTilt", -50.0),
        ],
    ];
    for config in configs {
        let mut state = shot::for_demo(Demo::Decals);
        for &(name, value) in config {
            state.set(name, value);
        }
        let frame = renderer.render_frame(request(&state, still), None);
        let content = frame.content.context("scene content")?;
        let shading = demos::shading(&state);
        let eye = Vec3::from(content.uniforms.camera.position);
        let projector = Projector::new(&content.uniforms.camera);
        let (mut tests, mut points) = (Vec::new(), Vec::new());
        for j in 0..24 {
            for i in 0..32 {
                // Each ray through a grid cell's centre finds the surface the kernel shades there.
                let (o, d) = projector.ray(
                    Vec2::new((i as f32 + 0.5) / 32.0, (j as f32 + 0.5) / 24.0)
                        * projector.viewport,
                );
                let hit = scene::trace_scene(&content.objects, o, d, 100.0, ids::PANEL as u32);
                let p = o + d * hit.t;
                if !hit.is_hit()
                    || !shading
                        .decals
                        .iter()
                        .any(|dc| super::super::decals::sample(dc, p, hit.normal).is_some())
                {
                    // Only points inside a decal's box are tested.
                    continue;
                }
                // A back face's normal is flipped to face the ray.
                let n = if hit.normal.dot(d) > 0.0 {
                    -hit.normal
                } else {
                    hit.normal
                };
                tests.extend([
                    p.extend(hit.object as f32).to_array(),
                    n.extend(0.0).to_array(),
                ]);
                points.push((p, n, hit.object));
            }
        }
        let words = renderer.run_shading_verify(
            "o3dePixelTests",
            points.len() as u32,
            &tests,
            8 * points.len(),
        );
        let mut worst = 0.0f32;
        for (k, &(p, n, object)) in points.iter().enumerate() {
            let cpu = brdf::pixel(
                &shading,
                &content.objects,
                &brdf::Fragment::at(object, p, n),
                eye,
                state.p("exposure"),
                map,
            );
            worst = worst.max(
                (f3(&words, 8 * k) - cpu.color).abs().max_element()
                    / cpu.color.max_element().max(0.05),
            );
        }
        ok &= worst < 3e-3 && points.len() > 100;
        detail.push(json!({"settings": config.iter().map(|(n, v)| format!("{n}={v}")).collect::<Vec<_>>(), "pointsInDecals": points.len(), "worstRelativeDifference": worst}));
    }
    let f0_ok = decal_on_metal()?;
    report.check(
        "decals: o3dePixel on the GPU matches brdf::pixel at points inside the decals (three settings), and ApplyDecal blends a metal's albedo by its opacity and leaves its specularF0 (26.05)",
        ok && f0_ok,
        json!({"settings": detail, "tolerance": 3e-3, "metallicF0": f0_ok}),
    );
    Ok(())
}

/// ApplyDecal on a metallic surface: O3DE 26.05 blends the albedo by the decal's opacity and leaves specularF0
/// as it was. Whether it does.
pub(super) fn decal_on_metal() -> Result<bool> {
    let decal = super::super::decals::scene_decals(&shot::for_demo(Demo::Decals))[0];
    let mut metal = brdf::Surface::new(
        decal.position,
        Vec3::Y,
        Vec3::new(0.9, 0.6, 0.3),
        0.75,
        0.5,
        0.4,
    );
    let before = metal;
    super::super::decals::apply_decal(&decal, &mut metal, Vec3::Y);
    let k = super::super::decals::sample(&decal, decal.position, Vec3::Y)
        .context("the splat's centre")?;
    let f0_ok = (metal.specular_f0 - before.specular_f0).abs().max_element() < 1e-6
        && (metal.albedo - before.albedo.lerp(k.blended, k.albedo_opacity))
            .abs()
            .max_element()
            < 1e-6
        && k.albedo_opacity > 0.9;
    Ok(f0_ok)
}

/// Section 5d: transmission and the sphere light, o3dePixel against brdf::pixel.
pub(super) fn check_transmission(
    report: &mut Report,
    renderer: &mut Renderer,
    still: &Timeline,
) -> Result<()> {
    let map = BrdfMap::get();
    // ---- 5d. Transmission and the sphere light: o3dePixel against brdf::pixel --
    let mut detail = Vec::new();
    let mut ok = true;
    let configs: [&[(&str, f32)]; 3] = [
        &[],
        &[
            ("transmissionPower", 2.0),
            ("transmissionDistortion", 0.5),
            ("transmissionThickness", 0.2),
            ("sphereLightBulb", 0.3),
        ],
        &[("teapotTransmission", 2.0)],
    ];
    for config in configs {
        let mut state = shot::for_demo(Demo::Transmission);
        for &(name, value) in config {
            state.set(name, value);
        }
        let frame = renderer.render_frame(request(&state, still), None);
        let content = frame.content.context("scene content")?;
        let shading = demos::shading(&state);
        let eye = Vec3::from(content.uniforms.camera.position);
        let projector = Projector::new(&content.uniforms.camera);
        let (mut tests, mut points) = (Vec::new(), Vec::new());
        let mut lit = 0usize;
        for j in 0..24 {
            for i in 0..32 {
                // Each ray through a grid cell's centre finds the surface the kernel shades there.
                let (o, d) = projector.ray(
                    Vec2::new((i as f32 + 0.5) / 32.0, (j as f32 + 0.5) / 24.0)
                        * projector.viewport,
                );
                let hit = scene::trace_scene(&content.objects, o, d, 100.0, ids::PANEL as u32);
                if !hit.is_hit() {
                    continue;
                }
                let p = o + d * hit.t;
                // A back face's normal is flipped to face the ray.
                let n = if hit.normal.dot(d) > 0.0 {
                    -hit.normal
                } else {
                    hit.normal
                };
                tests.extend([
                    p.extend(hit.object as f32).to_array(),
                    n.extend(0.0).to_array(),
                ]);
                points.push((p, n, hit.object));
            }
        }
        let words = renderer.run_shading_verify(
            "o3dePixelTests",
            points.len() as u32,
            &tests,
            8 * points.len(),
        );
        let mut worst = 0.0f32;
        for (k, &(p, n, object)) in points.iter().enumerate() {
            let cpu = brdf::pixel(
                &shading,
                &content.objects,
                &brdf::Fragment::at(object, p, n),
                eye,
                state.p("exposure"),
                map,
            );
            // Back-lit points are counted, so the test is known to reach the transmission.
            lit += usize::from(cpu.lighting.translucent_back_lighting.max_element() > 1e-3);
            worst = worst.max(
                (f3(&words, 8 * k) - cpu.color).abs().max_element()
                    / cpu.color.max_element().max(0.05),
            );
        }
        ok &= worst < 3e-3 && lit > 20;
        detail.push(json!({"settings": config.iter().map(|(n, v)| format!("{n}={v}")).collect::<Vec<_>>(), "points": points.len(), "backLit": lit, "worstRelativeDifference": worst}));
    }
    report.check(
        "transmission: o3dePixel with the sphere light matches brdf::pixel, thick and thin, with back lighting on the teapot",
        ok,
        json!({"settings": detail, "tolerance": 3e-3}),
    );
    Ok(())
}

/// The probes' capture camera.
pub(super) fn check_probe_camera(report: &mut Report) {
    // ---- 5d. The probes' capture camera ------------------------------------
    let probes = super::super::reflections::bake_check(4096);
    let ok = probes
        .iter()
        .all(|&(share, median)| share > 0.9 && median < 0.01);
    report.check(
        "probe captures: each cubemap, read along 4096 directions, shows what the room traced from the probe's position shows along them (90% within 5%)",
        ok,
        json!(probes.iter().map(|&(share, median)| json!({"withinFivePercent": share, "medianRelativeDifference": median})).collect::<Vec<_>>()),
    );
}

/// Section 5e: reflection probes, o3dePixel against brdf::pixel.
pub(super) fn check_probes(
    report: &mut Report,
    renderer: &mut Renderer,
    still: &Timeline,
) -> Result<()> {
    let map = BrdfMap::get();
    // ---- 5e. Reflection probes: o3dePixel against brdf::pixel -------------
    let mut detail = Vec::new();
    let mut ok = true;
    let mut grey = 0usize;
    // The last with a clear-coated sphere, whose forward-pass IBL blends in its own probe.
    let configs: [&[(&str, f32)]; 5] = [
        &[],
        &[("probeParallax", 0.0)],
        &[("probeInnerScale", 0.5), ("probeExposure", 1.0)],
        &[("ibl", 0.0)],
        &[("sphereClearCoat", 1.0), ("probeExposure", 0.5)],
    ];
    for config in configs {
        let mut state = shot::for_demo(Demo::Reflections);
        for &(name, value) in config {
            state.set(name, value);
        }
        let frame = renderer.render_frame(request(&state, still), None);
        let content = frame.content.context("scene content")?;
        let shading = demos::shading(&state);
        let eye = Vec3::from(content.uniforms.camera.position);
        let projector = Projector::new(&content.uniforms.camera);
        let (mut tests, mut points) = (Vec::new(), Vec::new());
        for j in 0..36 {
            for i in 0..48 {
                // Each ray through a grid cell's centre finds the surface the kernel shades there.
                let (o, d) = projector.ray(
                    Vec2::new((i as f32 + 0.5) / 48.0, (j as f32 + 0.5) / 36.0)
                        * projector.viewport,
                );
                let hit = scene::trace_scene(&content.objects, o, d, 100.0, ids::PANEL as u32);
                if !hit.is_hit() {
                    continue;
                }
                let p = o + d * hit.t;
                // A back face's normal is flipped to face the ray.
                let n = if hit.normal.dot(d) > 0.0 {
                    -hit.normal
                } else {
                    hit.normal
                };
                tests.extend([
                    p.extend(hit.object as f32).to_array(),
                    n.extend(0.0).to_array(),
                ]);
                points.push((p, n, hit.object));
            }
        }
        let words = renderer.run_shading_verify(
            "o3dePixelTests",
            points.len() as u32,
            &tests,
            8 * points.len(),
        );
        let (mut worst, mut inner, mut blended) = (0.0f32, 0usize, 0usize);
        for (k, &(p, n, object)) in points.iter().enumerate() {
            let cpu = brdf::pixel(
                &shading,
                &content.objects,
                &brdf::Fragment::at(object, p, n),
                eye,
                state.p("exposure"),
                map,
            );
            let r = cpu.ibl.reflection;
            let masked = r.stencil & super::super::reflections::STENCIL_REFLECTION_MASK;
            // Points inside an inner box, and points blending between boxes, are counted: both must be tested.
            inner += usize::from(masked > 3);
            blended += usize::from(masked == 3 && r.blend_weight > 0.0 && r.blend_weight < 1.0);
            // The IBL flag off: inside an inner volume the target's 0.4 grey reaches the composite.
            if !shading.ibl
                && masked > 3
                && (r.composited - super::super::reflections::REFLECTION_CLEAR)
                    .abs()
                    .max_element()
                    < 1e-6
            {
                grey += 1;
            }
            worst = worst.max(
                (f3(&words, 8 * k) - cpu.color).abs().max_element()
                    / cpu.color.max_element().max(0.05),
            );
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
    Ok(())
}

/// The sun's shadow maps follow the camera that renders.
pub(super) fn check_sun_shadows(report: &mut Report, renderer: &mut Renderer, still: &Timeline) {
    // ---- 5d. The sun's shadow maps follow the camera ------------------------
    // The cascades are fitted to the camera that renders, so moving it moves
    // them: a frame drawn just after another camera's (its maps last drawn
    // for that one) must match one whose maps were drawn for its own. A frame
    // with a shading value changed (specularAA, which the maps' redraw key
    // has always held) makes sure the maps are drawn for the camera before it.
    // The room with the sun on, as the story's sun beats show it.
    let mut here = shot::for_demo(Demo::Room);
    for (name, value) in [
        ("labels", 0.0),
        ("sunLux", 40.0),
        ("exposure", 0.18),
        ("sunYaw", 40.0),
        ("sunPitch", 52.0),
    ] {
        here.set(name, value);
    }
    let mut there = here.clone();
    there.camera.yaw -= 70.0;
    let frame_after = |renderer: &mut Renderer, before: &lab::shot::ShotState| {
        // A changed shading value redraws the maps for `before`; then the frame here is drawn after `before`'s.
        let mut nudged = before.clone();
        nudged.set("specularAA", 1.0 - before.p("specularAA"));
        renderer.render_frame(request(&nudged, still), None);
        renderer.render_frame(request(before, still), None);
        renderer.render_frame(request(&here, still), None);
        renderer.read_frame()
    };
    let settled = frame_after(renderer, &here);
    let after_move = frame_after(renderer, &there);
    let pixels = settled.len() / 4;
    // Pixels whose colour differs by more than 8 of 255 in any channel.
    let differing = (0..pixels)
        .filter(|&i| (0..3).any(|c| settled[4 * i + c].abs_diff(after_move[4 * i + c]) > 8))
        .count();
    let share = differing as f32 / pixels as f32;
    report.check("the sun's shadow maps follow the camera: a frame just after another camera's matches one after its own (pixels off by more than 8 of 255 under 0.5%)", share < 0.005, json!({"pixels": pixels, "differing": differing, "share": share, "yawMoved": -70.0}));
}
