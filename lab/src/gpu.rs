//! The GPU device and the texture formats the lab renders in. The scene's
//! pipelines are in src/pipelines.rs.

use anyhow::{Context, Result};

pub use crate::pipelines::*;

/// The optional features the lab turns on where the adapter has them.
pub const OPTIONAL_FEATURES: wgpu::Features = wgpu::Features::TIMESTAMP_QUERY
    .union(wgpu::Features::TEXTURE_ATOMIC)
    .union(wgpu::Features::CLEAR_TEXTURE)
    .union(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
    .union(wgpu::Features::TEXTURE_BINDING_ARRAY)
    .union(wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING);

pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const OVERLAY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// The RGBA frame shared by the viewer and the recorder. UNORM: shaders
/// write sRGB-encoded values explicitly, and egui draws in gamma space.
pub const FRAME_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const MSAA: u32 = 4;

/// Cloning shares the device: a loading thread builds a renderer on it
/// (src/loading.rs) while the window draws.
#[derive(Clone)]
pub struct Gpu {
    /// Kept for the lifetime of the device and any surface created from it.
    _instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
}

/// An instance over the GPU APIs this build has (Vulkan; Direct3D 12 on
/// Windows, Metal on macOS: lab/Cargo.toml), of which wgpu picks the adapter;
/// `WGPU_BACKEND` (e.g. `vulkan`, `dx12`, `metal`) narrows them. The viewer
/// passes winit's display handle so the instance can present to its surfaces.
pub fn instance(display: Option<Box<dyn wgpu::wgt::WgpuHasDisplayHandle>>) -> wgpu::Instance {
    let mut desc = match display {
        Some(d) => wgpu::InstanceDescriptor::new_with_display_handle(d),
        None => wgpu::InstanceDescriptor::new_without_display_handle(),
    };
    desc.backends = wgpu::Backends::PRIMARY.with_env();
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
        .context("no GPU adapter found (WGPU_BACKEND narrows the APIs tried)")?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("render-lab"),
            // GPU time per pass for experiments (src/timing.rs), and what an
            // investigation's program may use (`ShadingProgram`): texture
            // atomics, texture clears, the adapter's own storage formats and access, and arrays
            // of textures indexed per invocation, where the adapter has them.
            required_features: adapter.features() & OPTIONAL_FEATURES,
            // The verification kernels bind ten storage buffers in one stage.
            required_limits: adapter.limits(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .context("request GPU device")?;
        Ok(Gpu { _instance: instance, adapter, device, queue, info })
    }
}
