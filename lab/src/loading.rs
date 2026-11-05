//! The loading step before a scene plays. Nothing a scene needs is read at
//! start-up: when the viewer opens, or the homepage shows its backdrop, a
//! worker thread prepares the scene's resources (src/resources.rs), its
//! `Statics`, and a `Renderer` on a clone of the window's device, while the
//! window draws `Screen`, the progress so far.

use crate::egui_pass::PagePainter;
use crate::frame::Statics;
use crate::gpu::Gpu;
use crate::i18n::{args, tr, tr_with};
use crate::investigation::{self, Investigation};
use crate::renderer::Renderer;
use crate::scene::SceneId;
use crate::{palette, resources};
use anyhow::{anyhow, Result};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Rect};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Where the worker is: its current step, and how many of all its steps are done.
#[derive(Clone)]
pub struct Progress {
    pub step: String,
    pub done: usize,
    pub total: usize,
}

impl Progress {
    pub fn fraction(&self) -> f32 {
        self.done as f32 / self.total.max(1) as f32
    }
}

/// A scene being prepared on a worker thread.
pub struct Loading {
    progress: Arc<Mutex<Progress>>,
    worker: Option<JoinHandle<Result<Renderer>>>,
}

impl Loading {
    /// Starts preparing `scene` of `investigation` (current on the worker),
    /// for a renderer of `size` pixels.
    pub fn start(investigation: &'static dyn Investigation, gpu: Gpu, scene: SceneId, size: [u32; 2]) -> Loading {
        let ids = scene.resources();
        // Each resource, the scene's statics, then the renderer.
        let total = ids.len() + 2;
        let progress = Arc::new(Mutex::new(Progress { step: tr("loading-waiting").to_string(), done: 0, total }));
        let shared = progress.clone();
        let worker = std::thread::spawn(move || {
            investigation::scoped(investigation, || {
                let step = |step: String, done: usize| *shared.lock().unwrap() = Progress { step, done, total };
                for (i, id) in ids.iter().enumerate() {
                    step(tr_with("loading-resource", args!(name = id)), i);
                    resources::get(id).prepare()?;
                }
                step(tr("loading-scene").to_string(), ids.len());
                let statics = Statics::prepare(scene)?;
                step(tr("loading-renderer").to_string(), ids.len() + 1);
                let renderer = Renderer::new(gpu, statics, size[0].max(64), size[1].max(64))?;
                step(tr("loading-done").to_string(), total);
                Ok(renderer)
            })
        });
        Loading { progress, worker: Some(worker) }
    }

    pub fn progress(&self) -> Progress {
        self.progress.lock().unwrap().clone()
    }

    /// The renderer once the worker has finished, or why it failed; None while
    /// it is still working.
    pub fn poll(&mut self) -> Option<Result<Renderer>> {
        if !self.worker.as_ref()?.is_finished() {
            return None;
        }
        Some(join(self.worker.take().unwrap()))
    }

    /// Waits for the worker (offscreen pages, which draw no progress).
    pub fn wait(mut self) -> Result<Renderer> {
        join(self.worker.take().expect("the loading has finished"))
    }
}

fn join(worker: JoinHandle<Result<Renderer>>) -> Result<Renderer> {
    worker.join().unwrap_or_else(|panic| {
        let message = panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
        Err(anyhow!("loading the scene failed: {message}"))
    })
}

/// The window's page while a scene loads: its name, the current step, and a
/// bar of the steps done.
pub struct Screen {
    painter: PagePainter,
}

impl Screen {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Screen {
        Screen { painter: PagePainter::new(device, format) }
    }

    /// Draws the page into `view` (`size` pixels): `title` over the progress.
    pub fn draw(&mut self, gpu: &Gpu, view: &wgpu::TextureView, size: [u32; 2], ui_scale: f32, title: &str, progress: &Progress) {
        let (raw, canvas) = PagePainter::input(size, ui_scale, egui::RawInput::default());
        let output = self.painter.run(raw, |ctx| page(ctx, canvas, title, progress));
        self.painter.draw(gpu, "loading", view, size, palette::background_clear(), output, &mut ());
    }
}

/// The loading page on a canvas of `canvas` points, on the plain background:
/// `title` over the progress. The homepage draws it too, while its backdrop loads.
pub fn page(ctx: &egui::Context, canvas: egui::Vec2, title: &str, progress: &Progress) {
    let painter = ctx.layer_painter(egui::LayerId::background());
    let centre = pos2(canvas.x / 2.0, canvas.y / 2.0);
    painter.text(centre - vec2(0.0, 60.0), Align2::CENTER_BOTTOM, title, FontId::new(40.0, FontFamily::Name("Bold".into())), palette::text());
    painter.text(centre - vec2(0.0, 20.0), Align2::CENTER_BOTTOM, &progress.step, FontId::new(24.0, FontFamily::Proportional), palette::text_faint());
    let bar = Rect::from_center_size(centre + vec2(0.0, 12.0), vec2(520.0, 10.0));
    painter.rect_filled(bar, CornerRadius::same(5), Color32::from_white_alpha(24));
    let filled = Rect::from_min_size(bar.min, vec2(bar.width() * progress.fraction(), bar.height()));
    painter.rect_filled(filled, CornerRadius::same(5), palette::accent());
}

/// The viewer's loading page offscreen (`home --snapshot --page loading`):
/// starts loading the current investigation's first scene, draws the page
/// while the worker runs into `png`, then waits for the renderer.
pub fn snapshot(png: &std::path::Path, size: [u32; 2], ui_scale: f32) -> Result<()> {
    use crate::readback::{read_texture_rgba, write_png};
    let gpu = Gpu::new(crate::gpu::instance(None), None)?;
    let format = crate::gpu::FRAME_FORMAT;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("loading"),
        size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let current = investigation::current();
    let loading = Loading::start(current, gpu.clone(), current.first_scene(), size);
    let mut screen = Screen::new(&gpu.device, format);
    let progress = loading.progress();
    screen.draw(&gpu, &view, size, ui_scale, current.title(), &progress);
    write_png(png, size[0], size[1], &read_texture_rgba(&gpu.device, &gpu.queue, &texture, size))?;
    let started = std::time::Instant::now();
    let renderer = loading.wait()?;
    eprintln!("loading page: {} (drawn at \"{}\", step {} of {}); the renderer ({}x{}) was ready {:.1} s later", png.display(), progress.step, progress.done, progress.total, renderer.width, renderer.height, started.elapsed().as_secs_f32());
    Ok(())
}
