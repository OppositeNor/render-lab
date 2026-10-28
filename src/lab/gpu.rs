//! The GPU device and the texture formats the lab renders in. The scene's
//! pipelines are in src/lab/pipelines.rs.

use anyhow::{Context, Result};

pub use crate::lab::pipelines::*;

pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const OVERLAY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// The RGBA frame shared by the viewer and the recorder. UNORM: shaders
/// write sRGB-encoded values explicitly, and egui draws in gamma space.
pub const FRAME_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const MSAA: u32 = 4;

/// Cloning shares the device: a loading thread builds a renderer on it
/// (src/lab/loading.rs) while the window draws.
#[derive(Clone)]
pub struct Gpu {
    /// Kept for the lifetime of the device and any surface created from it.
    _instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
}

/// A Vulkan instance. The viewer passes winit's display handle so the
/// instance can present to Wayland or X11 surfaces.
pub fn instance(display: Option<Box<dyn wgpu::wgt::WgpuHasDisplayHandle>>) -> wgpu::Instance {
    let mut desc = match display {
        Some(d) => wgpu::InstanceDescriptor::new_with_display_handle(d),
        None => wgpu::InstanceDescriptor::new_without_display_handle(),
    };
    desc.backends = wgpu::Backends::VULKAN;
    wgpu::Instance::new(desc)
}

impl Gpu {
    pub fn new(instance: wgpu::Instance, surface: Option<&wgpu::Surface<'static>>) -> Result<Gpu> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: surface,
            apply_limit_buckets: false,
        }))
        .context("no Vulkan adapter found")?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("render-lab"),
            required_features: wgpu::Features::empty(),
            // The verification kernels bind ten storage buffers in one stage.
            required_limits: adapter.limits(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .context("request Vulkan device")?;
        Ok(Gpu { _instance: instance, adapter, device, queue, info })
    }
}
