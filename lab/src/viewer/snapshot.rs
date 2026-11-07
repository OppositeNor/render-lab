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

/// The viewer offscreen with its windows moved by the pointer (`home --snapshot X.png --page
/// viewer-layout`), as a user moves them (src/ui/windows.rs): the Panel window's title bar dragged off its
/// edge into the viewport (it floats there), the Controls window's top edge dragged 120 points up, and the
/// Panel window dragged back to the right edge (docked again) and then away once more. The layout is
/// written to X.toml (a settings file), not to the user's settings.
pub fn snapshot_layout(options: ViewerOptions, png: &std::path::Path) -> Result<()> {
    use crate::readback::{read_rgba, readback_buffer, write_png};
    use crate::ui::windows::WindowId;
    let settings = png.with_extension("toml");
    let _ = std::fs::remove_file(&settings);
    let (mut app, gpu, size) = offscreen_app(ViewerOptions { settings: Some(settings.clone()), layout: Default::default(), ..options }, None)?;
    app.state = app.default_state(app.state.demo);
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    settle(&mut app, &buffer, 3)?;
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
        // The drop's actions apply after the frame that saw it: one more, to lay it out.
        app.render(Some(&buffer))?;
        Ok(())
    };
    let describe = |l: &crate::ui::Layout| {
        WindowId::ALL.iter().filter_map(|id| l.window(*id)).map(|w| format!("{} {:?} {:.0}×{:.0} at ({:.0}, {:.0})", w.id.key(), w.dock, w.frame.width(), w.frame.height(), w.frame.min.x, w.frame.min.y)).collect::<Vec<_>>().join(", ")
    };
    let before = app.renderer().frame_layout();
    let mut steps = Vec::new();
    // The panel's title bar into the viewport: it floats where it is dropped.
    if let Some(panel) = before.window(WindowId::Panel) {
        let grab = panel.title.left_center() + egui::vec2(60.0, 0.0);
        drag(&mut app, grab, before.viewport.center())?;
        let l = app.renderer().frame_layout();
        steps.push(format!("panel dropped in the viewport: {:?}, viewport {:.0} wide", l.window(WindowId::Panel).map(|w| w.dock), l.viewport.width()));
    }
    // The controls' top edge up.
    if let Some(controls) = app.renderer().frame_layout().window(WindowId::Controls) {
        let edge = egui::pos2(controls.frame.center().x, controls.frame.min.y - crate::ui::windows::GAP / 2.0);
        drag(&mut app, edge, edge - egui::vec2(0.0, 120.0))?;
    }
    // The panel back to the right edge (docked), then into the viewport again for the picture.
    if let Some(panel) = app.renderer().frame_layout().window(WindowId::Panel) {
        let l = app.renderer().frame_layout();
        let grab = panel.title.left_center() + egui::vec2(60.0, 0.0);
        drag(&mut app, grab, egui::pos2(l.area.max.x - 10.0, l.area.center().y))?;
        let l = app.renderer().frame_layout();
        steps.push(format!("panel dropped at the right edge: {:?}", l.window(WindowId::Panel).map(|w| w.dock)));
        if let Some(panel) = l.window(WindowId::Panel) {
            let grab = panel.title.left_center() + egui::vec2(60.0, 0.0);
            drag(&mut app, grab, grab - egui::vec2(500.0, -120.0))?;
        }
    }
    settle(&mut app, &buffer, 16)?;
    let after = app.renderer().frame_layout();
    write_png(png, size[0], size[1], &read_rgba(&gpu.device, &gpu.queue, &buffer, size))?;
    let saved = std::fs::read_to_string(&settings).map(|t| t.lines().filter(|l| l.starts_with("viewer_window_")).collect::<Vec<_>>().join("; ")).unwrap_or_else(|e| format!("{e:#}"));
    eprintln!("viewer layout: {} ({}x{}); before: {}; {}; after: {}; saved to {}: {saved}", png.display(), size[0], size[1], describe(&before), steps.join("; "), describe(&after), settings.display());
    Ok(())
}

