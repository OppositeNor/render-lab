//! The viewer without a window, for `--snapshot` pages: the experiments,
//! performance, free-camera, code, prerequisites, layout, paused, and graph
//! pages, and the inspector's resource view.

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

/// The viewer offscreen in experiments mode (`home --snapshot … --page
/// viewer-experiments` or `viewer-compare`): the Experiments tab; with
/// `compare`, the demonstration's state set as the reference, then one engine
/// setting (else any ranged value) moved to its other end, compared with a
/// wipe and measured.
pub fn snapshot_experiments(options: ViewerOptions, demo: Option<&str>, compare: bool, png: &std::path::Path) -> Result<()> {
    use crate::readback::{read_rgba, readback_buffer, write_png};
    let (mut app, gpu, size) = offscreen_app(ViewerOptions { experiments: true, ..options }, demo)?;
    app.state = app.default_state(app.state.demo);
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    let mut changed = None;
    if compare {
        app.apply(ViewerAction::SetReference);
        let entries = crate::values::list();
        let camera = crate::camera::OrbitCamera::schema();
        let ranged = |e: &&crate::values::Entry| e.range.is_some() && e.components.is_empty() && e.choices.is_none() && !camera.iter().any(|c| std::ptr::eq(c, *e));
        let entry = entries.iter().find(|e| e.engine.is_some() && ranged(e)).or_else(|| entries.iter().find(|e| ranged(e) && !e.labels.is_empty()));
        if let Some(e) = entry {
            let (lo, hi) = e.range.expect("ranged");
            let now = app.state.p(e.key);
            let to = if (now - lo).abs() > (now - hi).abs() { lo } else { hi };
            app.apply(ViewerAction::SetParam(e.key, to));
            changed = Some(format!("{} {now} → {to}", e.key));
        }
        app.apply(ViewerAction::SetCompare(crate::experiment::Compare::Wipe));
        // A setting baked into the scene data: its build waited for, so the snapshot shows it.
        let state = app.state.clone();
        app.renderer.as_mut().expect("renderer").prepare_scene_data(&state);
    }
    for i in 0..18 {
        if compare && i == 15 {
            app.apply(ViewerAction::Measure);
        }
        app.render(Some(&buffer))?;
        app.frames += 30;
    }
    write_png(png, size[0], size[1], &read_rgba(&gpu.device, &gpu.queue, &buffer, size))?;
    let measured = app.experiment.measurement.map(|m| format!("; PSNR {:.2} dB, SSIM {:.4}, FLIP {:.4}", m.psnr, m.ssim, m.flip)).unwrap_or_default();
    eprintln!("viewer experiments: {} ({}x{}){}{measured}", png.display(), size[0], size[1], changed.map(|c| format!("; reference compared with {c}")).unwrap_or_default());
    Ok(())
}

/// The viewer offscreen on the Quiz tab (`home --snapshot … --page viewer-quiz[:<question>[@<choice>]]`): the quiz
/// opened, at `question` (from 0; past the last: the score, every question answered with its first choice), with
/// `choice` picked when given.
pub fn snapshot_quiz(options: ViewerOptions, question: usize, choice: Option<usize>, png: &std::path::Path) -> Result<()> {
    use crate::readback::{read_rgba, readback_buffer, write_png};
    let (mut app, gpu, size) = offscreen_app(ViewerOptions { quiz: true, ..options }, None)?;
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    app.render(Some(&buffer))?;
    if let Some(e) = &app.quiz.view.error {
        anyhow::bail!("the quiz did not open: {e}");
    }
    let count = app.quiz.view.questions.len();
    if question >= count {
        for i in 0..count {
            app.apply(ViewerAction::QuizGo(i));
            app.apply(ViewerAction::QuizAnswer(0));
        }
    }
    app.apply(ViewerAction::QuizGo(question.min(count)));
    if let Some(c) = choice {
        app.apply(ViewerAction::QuizAnswer(c));
    }
    settle(&mut app, &buffer, 12)?;
    write_png(png, size[0], size[1], &read_rgba(&gpu.device, &gpu.queue, &buffer, size))?;
    let at = if question >= count { "the score".to_string() } else { format!("question {} of {count}", question + 1) };
    eprintln!("viewer quiz: {} ({}x{}); {at}{}", png.display(), size[0], size[1], app.quiz.view.error.as_ref().map(|e| format!("; {e}")).unwrap_or_default());
    Ok(())
}

