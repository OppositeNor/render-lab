//! The SSR investigation's verification suite: the WGSL passes against their
//! CPU mirror (trace.rs) on the GPU's own inputs, read back after a frame,
//! the formulas the story states, and the package's files. Every check records
//! what it compared and the result in the JSON report.

use super::gpu;
use super::shot::{self, Demo};
use super::trace::{self, Image, Inputs};
use anyhow::{Result, anyhow};
use lab::utility::selftest::{Report, request, still_timeline};
use serde_json::json;

/// An IEEE half float.
fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 {
        -1.0
    } else {
        1.0
    };
    let exp = ((bits >> 10) & 0x1f) as i32;
    let man = (bits & 0x3ff) as f32;
    sign * match exp {
        // Exponent 0: a subnormal, the mantissa alone in units of 2^-24.
        0 => man * 2f32.powi(-24),
        31 => {
            // Exponent 31 with no mantissa is infinity; with one, NaN.
            if man == 0.0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
        // Any other: the implicit leading 1, scaled by the unbiased exponent.
        e => (1.0 + man / 1024.0) * 2f32.powi(e - 15),
    }
}

/// A program image read back as floats.
fn read(
    renderer: &lab::renderer::Renderer,
    name: &str,
    mip: u32,
    format: wgpu::TextureFormat,
) -> Result<Image> {
    let (bytes, size) = renderer
        .read_program_mip(name, 0, mip)
        .ok_or_else(|| anyhow!("no image {name} mip {mip}"))?;
    let (channels, data): (usize, Vec<f32>) = match format {
        // Each format's bytes as floats: 32-bit floats as they are, 8-bit unorm over 255, halves decoded.
        wgpu::TextureFormat::R32Float => (
            1,
            bytes
                .chunks(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect(),
        ),
        wgpu::TextureFormat::R8Unorm => (1, bytes.iter().map(|b| *b as f32 / 255.0).collect()),
        wgpu::TextureFormat::Rgba8Unorm => (4, bytes.iter().map(|b| *b as f32 / 255.0).collect()),
        wgpu::TextureFormat::Rgba16Float => (
            4,
            bytes
                .chunks(2)
                .map(|c| half(u16::from_le_bytes([c[0], c[1]])))
                .collect(),
        ),
        f => return Err(anyhow!("no reader for {f:?}")),
    };
    Ok(Image {
        size: [size[0], size[1]],
        channels,
        data,
    })
}

pub fn run(options: &lab::investigation::SelftestOptions) -> Result<bool> {
    std::fs::create_dir_all(&options.work_dir)?;
    let mut report = Report::new();

    // ---- 1. The fade's ceiling (research.md 0.3#1): with the defaults, at most 0.58 ----
    check_fade(&mut report);

    // ---- 2. The passes: the WGSL port against trace.rs, on the GPU's own inputs ----
    check_passes(&mut report)?;

    // Listings: each cites a source entry and a demonstration, and every demonstration has one.
    check_listings(&mut report)?;

    // The demos' reads (lab::fetch): a Trace frame drawn alone reads its own
    // frame's pyramid and normals back, equal to the GPU's; P's surface on them
    // projects back into P.
    check_reads(&mut report)?;
    check_inset_clicks(&mut report)?;

    // The story's worked example (video/story.lam `spoutPick`): at the trace demonstration's own size, P on the
    // spout walks as the narration says, and the pixel the exits beat moves it to leaves through the top.
    check_worked_example(&mut report)?;

    // The field and the cross-section are views of their own over the main one (lab/src/view.rs): shown by
    // their own values, each with its own camera, which the pointer moves without moving the main one.
    check_views(&mut report)?;

    // Every demonstration builds its state.
    let states_ok = Demo::ALL.iter().all(|&d| shot::for_demo(d).demo == d.id());
    report.check(
        "every demonstration's file loads",
        states_ok,
        json!(Demo::ALL.iter().map(|d| d.key()).collect::<Vec<_>>()),
    );

    let passed = report.passed();
    std::fs::write(
        &options.report,
        serde_json::to_string_pretty(&json!({"passed": passed, "checks": report.checks}))?,
    )?;
    Ok(passed)
}

/// The fade's ceiling (research.md 0.3#1): with the defaults, at most 0.58.
fn check_fade(report: &mut Report) {
    let (peak_len, peak) = (1..2000)
        .map(|k| k as f32 / 2000.0)
        .map(|l| (l, trace::fade(l, 0.15, 2.0)))
        .fold((0.0, 0.0), |a, b| {
            if b.1 > a.1 {
                b
            } else {
                a
            }
        });
    let ok = (peak - 0.581).abs() < 0.002 && (peak_len - 0.15 / 2.15).abs() < 0.002;
    report.check("the default fades (0.15, 2.0) peak at 0.58 at a screen length of 0.07: SSR covers at most 58% of the probe's reflection", ok, json!({"peak": peak, "at": peak_len}));
}

/// The passes: the WGSL port against trace.rs, on the GPU's own inputs (the Hi-Z levels, the half-size normals,
/// the trace per pixel, a worked ray, the resolve).
fn check_passes(report: &mut Report) -> Result<()> {
    let gpu = lab::gpu::Gpu::new(lab::gpu::instance(None), None)?;
    let mut renderer = lab::renderer::Renderer::new(
        gpu,
        lab::frame::Statics::prepare(lab::scene::SceneId::STUDY_ROOM)?,
        640,
        360,
    )?;
    let still = still_timeline(640, 360);
    let state = shot::for_demo(Demo::Room);
    renderer.render_frame(request(&state, &still), None);
    let viewport = renderer
        .read_program("final", 0)
        .map(|(_, s)| [s[0], s[1]])
        .ok_or_else(|| anyhow!("no final image"))?;
    let size = gpu::ssr_size(viewport);
    let mips = gpu::mipmaps(size);

    // The Hi-Z levels: each the reduction of the one before.
    let hiz: Vec<Image> = (0..mips)
        .map(|m| read(&renderer, "hiz", m, wgpu::TextureFormat::R32Float))
        .collect::<Result<_>>()?;
    let mut levels_ok = hiz[0].size == size;
    let mut worst = Vec::new();
    for m in 1..mips as usize {
        // Each level reduced on the CPU from the GPU's level before must equal the GPU's, bit for bit.
        let cpu = trace::hiz_level(&hiz[m - 1], gpu::level_size(size, m as u32));
        let same = cpu.size == hiz[m].size && cpu.data == hiz[m].data;
        levels_ok &= same;
        if !same {
            worst.push(m);
        }
    }
    let filled = hiz[0].data.iter().filter(|d| **d > 0.0).count();
    report.check("Hi-Z: level 0 is the SSR size, and each level is exactly the max of its parents (3 wide on odd sides)", levels_ok && filled > 0, json!({"size": size, "mipmaps": mips, "levelsDiffering": worst, "level0Covered": filled}));

    // The trace, per pixel.
    let normal = read(&renderer, "halfNormal", 0, wgpu::TextureFormat::Rgba8Unorm)?;
    let last = read(&renderer, "lastFrame", 0, wgpu::TextureFormat::Rgba16Float)?;
    let color = read(&renderer, "ssrColor", 0, wgpu::TextureFormat::Rgba16Float)?;
    let level = read(&renderer, "mipLevel", 0, wgpu::TextureFormat::R8Unorm)?;
    let normal_filled = normal
        .data
        .chunks(4)
        .filter(|c| c.iter().any(|v| *v > 0.0))
        .count();
    report.check("the half-size normal-roughness is the SSR size and filled where there is geometry", normal.size == size && normal_filled * 2 > (size[0] * size[1]) as usize, json!({"size": normal.size, "filled": normal_filled, "center": normal.load([size[0] as i32 / 2, (size[1] as i32 * 4) / 5]).to_array()}));
    let (projection, w2v) = gpu::camera(&state, viewport);
    let inputs = Inputs {
        hiz: &hiz,
        normal_roughness: &normal,
        last_frame: &last,
        projection,
        inv_projection: projection.inverse(),
        reprojection: gpu::reprojection((projection, w2v), (projection, w2v)),
        num_steps: state.p("ssrMaxSteps").round() as i32,
        distance_fade: state.p("ssrFadeOut"),
        curve_fade_in: state.p("ssrFadeIn"),
        depth_tolerance: state.p("ssrDepthTolerance"),
    };
    let (agree, traced, valid, examples) = compare_traces(&inputs, (&color, &level), size);
    let total = (size[0] * size[1]) as usize;
    let share = agree as f32 / total as f32;
    // The best-fit table: the CPU mirror the story's demonstration draws from, against the GPU's own entries.
    let table = read(&renderer, "bestFit", 0, wgpu::TextureFormat::R8Unorm)?;
    let (mut table_same, mut table_all, mut table_worst) = (0, 0, 0.0f32);
    for i in (0..1024u32).step_by(37) {
        for j in (0..1024u32).step_by(41) {
            // Each sampled entry, the GPU's against the mirror's; a near-tie between two lengths may fall either way.
            let d = (table.load([i as i32, j as i32]).x - super::demos::rules::table_entry([i, j]))
                .abs();
            table_all += 1;
            table_same += usize::from(d <= 1.0 / 255.0 + 1e-6);
            table_worst = table_worst.max(d);
        }
    }
    report.check("the best-fit table: the CPU mirror (demos/rules.rs) matches the GPU's entries within one 8-bit step at 99% of 700 texels", table_same * 100 >= table_all * 99, json!({"same": table_same, "texels": table_all, "worst": table_worst}));
    report.check("the trace: validity and blur level agree with trace.rs at 99% of the SSR pixels, on the GPU's own Hi-Z and normals", share >= 0.99 && traced > total / 4 && valid > 0, json!({"agree": share, "traced": traced, "valid": valid, "pixels": total, "examples": examples}));

    // A worked ray: the floor pixel below the image's centre, its levels step by one (a miss up, a hit down).
    check_worked_ray(report, &inputs, size, mips);

    // The resolved reflections are premultiplied by a validity no higher than the fades allow.
    let fin = read(&renderer, "final", 0, wgpu::TextureFormat::Rgba16Float)?;
    let max_alpha = fin.data.chunks(4).map(|c| c[3]).fold(0.0f32, f32::max);
    report.check(
        "the resolved reflections' alpha never exceeds the fades' ceiling (0.58)",
        max_alpha > 0.0 && max_alpha <= 0.582,
        json!({"maxAlpha": max_alpha}),
    );
    Ok(())
}

/// The trace per pixel against the GPU's: how many agree on the validity and the blur level, how many were
/// traced, how many are valid, and the first disagreements.
fn compare_traces(
    inputs: &Inputs,
    (color, level): (&Image, &Image),
    size: [u32; 2],
) -> (usize, usize, usize, Vec<serde_json::Value>) {
    let (mut agree, mut traced, mut valid) = (0usize, 0usize, 0usize);
    let mut examples = Vec::new();
    for y in 0..size[1] as i32 {
        for x in 0..size[0] as i32 {
            // Trace each pixel on the CPU, and read the GPU's validity (alpha) and blur level (stored over 14 levels).
            let t = inputs.trace([x, y]);
            let g = color.load([x, y]);
            let g_level = level.load([x, y]).x * 14.0;
            if t.exit != trace::Exit::NotTraced {
                traced += 1;
            }
            if t.validity > 0.0 {
                valid += 1;
            }
            let ok = (g.w - t.validity).abs() < 0.02 && (g_level - t.mip_level).abs() < 0.1;
            if ok {
                agree += 1;
            } else if examples.len() < 8 {
                examples.push(json!({"pixel": [x, y], "gpu": [g.w, g_level], "cpu": [t.validity, t.mip_level], "exit": format!("{:?}", t.exit), "steps": t.steps.len()}));
            }
        }
    }
    (agree, traced, valid, examples)
}

/// A worked ray: the floor pixel below the image's centre, its levels step by one (a miss up, a hit down).
fn check_worked_ray(report: &mut Report, inputs: &Inputs, size: [u32; 2], mips: u32) {
    let pixel = [size[0] as i32 / 2, (size[1] as i32 * 4) / 5];
    let t = inputs.trace(pixel);
    let rule = t.steps.iter().all(|s| {
        s.next_level
            == if s.held {
                s.level
            } else if s.hit {
                s.level - 1
            } else {
                (s.level + 1).min(mips as i32 - 1)
            }
    });
    let steps: Vec<serde_json::Value> = t.steps.iter().map(|s| json!({"level": s.level, "t": s.t, "cell": s.cell, "cellDepth": s.cell_depth, "depthT": s.depth_t, "edgeT": s.edge_t, "hit": s.hit, "held": s.held, "next": [s.next_level, s.next_t]})).collect();
    report.check("a worked floor ray: each iteration climbs a level on a miss, descends on a hit, and holds at level 0 behind a surface", rule && !t.steps.is_empty(), json!({"pixel": pixel, "exit": format!("{:?}", t.exit), "validity": t.validity, "terms": t.terms, "rayLen": t.ray_len, "start": t.start.to_array(), "normal": t.normal.to_array(), "geomNormal": t.geom_normal.to_array(), "rayDir": t.ray_dir.to_array(), "bounced": t.bounced, "screenPos": t.screen_pos.to_array(), "screenRay": t.screen_ray.to_array(), "tMax": t.t_max, "steps": steps}));
}

/// Listings: each cites a source entry and a demonstration, and every demonstration has one.
fn check_listings(report: &mut Report) -> Result<()> {
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
    report.check(
        "every listing cites a source entry and a demonstration; every demonstration has a listing",
        bad.is_empty() && without.is_empty(),
        json!({"listings": listings.len(), "bad": bad, "demosWithout": without}),
    );
    Ok(())
}

/// The demos' reads (lab::fetch): a Trace frame drawn alone reads its own frame's pyramid and normals back,
/// equal to the GPU's; P's surface on them projects back into P.
fn check_reads(report: &mut Report) -> Result<()> {
    let gpu = lab::gpu::Gpu::new(lab::gpu::instance(None), None)?;
    let mut renderer = lab::renderer::Renderer::new(
        gpu,
        lab::frame::Statics::prepare(lab::scene::SceneId::STUDY_ROOM)?,
        640,
        360,
    )?;
    let still = still_timeline(640, 360);
    let state = shot::for_demo(Demo::Trace);
    let mut content = renderer
        .render_frame(request(&state, &still), None)
        .content
        .ok_or_else(|| anyhow!("no frame content"))?;
    let direct = read(&renderer, "hiz", 0, wgpu::TextureFormat::R32Float)?;
    let fetched = lab::fetch::get("godot_ssr.hiz/0");
    let same = fetched
        .as_ref()
        .is_some_and(|f| f.floats == direct.data && f.region.size[..2] == direct.size[..]);
    let drawn = content.readout.is_some() && !content.insets.is_empty();
    let screen = super::cpu::screen(&mut content);
    let (p, back) = match &screen {
        Some(screen) => {
            // P's world surface, projected back, must land in P.
            let p = super::demos::picked(&state, screen, &content.objects);
            (Some(p), screen.pixel_of(screen.surface(p)))
        }
        None => (None, None),
    };
    report.check(
        "the demos' GPU reads: a frame drawn alone fetches its own Hi-Z (equal to the GPU's), the trace demo draws from it, and P's surface projects back into P",
        same && drawn && back.is_some() && back == p,
        json!({"fetched": fetched.map(|f| f.region.size), "gpu": direct.size, "same": same, "drawn": drawn, "pixel": p, "back": back}),
    );
    // The click on the scene (script/logic.lam `scene`): in the trace demonstration the half-resolution pixel
    // under it, one pixel wide, as the Rust click picked it; in another, nothing.
    let size = glam::Vec2::from(content.uniforms.camera.viewport_size);
    let mut differ = Vec::new();
    let mut clicks = 0;
    for demo in [Demo::Trace, Demo::Room] {
        // Click at 41 x 41 points of the view, each from the demonstration's own state.
        let shown = shot::for_demo(demo);
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
                let ran = lab::script::runtime::click("scene", &mut frame);
                // In f64, as the script computes: at a pixel's edge an f32 product can round across it.
                let pixel = uv.as_dvec2() * size.as_dvec2();
                let expected = (demo == Demo::Trace).then(|| {
                    [
                        (pixel.x / 2.0).floor() as f32,
                        (pixel.y / 2.0).floor() as f32,
                        0.0,
                        0.0,
                    ]
                });
                let got = ["pickX", "pickY", "pickWidth", "pickHeight"].map(|k| state.p(k));
                let before = ["pickX", "pickY", "pickWidth", "pickHeight"].map(|k| shown.p(k));
                clicks += 1;
                let same = match expected {
                    // The trace demonstration's click picks that pixel.
                    Some(e) => ran == Some(true) && got == e,
                    // Any other demonstration's click changes nothing.
                    None => ran == Some(false) && got == before,
                };
                if !same {
                    differ.push(json!({"demo": demo.key(), "uv": [uv.x, uv.y], "script": got, "rust": expected}));
                }
            }
        }
    }
    report.check("the scene click (script/logic.lam `scene`) picks the trace's half-resolution pixel, as the Rust click did, and nothing in another demonstration (41 x 41 points in each)", differ.is_empty(), json!({"clicks": clicks, "differ": differ.iter().take(20).collect::<Vec<_>>()}));
    Ok(())
}

