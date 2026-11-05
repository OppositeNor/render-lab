//! The homepage's window, its event loop, and the viewer it opens and comes
//! back from.

use super::*;

pub(super) struct Surface {
    pub(super) window: Arc<Window>,
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) config: wgpu::SurfaceConfiguration,
    pub(super) backdrop: Backdrop,
    pub(super) painter: Painter,
}

/// The homepage's window, for one run of the event loop.
pub(super) struct HomeApp<'a> {
    pub(super) home: &'a mut Home,
    pub(super) options: &'a HomeOptions,
    pub(super) size: PhysicalSize<u32>,
    pub(super) surface: Option<Surface>,
    pub(super) events: Vec<egui::Event>,
    pub(super) cursor: Pos2,
    pub(super) started: Instant,
    /// Whether the window is fullscreen now; it follows `Home::fullscreen`.
    pub(super) shown_fullscreen: bool,
    /// F11: save the next presented frame (`readback::screenshot`).
    pub(super) screenshot: bool,
    pub(super) choice: Option<Choice>,
    pub(super) error: Option<anyhow::Error>,
    /// The sound of a control being clicked.
    pub(super) click: &'a mut Option<AudioPlayer>,
    /// The music, and the volume it plays at now; it follows `Home::applied`.
    pub(super) music: &'a mut Option<AudioPlayer>,
    pub(super) heard_music: Volume,
}

impl HomeApp<'_> {
    pub(super) fn open(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() || self.error.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title("Render Lab").with_inner_size(self.size).with_fullscreen(fullscreen_mode(self.home.applied.fullscreen));
        let result = (|| -> Result<Surface> {
            let crate::window::Opened { window, surface, gpu, config } = crate::window::open(event_loop, attributes)?;
            let format = config.format;
            let painter = Painter::new(&gpu.device, format);
            let backdrop = Backdrop::new(gpu, [config.width, config.height], format, false)?;
            Ok(Surface { window, surface, config, backdrop, painter })
        })();
        match result {
            Ok(s) => {
                self.started = Instant::now();
                self.shown_fullscreen = self.home.applied.fullscreen;
                // The new window draws with a new egui context: textures of the last one are gone.
                self.home.globe = None;
                self.surface = Some(s);
            }
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    pub(super) fn scale(&self) -> f32 {
        let size = self.surface.as_ref().map_or(self.size, |s| s.window.inner_size());
        canvas_for(size.width.max(1), size.height.max(1), self.options.ui_scale).0
    }

    pub(super) fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(s) = self.surface.as_mut() else { return };
        let frame = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                s.surface.configure(&s.backdrop.gpu().device, &s.config);
                return;
            }
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let events = std::mem::take(&mut self.events);
        let time = self.started.elapsed().as_secs_f32();
        // The frame a screenshot is taken of shows no popup.
        self.home.popups.hidden = self.screenshot;
        let choice = s.painter.paint(&mut s.backdrop, &view, [s.config.width, s.config.height], self.options.ui_scale, events, time, self.home, None);
        if std::mem::take(&mut self.home.clicked) && let Some(click) = self.click.as_mut() {
            click.play_from(0.0);
        }
        if std::mem::take(&mut self.screenshot) {
            let gpu = s.backdrop.gpu();
            let saved = crate::readback::screenshot(&gpu.device, &gpu.queue, &frame.texture, &self.options.manifest_dir);
            self.home.popups.push(Popup::screenshot(&saved));
        }
        for action in self.home.popups.take_actions() {
            popup::perform(action);
        }
        s.window.pre_present_notify();
        s.backdrop.gpu().queue.present(frame);
        // Settings being tried or put back.
        if self.shown_fullscreen != self.home.applied.fullscreen {
            self.shown_fullscreen = self.home.applied.fullscreen;
            s.window.set_fullscreen(fullscreen_mode(self.shown_fullscreen));
        }
        if self.heard_music != self.home.applied.music {
            self.heard_music = self.home.applied.music;
            // FFplay takes a volume when it starts: the loop starts again at the new one.
            if let Some(m) = self.music.as_mut() {
                m.set_gain(self.heard_music.gain());
                m.play_from(0.0);
            }
        }
        if let Some(c) = choice {
            self.choice = Some(c);
            // The windowed size, for the next window; a fullscreen one fills the screen anyway.
            if !self.shown_fullscreen {
                self.size = s.window.inner_size();
            }
            event_loop.exit();
        }
    }
}

