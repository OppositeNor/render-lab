//! What draws the homepage: the title investigation's scene behind it,
//! frosted, and the page over it.

use super::*;

/// The scene behind the page: the title investigation's backdrop
/// (`investigations::title`), drawn full-screen and blurred by the frost. It
/// loads after the window opens (src/loading.rs), which shows the loading
/// page until then; if it fails, the page has a plain background.
pub(super) struct Backdrop {
    pub(super) gpu: Gpu,
    pub(super) loading: Option<Loading>,
    pub(super) renderer: Option<Renderer>,
    pub(super) frost: Frost,
}

impl Backdrop {
    /// Starts loading the scene; `wait` loads it before returning (offscreen
    /// pages, so every frame shows it).
    pub(super) fn new(
        gpu: Gpu,
        size: [u32; 2],
        output: wgpu::TextureFormat,
        wait: bool,
    ) -> Result<Backdrop> {
        let frost = Frost::new(&gpu.device, output)?;
        let title = investigations::title();
        let scene = investigation::scoped(title, || title.first_scene());
        let loading = Loading::start(title, gpu.clone(), scene, size);
        let (loading, renderer) = if wait {
            (None, Some(loading.wait()?))
        } else {
            (Some(loading), None)
        };
        Ok(Backdrop {
            gpu,
            loading,
            renderer,
            frost,
        })
    }

    pub(super) fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// Take the scene once it has loaded; while it loads, how far it has come.
    pub(super) fn poll(&mut self) -> Option<Progress> {
        if let Some(result) = self.loading.as_mut().and_then(|l| l.poll()) {
            self.loading = None;
            match result {
                Ok(renderer) => self.renderer = Some(renderer),
                // The page stays on its plain background.
                Err(e) => eprintln!("the homepage's backdrop did not load: {e:#}"),
            }
        }
        self.loading.as_ref().map(Loading::progress)
    }

    /// Render the scene at `time` seconds into the frame target, at `size`,
    /// once it has loaded.
    pub(super) fn render(&mut self, size: [u32; 2], time: f32) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let size = [size[0].max(64), size[1].max(64)];
        if [renderer.width, renderer.height] != size {
            renderer.resize(size[0], size[1]);
        }
        let title = investigations::title();
        investigation::scoped(title, || {
            let state = title.backdrop(time);
            renderer.render_backdrop(state.as_ref(), time);
        });
    }

    /// The device, the rendered frame and its size (once loaded), and the frost.
    pub(super) fn parts(&mut self) -> (&Gpu, Option<(&wgpu::TextureView, [u32; 2])>, &mut Frost) {
        (
            &self.gpu,
            self.renderer
                .as_ref()
                .map(|r| (&r.frame_view, [r.width, r.height])),
            &mut self.frost,
        )
    }
}

impl Drop for Backdrop {
    /// A scene still loading is waited for: its worker uses the device.
    fn drop(&mut self) {
        if let Some(loading) = self.loading.take() {
            let _ = loading.wait();
        }
    }
}

/// egui on wgpu: draws the homepage into any colour target.
pub(super) struct Painter {
    pub(super) page: PagePainter,
}

/// The homepage's pass besides the page: the backdrop frosted under it, and
/// the copy a snapshot reads back.
pub(super) struct Frosted<'a> {
    pub(super) device: &'a wgpu::Device,
    pub(super) frost: &'a mut Frost,
    pub(super) frame: Option<(&'a wgpu::TextureView, [u32; 2])>,
    pub(super) readback: Option<(&'a wgpu::Texture, &'a wgpu::Buffer)>,
    pub(super) size: [u32; 2],
}

impl crate::egui_pass::Extra for Frosted<'_> {
    fn before(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some((frame, frame_size)) = self.frame {
            self.frost.blur(self.device, encoder, frame, frame_size);
        }
    }

    fn under(&mut self, pass: &mut wgpu::RenderPass<'static>) {
        if self.frame.is_some() {
            self.frost.draw(pass);
        }
    }

    fn after(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some((texture, buffer)) = self.readback {
            copy_to_readback(encoder, texture, buffer, self.size);
        }
    }
}

/// One frame's input to the page: its UI scale, the events since the last
/// frame, and the page's clock.
pub(super) struct PageInput {
    pub ui_scale: f32,
    pub events: Vec<egui::Event>,
    pub time: f32,
}

impl Painter {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Painter {
        Painter {
            page: PagePainter::new(device, format),
        }
    }

    /// One frame of the page into `target`: the backdrop at the input's
    /// time, frosted, then the page; the loading page while the backdrop
    /// loads. Returns the choice made.
    pub(super) fn paint(
        &mut self,
        backdrop: &mut Backdrop,
        target: PageTarget,
        input: PageInput,
        home: &mut Home,
        readback: Option<(&wgpu::Texture, &wgpu::Buffer)>,
    ) -> Option<Choice> {
        let PageInput {
            ui_scale,
            events,
            time,
        } = input;
        let size = target.size;
        let (raw, canvas) = PagePainter::input(
            size,
            ui_scale,
            egui::RawInput {
                time: Some(time as f64),
                events,
                ..Default::default()
            },
        );
        let mut choice = None;
        let loading = backdrop.poll();
        let output = self.page.run(raw, |ctx| match &loading {
            Some(progress) => loading::page(ctx, canvas, "Render Lab", progress),
            None => choice = home.draw(ctx, canvas, time),
        });
        backdrop.render(size, time);
        let (gpu, frame, frost) = backdrop.parts();
        // Before the backdrop has loaded, the page is on the plain background.
        let clear = if frame.is_some() {
            wgpu::Color::BLACK
        } else {
            palette::background_clear()
        };
        self.page.draw(
            gpu,
            "home",
            target,
            clear,
            output,
            &mut Frosted {
                device: &gpu.device,
                frost,
                frame,
                readback,
                size,
            },
        );
        choice
    }
}
