//! A page drawn with egui straight into a texture, outside the viewer's
//! frame: the loading page and the homepage. Its context, its renderer, a
//! pass's input for a frame's size, and the pass that draws its output.

use crate::gpu::Gpu;
use crate::text;
use egui::{Pos2, Rect, Vec2};

pub struct PagePainter {
    pub ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
}

impl PagePainter {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> PagePainter {
        let ctx = egui::Context::default();
        text::install_fonts(&ctx);
        let renderer = egui_wgpu::Renderer::new(device, format, egui_wgpu::RendererOptions { msaa_samples: 1, depth_stencil_format: None, dithering: false, predictable_texture_filtering: true });
        PagePainter { ctx, renderer }
    }

    /// A pass's input for a frame of `size` pixels drawn `ui_scale` times its
    /// size (`raw`'s time and events kept), and the canvas it lays out.
    pub fn input(size: [u32; 2], ui_scale: f32, raw: egui::RawInput) -> (egui::RawInput, Vec2) {
        let (scale, canvas) = text::canvas_for(size[0], size[1], ui_scale);
        let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, canvas)), ..raw };
        raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(scale);
        (raw, canvas)
    }

    /// Run `page` over `raw`, in the theme's current style.
    pub fn run(&mut self, raw: egui::RawInput, page: impl FnMut(&egui::Context)) -> egui::FullOutput {
        text::restyle(&self.ctx);
        self.ctx.run_pass(raw, page)
    }

    /// Draw `output` into `view` (`size` pixels), cleared to `clear`, with
    /// `extra`'s encoding around the page, and submit it.
    pub fn draw(&mut self, gpu: &Gpu, label: &str, view: &wgpu::TextureView, size: [u32; 2], clear: wgpu::Color, mut output: egui::FullOutput, extra: &mut impl Extra) {
        let jobs = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: size, pixels_per_point: output.pixels_per_point };
        let (device, queue) = (&gpu.device, &gpu.queue);
        // Drain the deltas: epaint asserts (in debug builds) that a
        // TexturesDelta is empty when it is dropped.
        for (id, deltas) in output.textures_delta.set.drain() {
            for delta in &deltas {
                self.renderer.update_texture(device, queue, id, delta);
            }
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
        let mut commands = self.renderer.update_buffers(device, queue, &mut encoder, &jobs, &screen);
        extra.before(&mut encoder);
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(label),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            extra.under(&mut pass);
            self.renderer.render(&mut pass, &jobs, &screen);
        }
        extra.after(&mut encoder);
        commands.push(encoder.finish());
        queue.submit(commands);
        for id in output.textures_delta.free.drain() {
            self.renderer.free_texture(&id);
        }
    }
}

/// What a page's pass encodes besides the page: before the pass, in it
/// before the page, and after it. `()` encodes nothing.
pub trait Extra {
    fn before(&mut self, _encoder: &mut wgpu::CommandEncoder) {}
    fn under(&mut self, _pass: &mut wgpu::RenderPass<'static>) {}
    fn after(&mut self, _encoder: &mut wgpu::CommandEncoder) {}
}

impl Extra for () {}
