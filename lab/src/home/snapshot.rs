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
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
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
        Ok(Offscreen {
            backdrop,
            painter,
            texture,
            view,
            buffer,
            size,
            ui_scale: options.ui_scale,
            frames: 0,
            clicks: 0,
            heard: 0,
            hovers_heard: 0,
        })
    }

    /// The page's clock: deterministic, one sixtieth of a second per frame.
    pub(super) fn time(&self) -> f32 {
        self.frames as f32 / 60.0
    }

    pub(super) fn frame(&mut self, home: &mut Home, events: Vec<egui::Event>) -> Option<Choice> {
        self.frames += 1;
        let time = self.time();
        self.painter.paint(
            &mut self.backdrop,
            PageTarget {
                view: &self.view,
                size: self.size,
            },
            PageInput {
                ui_scale: self.ui_scale,
                events,
                time,
            },
            home,
            None,
        )
    }

    pub(super) fn capture(&mut self, home: &mut Home) -> Vec<u8> {
        self.frames += 1;
        let time = self.time();
        self.painter.paint(
            &mut self.backdrop,
            PageTarget {
                view: &self.view,
                size: self.size,
            },
            PageInput {
                ui_scale: self.ui_scale,
                events: Vec::new(),
                time,
            },
            home,
            Some((&self.texture, &self.buffer)),
        );
        let gpu = self.backdrop.gpu();
        read_rgba(&gpu.device, &gpu.queue, &self.buffer, self.size)
    }

    /// Press and release the primary button on a control; None if the page
    /// has no such control, else what the click chose.
    pub(super) fn click(&mut self, home: &mut Home, name: &str) -> Option<Option<Choice>> {
        let at = home.widgets.iter().find(|w| w.0 == name)?.1.center();
        let press = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
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
    eprintln!(
        "homepage: {} ({}x{}, smallest text {:.1} pt)",
        png.display(),
        offscreen.size[0],
        offscreen.size[1],
        home.min_text
    );
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
    eprintln!(
        "homepage knowledge graph: {} ({}x{}, smallest text {:.1} pt)",
        png.display(),
        offscreen.size[0],
        offscreen.size[1],
        home.min_text
    );
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
    eprintln!(
        "homepage while it loads: {} ({}x{}){}",
        png.display(),
        offscreen.size[0],
        offscreen.size[1],
        if loading {
            ""
        } else {
            "; the scene had already loaded, so this is the title page"
        }
    );
    Ok(())
}

/// Render the viewer offscreen to a PNG as the homepage opens it, with the
/// settings: the current investigation's demonstrations, or (`timeline`) its
/// first timeline playing, in the edition the homepage offers first; or the
/// demonstration `demo` (its key).
pub fn snapshot_viewer(
    options: &HomeOptions,
    timeline: bool,
    demo: Option<&str>,
    png: &Path,
) -> Result<()> {
    snapshot_viewer_page(options, timeline, demo, None, png)
}

/// The viewer's options as the homepage opens it on the current
/// investigation with the choice `choose` makes, for a snapshot.
pub(super) fn opened_by(
    options: &HomeOptions,
    choose: impl FnOnce(&'static dyn Investigation) -> Choice,
) -> ViewerOptions {
    let home = Home::new(options.manifest_dir.clone(), options.config_path.clone());
    let size = PhysicalSize::new(options.width, options.height);
    viewer_options(choose(investigation::current()), &home, options, size)
        .expect("a choice that opens the viewer")
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

/// The viewer's camera modes driven and checked offscreen (`viewer::snapshot_camera`).
pub fn snapshot_camera(options: &HomeOptions, png: &Path) -> Result<()> {
    let viewer_options = opened_by(options, |investigation| Choice::Explore { investigation });
    viewer::snapshot_camera(viewer_options, png)
}

/// The viewer offscreen with its Scene window open and an entity's gizmo dragged (`viewer::snapshot_scene`).
pub fn snapshot_scene(
    options: &HomeOptions,
    entity: Option<&str>,
    mode: crate::gizmo::Mode,
    png: &Path,
) -> Result<()> {
    let viewer_options = opened_by(options, |investigation| Choice::Explore { investigation });
    viewer::snapshot_scene(viewer_options, entity, mode, png)
}

/// The viewer offscreen as a feature's homepage button opens it (src/feature/), at its snapshot page `page`
/// (`viewer::snapshot_feature`).
pub fn snapshot_feature(
    options: &HomeOptions,
    feature: &'static dyn crate::feature::Feature,
    page: &str,
    args: Option<&str>,
    png: &Path,
) -> Result<()> {
    let viewer_options = opened_by(options, |investigation| Choice::Feature {
        investigation,
        feature,
    });
    viewer::snapshot_feature(viewer_options, feature, page, args, png)
}

/// `snapshot_viewer`, or with `paused`, its timeline paused on a 2D scene.
pub fn snapshot_viewer_page(
    options: &HomeOptions,
    timeline: bool,
    demo: Option<&str>,
    paused: Option<viewer::PausedPage>,
    png: &Path,
) -> Result<()> {
    let mut home = Home::new(options.manifest_dir.clone(), options.config_path.clone());
    let current = investigation::current();
    let choice = if timeline {
        let rows = home.timeline_rows(current);
        let row = rows
            .get(preferred_edition(&rows, home.applied.language))
            .with_context(|| format!("{} has no timeline", current.path()))?;
        Choice::Watch {
            investigation: current,
            timeline: row.info,
            start_frame: 0,
        }
    } else {
        Choice::Explore {
            investigation: current,
        }
    };
    let size = PhysicalSize::new(options.width, options.height);
    let viewer_options =
        viewer_options(choice, &home, options, size).expect("a choice that opens the viewer");
    viewer::snapshot_page(viewer_options, demo, paused, png)
}
