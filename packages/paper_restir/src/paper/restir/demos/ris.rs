//! RIS at one pixel, P (the related work's demonstration, and streaming RIS's):
//! P's surface read back from the candidate pass, its M candidates recomputed
//! by cpu.rs exactly as the GPU drew them (the frozen seed makes them the same
//! every frame; the selftest checks they match), drawn as lines from P to the
//! points on the lights, each as bright as its weight w = p̂ / p; streamed one by
//! one into the reservoir as far as `risStep`, the kept candidate lit; a line
//! an object blocks (cpu.rs `visible`, the GPU's shadow ray) is drawn red.

use super::super::cpu::{self, Reservoir, Rng, Sample, Scene, Surface};
use super::super::lights::{self, Light};
use crate::paper::restir::text::{tr, tr_with};
use egui::Color32;
use glam::{UVec2, Vec3};
use lab::fetch::{Region, Source};
use lab::frame::{FrameContent, Readout};
use lab::palette;
use lab::shot::ShotState;

/// One candidate as the stream met it.
pub struct Candidate {
    pub x: Sample,
    pub point: Vec3,
    pub p: f32,
    pub p_hat: f32,
    pub w: f32,
    /// Whether it was kept when it was streamed.
    pub taken: bool,
}

/// P, in pixels of a viewport of `size`: the picked one (`pickU`, `pickV`, fractions of the viewport).
pub fn pixel(state: &ShotState, size: [f32; 2]) -> [i32; 2] {
    let u = state.p("pickU").clamp(0.0, 1.0);
    let v = state.p("pickV").clamp(0.0, 1.0);
    [
        ((u * size[0]) as i32).min(size[0] as i32 - 1),
        ((v * size[1]) as i32).min(size[1] as i32 - 1),
    ]
}

/// P's surface, read back from the candidate pass's images (the frame before this one's, while it lands).
pub fn surface(content: &mut FrameContent, q: [i32; 2]) -> Option<Surface> {
    let mut texel = |key: &str, image: &str| {
        content
            .fetch(key, Source::image(image, 0), Region::around(q, 0))
            .and_then(|f| f.texel(q))
            .map(|v| v.to_array())
    };
    // All three are asked for before any is used, so one frame's reads bring them together.
    let (p, m, a) = (
        texel("restir-p-surfP", "surfP"),
        texel("restir-p-surfM", "surfM"),
        texel("restir-p-surfA", "surfA"),
    );
    let s = Surface::from_texels(p?, m?, a?);
    s.valid.then_some(s)
}

/// The candidates of P in the order they were drawn, and the reservoir after the first `steps` of them.
pub fn stream(
    scene: &Scene,
    s: &Surface,
    rng: &mut Rng,
    steps: u32,
) -> (Vec<Candidate>, Reservoir) {
    let mut r = Reservoir::empty();
    let mut out = Vec::new();
    for i in 0..scene.candidates {
        // As restirCandidates: a light by power, a point on it, the weight; streamed while within the step.
        let light = scene.pick(rng.rand());
        let uv = (rng.next() >> 16) | (rng.next() & 0xffff_0000);
        let x = Sample { light, uv };
        let (p, p_hat) = (scene.source_pdf(light), scene.target(s, x));
        let w = p_hat / p;
        let before = r.y;
        if i < steps {
            r.update(x, w, rng);
        } else {
            rng.rand();
        }
        out.push(Candidate {
            x,
            point: scene.point(x),
            p,
            p_hat,
            w,
            taken: i < steps && r.y != before,
        });
    }
    r.finish(scene.target(s, r.y), r.m);
    (out, r)
}