/// The story's worked example: P on the teapot's spout walks as the narration says, and the exits beat's pixel
/// leaves through the top.
fn check_worked_example(report: &mut Report) -> Result<()> {
    let frame = shot::demos().spec(Demo::Trace.id()).frame;
    let size = frame.size().unwrap_or([0, 0]);
    let gpu = lab::gpu::Gpu::new(lab::gpu::instance(None), None)?;
    let mut renderer = lab::renderer::Renderer::new(
        gpu,
        lab::frame::Statics::prepare(lab::scene::SceneId::STUDY_ROOM)?,
        1920,
        1080,
    )?;
    let still = still_timeline(1920, 1080);
    let mut state = shot::for_demo(Demo::Trace);
    for (k, x) in [
        // The worked example's shot: a mirror-like floor, no sky reflection, SSR composed, the camera on the spout.
        ("floorMetallic", 1.0),
        ("skyReflection", 0.0),
        ("ssrEnabled", 1.0),
        ("ssrComposeStep", 3.0),
        ("floorRoughness", 0.15),
    ] {
        state.set(k, x);
    }
    state.panel = lab::shot::PanelMode::Code;
    state.code = Some("trace".into());
    state.camera.target = [-0.15, 0.38, -1.7];
    state.camera.yaw = 35.0;
    state.camera.pitch = 12.0;
    state.camera.distance = 1.8;
    let mut content = renderer
        .render_frame(request(&state, &still), None)
        .content
        .ok_or_else(|| anyhow!("no frame content"))?;
    let walks = super::cpu::screen(&mut content).map(|screen| {
        let last = Image::new([1, 1], 4);
        let inputs = screen.inputs(
            &last,
            state.p("ssrMaxSteps").round() as i32,
            state.p("ssrFadeIn"),
            state.p("ssrFadeOut"),
            state.p("ssrDepthTolerance"),
        );
        let walk = |p: [i32; 2]| {
            // A walk as the narration reads it: each iteration's level and m (miss), h (hit), or t (held by tolerance).
            let t = inputs.trace(p);
            let steps: Vec<String> = t
                .steps
                .iter()
                .map(|s| {
                    format!(
                        "{}{}",
                        s.level,
                        if s.held {
                            "t"
                        } else if s.hit {
                            "h"
                        } else {
                            "m"
                        }
                    )
                })
                .collect();
            (steps.join(" "), t.exit, t.hit_screen.y, t.screen_ray.z)
        };
        (screen.size, walk([366, 294]), walk([398, 277]))
    });
    // The sky's ray: misses all the way, away from the camera (its reversed depth falling), out through the top.
    let ok = walks
        .as_ref()
        .is_some_and(|(ssr, (p, p_exit, _, _), (up, up_exit, up_y, up_dz))| {
            *ssr == [size[0] / 2, size[1] / 2]
                && p == "0m 1m 2m 3m 4h 3h 2m 3h 2h 1m 2h 1h 0h"
                && *p_exit == trace::Exit::Hit
                && up.split(' ').all(|s| s.ends_with('m'))
                && *up_exit == trace::Exit::Screen
                && *up_y <= 0.01
                && *up_dz < 0.0
        });
    report.check(
        "the story's worked example at the trace demonstration's size (1100 x 1080): P (366, 294) walks as narrated to a hit, and (398, 277) leaves through the top of the screen in misses, heading away",
        size == [1100, 1080] && ok,
        json!({"frame": size, "walks": walks.map(|(ssr, p, up)| json!({"ssr": ssr, "p": p.0, "pExit": format!("{:?}", p.1), "up": up.0, "upExit": format!("{:?}", up.1), "upY": up.2, "upDepthStep": up.3}))}),
    );
    Ok(())
}

