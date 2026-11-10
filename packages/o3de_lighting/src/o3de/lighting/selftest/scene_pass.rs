//! Section 6: the scene pass on the GPU against the CPU shading, SSAO stage by stage, and tiled light culling.

use super::*;

/// Section 6: the scene pass draws with the port.
pub(super) fn check_scene_pass(
    report: &mut Report,
    renderer: &mut Renderer,
    still: &Timeline,
) -> Result<()> {
    let map = BrdfMap::get();
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
        let frame = renderer.render_frame(request(&state, still), None);
        let content = frame.content.context("scene content")?;
        let rgba = renderer.read_frame();
        // ModulateWithSsao: the GPU's upsampled AO, which section 6c checks against the CPU mirror.
        let (ao_words, ao_size) = renderer.read_chain_buffer("ssaoFull").context("ssao")?;
        let layout = renderer.frame_layout();
        let vp = layout.viewport_px();
        // A recording draws its tag, watermark, and progress over the viewport: compare only where they are not.
        let safe = layout.safe();
        let projector = Projector::new(&content.uniforms.camera);
        let eye = Vec3::from(content.uniforms.camera.position);
        let light_px = projector.project(shading.light.position).map(|(p, _)| p);
        let mut worst = 0u8;
        let mut compared = 0;
        for j in 0..24 {
            for i in 0..32 {
                // A grid cell's centre pixel is compared, unless it is near the light, under an overlay, or not flat.
                let pixel = Vec2::new(
                    ((i as f32 + 0.5) / 32.0 * vp[2] as f32).floor() + 0.5,
                    ((j as f32 + 0.5) / 24.0 * vp[3] as f32).floor() + 0.5,
                );
                if light_px.is_some_and(|l| l.distance(pixel) < 40.0) {
                    continue;
                }
                let (sx, sy) = (
                    (vp[0] as f32 + pixel.x) / layout.scale,
                    (vp[1] as f32 + pixel.y) / layout.scale,
                );
                if sx < safe.min.x || sx > safe.max.x || sy < safe.min.y || sy > safe.max.y {
                    continue;
                }
                let Some((object, p, n)) = flat_hit(&projector, &content.objects, pixel) else {
                    continue;
                };
                // Flat surfaces: the normal does not change from pixel to pixel.
                let px = brdf::pixel(
                    &shading,
                    &content.objects,
                    &brdf::Fragment::at(object, p, n),
                    eye,
                    state.p("exposure"),
                    map,
                );
                let ao = f32::from_bits(
                    ao_words[(pixel.y as u32 * ao_size[0] + pixel.x as u32) as usize],
                );
                let diffuse = px.diffuse()
                    + px.ibl.diffuse
                        * if shading.ibl {
                            1.0
                        } else {
                            0.0
                        };
                // The expected colour is the CPU's, its diffuse light darkened by the GPU's AO.
                let expected =
                    scene::display_rgb(px.color - diffuse + diffuse * ao, state.p("exposure"));
                let (x, y) = (vp[0] + pixel.x as u32, vp[1] + pixel.y as u32);
                let at = ((y * renderer.width + x) * 4) as usize;
                let diff = (0..3)
                    .map(|c| rgba[at + c].abs_diff(expected[c]))
                    .max()
                    .unwrap_or(0);
                worst = worst.max(diff);
                compared += 1;
            }
        }
        raster_ok &= worst <= 3 && compared > 100;
        raster_detail.push(json!({"lightOn": light_on, "probes": probes, "pixels": compared, "worstLevels": worst}));
    }
    report.check("the scene pass's pixels on flat surfaces match the CPU shading (within 3 of 255, its diffuse times the GPU's SSAO), without and with reflection probes", raster_ok, json!(raster_detail));
    Ok(())
}

