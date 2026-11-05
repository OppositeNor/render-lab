//! What the homepage's and the viewer's windows share: the window and its
//! swapchain, and winit's pointer input as egui's events.

use crate::gpu::{self, Gpu};
use anyhow::Result;
use std::sync::Arc;
use winit::dpi::PhysicalPosition;
use winit::event::{MouseButton, MouseScrollDelta};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

/// Borderless fullscreen on the window's current monitor, or windowed.
pub fn fullscreen_mode(on: bool) -> Option<winit::window::Fullscreen> {
    on.then_some(winit::window::Fullscreen::Borderless(None))
}

/// A window, the GPU, and the window's swapchain, configured.
pub struct Opened {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub gpu: Gpu,
    pub config: wgpu::SurfaceConfiguration,
}

/// Create a window of `attributes`, the GPU for it, and its swapchain: a
/// UNORM one (the frames hold sRGB-encoded bytes), presented at the display's
/// rate, and copyable where the surface allows it (so F11 can save the frame
/// it presents).
pub fn open(event_loop: &ActiveEventLoop, attributes: WindowAttributes) -> Result<Opened> {
    let window = Arc::new(event_loop.create_window(attributes)?);
    let instance = gpu::instance(Some(Box::new(event_loop.owned_display_handle())));
    let surface = instance.create_surface(window.clone())?;
    let gpu = Gpu::new(instance, Some(&surface))?;
    let size = window.inner_size();
    let caps = surface.get_capabilities(&gpu.adapter);
    let format = caps.formats.iter().copied().find(|f| !f.is_srgb() && matches!(f, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm)).unwrap_or(caps.formats[0]);
    let mut config = surface.get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1)).ok_or_else(|| anyhow::anyhow!("surface unsupported"))?;
    config.format = format;
    config.present_mode = wgpu::PresentMode::Fifo;
    if caps.usages.contains(wgpu::TextureUsages::COPY_SRC) {
        config.usage |= wgpu::TextureUsages::COPY_SRC;
    }
    surface.configure(&gpu.device, &config);
    Ok(Opened { window, surface, gpu, config })
}

/// egui's button for a mouse button (left, right, middle; none for the others).
pub fn pointer_button(button: MouseButton) -> Option<egui::PointerButton> {
    match button {
        MouseButton::Left => Some(egui::PointerButton::Primary),
        MouseButton::Right => Some(egui::PointerButton::Secondary),
        MouseButton::Middle => Some(egui::PointerButton::Middle),
        _ => None,
    }
}

/// A wheel turn as egui's event (pixel deltas in points of `scale` pixels),
/// and how many lines it turned (a pixel delta's 40 pixels a line).
pub fn wheel_event(delta: MouseScrollDelta, scale: f32, modifiers: egui::Modifiers) -> (egui::Event, f32) {
    let (unit, scroll, lines) = match delta {
        MouseScrollDelta::LineDelta(x, y) => (egui::MouseWheelUnit::Line, egui::vec2(x, y), y),
        MouseScrollDelta::PixelDelta(PhysicalPosition { x, y }) => (egui::MouseWheelUnit::Point, egui::vec2(x as f32, y as f32) / scale, y as f32 / 40.0),
    };
    (egui::Event::MouseWheel { unit, delta: scroll, phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Unknown, modifiers }, lines)
}