/// The field and the cross-section are views of their own over the main one: shown by their own values, each
/// with its own camera.
fn check_views(report: &mut Report) -> Result<()> {
    let mut state = shot::for_demo(Demo::Trace);
    let none = super::demos::views(&state).is_empty();
    state.set("view.field.fieldShow", 1.0);
    let views = super::demos::views(&state);
    let field = views.len() == 1 && views[0].name == super::demos::FIELD && views[0].alpha == 1.0;
    let main = state.camera;
    let mut turned = views[0].camera;
    turned.orbit(glam::Vec2::new(40.0, -20.0));
    state.set_own_camera(super::demos::FIELD, &turned);
    let own = super::demos::views(&state)[0].camera == turned && state.camera == main;
    state.set("view.profile.profileShow", 1.0);
    let both = super::demos::views(&state)
        .iter()
        .map(|v| v.name)
        .collect::<Vec<_>>()
        == [super::demos::FIELD, super::demos::PROFILE];
    report.check("the height field and the cross-section are views of their own: shown by their own values, each with its own camera, which moves without moving the main one", none && field && own && both, json!({"none": none, "field": field, "ownCamera": own, "both": both}));
    Ok(())
}

/// The insets' clicks (script/logic.lam): a click on a thread of P's group picks that thread's pixel, and P is then
/// that pixel (a round trip); a click on the best-fit chart and on the fade chart marks the length under it.
fn check_inset_clicks(report: &mut Report) -> Result<()> {
    let gpu = lab::gpu::Gpu::new(lab::gpu::instance(None), None)?;
    let mut renderer = lab::renderer::Renderer::new(
        gpu,
        lab::frame::Statics::prepare(lab::scene::SceneId::STUDY_ROOM)?,
        640,
        360,
    )?;
    let still = still_timeline(640, 360);
    let mut shown = shot::for_demo(Demo::Trace);
    shown.set("laneStep", 2.0);
    let mut content = renderer
        .render_frame(request(&shown, &still), None)
        .content
        .ok_or_else(|| anyhow!("no frame content"))?;
    let screen = super::cpu::screen(&mut content).ok_or_else(|| anyhow!("no screen read back"))?;
    let group = content
        .script
        .clone()
        .and_then(|d| d.downcast::<super::script::LaneGroup>().ok())
        .ok_or_else(|| anyhow!("no thread group drawn"))?;
    let mut differ = Vec::new();
    let click = |handler: &str, uv: glam::Vec2| {
        // Run a click handler on the shown frame, at `uv` of its inset.
        let mut state = shown.clone();
        let mut frame = lab::actions::ClickFrame {
            shown: &shown,
            state: &mut state,
            uv,
            content: &content,
            statics: &renderer.statics,
        };
        let ran = lab::script::runtime::click(handler, &mut frame);
        (ran, state)
    };
    for k in 0..64 {
        // Each thread's cell centre picks that thread, and P is then its pixel.
        let (i, j) = (k % 8, k / 8);
        let uv = glam::Vec2::new((i as f32 + 0.5) / 8.0, (j as f32 + 0.5) / 8.0);
        let (ran, state) = click("pickLane", uv);
        let want = [group.x as i32 + i, group.y as i32 + j];
        let p = super::demos::picked(&state, &screen, &content.objects);
        if ran != Some(true) || p != want {
            differ.push(json!({"cell": [i, j], "picked": p, "want": want}));
        }
    }
    let (fit_ran, fit) = click("pickFitLength", glam::Vec2::new(0.3, 0.5));
    let (fade_ran, fade) = click("pickFadeLength", glam::Vec2::new(0.3, 0.5));
    let charts = fit_ran == Some(true)
        && (fit.p("fitLen") - (0.05 + 0.95 * 0.3)).abs() < 1e-5
        && fade_ran == Some(true)
        && (fade.p("fadeProbe") - 0.3).abs() < 1e-5;
    report.check("the insets' clicks: each of the 64 threads of P's group, clicked, becomes P (a round trip); the best-fit and fade charts mark the length clicked", differ.is_empty() && charts, json!({"group": [group.x, group.y, group.w, group.h], "differ": differ, "fitLen": fit.p("fitLen"), "fadeProbe": fade.p("fadeProbe")}));
    Ok(())
}