/// The scene's flat surface (a plane or a box: the sphere and teapot are tessellated) seen at `pixel`, away from
/// its edges: the object, the point, and its normal facing the camera.
pub(super) fn flat_hit(
    projector: &Projector,
    objects: &[layout::ObjectData],
    pixel: Vec2,
) -> Option<(u32, Vec3, Vec3)> {
    // Flat surfaces only (the sphere and teapot are tessellated), away from edges.
    let hit_at = |px: Vec2| {
        let (o, d) = projector.ray(px);
        (
            o,
            d,
            scene::trace_scene(objects, o, d, 100.0, ids::PANEL as u32),
        )
    };
    let (o, d, hit) = hit_at(pixel);
    if !hit.is_hit() || ![SHAPE_PLANE, SHAPE_BOX].contains(&objects[hit.object as usize].shape_kind)
    {
        return None;
    }
    if [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y]
        .iter()
        .any(|step| hit_at(pixel + *step * 3.0).2.object != hit.object)
    {
        // A neighbour 3 px away sees another object, so the pixel is too near an edge.
        return None;
    }
    let p = o + d * hit.t;
    let n = if hit.normal.dot(d) > 0.0 {
        -hit.normal
    } else {
        hit.normal
    };
    Some((hit.object, p, n))
}

/// Section 6c: SSAO, each GPU stage against the CPU mirror of it.
pub(super) fn check_ssao(
    report: &mut Report,
    renderer: &mut Renderer,
    still: &Timeline,
) -> Result<()> {
    // ---- 6c. SSAO: each GPU stage against the CPU mirror of it ------------
    use super::super::ssao::{self, Image};
    let mut detail = Vec::new();
    let mut ssao_ok = true;
    for blur in [1.0, 0.0] {
        let mut state = shot::for_demo(Demo::Room);
        state.set("labels", 0.0);
        state.set("ssaoBlur", blur);
        let frame = renderer.render_frame(request(&state, still), None);
        let content = frame.content.context("scene content")?;
        let settings = ssao::Settings::from_records(&demos::ssao_records(&state));
        let projection = glam::Mat4::from_cols_array_2d(&content.uniforms.camera.view_to_clip);
        let tan = Vec2::new(1.0 / projection.x_axis.x, 1.0 / projection.y_axis.y);
        let read = |name: &str, half: bool| -> Result<Image> {
            let (words, size) = renderer.read_chain_buffer(name).context("ssao buffer")?;
            // A half-resolution stage holds half the buffer's width and height, packed at its start.
            let size = if half {
                [size[0].div_ceil(2), size[1].div_ceil(2)]
            } else {
                size
            };
            Ok(Image {
                size,
                data: words[..(size[0] * size[1]) as usize]
                    .iter()
                    .map(|w| f32::from_bits(*w))
                    .collect(),
            })
        };
        let (linear, half, raw, blur_v, blur_h, full) = (
            read("linearDepth", false)?,
            read("halfDepth", true)?,
            read("ssao", true)?,
            read("ssaoBlurV", true)?,
            read("ssaoBlur", true)?,
            read("ssaoFull", false)?,
        );
        let stages = ssao_stages(
            [&linear, &half, &raw, &blur_v, &blur_h, &full],
            &settings,
            tan,
        );
        // Off by one 8-bit level where float rounding lands on the other side of a step; anything more in at most 0.1% of pixels.
        let stage_ok = stages
            .iter()
            .all(|(_, w, n)| *w <= 1.5 / 255.0 || *n * 1000 <= full.data.len());
        // The readout's SSAO, from ray-cast depths around the pixel, against the GPU's on flat surfaces.
        let projector = Projector::new(&content.uniforms.camera);
        let view_m = glam::Mat4::from_cols_array_2d(&content.uniforms.camera.world_to_view);
        let linear_depth = |x: i32, y: i32| -> f32 {
            let (o, d) = projector.ray(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
            let hit = scene::trace_scene(&content.objects, o, d, 100.0, ids::PANEL as u32);
            if hit.is_hit() {
                -view_m.transform_point3(o + d * hit.t).z
            } else {
                content.uniforms.camera.far_plane
            }
        };
        let (mut probed, mut close) = (0usize, 0usize);
        for j in 0..4 {
            for i in 0..6 {
                // 24 pixels, at the cell centres of a 6 × 4 grid.
                let (x, y) = (
                    (full.size[0] as i32) * (2 * i + 1) / 12,
                    (full.size[1] as i32) * (2 * j + 1) / 8,
                );
                let k = ssao::at_pixel(&linear_depth, full.size, &settings, tan, x, y);
                probed += 1;
                close += usize::from((k.upsampled - full.at(x, y)).abs() <= 2.5 / 255.0);
            }
        }
        ssao_ok &= close * 5 >= probed * 4;
        // Some pixels must be darkened, so SSAO is known to be at work.
        let darkened = full.data.iter().filter(|v| **v < 0.9).count();
        ssao_ok &= stage_ok && darkened > 100;
        detail.push(json!({"blur": blur, "stages": stages.iter().map(|(name, w, n)| json!({"stage": name, "worstDifference": w, "pixelsOffByMoreThanOneLevel": n})).collect::<Vec<_>>(), "pixelsDarkenedBelow0.9": darkened, "readoutPixels": probed, "readoutWithin2Levels": close}));
    }
    report.check("SSAO: the GPU's downsample, SsaoCompute, both blurs, and the upsample match the CPU mirror stage by stage (within one 8-bit level), with the blur on and off; the readout's ray-cast SSAO agrees within two levels at 80% of 24 pixels", ssao_ok, json!(detail));
    Ok(())
}

/// Each SSAO stage the GPU wrote (`[linear, half, raw, blur_v, blur_h, full]`) against the CPU mirror run on the
/// GPU's previous stage, so differences do not pile up: its name, its worst difference, and the pixels off by
/// more than one level.
pub(super) fn ssao_stages(
    [linear, half, raw, blur_v, blur_h, full]: [&super::super::ssao::Image; 6],
    settings: &super::super::ssao::Settings,
    tan: Vec2,
) -> [(&'static str, f32, usize); 5] {
    use super::super::ssao::{self, Image};
    // Each stage from the GPU's previous one, so differences do not pile up.
    let worst = |a: &Image, b: &Image| {
        a.data
            .iter()
            .zip(&b.data)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max)
    };
    let off = |a: &Image, b: &Image| {
        a.data
            .iter()
            .zip(&b.data)
            .filter(|(x, y)| (*x - *y).abs() > 1.5 / 255.0)
            .count()
    };

    [
        (
            "DepthDownsample",
            worst(half, &ssao::downsample(linear)),
            0usize,
        ),
        (
            "SsaoCompute",
            worst(raw, &ssao::compute(half, settings, tan)),
            off(raw, &ssao::compute(half, settings, tan)),
        ),
        (
            "blur, vertical",
            worst(blur_v, &ssao::blur(raw, half, settings, false)),
            off(blur_v, &ssao::blur(raw, half, settings, false)),
        ),
        (
            "blur, horizontal",
            worst(blur_h, &ssao::blur(blur_v, half, settings, true)),
            off(blur_h, &ssao::blur(blur_v, half, settings, true)),
        ),
        (
            "DepthUpsample",
            worst(full, &ssao::upsample(blur_h, half, linear)),
            off(full, &ssao::upsample(blur_h, half, linear)),
        ),
    ]
}

/// Section 6b: tiled light culling, the GPU's lists against the CPU mirror.
pub(super) fn check_tiled_lights(
    report: &mut Report,
    renderer: &mut Renderer,
    still: &Timeline,
) -> Result<()> {
    // ---- 6b. Tiled light culling: the GPU's lists against the CPU mirror ---
    use super::super::culling::{self, TileLightData};
    let mut state = shot::for_demo(Demo::Room);
    state.set("extraLights", 24.0);
    state.set("decals", 1.0);
    state.set("sphereLight", 1.0);
    let frame = renderer.render_frame(request(&state, still), None);
    let content = frame.content.context("scene content")?;
    let shading = demos::shading(&state);
    let lights = shading.lights();
    let camera = content.uniforms.camera;
    let (tile_words, size) = renderer
        .read_chain_buffer("tileData")
        .context("tile data")?;
    let (list_words, _) = renderer
        .read_chain_buffer("lightList")
        .context("light list")?;
    let (remap_words, _) = renderer
        .read_chain_buffer("lightListRemapped")
        .context("remapped list")?;
    let grid = [
        size[0].div_ceil(culling::TILE_DIM),
        size[1].div_ceil(culling::TILE_DIM),
    ];
    let (mut tiles, mut list_mismatch, mut remap_mismatch, mut listed) =
        (0usize, 0usize, 0usize, 0usize);
    for ty in 0..grid[1] {
        for tx in 0..grid[0] {
            // Each tile's list on the GPU must match the CPU's cull of the same tile data.
            let t = (ty * grid[0] + tx) as usize;
            let data = TileLightData::unpack(&tile_words[4 * t..4 * t + 4]);
            let cpu = culling::cull_tile(
                &camera,
                &data,
                [tx, ty],
                &lights,
                &shading.decals,
                &shading.sphere_lights,
            );
            // A tile's list has 256 words; the CPU's length says how many to compare.
            let gpu = &list_words[t * 256..t * 256 + cpu.len()];
            tiles += 1;
            listed += cpu.len() - culling::LIGHT_GROUPS as usize;
            list_mismatch += usize::from(gpu != cpu.as_slice());
            // The remap from the GPU's own list is integer work: it must match exactly.
            let base = t * 256 * 8;
            for (bin, entries) in culling::remap(gpu).iter().enumerate() {
                remap_mismatch += usize::from(
                    &remap_words[base + bin * 256..base + bin * 256 + entries.len()]
                        != entries.as_slice(),
                );
            }
        }
    }
    // Every light that reaches a surface point is in that pixel's bin.
    let projector = Projector::new(&camera);
    let view = glam::Mat4::from_cols_array_2d(&camera.world_to_view);
    let (mut points, mut missing) = (0usize, 0usize);
    for y in (4..size[1]).step_by(8) {
        for x in (4..size[0]).step_by(8) {
            // A surface point every 8 pixels: each light that reaches it must be in its bin.
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
            points += 1;
            let base = (t * 8 + data.bin(distance) as usize) * 256;
            missing += missing_from_bin(
                &remap_words[base..base + 256],
                (&shading, &lights),
                p,
                hit.normal,
            );
        }
    }
    let ok = tiles > 0
        && list_mismatch * 200 <= tiles
        && remap_mismatch == 0
        && missing == 0
        && points > 100
        && listed > 0;
    report.check(
        "light culling: the GPU's tile lists match the CPU mirror (all but at most 0.5% of tiles, float rounding), the remap exactly, and every light that reaches a surface point (and every decal whose box holds it) is in its pixel's bin; with a sphere light",
        ok,
        json!({"tiles": tiles, "lightsListed": listed, "listMismatchTiles": list_mismatch, "remapMismatchBins": remap_mismatch, "points": points, "lightsMissing": missing, "lights": lights.len()}),
    );
    Ok(())
}

/// The lights (simple point lights) and decals that reach the surface point `p` (normal `normal`) but are not in
/// its bin (`bin_words`, the remapped list's 256 words): how many.
pub(super) fn missing_from_bin(
    bin_words: &[u32],
    (shading, lights): (&brdf::Shading, &[brdf::PointLight]),
    p: Vec3,
    normal: Vec3,
) -> usize {
    use super::super::culling;
    let mut missing = 0;
    let bin: Vec<u32> = bin_words
        .iter()
        .copied()
        .take_while(|&v| v != culling::END_OF_LIST)
        .collect();
    let group = bin
        .split(|&v| v == culling::END_OF_GROUP)
        .nth(culling::GROUP_SIMPLE_POINT as usize)
        .unwrap_or(&[]);
    let decal_group = bin
        .split(|&v| v == culling::END_OF_GROUP)
        .next()
        .unwrap_or(&[]);
    for (i, decal) in shading.decals.iter().enumerate() {
        // A decal whose box holds the point must be in the bin's decal group, the first.
        if super::super::decals::sample(decal, p, normal).is_some()
            && !decal_group.contains(&(i as u32))
        {
            missing += 1;
        }
    }
    for (i, light) in lights.iter().enumerate() {
        // A light whose attenuation radius reaches the point must be in the simple point lights' group.
        let reaches =
            (light.position - p).length_squared() * light.inv_attenuation_radius_squared < 1.0;
        if reaches && !group.contains(&(i as u32)) {
            missing += 1;
        }
    }
    missing
}
