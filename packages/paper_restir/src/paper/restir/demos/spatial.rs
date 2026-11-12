//! Spatial reuse at P (Alg. 4 and Alg. 5 lines 16-20): P's k neighbours for the
//! first round, drawn where they are in the room, green where the geometry test
//! keeps them and red where it rejects them, each with a line to the sample its
//! reservoir holds; the readout sums the M that P's combined reservoir stands
//! for. The neighbours are the GPU's (cpu.rs `neighbour`, the frozen seed).

use super::super::cpu::{self, Reservoir, Rng, Sample, Surface};
use super::super::lights::Light;
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
pub fn draw(state: &ShotState, content: &mut FrameContent, lights: &[Light]) {
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
    let seed = state.p("restirSeed").max(0.0) as u32;
    let mut rng = Rng::new(UVec2::new(q[0] as u32, q[1] as u32), seed, 2);
    let rotation = [rng.rand(), rng.rand()];
    let test = state.p("restirGeometryTest") > 0.5;
    let own = words(&disc.reservoirs, q)
        .map(Reservoir::unpack)
        .unwrap_or(Reservoir::empty());
    let (mut kept, mut m) = (0, own.m);
    let k = (state.p("restirNeighbours").round() as u32).min(16);
    for j in 0..k {
        // Each neighbour where it is in the room: green if the test keeps it, red if it rejects it.
        let qn = cpu::neighbour(q, j, rotation, radius, [size[0] as u32, size[1] as u32]);
        let (Some(sn), Some(r)) = (
            disc.surface(qn),
            words(&disc.reservoirs, qn).map(Reservoir::unpack),
        ) else {
            continue;
        };
        if !sn.valid {
            continue;
        }
        let ok = qn != q && cpu::similar(test, &s, &sn);
        let color = if ok {
            palette::SELECTED
        } else {
            egui::Color32::from_rgb(235, 80, 70)
        };
        content
            .draw
            .segment(s.position, sn.position, color, 0.8, 2.0, 0);
        content.draw.point(sn.position, 7.0, color, 1.0);
        if ok && r.y.light != cpu::NONE {
            // The sample the neighbour's reservoir holds, which P may now take.
            kept += 1;
            m += r.m;
            content.draw.segment(
                sn.position,
                sample_point(lights, r.y),
                palette::INCIDENT,
                0.6,
                1.5,
                0,
            );
        }
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

fn sample_point(lights: &[Light], y: Sample) -> Vec3 {
    let l = &lights[y.light as usize];
    l.center + l.radius * cpu::sample_dir(y.uv)
}
