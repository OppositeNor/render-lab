//! Temporal and visibility reuse at P (Alg. 5 lines 7-14): P's reservoir read
//! back after visibility and after temporal reuse, its M growing frame by frame
//! up to the clamp, and the shadow ray from P to the sample it keeps, green
//! where the room lets it through and red where an object blocks it (cpu.rs
//! `visible`, the same test as the GPU's).

use super::super::cpu::{self, Reservoir, Scene};
use super::super::lights::{self, Light};
use super::ris;
use super::spatial::words;
use crate::paper::restir::text::tr;
use glam::Vec3;
use lab::fetch::{Region, Source};
use lab::frame::{FrameContent, Readout};
use lab::palette;
use lab::shot::ShotState;

/// Draws P's shadow ray to its sample after temporal reuse, and a readout of its reservoir before and after.
pub fn draw(
    state: &ShotState,
    content: &mut FrameContent,
    placed: &[Light],
    meshes: &lab::scene::Meshes,
) {
    let size = content.uniforms.camera.viewport_size;
    let q = ris::pixel(state, size);
    let region = Region::around(q, 0);
    // Every read is asked for before any is used, so one frame's reads bring them together.
    let visible = content.fetch("restir-reuse-visible", Source::image("visible", 0), region);
    let temporal = content.fetch(
        "restir-reuse-temporal",
        Source::image("temporal", 0),
        region,
    );
    let surface = ris::surface(content, q);
    let (Some(visible), Some(temporal), Some(s)) = (visible, temporal, surface) else {
        content.readout = Some(ris::waiting());
        return;
    };
    let before = words(&visible, q)
        .map(Reservoir::unpack)
        .unwrap_or(Reservoir::empty());
    let after = words(&temporal, q)
        .map(Reservoir::unpack)
        .unwrap_or(Reservoir::empty());
    let cdf = lights::cdf(placed);
    let triangles = cpu::occluders(meshes, &content.objects);
    let scene = Scene {
        lights: placed,
        cdf: &cdf,
        eye: Vec3::from(content.uniforms.camera.position),
        triangles: &triangles,
        candidates: 1,
        unbiased: false,
        visibility_reuse: true,
    };
    if after.y.light != cpu::NONE {
        // The shadow ray to the kept sample, coloured by what it meets.
        let clear = scene.visible(&s, after.y);
        let color = if clear {
            palette::SELECTED
        } else {
            egui::Color32::from_rgb(235, 80, 70)
        };
        content
            .draw
            .segment(s.position, scene.point(after.y), color, 1.0, 3.0, 0);
        content.draw.point(scene.point(after.y), 8.0, color, 1.0);
    }
    content.draw.point(s.position, 10.0, palette::SELECTED, 1.0);
    content
        .draw
        .text(s.position, "P", palette::SELECTED, [1.2, -0.6]);
    content.readout = Some(Readout {
        title: tr("demos-reuse_title").into(),
        subtitle: tr("demos-reuse_subtitle").into(),
        formula: None,
        formula_lit: None,
        rows: vec![
            (
                tr("demos-m_before").into(),
                format!("{}", before.m as u32),
                palette::THEME_ACCENT,
            ),
            (
                tr("demos-w_before").into(),
                format!("{:.4}", before.w),
                palette::THEME_ACCENT,
            ),
            (
                tr("demos-m_after").into(),
                format!("{}", after.m as u32),
                palette::SELECTED,
            ),
            (
                tr("demos-w_after").into(),
                format!("{:.4}", after.w),
                palette::SELECTED,
            ),
        ],
    });
}