/// Draws P's candidates and the reservoir after `risStep` of them, with a readout of the stream.
pub fn draw(
    state: &ShotState,
    content: &mut FrameContent,
    lights: &[Light],
    meshes: &lab::scene::Meshes,
) {
    let size = content.uniforms.camera.viewport_size;
    let q = pixel(state, size);
    let Some(s) = surface(content, q) else {
        // Until P's surface is read back (or where P sees no lit surface), the panel says so.
        content.readout = Some(waiting());
        return;
    };
    let cdf = lights::cdf(lights);
    let triangles = cpu::occluders(meshes, &content.objects);
    let scene = Scene {
        lights,
        cdf: &cdf,
        eye: Vec3::from(content.uniforms.camera.position),
        triangles: &triangles,
        candidates: state.p("restirCandidates").round().max(1.0) as u32,
        unbiased: false,
        visibility_reuse: false,
    };
    let seed = state.p("restirSeed").max(0.0) as u32;
    let mut rng = Rng::new(UVec2::new(q[0] as u32, q[1] as u32), seed, 0);
    let steps = (state.p("risStep").round().max(0.0) as u32).min(scene.candidates);
    let (candidates, r) = stream(&scene, &s, &mut rng, steps);
    let top = candidates.iter().map(|c| c.w).fold(1e-12, f32::max);
    let weigh = state.p("risWeigh").clamp(0.0, 1.0);
    let lit = picked(&candidates, steps, state.p("risPick"), &r);
    for (i, c) in candidates.iter().enumerate() {
        // Each candidate's line: dim until streamed, then as bright as its weight; the lit one (the kept, or the
        // pick marker's) highlighted, a blocked one red.
        let kept = lit == Some(i);
        let (mut color, alpha, width) = if kept {
            (palette::SELECTED, 1.0, 4.0)
        } else if (i as u32) < steps {
            let by_weight = 0.45 + 0.55 * (c.w / top).sqrt();
            (palette::INCIDENT, 1.0 + weigh * (by_weight - 1.0), 2.5)
        } else {
            (palette::THEME_ACCENT, 0.15, 1.0)
        };
        if !scene.visible(&s, c.x) {
            color = super::BLOCKED;
        }
        content
            .draw
            .segment(s.position, c.point, color, alpha, width, 0);
        content.draw.point(
            c.point,
            if kept {
                9.0
            } else {
                5.0
            },
            color,
            alpha,
        );
    }
    content.draw.point(s.position, 10.0, palette::SELECTED, 1.0);
    content
        .draw
        .text(s.position, "P", palette::SELECTED, [1.2, -0.6]);
    content.readout = Some(readout(&candidates, &r, steps));
}

/// The candidate to light: with the pick marker between 0 and 1, the streamed one under it when they are laid end to
/// end by weight; otherwise (and at 1) the one the reservoir kept.
fn picked(candidates: &[Candidate], steps: u32, marker: f32, r: &Reservoir) -> Option<usize> {
    let streamed = &candidates[..(steps as usize).min(candidates.len())];
    let kept = streamed.iter().position(|c| c.x == r.y);
    let total: f32 = streamed.iter().map(|c| c.w).sum();
    if marker <= 0.0 || marker >= 1.0 || total <= 0.0 {
        return kept;
    }
    let mut running = 0.0;
    for (i, c) in streamed.iter().enumerate() {
        // The marker's share of the sum falls in this candidate's stretch of it.
        running += c.w;
        if marker * total < running {
            return Some(i);
        }
    }
    kept
}

fn readout(candidates: &[Candidate], r: &Reservoir, steps: u32) -> Readout {
    let mut rows = Vec::new();
    for (i, c) in candidates
        .iter()
        .enumerate()
        .take(steps.max(1) as usize)
        .skip((steps as usize).saturating_sub(6))
    {
        // The last six candidates streamed: their light, p̂, p, and weight, the kept ones marked.
        let color = if c.taken {
            palette::SELECTED
        } else {
            Color32::GRAY
        };
        rows.push((
            tr_with("demos-candidate", lab::args!(i = i + 1, light = c.x.light)),
            format!("{:.3} / {:.2} = {:.3}", c.p_hat, c.p, c.w),
            color,
        ));
    }
    rows.push((
        tr("demos-wsum").into(),
        format!("{:.4}", r.wsum),
        palette::INCIDENT,
    ));
    rows.push((
        tr("demos-m").into(),
        format!("{}", r.m as u32),
        palette::THEME_ACCENT,
    ));
    rows.push((
        tr("demos-w").into(),
        format!("{:.4}", r.w),
        palette::SELECTED,
    ));
    Readout {
        title: tr("demos-reservoir_title").into(),
        subtitle: tr("demos-reservoir_subtitle").into(),
        formula: Some(tr("formula-ris").into()),
        formula_lit: None,
        rows,
    }
}

/// The panel while P's surface is not read back yet, or P is not on a lit surface.
pub fn waiting() -> Readout {
    Readout {
        title: tr("demos-reservoir_title").into(),
        subtitle: tr("demos-no_surface").into(),
        formula: None,
        formula_lit: None,
        rows: Vec::new(),
    }
}
