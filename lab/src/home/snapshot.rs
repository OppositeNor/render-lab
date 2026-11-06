//! The homepage without a window: its pages as PNGs, the viewer as the
//! homepage opens it, and the autotest that clicks through its controls.

use super::*;

/// The homepage on an offscreen target, for `--snapshot` and `--autotest`.
pub(super) struct Offscreen {
    pub(super) backdrop: Backdrop,
    pub(super) painter: Painter,
    pub(super) texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    pub(super) buffer: wgpu::Buffer,
    pub(super) size: [u32; 2],
    pub(super) ui_scale: f32,
    pub(super) frames: u64,
    /// Controls clicked, and how many of those clicks (and of the hovers before
    /// them) would have played the click sound.
    pub(super) clicks: usize,
    pub(super) heard: usize,
    pub(super) hovers_heard: usize,
}

impl Offscreen {
    /// `wait`: load the backdrop first, so every frame shows the page on it.
    pub(super) fn new(options: &HomeOptions, wait: bool) -> Result<Offscreen> {
        let gpu = Gpu::new(gpu::instance(None), None)?;
        let size = [options.width.max(64), options.height.max(64)];
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("home"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FRAME_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let buffer = readback_buffer(&gpu.device, size[0], size[1]);
        let painter = Painter::new(&gpu.device, FRAME_FORMAT);
        let backdrop = Backdrop::new(gpu, size, FRAME_FORMAT, wait)?;
        Ok(Offscreen { backdrop, painter, texture, view, buffer, size, ui_scale: options.ui_scale, frames: 0, clicks: 0, heard: 0, hovers_heard: 0 })
    }

    /// The page's clock: deterministic, one sixtieth of a second per frame.
    pub(super) fn time(&self) -> f32 {
        self.frames as f32 / 60.0
    }

    pub(super) fn frame(&mut self, home: &mut Home, events: Vec<egui::Event>) -> Option<Choice> {
        self.frames += 1;
        let time = self.time();
        self.painter.paint(&mut self.backdrop, PageTarget { view: &self.view, size: self.size }, PageInput { ui_scale: self.ui_scale, events, time }, home, None)
    }

    pub(super) fn capture(&mut self, home: &mut Home) -> Vec<u8> {
        self.frames += 1;
        let time = self.time();
        self.painter.paint(&mut self.backdrop, PageTarget { view: &self.view, size: self.size }, PageInput { ui_scale: self.ui_scale, events: Vec::new(), time }, home, Some((&self.texture, &self.buffer)));
        let gpu = self.backdrop.gpu();
        read_rgba(&gpu.device, &gpu.queue, &self.buffer, self.size)
    }

    /// Press and release the primary button on a control; None if the page
    /// has no such control, else what the click chose.
    pub(super) fn click(&mut self, home: &mut Home, name: &str) -> Option<Option<Choice>> {
        let at = home.widgets.iter().find(|w| w.0 == name)?.1.center();
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        self.frame(home, vec![egui::Event::PointerMoved(at)]);
        self.hovers_heard += std::mem::take(&mut home.clicked) as usize;
        self.frame(home, vec![press(true)]);
        let choice = self.frame(home, vec![press(false)]);
        // One more pass, so the page shows (and lists the controls of) what was selected.
        self.frame(home, vec![egui::Event::PointerGone]);
        self.clicks += 1;
        self.heard += std::mem::take(&mut home.clicked) as usize;
        Some(choice)
    }
}

/// Render the homepage offscreen to a PNG; `trial` shows the Config page's
/// popup, with Full Screen being tried (the window stays as it is).
pub fn snapshot(options: &HomeOptions, page: Page, trial: bool, png: &Path) -> Result<()> {
    let mut home = Home::new(options.manifest_dir.clone(), options.config_path.clone());
    home.page = page;
    // The photosensitivity warning covers the title, as at start; another page shows itself.
    home.pse = page == Page::Title;
    if trial {
        home.page = Page::Config;
        home.settings.category = Category::Render;
        home.settings.pending.fullscreen = true;
        home.try_settings(0.0);
    }
    let mut offscreen = Offscreen::new(options, true)?;
    // egui sizes the areas on the first pass.
    offscreen.frame(&mut home, Vec::new());
    let rgba = offscreen.capture(&mut home);
    crate::readback::write_png(png, offscreen.size[0], offscreen.size[1], &rgba)?;
    eprintln!("homepage: {} ({}x{}, smallest text {:.1} pt)", png.display(), offscreen.size[0], offscreen.size[1], home.min_text);
    Ok(())
}

/// The selection page offscreen with the chosen feature's knowledge graph full
/// screen over it (`home --snapshot … --page select-knowledge`).
pub fn snapshot_knowledge(options: &HomeOptions, png: &Path) -> Result<()> {
    let mut home = Home::new(options.manifest_dir.clone(), options.config_path.clone());
    home.page = Page::Select;
    home.pse = false;
    home.knowledge = true;
    let mut offscreen = Offscreen::new(options, true)?;
    offscreen.frame(&mut home, Vec::new());
    let rgba = offscreen.capture(&mut home);
    crate::readback::write_png(png, offscreen.size[0], offscreen.size[1], &rgba)?;
    eprintln!("homepage knowledge graph: {} ({}x{}, smallest text {:.1} pt)", png.display(), offscreen.size[0], offscreen.size[1], home.min_text);
    Ok(())
}

/// Render the homepage's first frame offscreen to a PNG: the loading page,
/// while the title's scene loads.
pub fn snapshot_starting(options: &HomeOptions, png: &Path) -> Result<()> {
    let mut home = Home::new(options.manifest_dir.clone(), options.config_path.clone());
    let mut offscreen = Offscreen::new(options, false)?;
    let rgba = offscreen.capture(&mut home);
    let loading = offscreen.backdrop.loading.is_some();
    crate::readback::write_png(png, offscreen.size[0], offscreen.size[1], &rgba)?;
    eprintln!("homepage while it loads: {} ({}x{}){}", png.display(), offscreen.size[0], offscreen.size[1], if loading { "" } else { "; the scene had already loaded, so this is the title page" });
    Ok(())
}

/// Render the viewer offscreen to a PNG as the homepage opens it, with the
/// settings: the current investigation's demonstrations, or (`timeline`) its
/// first timeline playing, in the edition the homepage offers first; or the
/// demonstration `demo` (its key).
pub fn snapshot_viewer(options: &HomeOptions, timeline: bool, demo: Option<&str>, png: &Path) -> Result<()> {
    snapshot_viewer_page(options, timeline, demo, None, png)
}

/// The viewer's options as the homepage opens it on the current
/// investigation with the choice `choose` makes, for a snapshot.
pub(super) fn opened_by(options: &HomeOptions, choose: impl FnOnce(&'static dyn Investigation) -> Choice) -> ViewerOptions {
    let home = Home::new(options.manifest_dir.clone(), options.config_path.clone());
    let size = PhysicalSize::new(options.width, options.height);
    viewer_options(choose(investigation::current()), &home, options, size).expect("a choice that opens the viewer")
}

/// The viewer offscreen as Explore opens it, with the performance panel (`viewer::snapshot_performance`).
pub fn snapshot_performance(options: &HomeOptions, demo: Option<&str>, png: &Path) -> Result<()> {
    let viewer_options = opened_by(options, |investigation| Choice::Explore { investigation });
    viewer::snapshot_performance(viewer_options, demo, png)
}

/// The viewer offscreen with the frame graph in the panel (`viewer::snapshot_graph`).
pub fn snapshot_graph(options: &HomeOptions, demo: Option<&str>, inspect: Option<&str>, how: viewer::GraphSnapshot, png: &Path) -> Result<()> {
    let viewer_options = opened_by(options, |investigation| Choice::Explore { investigation });
    viewer::snapshot_graph(viewer_options, demo, inspect, how, png)
}

/// The viewer offscreen with the free camera on (`viewer::snapshot_free`).
pub fn snapshot_free(options: &HomeOptions, demo: Option<&str>, png: &Path) -> Result<()> {
    let viewer_options = opened_by(options, |investigation| Choice::Explore { investigation });
    viewer::snapshot_free(viewer_options, demo, png)
}

/// The viewer offscreen as the homepage opens it, its edges dragged (`viewer::snapshot_layout`).
pub fn snapshot_layout(options: &HomeOptions, png: &Path) -> Result<()> {
    let viewer_options = opened_by(options, |investigation| Choice::Explore { investigation });
    viewer::snapshot_layout(viewer_options, png)
}

/// The viewer offscreen as the homepage's Experiment button opens it, on
/// `demo` or the first demonstration: the Experiments tab, and with
/// `compare`, a reference set and compared (src/experiment.rs).
pub fn snapshot_experiments(options: &HomeOptions, demo: Option<&str>, compare: bool, png: &Path) -> Result<()> {
    let viewer_options = opened_by(options, |investigation| Choice::Experiment { investigation });
    viewer::snapshot_experiments(viewer_options, demo, compare, png)
}

/// The viewer offscreen as the homepage's Quiz button opens it (src/quiz.rs), at question `spec`
/// (`<question>[@<choice>]`: 1 first, A first; past the last: the score).
pub fn snapshot_quiz(options: &HomeOptions, spec: Option<&str>, png: &Path) -> Result<()> {
    let viewer_options = opened_by(options, |investigation| Choice::Quiz { investigation });
    let (question, choice) = spec.map_or((None, None), |s| s.split_once('@').map_or((Some(s), None), |(q, c)| (Some(q), Some(c))));
    let question = question.map(|q| q.parse::<usize>().ok().filter(|n| *n >= 1).ok_or_else(|| anyhow::anyhow!("a quiz page's question is a number from 1: `{q}`"))).transpose()?;
    let choice = choice
        .map(|c| {
            let c = c.trim().to_ascii_uppercase();
            c.chars().next().filter(|l| c.len() == 1 && l.is_ascii_uppercase()).map(|l| (l as u8 - b'A') as usize).ok_or_else(|| anyhow::anyhow!("a quiz page's choice is a letter: `{c}`"))
        })
        .transpose()?;
    viewer::snapshot_quiz(viewer_options, question.map_or(0, |q| q - 1), choice, png)
}

/// The viewer offscreen on its Code tab, showing `listing` (`viewer::snapshot_code`).
pub fn snapshot_code(options: &HomeOptions, listing: &str, tag: Option<&str>, png: &Path) -> Result<()> {
    let viewer_options = opened_by(options, |investigation| Choice::Explore { investigation });
    viewer::snapshot_code(viewer_options, listing, tag, png)
}

/// The viewer offscreen on its Prerequisites tab, the knowledge graph in the
/// panel, on `demo` or the first demonstration (`viewer::snapshot_prerequisites`).
pub fn snapshot_prerequisites(options: &HomeOptions, demo: Option<&str>, focus: bool, overlay: bool, png: &Path) -> Result<()> {
    let viewer_options = opened_by(options, |investigation| Choice::Explore { investigation });
    viewer::snapshot_prerequisites(viewer_options, demo, focus, overlay, png)
}

/// `snapshot_viewer`, or with `paused`, its timeline paused on a 2D scene.
pub fn snapshot_viewer_page(options: &HomeOptions, timeline: bool, demo: Option<&str>, paused: Option<viewer::PausedPage>, png: &Path) -> Result<()> {
    let mut home = Home::new(options.manifest_dir.clone(), options.config_path.clone());
    let current = investigation::current();
    let choice = if timeline {
        let rows = home.timeline_rows(current);
        let row = rows.get(preferred_edition(&rows, home.applied.language)).with_context(|| format!("{} has no timeline", current.path()))?;
        Choice::Watch { investigation: current, timeline: row.info, start_frame: 0 }
    } else {
        Choice::Explore { investigation: current }
    };
    let size = PhysicalSize::new(options.width, options.height);
    let viewer_options = viewer_options(choice, &home, options, size).expect("a choice that opens the viewer");
    viewer::snapshot_page(viewer_options, demo, paused, png)
}

/// Click through the homepage offscreen and check what each control opens.
pub fn autotest(options: &HomeOptions, report: &Path) -> Result<()> {
    // A settings file of its own beside the report, so the user's is never touched.
    let config_path = report.with_file_name("home-autotest-config.toml");
    let zh_cn = Locale::from_code("zh-CN").expect("the registry has zh-CN");
    let _ = std::fs::remove_file(&config_path);
    let mut home = Home::new(options.manifest_dir.clone(), Some(config_path.clone()));
    let mut offscreen = Offscreen::new(options, true)?;
    offscreen.frame(&mut home, Vec::new());
    offscreen.frame(&mut home, Vec::new());
    let mut checks = Vec::new();
    let mut check = |name: &str, passed: bool, detail: String| {
        eprintln!("home autotest {name}: {} {detail}", if passed { "ok" } else { "FAILED" });
        checks.push(serde_json::json!({ "name": name, "passed": passed, "detail": detail }));
    };
    let describe = |c: Option<Choice>| c.map(|c| c.describe()).unwrap_or_else(|| "nothing".into());

    // The Config page: render settings are tried, then kept or put back; only Apply writes them.
    let stored = || Config::load(&config_path).ok().filter(|_| config_path.exists());
    let has = |home: &Home, name: &str| home.widgets.iter().any(|w| w.0 == name);
    // The photosensitivity warning first, over the title, until Continue.
    check("the photosensitivity warning is up at start, in the configured language", home.pse && has(&home, "pse:continue") && tr("home-pse_title") == "Photosensitivity warning", tr("home-pse_title").to_string());
    let exit = offscreen.click(&mut home, "pse:exit").flatten();
    check("the warning's Exit quits", matches!(exit, Some(Choice::Quit)), describe(exit));
    offscreen.click(&mut home, "pse:continue");
    check("Continue closes the warning", !home.pse && !has(&home, "pse:continue"), String::new());
    offscreen.click(&mut home, "config");
    check("the title's Config opens the Config page at the Interface tab", home.page == Page::Config && home.settings.category == Category::Interface && has(&home, "option:language:zh-CN") && !has(&home, "option:fullscreen"), format!("{:?}, {:?}", home.page, home.settings.category));
    check("with nothing changed, the first button is Return, and no option is marked", has(&home, "return") && !has(&home, "save") && !has(&home, "restore:language"), String::new());
    offscreen.click(&mut home, "category:render");
    check("the Render tab shows Full Screen", home.settings.category == Category::Render && has(&home, "option:fullscreen"), format!("{:?}", home.settings.category));
    offscreen.click(&mut home, "option:fullscreen");
    check("a change turns Return into Save and Apply, and marks the option with its Restore", has(&home, "save") && !has(&home, "return") && has(&home, "restore:fullscreen"), String::new());
    check("ticking Full Screen changes nothing until it is applied", home.settings.pending.fullscreen && !home.applied.fullscreen && stored().is_none(), format!("pending {:?}, fullscreen {}", home.settings.pending, home.applied.fullscreen));
    offscreen.click(&mut home, "save");
    let trying = home.settings.trial.is_some() && home.applied.fullscreen;
    check("Save and Apply tries a render setting without writing it", trying && stored().is_none(), format!("trial {}, fullscreen {}, file {:?}", home.settings.trial.is_some(), home.applied.fullscreen, stored()));
    offscreen.click(&mut home, "trial:restore");
    check("the popup's Restore puts the previous settings back", home.settings.trial.is_none() && !home.applied.fullscreen && !home.settings.pending.fullscreen && stored().is_none(), format!("fullscreen {}, pending {:?}", home.applied.fullscreen, home.settings.pending));
    offscreen.click(&mut home, "option:fullscreen");
    offscreen.click(&mut home, "save");
    offscreen.frames += (TRIAL_SECONDS * 60.0) as u64 + 30;
    offscreen.frame(&mut home, Vec::new());
    check("after ten seconds without Apply, the previous settings come back", home.settings.trial.is_none() && !home.applied.fullscreen && stored().is_none(), format!("fullscreen {}, file {:?}", home.applied.fullscreen, stored()));
    offscreen.click(&mut home, "option:fullscreen");
    offscreen.click(&mut home, "save");
    offscreen.click(&mut home, "trial:apply");
    check("the popup's Apply keeps the settings and writes the file", home.applied.fullscreen && stored() == Some(Config { fullscreen: true, ..Config::default() }), format!("fullscreen {}, file {:?}", home.applied.fullscreen, stored()));
    check("right after saving, the first button is Return again", has(&home, "return") && !has(&home, "save"), String::new());
    offscreen.click(&mut home, "restore:fullscreen");
    check("an option's Restore shows its default without applying it", !home.settings.pending.fullscreen && home.applied.fullscreen && has(&home, "save") && !has(&home, "restore:fullscreen") && stored() == Some(Config { fullscreen: true, ..Config::default() }), format!("pending {:?}, fullscreen {}", home.settings.pending, home.applied.fullscreen));
    offscreen.click(&mut home, "option:fullscreen");
    offscreen.click(&mut home, "defaults");
    check("Restore Defaults shows the defaults without applying them", home.settings.pending == Config::default() && home.applied.fullscreen && stored() == Some(Config { fullscreen: true, ..Config::default() }), format!("pending {:?}, fullscreen {}", home.settings.pending, home.applied.fullscreen));
    offscreen.click(&mut home, "discard");
    check("Discard drops the edits and returns to the title", home.page == Page::Title && home.settings.pending == home.settings.saved, format!("{:?}, pending {:?}", home.page, home.settings.pending));
    offscreen.click(&mut home, "config");
    offscreen.click(&mut home, "return");
    check("Return goes back to the title", home.page == Page::Title, format!("{:?}", home.page));

    // The language: the title's globe opens the Config page at it; saved, it changes the text at once.
    offscreen.click(&mut home, "language");
    check("the title's globe opens the Config page with the Language option outlined", home.page == Page::Config && home.settings.flash.is_some() && has(&home, "option:language:zh-CN"), format!("{:?}", home.page));
    offscreen.click(&mut home, "option:language:zh-CN");
    check("picking a language changes nothing until it is saved", home.settings.pending.language == zh_cn && i18n::current() == Locale::EN && has(&home, "restore:language"), format!("pending {:?}, text {:?}", home.settings.pending.language, i18n::current()));
    offscreen.click(&mut home, "save");
    check("a saved language applies and is written at once, without the popup", home.settings.trial.is_none() && i18n::current() == zh_cn && tr("home-config") != "Config" && stored().is_some_and(|c| c.language == zh_cn && c.fullscreen), format!("text {:?}: {}, file {:?}", i18n::current(), tr("home-config"), stored()));
    check("a new language shows the photosensitivity warning again, in that language", home.pse && has(&home, "pse:continue") && tr("home-pse_title") != "Photosensitivity warning", tr("home-pse_title").to_string());
    offscreen.click(&mut home, "pse:continue");
    // The edition offered first is in the language, else English.
    for i in investigations::all().iter().copied().filter(|i| !i.timelines().is_empty()) {
        let rows = home.timeline_rows(i);
        let offered = &rows[preferred_edition(&rows, zh_cn)];
        let wanted = rows.iter().find(|r| r.locale == Some(zh_cn)).or_else(|| rows.iter().find(|r| r.locale == Some(Locale::EN))).unwrap_or(&rows[0]);
        check(&format!("{}: in Chinese, the edition offered first is the Chinese one, else the English one", i.path()), offered.info.id == wanted.info.id, format!("{} ({:?})", offered.info.id, offered.locale));
    }

    // The theme: offered from the themes beside the binary, the default first in use.
    let ids: Vec<String> = home.settings.themes.iter().map(|t| t.id.clone()).collect();
    let offered = home.settings.themes.clone();
    let colors = |id: &str| offered.iter().find(|t| t.id == id).map(|t| t.theme);
    check("the Interface tab offers the built-in theme and the default one, which is in use", has(&home, &format!("option:theme:{}", theme::BUILT_IN)) && has(&home, &format!("option:theme:{}", theme::DEFAULT)) && colors(theme::DEFAULT) == Some(palette::theme()), format!("{ids:?}"));
    let other = ids.iter().find(|id| *id != theme::DEFAULT && *id != theme::BUILT_IN).cloned().unwrap_or_else(|| theme::BUILT_IN.into());
    // A theme and a render setting at once: the theme applies and is written; the render setting is tried.
    offscreen.click(&mut home, &format!("option:theme:{other}"));
    offscreen.click(&mut home, "category:render");
    offscreen.click(&mut home, "option:fullscreen");
    offscreen.click(&mut home, "save");
    check("a saved theme applies and is written at once, while a render change beside it is tried", home.settings.trial.is_some() && colors(&other) == Some(palette::theme()) && stored().is_some_and(|c| c.theme == other && c.fullscreen), format!("trial {}, file {:?}", home.settings.trial.is_some(), stored()));
    offscreen.click(&mut home, "trial:restore");
    check("Restore puts back the render setting and keeps the theme", home.applied.fullscreen && home.applied.theme == other && colors(&other) == Some(palette::theme()) && home.settings.pending == home.settings.saved, format!("applied {:?}", home.applied));

    // The volumes: a slider and its mute button, applied and written at once.
    offscreen.click(&mut home, "category:audio");
    offscreen.click(&mut home, "mute:music");
    check("the music's mute button mutes it, and keeps its level", home.settings.pending.music.muted && home.settings.pending.music.percent == Config::default().music.percent && has(&home, "restore:music"), format!("{:?}", home.settings.pending.music));
    let rail = home.widgets.iter().find(|w| w.0 == "option:narration").map(|w| w.1);
    if let Some(rail) = rail {
        // A click a quarter along the rail.
        let at = pos2(rail.min.x + rail.width() * 0.25, rail.center().y);
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        offscreen.frame(&mut home, vec![egui::Event::PointerMoved(at)]);
        offscreen.frame(&mut home, vec![press(true)]);
        offscreen.frame(&mut home, vec![press(false)]);
        offscreen.frame(&mut home, vec![egui::Event::PointerGone]);
        home.clicked = false;
    }
    let narration = home.settings.pending.narration;
    check("a click on the narration's slider sets its level", rail.is_some() && narration.percent < 60 && !narration.muted, format!("{narration:?}"));
    offscreen.click(&mut home, "save");
    check("saved volumes are in use and written at once, without the popup", home.settings.trial.is_none() && home.applied.music.muted && home.applied.narration == narration && stored().is_some_and(|c| c.music.muted && c.narration == narration), format!("music {:?}, file {:?}", home.applied.music, stored()));
    home.keep_narration(Volume::new(80));
    check("the narration's volume as the viewer leaves it is kept and written", home.applied.narration == Volume::new(80) && stored().is_some_and(|c| c.narration == Volume::new(80) && c.language == zh_cn), format!("file {:?}", stored()));
    offscreen.click(&mut home, "config");
    offscreen.click(&mut home, "defaults");
    offscreen.click(&mut home, "save");
    offscreen.click(&mut home, "trial:apply");
    check("back in English, the photosensitivity warning shows again, in English", home.pse && tr("home-pse_title") == "Photosensitivity warning", tr("home-pse_title").to_string());
    offscreen.click(&mut home, "pse:continue");
    check("Restore Defaults, applied, puts English, the default theme, and the default volumes back", home.applied == Config::default() && i18n::current() == Locale::EN && colors(theme::DEFAULT) == Some(palette::theme()) && stored() == Some(Config::default()), format!("file {:?}", stored()));
    offscreen.click(&mut home, "return");
    let _ = std::fs::remove_file(&config_path);

    // The title page.
    let quit = offscreen.click(&mut home, "quit").flatten();
    check("the title's Quit quits", matches!(quit, Some(Choice::Quit)), describe(quit));
    let play = offscreen.click(&mut home, "play");
    check("the title's Play opens the selection page", play.is_some_and(|c| c.is_none()) && home.page == Page::Select, format!("{:?}", home.page));

    // The selection page.
    let engines: Vec<&str> = investigations::engines().iter().map(|e| e.id).collect();
    let listed = engines.iter().all(|e| home.widgets.iter().any(|w| w.0 == format!("engine:{e}")));
    check("every engine is listed", listed && !engines.is_empty(), engines.join(", "));
    for i in investigations::all() {
        let engine = format!("engine:{}", i.engine().id);
        // A part of a series: the series in the Feature column, then the part in the detail panel.
        let series = i.package().and_then(crate::resources::part_of);
        let feature = series.map_or_else(|| format!("feature:{}", i.path()), |(s, _)| format!("series:{}", s.key));
        let mut selected = offscreen.click(&mut home, &engine).is_some() && offscreen.click(&mut home, &feature).is_some();
        if series.is_some() {
            selected &= offscreen.click(&mut home, &format!("part:{}", i.path())).is_some();
        }
        let selected = selected && home.selected().path() == i.path();
        check(&format!("{} can be selected", i.path()), selected, feature.clone());
        let explore = offscreen.click(&mut home, "explore").flatten();
        let ok = matches!(explore, Some(Choice::Explore { investigation }) if investigation.path() == i.path());
        check(&format!("{}: Explore opens the demonstrations", i.path()), ok, describe(explore));
        let experiment = offscreen.click(&mut home, "experiment").flatten();
        let ok = matches!(experiment, Some(Choice::Experiment { investigation }) if investigation.path() == i.path());
        check(&format!("{}: Experiment opens the experiments", i.path()), ok, describe(experiment));
        // Its quiz, when its package has one (src/quiz.rs); the button does nothing without.
        let quiz = offscreen.click(&mut home, "quiz").flatten();
        let has_quiz = !crate::quiz::ids(*i).is_empty();
        let ok = if has_quiz { matches!(quiz, Some(Choice::Quiz { investigation }) if investigation.path() == i.path()) } else { quiz.is_none() };
        check(&format!("{}: Quiz opens its quiz (has one: {has_quiz})", i.path()), ok, describe(quiz));
        let Some(timeline) = i.timelines().first() else { continue };
        let watch = offscreen.click(&mut home, "watch").flatten();
        let ok = matches!(watch, Some(Choice::Watch { investigation, timeline: t, start_frame: 0, .. }) if investigation.path() == i.path() && t.id == timeline.id);
        check(&format!("{}: Watch opens the timeline from its start", i.path()), ok, describe(watch));
        // A part of a series: its parts can be picked.
        if let Some((series, index)) = i.package().and_then(crate::resources::part_of) {
            let parts: Vec<&'static dyn Investigation> = series.parts.iter().filter_map(|p| investigations::all().iter().copied().find(|x| x.package() == Some(p.as_str()))).collect();
            let listed = parts.iter().all(|p| home.widgets.iter().any(|w| w.0 == format!("part:{}", p.path())));
            check(&format!("{}: part {} of {} of {}, every part listed", i.path(), index + 1, series.parts.len(), series.key), listed && parts.len() == series.parts.len(), parts.iter().map(|p| p.path()).collect::<Vec<_>>().join(" -> "));
            // Picking another part selects its investigation; then back to this one.
            if let Some(other) = parts.iter().find(|p| p.path() != i.path()) {
                let picked = offscreen.click(&mut home, &format!("part:{}", other.path())).is_some() && home.selected().path() == other.path();
                check(&format!("{}: picking part {} selects it", i.path(), other.path()), picked, home.selected().path());
                offscreen.click(&mut home, &format!("part:{}", i.path()));
            }
        }
        match TimelineSummary::load(&options.manifest_dir, timeline) {
            Ok(summary) => {
                if let Some((number, _, frame)) = summary.chapters.last().cloned() {
                    let chosen = offscreen.click(&mut home, &format!("chapter:{number}")).flatten();
                    let ok = matches!(chosen, Some(Choice::Watch { start_frame, .. }) if start_frame == frame);
                    check(&format!("{}: a chapter opens the timeline at its start", i.path()), ok, format!("chapter {number} (frame {frame}) → {}", describe(chosen)));
                }
            }
            Err(e) => check(&format!("{}: its timeline loads", i.path()), false, format!("{e:#}")),
        }
        // Every other edition: picking it makes Watch play that timeline.
        for other in i.timelines().iter().skip(1) {
            let picked = offscreen.click(&mut home, &format!("edition:{}", other.id)).is_some();
            let watch = offscreen.click(&mut home, "watch").flatten();
            let ok = picked && matches!(watch, Some(Choice::Watch { timeline: t, start_frame: 0, .. }) if t.id == other.id);
            check(&format!("{}: the {} edition can be picked and watched", i.path(), other.id), ok, describe(watch));
        }
        if let Some(first) = i.timelines().first() {
            offscreen.click(&mut home, &format!("edition:{}", first.id));
        }
    }
    let back = offscreen.click(&mut home, "back");
    check("Back returns to the title", back.is_some() && home.page == Page::Title, format!("{:?}", home.page));
    check("text is at least 24 pt at 1080p", home.min_text >= ui::MIN_TEXT_PT, format!("{:.1} pt", home.min_text));
    check("every click on a control plays the click sound, and hovering plays none", offscreen.heard == offscreen.clicks && offscreen.hovers_heard == 0,
          format!("{} of {} clicks, {} hovers", offscreen.heard, offscreen.clicks, offscreen.hovers_heard));
    let passed = checks.iter().all(|c| c["passed"] == true);
    std::fs::write(report, serde_json::to_string_pretty(&serde_json::json!({ "passed": passed, "checks": checks }))?)?;
    if !passed {
        anyhow::bail!("homepage autotest failed; see {}", report.display());
    }
    eprintln!("homepage autotest passed; report: {}", report.display());
    Ok(())
}