/// The viewer offscreen with the performance panel (`home --snapshot X.png
/// --page viewer-performance[:<demo>]`): measured over 60 frames, then the
/// most expensive pass called out, as a story's `highlight` would.
pub fn snapshot_performance(options: ViewerOptions, demo: Option<&str>, png: &std::path::Path) -> Result<()> {
    use crate::readback::{read_rgba, readback_buffer, write_png};
    let (mut app, gpu, size) = offscreen_app(options, demo)?;
    app.state = app.default_state(app.state.demo);
    app.state.panel = PanelMode::Performance;
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    settle(&mut app, &buffer, 60)?;
    let passes = app.renderer().timing.as_ref().map(|t| t.summary()).unwrap_or_default();
    let top = passes.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|p| p.0.clone());
    if let Some(name) = &top {
        app.state.highlight = vec![name.clone()];
    }
    settle(&mut app, &buffer, 4)?;
    write_png(png, size[0], size[1], &read_rgba(&gpu.device, &gpu.queue, &buffer, size))?;
    let list: Vec<String> = passes.iter().map(|(n, mean, _)| format!("{n} {mean:.2} ms")).collect();
    eprintln!("viewer performance: {} ({}x{}); {}; called out: {}", png.display(), size[0], size[1], if list.is_empty() { "no GPU timestamps".into() } else { list.join(", ") }, top.unwrap_or_else(|| "none".into()));
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

/// The viewer offscreen on its Code tab (`home --snapshot … --page
/// viewer-code:<listing>[@<tag>]`), showing `listing` (its name) on its
/// demonstration, with the lines of `tag` lit (default: its first tag), in
/// the listing style in use (`--listing-style`).
pub fn snapshot_code(options: ViewerOptions, listing: &str, tag: Option<&str>, png: &std::path::Path) -> Result<()> {
    use crate::readback::{read_rgba, readback_buffer, write_png};
    let current = investigation::current();
    let found = current.listing(listing).ok_or_else(|| {
        let names: Vec<&str> = current.listings().iter().map(|l| l.name).collect();
        anyhow::anyhow!("no listing `{listing}`; known: {}", names.join(", "))
    })?;
    let (mut app, gpu, size) = offscreen_app(options, found.demo.as_deref())?;
    app.state = app.default_state(app.state.demo);
    app.tab = DockTab::Code;
    app.show_tab();
    app.state.code = Some(found.name.to_string());
    let tag = tag.map(str::to_string).or_else(|| found.lines.iter().flat_map(|l| l.tags.iter()).next().cloned());
    app.state.highlight = tag.iter().cloned().collect();
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    settle(&mut app, &buffer, 16)?;
    write_png(png, size[0], size[1], &read_rgba(&gpu.device, &gpu.queue, &buffer, size))?;
    eprintln!("viewer code: {} ({}x{}), listing {listing} as {}, lit: {}", png.display(), size[0], size[1], crate::pseudocode::style().key(), tag.unwrap_or_else(|| "nothing".into()));
    Ok(())
}

