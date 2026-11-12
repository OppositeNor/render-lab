//! Spatial reuse at P (Alg. 4 and Alg. 5 lines 16-20): P's k neighbours for the
//! first round, drawn where they are in the room, green where the geometry test
//! keeps them and red where it rejects them, each with a line to the sample its
//! reservoir holds (red where an object blocks it, cpu.rs `visible`); the readout sums the M that P's combined reservoir stands
//! for. The neighbours are the GPU's (cpu.rs `neighbour`, the frozen seed).

use super::super::cpu::{self, Reservoir, Rng, Sample, Scene, Surface};
use super::super::lights::{self, Light};
use super::ris;
use crate::paper::restir::text::{tr, tr_with};
use glam::{UVec2, Vec3};
use lab::fetch::{Fetched, Region, Source};
use lab::frame::{FrameContent, Readout};
use lab::palette;
use lab::shot::ShotState;
use std::sync::Arc;

/// A 32-bit-word texel of a fetched region (the reservoirs' Rgba32Uint), or None outside it.
pub fn words(f: &Fetched, p: [i32; 2]) -> Option<[u32; 4]> {
    let x = p[0] - f.region.origin[0] as i32;
    let y = p[1] - f.region.origin[1] as i32;
    if x < 0 || y < 0 || x >= f.region.size[0] as i32 || y >= f.region.size[1] as i32 {
        return None;
    }
    let i = 16 * (y as usize * f.region.size[0] as usize + x as usize);
    let w = |k: usize| {
        u32::from_le_bytes([
            f.bytes[i + k],
            f.bytes[i + k + 1],
            f.bytes[i + k + 2],
            f.bytes[i + k + 3],
        ])
    };
    Some([w(0), w(4), w(8), w(12)])
}

/// The disc around P, read back from four images: the surfaces and the round's input reservoirs.
struct Disc {
    p: Arc<Fetched>,
    m: Arc<Fetched>,
    a: Arc<Fetched>,
    reservoirs: Arc<Fetched>,
}

impl Disc {
    fn fetch(content: &mut FrameContent, q: [i32; 2], radius: u32) -> Option<Disc> {
        let region = Region::around(q, radius);
        let mut get = |key: &str, image: &str| content.fetch(key, Source::image(image, 0), region);
        // All four are asked for before any is used, so one frame's reads bring them together.
        let (p, m, a) = (
            get("restir-disc-surfP", "surfP"),
            get("restir-disc-surfM", "surfM"),
            get("restir-disc-surfA", "surfA"),
        );
        let reservoirs = get("restir-disc-temporal", "temporal");
        Some(Disc {
            p: p?,
            m: m?,
            a: a?,
            reservoirs: reservoirs?,
        })
    }

    fn surface(&self, q: [i32; 2]) -> Option<Surface> {
        let t = |f: &Fetched| f.texel(q).map(|v| v.to_array());
        Some(Surface::from_texels(t(&self.p)?, t(&self.m)?, t(&self.a)?))
    }
}