/// The viewer's camera driven offscreen through its own input handlers (`home --snapshot X.png --page
/// viewer-camera`), each mode checked: a left drag orbits about the target; a Shift-left drag pans it; the
/// right button held flies (a drag turns the view about the eye; W moves along the view, E up and Q down the
/// world's Y; the wheel sets the fly speed, not the distance). Fails, naming the check, when one does not
/// hold; the picture is the camera where the checks left it.
pub fn snapshot_camera(options: ViewerOptions, png: &std::path::Path) -> Result<()> {
    use crate::readback::{read_rgba, readback_buffer, write_png};
    let (mut app, gpu, size) = offscreen_app(ViewerOptions { settings: None, ..options }, None)?;
    app.state = app.default_state(app.state.demo);
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    settle(&mut app, &buffer, 2)?;
    let vp = app.renderer().frame_layout().viewport_px();
    let center = Vec2::new((vp[0] + vp[2] / 2) as f32, (vp[1] + vp[3] / 2) as f32);
    let mut checks: Vec<String> = Vec::new();
    let mut check = |ok: bool, what: String| -> Result<()> {
        anyhow::ensure!(ok, "viewer camera: {what}");
        checks.push(what);
        Ok(())
    };
    // A drag of `button` by `by` window pixels, a frame a step.
    let drag = |app: &mut App, button: MouseButton, by: Vec2| -> Result<()> {
        app.cursor_moved(center);
        app.render(Some(&buffer))?;
        app.mouse_button(button, true);
        for k in 1..=5 {
            app.cursor_moved(center + by * (k as f32 / 5.0));
            app.render(Some(&buffer))?;
        }
        app.mouse_button(button, false);
        app.render(Some(&buffer))?;
        Ok(())
    };
    let before = app.state.camera;
    drag(&mut app, MouseButton::Left, Vec2::new(120.0, 0.0))?;
    let orbit = app.state.camera;
    check(orbit.target == before.target && (orbit.yaw - before.yaw).abs() > 10.0 && (orbit.distance - before.distance).abs() < 1e-4, format!("left drag orbits: yaw {:.1} → {:.1}, target kept", before.yaw, orbit.yaw))?;
    app.modifiers.shift = true;
    drag(&mut app, MouseButton::Left, Vec2::new(120.0, 60.0))?;
    app.modifiers.shift = false;
    let pan = app.state.camera;
    let moved = (glam::Vec3::from(pan.target) - glam::Vec3::from(orbit.target)).length();
    check(pan.yaw == orbit.yaw && pan.pitch == orbit.pitch && moved > 0.1, format!("Shift-left drag pans: target moved {moved:.2} m, view kept"))?;
    // Fly: the right button held over the viewport.
    app.cursor_moved(center);
    app.render(Some(&buffer))?;
    app.mouse_button(MouseButton::Right, true);
    let eye = pan.eye();
    for k in 1..=5 {
        app.cursor_moved(center + Vec2::new(16.0 * k as f32, 0.0));
        app.render(Some(&buffer))?;
    }
    let look = app.state.camera;
    check(look.eye().distance(eye) < 1e-3 && (look.yaw - pan.yaw).abs() > 5.0, format!("right drag turns the view about the eye: yaw {:.1} → {:.1}, eye kept", pan.yaw, look.yaw))?;
    // Each key held for a few frames, through the keyboard's handler too (a fly key is not a shortcut then).
    let hold = |app: &mut App, code: KeyCode| -> Result<crate::camera::OrbitCamera> {
        app.held.push(code);
        app.key(code, None);
        for _ in 0..6 {
            std::thread::sleep(std::time::Duration::from_millis(15));
            app.render(Some(&buffer))?;
        }
        app.held.retain(|k| *k != code);
        app.render(Some(&buffer))?;
        Ok(app.state.camera)
    };
    let a = app.state.camera;
    let w = hold(&mut app, KeyCode::KeyW)?;
    let (forward, _) = a.forward_right();
    let step = w.eye() - a.eye();
    check(step.length() > 1e-3 && step.normalize().dot(forward) > 0.999 && w.yaw == a.yaw, format!("W flies along the view: {:.3} m", step.length()))?;
    let e = hold(&mut app, KeyCode::KeyE)?;
    let up = e.eye() - w.eye();
    check(up.y > 1e-3 && up.x.abs() < 1e-5 && up.z.abs() < 1e-5, format!("E flies up the world's Y: {:.3} m (pitch {:.1}°)", up.y, e.pitch))?;
    let q = hold(&mut app, KeyCode::KeyQ)?;
    let down = q.eye() - e.eye();
    check(down.y < -1e-3 && down.x.abs() < 1e-5 && down.z.abs() < 1e-5, format!("Q flies down the world's Y: {:.3} m", down.y))?;
    let speed = app.fly_speed;
    app.wheel(3.0);
    check(app.fly_speed > speed && app.state.camera.distance == q.distance, format!("the wheel while flying sets the speed: {speed:.2} → {:.2} m/s, distance kept", app.fly_speed))?;
    app.mouse_button(MouseButton::Right, false);
    // Released, the keys and the wheel are the viewer's again.
    app.held.push(KeyCode::KeyW);
    app.render(Some(&buffer))?;
    app.held.clear();
    check(app.state.camera.eye().distance(q.eye()) < 1e-4, "after the right button is let go, W no longer moves the camera".into())?;
    settle(&mut app, &buffer, 4)?;
    write_png(png, size[0], size[1], &read_rgba(&gpu.device, &gpu.queue, &buffer, size))?;
    eprintln!("viewer camera: {} ({}x{}); every check held: {}", png.display(), size[0], size[1], checks.join("; "));
    Ok(())
}

