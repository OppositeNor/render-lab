//! Blocking reads of what the renderer holds: a program's images, a volume,
//! the views' frames, a chain buffer, and the frame.

use super::*;

impl Renderer {
    /// Element `index` of the current shading's program resource `name`, as
    /// the last frame left it (a selftest): an image's texels (rows unpadded,
    /// slice after slice or layer after layer) and its size, or a buffer's
    /// bytes and `[bytes, 1, 1]`.
    pub fn read_program(&self, name: &str, index: u32) -> Option<(Vec<u8>, [u32; 3])> {
        self.read_program_mip(name, index, 0)
    }

    /// `read_program` of an image's mip level `mip` (rows unpadded; its size is that level's).
    pub fn read_program_mip(
        &self,
        name: &str,
        index: u32,
        mip: u32,
    ) -> Option<(Vec<u8>, [u32; 3])> {
        let source = investigation::current().scene_shading()?;
        let shading = self
            .shadings
            .iter()
            .find(|s| std::ptr::eq(s.source, source))?;
        let handle = shading.program_handle(name)?;
        let device = &self.gpu.device;
        match &handle {
            crate::program::ProgramHandle::Buffer(buffers) => {
                let b = buffers.get(index as usize)?;
                let readback = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("program readback"),
                    size: b.size(),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
                let mut encoder = device.create_command_encoder(&Default::default());
                encoder.copy_buffer_to_buffer(b, 0, &readback, 0, b.size());
                self.gpu.queue.submit([encoder.finish()]);
                Some((
                    read_buffer(device, &self.gpu.queue, &readback, 0, b.size()),
                    [b.size() as u32, 1, 1],
                ))
            }
            crate::program::ProgramHandle::Image { textures, .. } => {
                let texture = textures.get(index as usize)?;
                if mip >= texture.mip_level_count() {
                    return None;
                }
                let layers = if texture.dimension() == wgpu::TextureDimension::D3 {
                    (texture.depth_or_array_layers() >> mip).max(1)
                } else {
                    texture.depth_or_array_layers()
                };
                let size = [
                    (texture.width() >> mip).max(1),
                    (texture.height() >> mip).max(1),
                    layers,
                ];
                let texel = texture.format().block_copy_size(None)?;
                let row = size[0] * texel;
                let padded = row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
                    * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("program readback"),
                    size: padded as u64 * size[1] as u64 * size[2] as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
                let mut encoder = device.create_command_encoder(&Default::default());
                encoder.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture,
                        mip_level: mip,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: &buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(padded),
                            rows_per_image: Some(size[1]),
                        },
                    },
                    wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: size[2],
                    },
                );
                self.gpu.queue.submit([encoder.finish()]);
                let bytes = read_buffer(device, &self.gpu.queue, &buffer, 0, buffer.size());
                let mut out = Vec::with_capacity((row * size[1] * size[2]) as usize);
                for r in 0..(size[1] * size[2]) as usize {
                    out.extend_from_slice(
                        &bytes[r * padded as usize..r * padded as usize + row as usize],
                    );
                }
                Some((out, size))
            }
        }
    }

    /// The resource a fetch reads (src/fetch.rs), of the current scene shading.
    pub(super) fn fetch_target(
        &self,
        source: &crate::fetch::Source,
    ) -> Option<crate::fetch::Target> {
        use crate::fetch::{Source, Target};
        use crate::program::ProgramHandle;
        let shading_source = investigation::current().scene_shading()?;
        let shading = self
            .shadings
            .iter()
            .find(|s| std::ptr::eq(s.source, shading_source))?;
        match source {
            Source::Image { name, index, level } => match shading.program_handle(name)? {
                ProgramHandle::Image { textures, .. } => Some(Target::Texture {
                    texture: textures.get(*index as usize)?.clone(),
                    level: *level,
                }),
                ProgramHandle::Buffer(_) => None,
            },
            Source::Buffer { name, index } => match shading.program_handle(name)? {
                ProgramHandle::Buffer(buffers) => {
                    Some(Target::Buffer(buffers.get(*index as usize)?.clone()))
                }
                ProgramHandle::Image { .. } => None,
            },
            Source::Volume { name, level } => {
                let v = shading_source.volumes.iter().position(|v| v.name == name)?;
                Some(Target::Texture {
                    texture: shading.volume_sets.get(shading.volume_set)?.textures[v].clone(),
                    level: *level,
                })
            }
        }
    }

    /// Mip `mip` of the current shading's volume `name`, as the state last
    /// drawn (or prepared) left it: its texels, rows unpadded, and its size.
    pub fn read_volume(&self, name: &str, mip: u32) -> Option<(Vec<u8>, [u32; 3])> {
        let source = investigation::current().scene_shading()?;
        let shading = self
            .shadings
            .iter()
            .find(|s| std::ptr::eq(s.source, source))?;
        let v = source.volumes.iter().position(|v| v.name == name)?;
        let volume = &source.volumes[v];
        let texture = &shading.volume_sets.get(shading.volume_set)?.textures[v];
        let size = [
            (volume.size[0] >> mip).max(1),
            (volume.size[1] >> mip).max(1),
            (volume.size[2] >> mip).max(1),
        ];
        let texel = volume.format.block_copy_size(None)?;
        let row = size[0] * texel;
        let padded =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let device = &self.gpu.device;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("volume readback"),
            size: padded as u64 * size[1] as u64 * size[2] as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: mip,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(size[1]),
                },
            },
            wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: size[2],
            },
        );
        self.gpu.queue.submit([encoder.finish()]);
        let bytes = read_buffer(device, &self.gpu.queue, &buffer, 0, buffer.size());
        let texels = bytes
            .chunks(padded as usize)
            .flat_map(|r| r[..row as usize].iter().copied())
            .collect();
        Some((texels, size))
    }

    /// The last frame's image of a view, without the overlay: displayed
    /// (tone mapped, sRGB-encoded) and linear (its HDR target). None when it
    /// was not drawn (the reference without a comparison).
    pub fn read_view(
        &mut self,
        which: ViewKind,
    ) -> Option<(crate::metrics::Image, crate::metrics::Image)> {
        if which == ViewKind::Reference && !self.reference_drawn {
            return None;
        }
        let targets = match which {
            ViewKind::Live => self.viewport.as_ref()?,
            ViewKind::Reference => self.slots.get(&ViewId::REFERENCE)?.viewport.as_ref()?,
        };
        let device = &self.gpu.device;
        let [w, h] = targets.size;
        // Displayed: the scene composite into a frame-sized target of its own.
        let display = target(
            device,
            "experiment display",
            [w, h],
            FRAME_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let display_view = display.create_view(&Default::default());
        let rgba_row = padded_row_bytes(w);
        let rgba_buffer = readback_buffer(device, w, h);
        let half_row = (w * 8).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let half_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("experiment hdr"),
            size: half_row as u64 * h as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("experiment readback"),
        });
        self.composite_display(&mut encoder, &display_view, targets);
        let extent = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &display,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &rgba_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(rgba_row),
                    rows_per_image: Some(h),
                },
            },
            extent,
        );
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &targets.hdr_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &half_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(half_row),
                    rows_per_image: Some(h),
                },
            },
            extent,
        );
        self.gpu.queue.submit([encoder.finish()]);
        let rgba = unpad_rows(
            &read_buffer(
                device,
                &self.gpu.queue,
                &rgba_buffer,
                0,
                rgba_row as u64 * h as u64,
            ),
            w,
            h,
        );
        let words = self.read_halves(&half_buffer, half_row, [w, h]);
        Some((
            crate::metrics::Image::from_rgba8(w as usize, h as usize, &rgba),
            crate::metrics::Image::from_rgba16f(w as usize, h as usize, &words),
        ))
    }

    /// The scene composite of `targets` into `display_view` (as displayed: tone mapped, sRGB-encoded).
    fn composite_display(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        display_view: &wgpu::TextureView,
        targets: &ViewportTargets,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("experiment display"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: display_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipelines.composite_scene);
        pass.set_bind_group(0, &self.frame_bind_group, &[]);
        pass.set_bind_group(1, &targets.composite, &[]);
        pass.draw(0..3, 0..1);
    }

    /// The half floats of a `w` × `h` RGBA16F image read back into `half_buffer` (rows `half_row` bytes apart).
    fn read_halves(&self, half_buffer: &wgpu::Buffer, half_row: u32, [w, h]: [u32; 2]) -> Vec<u16> {
        let device = &self.gpu.device;
        let halves_padded = read_buffer(
            device,
            &self.gpu.queue,
            half_buffer,
            0,
            half_row as u64 * h as u64,
        );
        let mut words = Vec::with_capacity((w * h * 4) as usize);
        for row in halves_padded.chunks_exact(half_row as usize) {
            words.extend(
                row[..(w * 8) as usize]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| u16::from_le_bytes(*b)),
            );
        }
        words
    }

    /// A buffer of the current shading's pass chain as the last frame left it
    /// (selftests), with the viewport size it was made for.
    pub fn read_chain_buffer(&self, name: &str) -> Option<(Vec<u32>, [u32; 2])> {
        let chain = self.chain.as_ref()?;
        let (_, buffer) = chain.buffers.iter().find(|(n, _)| *n == name)?;
        let bytes = read_buffer(&self.gpu.device, &self.gpu.queue, buffer, 0, buffer.size());
        Some((
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c))
                .collect(),
            chain.size,
        ))
    }

    /// Blocking read of the current frame (unpadded RGBA rows).
    pub fn read_frame(&self) -> Vec<u8> {
        crate::readback::read_texture_rgba(
            &self.gpu.device,
            &self.gpu.queue,
            &self.frame,
            [self.width, self.height],
        )
    }
}
