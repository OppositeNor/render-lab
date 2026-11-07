//! The viewer without a window, for `--snapshot` pages: a feature's (src/feature/: each step it takes), and
//! the viewer's own (its first demonstration, the timeline playing or paused, the free camera, the layout).

use super::*;

/// The viewer offscreen, as a PNG (`home --snapshot --page viewer`): the
/// current investigation's first demonstration with its dock, or, with
/// `options.timeline`, the timeline playing at `options.start_frame` with its
/// bar; `demo` (a key) chooses the demonstration. No window opens and no
/// sound plays.
pub fn snapshot(options: ViewerOptions, demo: Option<&str>, png: &std::path::Path) -> Result<()> {
    snapshot_page(options, demo, None, png)
}

/// A 2D scene a snapshot pauses the timeline on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PausedPage {
    /// The first 2D scene with a diagram, its panel zoomed in (`--page viewer-diagram`).
    Diagram,
    /// The first sources card (`--page viewer-sources`).
    Sources,
}

/// `frames` frames of an offscreen viewer drawn into `buffer`, each half a
/// second after the one before (at 60 fps), so what eases in has settled.
pub(super) fn settle(app: &mut App, buffer: &wgpu::Buffer, frames: usize) -> Result<()> {
    for _ in 0..frames {
        app.render(Some(buffer))?;
        app.frames += 30;
    }
    Ok(())
}

/// The viewer without a window, on `demo` (its key) or the first
/// demonstration, its scene loaded: for a snapshot.
pub(super) fn offscreen_app(options: ViewerOptions, demo: Option<&str>) -> Result<(App, Gpu, [u32; 2])> {
    let size = [options.width.max(64), options.height.max(64)];
    let mut app = App::new(ViewerOptions { audio: None, ..options })?;
    app.intro = None;
    if let Some(key) = demo {
        app.state.demo = *investigation::current().demos().iter().find(|d| d.key() == key).ok_or_else(|| anyhow::anyhow!("no demonstration `{key}`"))?;
    }
    let gpu = Gpu::new(gpu::instance(None), None)?;
    let scene = investigation::current().demo_scene(app.state.demo);
    let mut renderer = Loading::start(investigation::current(), gpu.clone(), scene, size).wait()?;
    renderer.ui_scale = app.options.ui_scale;
    app.renderer = Some(renderer);
    Ok((app, gpu, size))
}

/// The viewer offscreen on a feature's tab (src/feature/), as its homepage button opens it, at a snapshot page
/// of it (`home --snapshot … --page <page>[:<args>]`): on the demonstration the page names, each step the
/// feature takes rendered, until it is done.
pub fn snapshot_feature(options: ViewerOptions, feature: &'static dyn crate::feature::Feature, page: &str, args: Option<&str>, png: &std::path::Path) -> Result<()> {
    use crate::feature::Snapshot;
    use crate::readback::{read_rgba, readback_buffer, write_png};
    // A feature with no tab (the performance panel's) opens on the viewer's first.
    let tab = feature.label().map(|_| DockTab(feature.id()));
    let (mut app, gpu, size) = offscreen_app(ViewerOptions { tab, ..options }, feature.snapshot_demo(page, args))?;
    anyhow::ensure!(app.features.feature(feature.id()).is_some(), "{} has no {}", investigation::current().title(), feature.id());
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    let mut step = 0;
    let shown = loop {
        let mut next = Ok(Snapshot::Done(String::new()));
        app.with_features(|features, host| {
            if let Some(state) = features.state_mut(feature.id()) {
                next = state.snapshot(page, args, step, host);
            }
        });
        match next? {
            Snapshot::Render => app.render(Some(&buffer))?,
            Snapshot::Settle => settle(&mut app, &buffer, 1)?,
            Snapshot::Done(shown) => break shown,
        }
        step += 1;
    };
    write_png(png, size[0], size[1], &read_rgba(&gpu.device, &gpu.queue, &buffer, size))?;
    eprintln!("viewer {}: {} ({}x{}){}", feature.id(), png.display(), size[0], size[1], if shown.is_empty() { String::new() } else { format!("; {shown}") });
    Ok(())
}

/// The viewer offscreen with the free camera on (`home --snapshot … --page
/// viewer-free[:<demo>]`): the scene from beside the main camera, which is
/// drawn with its view cone, and the main camera's own view in a corner.
pub fn snapshot_free(options: ViewerOptions, demo: Option<&str>, png: &std::path::Path) -> Result<()> {
    use crate::readback::{read_rgba, readback_buffer, write_png};
    let (mut app, gpu, size) = offscreen_app(options, demo)?;
    app.state = app.default_state(app.state.demo);
    app.apply(ViewerAction::ToggleFreeCamera);
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    settle(&mut app, &buffer, 8)?;
    write_png(png, size[0], size[1], &read_rgba(&gpu.device, &gpu.queue, &buffer, size))?;
    let free = app.render_state().free_camera;
    eprintln!("viewer free camera: {} ({}x{}); {:?}, free camera yaw {:.0} pitch {:.0} distance {:.1}, main view {:?}", png.display(), size[0], size[1], app.state.demo, free.yaw, free.pitch, free.distance, app.renderer().last_main_view);
    Ok(())
}

