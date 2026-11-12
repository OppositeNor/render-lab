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

/// Hardware ray tracing (inline ray queries, Vulkan only in wgpu 30): requested where the adapter has it, unless
/// `RENDER_LAB_RAY_TRACING=off`. wgpu marks it experimental. A package that needs it says so in its manifest
/// (`requires = ["ray-tracing"]`, src/packages.rs); its passes trace the scene's TLAS (src/ray_tracing.rs).
pub const RAY_TRACING_FEATURES: wgpu::Features = wgpu::Features::EXPERIMENTAL_RAY_QUERY;

/// Whether the device the lab made traces rays (set when it is made).
static RAY_TRACING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the lab's device traces rays in hardware.
pub fn ray_tracing() -> bool {
    RAY_TRACING.load(std::sync::atomic::Ordering::Relaxed)
}

/// Whether ray tracing is turned off by `RENDER_LAB_RAY_TRACING=off`.
fn ray_tracing_disabled() -> bool {
    std::env::var("RENDER_LAB_RAY_TRACING").is_ok_and(|v| v.eq_ignore_ascii_case("off"))
}

/// Whether this machine's GPU would trace rays, without making a device: the adapter the lab would pick has ray
/// queries and they are not turned off. Asked once, for the packages that require it (src/investigations.rs).
pub fn ray_tracing_supported() -> bool {
    static SUPPORTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        if ray_tracing_disabled() {
            return false;
        }
        let instance = instance(None);
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .is_ok_and(|a| a.features().contains(RAY_TRACING_FEATURES))
    })
}

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
        let rays = !ray_tracing_disabled() && adapter.features().contains(RAY_TRACING_FEATURES);
        let features = adapter.features() & OPTIONAL_FEATURES;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("render-lab"),
            // GPU time per pass for experiments (src/timing.rs), and what an
            // investigation's program may use (`ShadingProgram`): texture
            // atomics, texture clears, the adapter's own storage formats and access, and arrays
            // of textures indexed per invocation, where the adapter has them.
            // Ray queries too, where the adapter has them: wgpu calls them experimental, so the device opts in.
            required_features: if rays {
                features | RAY_TRACING_FEATURES
            } else {
                features
            },
            // The verification kernels bind ten storage buffers in one stage.
            required_limits: adapter.limits(),
            experimental_features: if rays {
                // SAFETY: only the ray-query feature is experimental here, and only packages that require it use it.
                unsafe { wgpu::ExperimentalFeatures::enabled() }
            } else {
                wgpu::ExperimentalFeatures::disabled()
            },
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .context("request GPU device")?;
        RAY_TRACING.store(rays, std::sync::atomic::Ordering::Relaxed);
        Ok(Gpu {
            _instance: instance,
            adapter,
            device,
            queue,
            info,
        })
    }
}
