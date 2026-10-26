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
