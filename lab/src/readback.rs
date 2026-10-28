//! Reading a rendered frame back from the GPU, and saving it as a PNG.

use anyhow::{Context, Result};
use std::path::Path;

pub fn padded_row_bytes(width: u32) -> u32 {
    let unpadded = width * 4;
    unpadded.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}

pub fn readback_buffer(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frame readback"),
        size: padded_row_bytes(width) as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

/// Drop the per-row padding that buffer copies require.
pub fn unpad_rows(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let padded = padded_row_bytes(width) as usize;
    let row = width as usize * 4;
    let mut out = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        out.extend_from_slice(&data[y * padded..y * padded + row]);
    }
    out
}

/// Map a buffer (copying through a staging buffer if needed) and return its bytes.
pub fn read_buffer(device: &wgpu::Device, queue: &wgpu::Queue, buffer: &wgpu::Buffer, offset: u64, size: u64) -> Vec<u8> {
    let staging;
    let source = if buffer.usage().contains(wgpu::BufferUsages::MAP_READ) {
        buffer
    } else {
        staging = device.create_buffer(&wgpu::BufferDescriptor { label: Some("staging"), size, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(buffer, offset, &staging, 0, size);
        queue.submit([encoder.finish()]);
        &staging
    };
    let range = if std::ptr::eq(source, buffer) { offset..offset + size } else { 0..size };
    let slice = source.slice(range);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll");
    rx.recv().expect("map callback").expect("map buffer");
    let data = slice.get_mapped_range().expect("mapped range").to_vec();
    source.unmap();
    data
}

pub fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path).with_context(|| format!("create {}", path.display()))?);
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    Ok(())
}

/// F11 in a window: the frame about to be presented (a swapchain texture
/// configured with `COPY_SRC`, after its rendering was submitted), saved as
/// `build/screenshots/render-lab-2026-10-27_22-15-43.png` (the local date and
/// time; `-2`, `-3`, … for more in the same second) under `root`.
pub fn screenshot(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, root: &Path) -> Result<std::path::PathBuf> {
    let (width, height) = (texture.width(), texture.height());
    let bgra = match texture.format() {
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => true,
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => false,
        format => anyhow::bail!("cannot save a {format:?} window"),
    };
    anyhow::ensure!(texture.usage().contains(wgpu::TextureUsages::COPY_SRC), "the window cannot be copied (its surface lacks COPY_SRC)");
    let buffer = readback_buffer(device, width, height);
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded_row_bytes(width)), rows_per_image: Some(height) } },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
    let mut rgba = unpad_rows(&read_buffer(device, queue, &buffer, 0, buffer.size()), width, height);
    for pixel in rgba.chunks_exact_mut(4) {
        if bgra {
            pixel.swap(0, 2);
        }
        // The window shows the frame opaque, whatever its alpha.
        pixel[3] = 255;
    }
    let dir = root.join("build").join("screenshots");
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let stamp = local_stamp();
    let path = (1..)
        .map(|n| dir.join(if n == 1 { format!("render-lab-{stamp}.png") } else { format!("render-lab-{stamp}-{n}.png") }))
        .find(|p| !p.exists())
        .unwrap();
    write_png(&path, width, height, &rgba)?;
    Ok(path)
}

/// The local date and time, `2026-10-27_22-15-43`; in UTC, marked `Z`, where
/// the local time zone is unknown.
fn local_stamp() -> String {
    #[cfg(unix)]
    {
        // SAFETY: `time` with a null pointer only returns the time, and
        // `localtime_r` writes only to the `tm` it is given.
        let now = unsafe { libc::time(std::ptr::null_mut()) };
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if !unsafe { libc::localtime_r(&now, &mut tm) }.is_null() {
            return format!("{:04}-{:02}-{:02}_{:02}-{:02}-{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec);
        }
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let (days, seconds) = ((now.as_secs() / 86_400) as i64, now.as_secs() % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year}-{month:02}-{day:02}_{:02}-{:02}-{:02}Z", seconds / 3600, seconds / 60 % 60, seconds % 60)
}