/// The viewer offscreen with its Scene window open (`home --snapshot X.png --page
/// viewer-scene[:<entity>][@move|rotate|scale]`): the entity (its `Entity::id`, `object.teapot`; by default
/// the first that moves) selected, and its gizmo's handle dragged by the pointer as a user drags it (the
/// first arrow, the ring, or the scale handle), then the pointer left over the handle, lit.
pub fn snapshot_scene(options: ViewerOptions, entity: Option<&str>, mode: crate::gizmo::Mode, png: &std::path::Path) -> Result<()> {
    use crate::gizmo::Mode;
    use crate::readback::{read_rgba, readback_buffer, write_png};
    use crate::ui::windows::WindowId;
    let mut layout = crate::config::ViewerLayout::default();
    layout.window_mut(WindowId::Scene).collapsed = false;
    let (mut app, gpu, size) = offscreen_app(ViewerOptions { layout, settings: None, ..options }, None)?;
    app.state = app.default_state(app.state.demo);
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    settle(&mut app, &buffer, 2)?;
    let entities = crate::entity::list(&app.state, &app.renderer().statics);
    let chosen = match entity {
        Some(id) => entities.iter().find(|e| e.id == id).ok_or_else(|| anyhow::anyhow!("no entity `{id}` (the scene's: {})", entities.iter().map(|e| e.id.as_str()).collect::<Vec<_>>().join(", ")))?,
        None => entities.iter().find(|e| e.movable() && e.visible).ok_or_else(|| anyhow::anyhow!("nothing in the scene moves"))?,
    }
    .clone();
    app.apply(ViewerAction::Select(Some(chosen.id.clone())));
    app.apply(ViewerAction::SetGizmo(mode));
    settle(&mut app, &buffer, 2)?;
    // The handle's place on screen, in window pixels.
    let vp = app.renderer().frame_layout().viewport_px();
    let origin = Vec2::new(vp[0] as f32, vp[1] as f32);
    let values = |app: &App| chosen.position.iter().flatten().chain(&chosen.yaw).chain(&chosen.scale).map(|k| format!("{k} {:.2}", app.state.try_get(k).map_or(f32::NAN, |v| v.f32()))).collect::<Vec<_>>().join(", ");
    let before = values(&app);
    let mut dragged = String::from("no gizmo (it does not move, or is off screen)");
    let probe = origin + Vec2::new(vp[2] as f32, vp[3] as f32) * 0.5;
    if let Some((shape, _, _)) = app.gizmo_shape(probe) {
        let (grab, to) = match (mode, chosen.yaw.is_some()) {
            (Mode::Rotate, true) => (shape.ring[0], shape.ring[6]),
            (Mode::Scale, true) => (shape.scale, shape.center + (shape.scale - shape.center) * 1.5),
            _ => {
                let tip = shape.tips[0].unwrap_or(shape.center + Vec2::X * 80.0);
                let grab = shape.center.lerp(tip, 0.6);
                (grab, grab + (tip - shape.center) * 0.5)
            }
        };
        let (grab, to) = (origin + grab, origin + to);
        app.cursor_moved(grab);
        app.render(Some(&buffer))?;
        let hovered = app.gizmo_hover;
        app.mouse_button(MouseButton::Left, true);
        for k in 1..=8 {
            app.cursor_moved(grab + (to - grab) * (k as f32 / 8.0));
            app.render(Some(&buffer))?;
        }
        app.mouse_button(MouseButton::Left, false);
        app.render(Some(&buffer))?;
        // Back over the handle, as the pointer would rest: it lights up.
        app.cursor_moved(to);
        dragged = format!("handle {hovered:?} dragged {:.0} px", (to - grab).length());
    }
    settle(&mut app, &buffer, 8)?;
    write_png(png, size[0], size[1], &read_rgba(&gpu.device, &gpu.queue, &buffer, size))?;
    eprintln!("viewer scene: {} ({}x{}); {} entities, {} selected ({mode:?}); {dragged}; before: {before}; after: {}", png.display(), size[0], size[1], entities.len(), chosen.id, values(&app));
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