/// Draws P's neighbours of the first round, and what the round combines.
pub fn draw(
    state: &ShotState,
    content: &mut FrameContent,
    lights: &[Light],
    meshes: &lab::scene::Meshes,
) {
    let size = content.uniforms.camera.viewport_size;
    let q = ris::pixel(state, size);
    let radius = state.p("restirRadius");
    let Some(disc) = Disc::fetch(content, q, radius.ceil() as u32 + 1) else {
        content.readout = Some(ris::waiting());
        return;
    };
    let Some(s) = disc.surface(q).filter(|s| s.valid) else {
        content.readout = Some(ris::waiting());
        return;
    };
    let cdf = lights::cdf(lights);
    let triangles = cpu::occluders(meshes, &content.objects);
    let scene = Scene {
        lights,
        cdf: &cdf,
        eye: Vec3::from(content.uniforms.camera.position),
        triangles: &triangles,
        candidates: 1,
        unbiased: state.p("restirUnbiased") > 0.5,
        visibility_reuse: state.p("restirVisibilityReuse") > 0.5,
    };
    let seed = state.p("restirSeed").max(0.0) as u32;
    let mut rng = Rng::new(UVec2::new(q[0] as u32, q[1] as u32), seed, 2);
    let rotation = [rng.rand(), rng.rand()];
    let own = words(&disc.reservoirs, q)
        .map(Reservoir::unpack)
        .unwrap_or(Reservoir::empty());
    let k = (state.p("restirNeighbours").round() as u32).min(16);
    let round = Round {
        q,
        s: &s,
        k,
        step: state.p("spatialStep").round().max(0.0) as u32,
        rotation,
        radius,
        size: [size[0] as u32, size[1] as u32],
        test: state.p("restirGeometryTest") > 0.5,
    };
    let mut sources = vec![(own, s)];
    let kept = neighbours(content, &round, &disc, &scene, &mut sources);
    let m: f32 = sources.iter().map(|(r, _)| r.m).sum();
    // The reservoir P holds once the sources so far are combined (the GPU's stream, Alg. 4), and its shadow ray.
    let combined = scene.combine(&s, &sources, &mut rng);
    if combined.y.light != cpu::NONE {
        let color = if scene.visible(&s, combined.y) {
            palette::SELECTED
        } else {
            super::BLOCKED
        };
        let at = sample_point(lights, combined.y);
        content.draw.segment(s.position, at, color, 1.0, 3.5, 0);
        content.draw.point(at, 8.0, color, 1.0);
    }
    content.draw.point(s.position, 10.0, palette::SELECTED, 1.0);
    content
        .draw
        .text(s.position, "P", palette::SELECTED, [1.2, -0.6]);
    content.readout = Some(Readout {
        title: tr("demos-spatial_title").into(),
        subtitle: tr_with("demos-spatial_subtitle", lab::args!(k = k, kept = kept)),
        formula: None,
        formula_lit: None,
        rows: vec![
            (
                tr("demos-own_m").into(),
                format!("{}", own.m as u32),
                palette::THEME_ACCENT,
            ),
            (
                tr("demos-combined_m").into(),
                format!("{}", m as u32),
                palette::SELECTED,
            ),
        ],
    });
}

/// P's round of spatial reuse as the dock sets it: its k neighbours, the first `step` of them combined so far.
struct Round<'a> {
    q: [i32; 2],
    s: &'a Surface,
    k: u32,
    step: u32,
    rotation: [f32; 2],
    radius: f32,
    size: [u32; 2],
    test: bool,
}

/// Draws P's neighbours (green if the test keeps them, red if it rejects them, dim until the step reaches them),
/// each kept one's line to its sample; pushes the kept ones the step has reached onto `sources`, and counts them.
fn neighbours(
    content: &mut FrameContent,
    round: &Round,
    disc: &Disc,
    scene: &Scene,
    sources: &mut Vec<(Reservoir, Surface)>,
) -> u32 {
    let mut kept = 0;
    for j in 0..round.k {
        // Each neighbour where it is in the room, as bright as the step has made it.
        let qn = cpu::neighbour(round.q, j, round.rotation, round.radius, round.size);
        let (Some(sn), Some(r)) = (
            disc.surface(qn),
            words(&disc.reservoirs, qn).map(Reservoir::unpack),
        ) else {
            continue;
        };
        if !sn.valid {
            continue;
        }
        let ok = qn != round.q && cpu::similar(round.test, round.s, &sn);
        let reached = j < round.step;
        let alpha = if reached {
            1.0
        } else {
            0.3
        };
        let color = if ok {
            palette::SELECTED
        } else {
            super::BLOCKED
        };
        content
            .draw
            .segment(round.s.position, sn.position, color, 0.8 * alpha, 2.0, 0);
        content.draw.point(sn.position, 7.0, color, alpha);
        if !ok {
            continue;
        }
        if reached {
            sources.push((r, sn));
        }
        if r.y.light != cpu::NONE {
            // The sample the neighbour's reservoir holds, which P may take.
            kept += 1;
            let color = if scene.visible(&sn, r.y) {
                palette::INCIDENT
            } else {
                super::BLOCKED
            };
            let at = sample_point(scene.lights, r.y);
            content
                .draw
                .segment(sn.position, at, color, 0.6 * alpha, 1.5, 0);
        }
    }
    kept
}

fn sample_point(lights: &[Light], y: Sample) -> Vec3 {
    let l = &lights[y.light as usize];
    l.center + l.radius * cpu::sample_dir(y.uv)
}
