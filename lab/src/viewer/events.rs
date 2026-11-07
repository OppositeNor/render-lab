//! The window: opened, its loading page drawn while the scene loads, and its
//! events.

use super::*;

impl App {
    /// Create the window and the GPU, and start loading the scene (once per run).
    pub(super) fn open(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() || self.pending.is_some() || self.error.is_some() {
            return;
        }
        let title = format!("Render Lab · {}", investigation::current().title());
        let attributes = Window::default_attributes().with_title(title).with_inner_size(PhysicalSize::new(self.options.width, self.options.height)).with_fullscreen(fullscreen_mode(self.options.fullscreen));
        let result = (|| -> Result<()> {
            let crate::window::Opened { window, surface, gpu, config } = crate::window::open(event_loop, attributes)?;
            let (size, format) = (window.inner_size(), config.format);
            eprintln!("GPU: {} · swapchain {:?}", gpu.info.name, format);
            if let Some(a) = &self.audio {
                eprintln!("Narration: {}", a.track().display());
            }
            let scene = investigation::current().demo_scene(self.state.demo);
            let loading = Loading::start(investigation::current(), gpu.clone(), scene, [size.width, size.height]);
            let screen = loading::Screen::new(&gpu.device, format);
            self.pending = Some(Pending { window, surface, config, gpu, loading, screen });
            Ok(())
        })();
        if let Err(e) = result {
            self.error = Some(e);
            event_loop.exit();
        }
    }

    /// Draws the loading page, and opens the scene once its renderer is ready.
    pub(super) fn loading_frame(&mut self) -> Result<()> {
        let pending = self.pending.as_mut().expect("loading");
        if let Some(renderer) = pending.loading.poll() {
            let mut renderer = renderer?;
            let Pending { window, surface, config, .. } = self.pending.take().unwrap();
            renderer.ui_scale = self.options.ui_scale;
            let blit = renderer.pipelines.blit(&renderer.gpu.device, config.format);
            let blit_group = blit_group(&renderer);
            self.surface = Some(Surface { window, surface, config, blit, blit_group });
            self.renderer = Some(renderer);
            // The window may have changed size while it loaded; the clocks start now.
            let size = self.surface.as_ref().unwrap().window.inner_size();
            self.configure(size);
            let now = Instant::now();
            self.last_tick = now;
            self.timeline_anchor = (now, self.timeline_frame);
            if let Some((_, started)) = self.intro.as_mut() {
                *started = now;
            }
            return Ok(());
        }
        let frame = match pending.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                pending.surface.configure(&pending.gpu.device, &pending.config);
                return Ok(());
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let title = investigation::current().title();
        pending.screen.draw(&pending.gpu, crate::egui_pass::PageTarget { view: &view, size: [pending.config.width, pending.config.height] }, self.options.ui_scale, title, &pending.loading.progress());
        pending.window.pre_present_notify();
        pending.gpu.queue.present(frame);
        Ok(())
    }

    /// The window's events while its scene loads.
    pub(super) fn loading_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.exit = ViewerExit::Closed;
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                let pending = self.pending.as_mut().unwrap();
                pending.config.width = size.width.max(1);
                pending.config.height = size.height.max(1);
                pending.surface.configure(&pending.gpu.device, &pending.config);
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.loading_frame() {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
}

impl ApplicationHandler for App {
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
        if self.pending.is_some() {
            self.loading_event(event_loop, event);
            return;
        }
        if self.renderer.is_none() {
            return;
        }
        // The autotest drives the input itself; a real pointer over the window would move its sliders.
        let user_input = matches!(event, WindowEvent::CursorMoved { .. } | WindowEvent::CursorLeft { .. } | WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } | WindowEvent::KeyboardInput { .. });
        if self.autotest.is_some() && user_input {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.exit = ViewerExit::Closed;
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.configure(size);
                if let Some(t) = self.autotest.as_mut() {
                    t.resized(size.width, size.height);
                }
            }
            WindowEvent::CursorMoved { position, .. } => self.cursor_moved(Vec2::new(position.x as f32, position.y as f32)),
            WindowEvent::CursorLeft { .. } => self.egui_events.push(egui::Event::PointerGone),
            // Keys released while the window was not focused would stay held.
            WindowEvent::Focused(false) => self.held.clear(),
            WindowEvent::MouseInput { state, button, .. } => self.mouse_button(button, state == ElementState::Pressed),
            WindowEvent::MouseWheel { delta, .. } => {
                let (wheel, steps) = crate::window::wheel_event(delta, self.renderer().layout().scale, self.modifiers);
                self.egui_events.push(wheel);
                self.wheel(steps);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let m = modifiers.state();
                self.modifiers = egui::Modifiers { alt: m.alt_key(), ctrl: m.control_key(), shift: m.shift_key(), mac_cmd: false, command: m.control_key() };
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed && !event.repeat && event.physical_key == PhysicalKey::Code(KeyCode::F11) => {
                self.screenshot = true;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.held.retain(|k| *k != code);
                    if event.state == ElementState::Pressed {
                        self.held.push(code);
                    }
                }
                if self.renderer().egui_ctx.egui_wants_keyboard_input() {
                    // A slider's value is being typed: the keys belong to it.
                    self.egui_key(&event);
                } else if event.state == ElementState::Pressed && !event.repeat && let PhysicalKey::Code(code) = event.physical_key {
                    self.key(code, Some(event_loop));
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.redraw() {
                    self.error = Some(e);
                    event_loop.exit();
                    return;
                }
                if self.autotest.is_some() {
                    let mut test = self.autotest.take().unwrap();
                    let done = test.tick(self);
                    self.autotest = Some(test);
                    if done {
                        event_loop.exit();
                    }
                }
                if self.leaving {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(s) = &self.surface {
            s.window.request_redraw();
        }
        if let Some(p) = &self.pending {
            p.window.request_redraw();
        }
    }
}