impl ApplicationHandler for HomeApp<'_> {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        // A run on a reused event loop starts here; `resumed` may not follow.
        if cause == StartCause::Init {
            self.open(event_loop);
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.open(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let scale = self.scale();
        match event {
            WindowEvent::CloseRequested => {
                self.choice = Some(Choice::Quit);
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(s) = self.surface.as_mut() {
                    s.config.width = size.width.max(1);
                    s.config.height = size.height.max(1);
                    s.surface.configure(&s.backdrop.gpu().device, &s.config);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = pos2(position.x as f32 / scale, position.y as f32 / scale);
                self.events.push(egui::Event::PointerMoved(self.cursor));
            }
            WindowEvent::CursorLeft { .. } => self.events.push(egui::Event::PointerGone),
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(button) = crate::window::pointer_button(button) {
                    self.events.push(egui::Event::PointerButton { pos: self.cursor, button, pressed: state == ElementState::Pressed, modifiers: Default::default() });
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.events.push(crate::window::wheel_event(delta, scale, Default::default()).0);
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed && !event.repeat => match event.physical_key {
                // Esc: from the selection page to the title; on the title, quit.
                PhysicalKey::Code(KeyCode::Escape) if !self.home.back() => {
                    self.choice = Some(Choice::Quit);
                    event_loop.exit();
                }
                // Saved when the next frame is presented.
                PhysicalKey::Code(KeyCode::F11) => self.screenshot = true,
                _ => {}
            },
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(s) = &self.surface {
            s.window.request_redraw();
        }
    }
}

/// The homepage's music and the sound of its controls (packages/render_lab/assets/audio/README.md); the
/// lab is silent without them or without FFplay.
pub(super) const MUSIC: &str = "render_lab.audio/music";
pub(super) const CLICK: &str = "render_lab.audio/click";
/// The title page's language button.
pub(super) const GLOBE: &str = "render_lab.icon/globe";

/// The homepage, then whatever it opens, until it quits or a window is closed.
pub fn run(options: HomeOptions) -> Result<()> {
    let mut event_loop = EventLoop::new()?;
    let mut home = Home::new(options.manifest_dir.clone(), options.config_path.clone());
    let mut size = PhysicalSize::new(options.width, options.height);
    // The main theme's music plays while the homepage is shown, not in a demonstration or a video.
    let mut music = AudioPlayer::new(resources::get(MUSIC).path(), &options.manifest_dir).map(|m| m.looping(2.0));
    // So are its clicks. The player outlives each page, so the click that opens the viewer is heard.
    let mut click = AudioPlayer::new(resources::get(CLICK).path(), &options.manifest_dir);
    loop {
        let heard_music = home.applied.music;
        if let Some(m) = music.as_mut() {
            m.set_gain(heard_music.gain());
            m.play_from(0.0);
        }
        event_loop.set_control_flow(ControlFlow::Poll);
        let mut app = HomeApp { home: &mut home, options: &options, size, surface: None, events: Vec::new(), cursor: Pos2::ZERO, started: Instant::now(), shown_fullscreen: false, screenshot: false, choice: None, error: None, click: &mut click, music: &mut music, heard_music };
        event_loop.run_app_on_demand(&mut app)?;
        if let Some(e) = app.error.take() {
            return Err(e);
        }
        let choice = app.choice.unwrap_or(Choice::Quit);
        size = app.size;
        drop(app);
        if let Some(m) = music.as_mut() {
            m.stop();
        }
        if !open(choice, &mut event_loop, &mut home, &options, size) {
            return Ok(());
        }
    }
}

/// The viewer's options for what the homepage chose, with the settings in
/// use (the investigation becomes the current one); None for Quit. A timeline
/// is in its own language; the demonstrations are in the homepage's.
pub(super) fn viewer_options(choice: Choice, home: &Home, options: &HomeOptions, size: PhysicalSize<u32>) -> Option<ViewerOptions> {
    let (investigation, timeline, audio, start_frame, locale) = match choice {
        Choice::Quit => return None,
        Choice::Watch { investigation, timeline, start_frame, .. } => (investigation, Some(options.manifest_dir.join(&timeline.path)), Some(options.manifest_dir.join(&timeline.narration)), start_frame, None),
        Choice::Explore { investigation } | Choice::Experiment { investigation } => (investigation, None, None, 0, Some(home.applied.language)),
    };
    let experiments = matches!(choice, Choice::Experiment { .. });
    investigation::set(investigation);
    Some(ViewerOptions {
        timeline,
        audio,
        manifest_dir: options.manifest_dir.clone(),
        ui_scale: options.ui_scale,
        autotest: None,
        width: size.width,
        height: size.height,
        locale,
        start_frame,
        fullscreen: home.applied.fullscreen,
        narration: home.applied.narration,
        experiments,
        layout: home.applied.layout,
        settings: home.settings.path.clone(),
        next: false,
        autoplay: false,
    })
}

/// The part after the one `choice` watches, in its language (else its first
/// built timeline), when the choice goes on to the next parts and there is one.
pub(super) fn next_part(choice: Choice, home: &mut Home) -> Option<Choice> {
    let Choice::Watch { investigation, timeline, continue_parts: true, .. } = choice else { return None };
    let (series, index) = crate::resources::part_of(investigation.package()?)?;
    let package = series.parts.get(index + 1)?;
    let next = investigations::all().iter().copied().find(|i| i.package() == Some(package.as_str()))?;
    let locale = TimelineSummary::load(&home.manifest_dir, timeline).ok().map(|s| s.locale);
    let rows = home.timeline_rows(next);
    let pick = rows.iter().position(|r| r.summary.is_ok() && r.locale.is_some() && r.locale == locale).or_else(|| rows.iter().position(|r| r.summary.is_ok()))?;
    Some(Choice::Watch { investigation: next, timeline: rows[pick].info, start_frame: 0, continue_parts: true })
}

/// Run the viewer on what the homepage chose (and, for a part of a series
/// watched to its end, on the parts after it); false to quit.
pub(super) fn open(mut choice: Choice, event_loop: &mut EventLoop<()>, home: &mut Home, options: &HomeOptions, size: PhysicalSize<u32>) -> bool {
    let mut autoplay = false;
    loop {
        let next = next_part(choice, home);
        let Some(mut viewer_options) = viewer_options(choice, home, options, size) else { return false };
        viewer_options.next = next.is_some();
        viewer_options.autoplay = autoplay;
        let exit = viewer::run_on(event_loop, viewer_options);
        if let (Ok((ViewerExit::Next, _, _)), Some(next)) = (&exit, next) {
            if let Ok((_, narration, layout)) = exit {
                home.keep_narration(narration);
                home.keep_layout(layout);
            }
            // The homepage, when it comes back, is on the part last watched.
            if let Choice::Watch { investigation, .. } = next {
                home.select(investigation);
            }
            choice = next;
            autoplay = true;
            continue;
        }
        return finish(exit, home);
    }
}

/// After the viewer: the settings it changed kept, and whether to go on (false to quit).
pub(super) fn finish(exit: Result<(ViewerExit, Volume, crate::config::ViewerLayout)>, home: &mut Home) -> bool {
    // A timeline in another language switched the text; the homepage keeps its own.
    i18n::set(home.applied.language);
    let exit = exit.map(|(exit, narration, layout)| {
        home.keep_narration(narration);
        home.keep_layout(layout);
        exit
    });
    match exit {
        Ok(ViewerExit::Back | ViewerExit::Next) => {}
        Ok(ViewerExit::Closed) => return false,
        // Show what went wrong on the selection page instead of quitting.
        Err(e) => home.error = Some(format!("{e:#}")),
    }
    true
}