/// The viewer offscreen on its Prerequisites tab (`home --snapshot … --page
/// viewer-prerequisites[:<demo>]`), the knowledge graph in the panel fitted to
/// the demonstration's chapters; with `focus` (`viewer-knowledge[:<demo>]`), the
/// panel zoomed onto the first topic of the audience's level, selected; with
/// `overlay` (`viewer-knowledge-graph[:<demo>]`), the graph full screen.
pub fn snapshot_prerequisites(options: ViewerOptions, demo: Option<&str>, focus: bool, overlay: bool, png: &std::path::Path) -> Result<()> {
    use crate::readback::{read_rgba, readback_buffer, write_png};
    let (mut app, gpu, size) = offscreen_app(options, demo)?;
    app.state = app.default_state(app.state.demo);
    let graph = crate::knowledge::current().ok_or_else(|| anyhow::anyhow!("{} has no knowledge graph (its package's knowledge.toml)", investigation::current().title()))?;
    app.tab = DockTab::Prerequisites;
    app.show_tab();
    app.knowledge_overlay = overlay;
    if focus {
        let chapters = app.knowledge_chapters();
        let context = graph.context(&chapters);
        let p = graph.audience();
        app.knowledge_focus = p.level.iter().find(|&&i| context[i]).or(p.level.first()).map(|&i| graph.nodes[i].id.clone());
    }
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    settle(&mut app, &buffer, 16)?;
    write_png(png, size[0], size[1], &read_rgba(&gpu.device, &gpu.queue, &buffer, size))?;
    let chapters = app.knowledge_chapters();
    eprintln!("viewer prerequisites: {} ({}x{}); demonstration {}, chapters {:?}{}", png.display(), size[0], size[1], app.state.demo.key(), chapters, app.knowledge_focus.as_ref().map(|f| format!(", focused on {f}")).unwrap_or_default());
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

/// The graph panel's inspector (`ViewerAction::Inspect`): the resource
/// `<node>.<resource>` in the top-right inset (the image `graph`), its node
/// called out; none hides it.
pub fn inspect(state: &mut ShotState, key: Option<String>) {
    use crate::utility::inset::{Corner, ImageSource, StoryImage};
    state.images.retain(|i| i.name != INSPECTOR);
    state.highlight.clear();
    let Some(key) = key else { return };
    if let Some((node, _)) = key.rsplit_once('.') {
        state.highlight = vec![node.to_string()];
    }
    state.images.push(StoryImage { name: INSPECTOR.to_string(), source: ImageSource::Graph { key, show: None, params: Default::default() }, title: None, corner: Corner::TopRight, alpha: 1.0, click: None, mark: [0.0; 4], mark_alpha: 0.0, mark_text: None });
}

/// The name of the graph panel's inspector image.
pub const INSPECTOR: &str = "graph";

/// How a graph snapshot shows the frame graph: the grid in the panel, the
/// DAG in the panel, or the DAG full screen; the DAG's called-out nodes.
#[derive(Clone, Debug, Default)]
pub struct GraphSnapshot {
    pub dag: bool,
    /// The resources' cards instead of the DAG.
    pub resources: bool,
    /// The selected pass's WGSL beside the full-screen DAG (`viewer-source:<node>`).
    pub source: bool,
    pub full_screen: bool,
    pub highlight: Vec<String>,
}

/// The viewer offscreen with the frame graph in the panel (`home --snapshot …
/// --page viewer-graph[:<demo>]`, or `viewer-inspect:<node>.<resource>[/<show>]`;
/// `viewer-dag[:<demo>][@<node>,…]` and `viewer-dag-full[:<demo>][@<node>,…]`: the DAG):
/// the inspector showing `inspect`, or the first resource the most expensive
/// pass writes.
pub fn snapshot_graph(options: ViewerOptions, demo: Option<&str>, inspect_key: Option<&str>, how: GraphSnapshot, png: &std::path::Path) -> Result<()> {
    use crate::readback::{read_rgba, readback_buffer, write_png};
    let (mut app, gpu, size) = offscreen_app(options, demo)?;
    app.state = app.default_state(app.state.demo);
    app.state.panel = if how.resources { PanelMode::Resources } else if how.dag { PanelMode::Dag } else { PanelMode::Graph };
    app.dag_overlay = how.full_screen;
    app.graph_source = how.source;
    app.tab = DockTab::Graph;
    let graph = crate::graph::FrameGraph::current(&[]);
    if let Some(unknown) = how.highlight.iter().find(|h| graph.node(h).is_none()) {
        anyhow::bail!("no node `{unknown}` (nodes: {})", graph.nodes.iter().map(|n| n.name).collect::<Vec<_>>().join(", "));
    }
    let buffer = readback_buffer(&gpu.device, size[0], size[1]);
    settle(&mut app, &buffer, 30)?;
    // `?name=v,v&…`: the inspector's parameters (`graph::INSPECT_PARAMS`).
    let (inspect_key, query) = match inspect_key.and_then(|k| k.split_once('?')) {
        Some((k, q)) => (Some(k), q),
        None => (inspect_key, ""),
    };
    let params = crate::graph::parse_params(query).map_err(|e| anyhow::anyhow!(e))?;
    let (key, show) = match inspect_key {
        Some(k) => match k.split_once('/') {
            Some((key, show)) => (key.to_string(), Some(show.to_string())),
            None => (k.to_string(), None),
        },
        None => {
            // The most expensive live pass that writes something of its own.
            let passes = app.renderer().timing.as_ref().map(|t| t.summary()).unwrap_or_default();
            let cost = |n: &crate::graph::Node| passes.iter().find(|p| p.0 == n.timing).map_or(0.0, |p| p.1);
            let node = graph.order.iter().map(|&i| &graph.nodes[i]).filter(|n| !n.writes.is_empty() && n.writes[0] != "frame").max_by(|a, b| cost(a).total_cmp(&cost(b))).ok_or_else(|| anyhow::anyhow!("no node writes anything"))?;
            (format!("{}.{}", node.name, node.writes[0]), None)
        }
    };
    graph.inspected(&key).map_err(|e| anyhow::anyhow!(e))?;
    if let Some(show) = &show {
        crate::graph::Show::from_key(show).map_err(|e| anyhow::anyhow!(e))?;
    }
    inspect(&mut app.state, Some(key.clone()));
    // The page's call-outs over the inspector's.
    if !how.highlight.is_empty() {
        app.state.highlight = how.highlight.clone();
    }
    if let Some(crate::utility::inset::ImageSource::Graph { show: s, params: p, .. }) = app.state.images.iter_mut().find(|i| i.name == INSPECTOR).map(|i| &mut i.source) {
        *s = show;
        *p = params;
    }
    settle(&mut app, &buffer, 4)?;
    write_png(png, size[0], size[1], &read_rgba(&gpu.device, &gpu.queue, &buffer, size))?;
    eprintln!("viewer graph: {} ({}x{}); {} nodes, {} resources; inspecting {key}", png.display(), size[0], size[1], graph.nodes.len(), graph.resources.len());
    Ok(())
}