/// The viewer offscreen with its edges dragged by the pointer (`home
/// --snapshot X.png --page viewer-layout`): the panel's edge 200 points left,
/// the dock's edge 120 points up, as a user drags them. The layout is written
/// to X.toml (a settings file), not to the user's settings.
pub fn snapshot_layout(options: ViewerOptions, png: &std::path::Path) -> Result<()> {
    use crate::readback::{read_rgba, readback_buffer, write_png};
    let settings = png.with_extension("toml");
    let _ = std::fs::remove_file(&settings);
    let (mut app, gpu, size) = offscreen_app(ViewerOptions { settings: Some(settings.clone()), layout: Default::default(), ..options }, None)?;
    app.state = app.default_state(app.state.demo);
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    settle(&mut app, &buffer, 3)?;
    let before = app.renderer().frame_layout();
    let press = |app: &mut App, at: egui::Pos2, pressed: bool| app.egui_events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default() });
    // A drag from `from` to `to` in steps, a frame each, as egui sees a pointer.
    let drag = |app: &mut App, from: egui::Pos2, to: egui::Pos2| -> Result<()> {
        app.egui_events.push(egui::Event::PointerMoved(from));
        app.render(Some(&buffer))?;
        press(app, from, true);
        app.render(Some(&buffer))?;
        for k in 1..=6 {
            app.egui_events.push(egui::Event::PointerMoved(from + (to - from) * (k as f32 / 6.0)));
            app.render(Some(&buffer))?;
        }
        press(app, to, false);
        app.render(Some(&buffer))?;
        Ok(())
    };
    let panel_edge = egui::pos2((before.viewport.max.x + before.panel.min.x) / 2.0, before.content.center().y);
    if before.viewport.max.x + 1.0 < before.panel.min.x {
        drag(&mut app, panel_edge, panel_edge - egui::vec2(200.0, 0.0))?;
    }
    let dock_edge = egui::pos2(before.captions.center().x, (before.content.max.y + before.captions.min.y) / 2.0);
    drag(&mut app, dock_edge, dock_edge - egui::vec2(0.0, 120.0))?;
    settle(&mut app, &buffer, 16)?;
    let after = app.renderer().frame_layout();
    write_png(png, size[0], size[1], &read_rgba(&gpu.device, &gpu.queue, &buffer, size))?;
    let width = |l: &crate::ui::Layout| l.panel.max.x - l.panel.min.x;
    let dock = |l: &crate::ui::Layout| l.captions.max.y - l.captions.min.y;
    let saved = crate::config::Config::load(&settings).map(|c| format!("{:?}", c.layout)).unwrap_or_else(|e| format!("{e:#}"));
    eprintln!("viewer layout: {} ({}x{}); panel {:.0} → {:.0} points, dock {:.0} → {:.0} points; saved to {}: {saved}", png.display(), size[0], size[1], width(&before), width(&after), dock(&before), dock(&after), settings.display());
    Ok(())
}

/// `snapshot`, or with `paused`, the timeline paused on its first 2D scene of
/// that page (`home --snapshot … --page viewer-diagram` or `viewer-sources`).
pub fn snapshot_page(options: ViewerOptions, demo: Option<&str>, paused: Option<PausedPage>, png: &std::path::Path) -> Result<()> {
    use crate::readback::{read_rgba, readback_buffer, write_png};
    let playing = options.timeline.is_some() && paused.is_none();
    let (mut app, gpu, size) = offscreen_app(options, demo)?;
    if playing {
        app.playing = true;
        // The clock holds the frame: it starts counting an hour from now.
        app.timeline_anchor = (Instant::now() + std::time::Duration::from_secs(3600), app.timeline_frame);
    } else if let Some(page) = paused {
        let wanted = |b: &crate::shot::TimelineBeat| b.kind == SceneType::TwoD && b.shows_diagram() == (page == PausedPage::Diagram);
        let beat = app.timeline.as_ref().and_then(|t| t.beats.iter().find(|b| wanted(b))).ok_or_else(|| anyhow::anyhow!("the timeline has no {page:?} 2D scene"))?;
        app.timeline_frame = beat.start_frame + beat.frames / 2;
        app.timeline_mode = true;
        app.playing = false;
        app.render(None)?;
        if page == PausedPage::Diagram {
            app.diagram_preset = Some((2.5, egui::vec2(0.35, 0.5)));
        }
    } else {
        app.state = app.default_state(app.state.demo);
    }
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    // egui sizes its areas on the first passes, and fades them in over its
    // clock; a still view's history (FrameRequest::history) fills over 16 frames.
    settle(&mut app, &buffer, 16)?;
    write_png(png, size[0], size[1], &read_rgba(&gpu.device, &gpu.queue, &buffer, size))?;
    eprintln!("viewer: {} ({}x{}, {})", png.display(), size[0], size[1], match (playing, paused) { (true, _) => "the timeline playing", (_, Some(PausedPage::Diagram)) => "paused on a diagram", (_, Some(PausedPage::Sources)) => "paused on a sources card", _ => "the first demonstration" });
    Ok(())
}
