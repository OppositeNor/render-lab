//! Reading a part of a GPU resource back, without waiting for it.
//!
//! An investigation asks for a region of one of its shading's resources while
//! it builds a frame (`FrameContent::fetch`): a program image's mip level, a
//! program buffer's bytes, or a volume's mip. After the frame's passes the
//! renderer copies that region into a staging buffer and maps it; a later frame
//! finds it finished, and `get` returns it, decoded, to the next `build_frame`.
//!
//! - A request names its result by a key, and says what the result depends on
//!   by a tag (by default the frame's inputs: its records, uniforms, shading
//!   values, and viewport). A request already answered, or in flight, for the
//!   same source, region, and tag is not issued again: a still view fetches once.
//! - One request per key is in flight at a time; while it is, newer requests of
//!   that key wait, and the one current when it lands goes next. In the viewer a
//!   result therefore trails the image by a frame or two while the view moves,
//!   and `get` returns the latest (its `tag` says which frame it is of).
//! - A frame drawn alone (a recording, a snapshot, a selftest) is settled: when
//!   a request's result is not of this frame's tag, the frame is drawn, the read
//!   waited for, and the frame built again, so it shows its own frame's data.
//! - The viewer copies at most `BUDGET` bytes a frame; the rest wait.
//! - Results belong to the view that asked for them (src/view.rs): `get`
//!   reads the current view's; `get_in` another's, a view drawing what
//!   another computed (a data link).

use crate::view::ViewId;
use glam::Vec4;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// The most bytes the viewer copies back in one frame.
pub const BUDGET: u64 = 16 << 20;

/// The resource a request reads, of the current investigation's scene shading.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// Program image `name`'s element `index` at mip `level`.
    Image {
        name: String,
        index: u32,
        level: u32,
    },
    /// Program buffer `name`'s element `index`; the region is in bytes.
    Buffer { name: String, index: u32 },
    /// Volume `name` at mip `level`.
    Volume { name: String, level: u32 },
}

impl Source {
    /// Program image `name` (its first element) at mip `level`.
    pub fn image(name: &str, level: u32) -> Source {
        Source::Image {
            name: name.to_string(),
            index: 0,
            level,
        }
    }

    /// Program buffer `name` (its first element).
    pub fn buffer(name: &str) -> Source {
        Source::Buffer {
            name: name.to_string(),
            index: 0,
        }
    }

    /// Volume `name` at mip `level`.
    pub fn volume(name: &str, level: u32) -> Source {
        Source::Volume {
            name: name.to_string(),
            level,
        }
    }
}

/// A box of texels (x, y, layer or slice), clipped to the resource when the
/// request is issued; of a buffer, `origin[0]` and `size[0]` are bytes
/// (rounded out to whole words).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Region {
    pub origin: [u32; 3],
    pub size: [u32; 3],
}

impl Region {
    /// The whole resource.
    pub const ALL: Region = Region {
        origin: [0; 3],
        size: [u32::MAX; 3],
    };

    /// A rectangle of a 2D image's first layer.
    pub fn rect(x: u32, y: u32, width: u32, height: u32) -> Region {
        Region {
            origin: [x, y, 0],
            size: [width, height, 1],
        }
    }

    /// The square of texels `radius` around `center` (clipped at the image's edges).
    pub fn around(center: [i32; 2], radius: u32) -> Region {
        let r = radius as i32;
        let x = (center[0] - r).max(0) as u32;
        let y = (center[1] - r).max(0) as u32;
        let right = (center[0] + r + 1).max(0) as u32;
        let bottom = (center[1] + r + 1).max(0) as u32;
        Region::rect(x, y, right.saturating_sub(x), bottom.saturating_sub(y))
    }

    /// Bytes `offset..offset + bytes` of a buffer.
    pub fn bytes(offset: u64, bytes: u64) -> Region {
        Region {
            origin: [offset as u32, 0, 0],
            size: [bytes.min(u32::MAX as u64) as u32, 1, 1],
        }
    }

    /// This region clipped to a resource of `extent`, or None when nothing is left.
    pub fn clip(&self, extent: [u32; 3]) -> Option<Region> {
        let mut out = Region {
            origin: [0; 3],
            size: [0; 3],
        };
        for a in 0..3 {
            if self.origin[a] >= extent[a] {
                return None;
            }
            out.origin[a] = self.origin[a];
            out.size[a] = self.size[a].min(extent[a] - self.origin[a]);
            if out.size[a] == 0 {
                return None;
            }
        }
        Some(out)
    }

    /// Of a buffer of `bytes`: the byte range clipped and rounded out to whole words.
    pub fn clip_bytes(&self, bytes: u64) -> Option<Region> {
        let start = (self.origin[0] as u64 / 4) * 4;
        let end = (self.origin[0] as u64)
            .saturating_add(self.size[0] as u64)
            .min(bytes)
            .div_ceil(4)
            * 4;
        (start < end.min(bytes)).then(|| Region::bytes(start, end.min(bytes) - start))
    }
}

/// A read asked for while building a frame.
#[derive(Clone, Debug)]
pub struct Request {
    /// The result's name, for `get`: one result (and one read in flight) per key.
    pub key: String,
    pub source: Source,
    pub region: Region,
    /// What the result depends on; None is the frame's inputs (its records,
    /// uniforms, shading values, and viewport, as the renderer hashes them).
    pub tag: Option<u64>,
}

/// A finished read.
#[derive(Clone, Debug)]
pub struct Fetched {
    pub source: Source,
    /// The region read, clipped to the resource.
    pub region: Region,
    /// The resource's own size (of its mip; a buffer's `[bytes, 1, 1]`).
    pub extent: [u32; 3],
    /// The texture format, None for a buffer.
    pub format: Option<wgpu::TextureFormat>,
    /// The texels (or bytes), row after row of the region, unpadded.
    pub bytes: Vec<u8>,
    /// The texels as floats, `channels` a texel (empty for a buffer or a format not decoded).
    pub floats: Vec<f32>,
    pub channels: usize,
    /// The tag of the frame it was read after.
    pub tag: u64,
    /// The renderer's frame count when it was read.
    pub frame: u64,
    /// The region as asked for (before clipping), so a request matches its own result.
    asked: Region,
}

impl Fetched {
    /// The texel at `p` (in the resource's coordinates, layer 0), or None outside the region.
    pub fn texel(&self, p: [i32; 2]) -> Option<Vec4> {
        self.texel3([p[0], p[1], 0])
    }

    /// The texel at `p` (x, y, layer or slice), or None outside the region.
    pub fn texel3(&self, p: [i32; 3]) -> Option<Vec4> {
        let mut local = [0usize; 3];
        for a in 0..3 {
            let v = p[a] - self.region.origin[a] as i32;
            if v < 0 || v >= self.region.size[a] as i32 {
                return None;
            }
            local[a] = v as usize;
        }
        let (w, h) = (self.region.size[0] as usize, self.region.size[1] as usize);
        let i = ((local[2] * h + local[1]) * w + local[0]) * self.channels;
        let mut v = [0.0; 4];
        v[..self.channels].copy_from_slice(self.floats.get(i..i + self.channels)?);
        Some(Vec4::from_array(v))
    }

    /// A buffer's bytes as 32-bit words.
    pub fn words(&self) -> Vec<u32> {
        self.bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect()
    }

    /// Whether this is the result of a request for `source` and `region` at `tag`.
    pub fn answers(&self, source: &Source, region: &Region, tag: u64) -> bool {
        self.source == *source && self.tag == tag && self.asked == *region
    }
}

/// The latest result of `key` the current view asked for, if any has landed.
pub fn get(key: &str) -> Option<Arc<Fetched>> {
    get_in(crate::view::current(), key)
}

/// The latest result of `key` that `view` asked for: what a view reads of another's data.
pub fn get_in(view: ViewId, key: &str) -> Option<Arc<Fetched>> {
    store()
        .lock()
        .expect("fetch store")
        .results
        .get(&(view, key.to_string()))
        .cloned()
}

/// Every result dropped.
pub fn clear() {
    store().lock().expect("fetch store").results.clear();
}

/// The results of `view` dropped (its scene changed).
pub fn clear_view(view: ViewId) {
    store()
        .lock()
        .expect("fetch store")
        .results
        .retain(|(v, _), _| *v != view);
}

#[derive(Default)]
struct Store {
    results: HashMap<(ViewId, String), Arc<Fetched>>,
}

fn store() -> &'static Mutex<Store> {
    static STORE: std::sync::OnceLock<Mutex<Store>> = std::sync::OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Store::default()))
}

/// What a request reads, as the renderer finds it.
pub enum Target {
    Texture { texture: wgpu::Texture, level: u32 },
    Buffer(wgpu::Buffer),
}

/// The texel's floats of a format the fetch decodes, and their count.
pub fn channels_of(format: wgpu::TextureFormat) -> Option<usize> {
    use wgpu::TextureFormat as F;
    Some(match format {
        F::R8Unorm
        | F::R8Uint
        | F::R16Float
        | F::R32Float
        | F::R32Uint
        | F::R32Sint
        | F::R16Uint => 1,
        F::Rg8Unorm | F::Rg16Float | F::Rg32Float | F::Rg32Uint => 2,
        F::Rgba8Unorm
        | F::Rgba8UnormSrgb
        | F::Bgra8Unorm
        | F::Rgba16Float
        | F::Rgba32Float
        | F::Rgba32Uint
        | F::Rgb10a2Unorm => 4,
        F::Rg11b10Ufloat => 3,
        _ => return None,
    })
}

/// A half float's value.
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 {
        -1.0
    } else {
        1.0
    };
    let exponent = ((h >> 10) & 0x1f) as i32;
    let mantissa = (h & 0x3ff) as f32;
    sign * match exponent {
        0 => mantissa * 2f32.powi(-24),
        31 => {
            if mantissa == 0.0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
        e => (1.0 + mantissa / 1024.0) * 2f32.powi(e - 15),
    }
}

/// Texels of `format` as floats (unsigned and signed integers as their values; RGB10A2 and RG11B10 unpacked).
pub fn decode(format: wgpu::TextureFormat, bytes: &[u8]) -> Vec<f32> {
    use wgpu::TextureFormat as F;
    let word = |c: &[u8; 4]| u32::from_le_bytes(*c);
    match format {
        F::R8Unorm | F::Rg8Unorm | F::Rgba8Unorm | F::Rgba8UnormSrgb => {
            bytes.iter().map(|&b| b as f32 / 255.0).collect()
        }
        F::Bgra8Unorm => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|c| [c[2], c[1], c[0], c[3]].map(|b| b as f32 / 255.0))
            .collect(),
        F::R8Uint => bytes.iter().map(|&b| b as f32).collect(),
        F::R16Uint => bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c) as f32)
            .collect(),
        F::R16Float | F::Rg16Float | F::Rgba16Float => bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| f16_to_f32(u16::from_le_bytes(*c)))
            .collect(),
        F::R32Float | F::Rg32Float | F::Rgba32Float => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_bits(word(c)))
            .collect(),
        F::R32Uint | F::Rg32Uint | F::Rgba32Uint => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| word(c) as f32)
            .collect(),
        F::R32Sint => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| word(c) as i32 as f32)
            .collect(),
        F::Rgb10a2Unorm => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|c| {
                let w = word(c);
                [
                    (w & 1023) as f32 / 1023.0,
                    ((w >> 10) & 1023) as f32 / 1023.0,
                    ((w >> 20) & 1023) as f32 / 1023.0,
                    (w >> 30) as f32 / 3.0,
                ]
            })
            .collect(),
        F::Rg11b10Ufloat => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|c| {
                let w = word(c);
                // 11- and 10-bit floats: 5 exponent bits, 6 or 5 mantissa bits, no sign.
                let small = |bits: u32, mantissa_bits: u32| {
                    let m =
                        (bits & ((1 << mantissa_bits) - 1)) as f32 / (1u32 << mantissa_bits) as f32;
                    match (bits >> mantissa_bits) as i32 {
                        0 => m * 2f32.powi(-14),
                        31 => f32::INFINITY,
                        e => (1.0 + m) * 2f32.powi(e - 15),
                    }
                };
                [
                    small(w & 0x7ff, 6),
                    small((w >> 11) & 0x7ff, 6),
                    small(w >> 22, 5),
                ]
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// One read in flight: its staging buffer and what it holds.
struct Flight {
    /// The view that asked (its results are its own).
    view: ViewId,
    key: String,
    source: Source,
    asked: Region,
    region: Region,
    extent: [u32; 3],
    format: Option<wgpu::TextureFormat>,
    /// Bytes a row of the region and the row pitch in the staging buffer.
    row: u32,
    pitch: u32,
    rows: u32,
    tag: u64,
    frame: u64,
    staging: wgpu::Buffer,
    /// 0 copied, not yet mapped; 1 mapping; 2 mapped; 3 failed.
    state: Arc<AtomicU8>,
}

/// The renderer's reads in flight and the staging buffers it reuses.
#[derive(Default)]
pub struct Queue {
    flights: Vec<Flight>,
    pool: Vec<wgpu::Buffer>,
    frame: u64,
}

impl Queue {
    /// Whether `request` (at `tag`) needs a read: not answered by the latest
    /// result of its key, nor in flight.
    pub fn wants(&self, request: &Request, tag: u64) -> bool {
        let tag = request.tag.unwrap_or(tag);
        let view = crate::view::current();
        if self.flights.iter().any(|f| {
            f.view == view
                && f.key == request.key
                && f.source == request.source
                && f.asked == request.region
                && f.tag == tag
        }) {
            return false;
        }
        !get(&request.key).is_some_and(|r| r.answers(&request.source, &request.region, tag))
    }

    /// Whether every one of `requests` (at `tag`) is answered by its key's latest result.
    pub fn answered(requests: &[Request], tag: u64) -> bool {
        requests.iter().all(|r| {
            get(&r.key).is_some_and(|f| f.answers(&r.source, &r.region, r.tag.unwrap_or(tag)))
        })
    }

    /// Copy the regions `requests` ask for (those not answered nor in flight,
    /// one a key, while `budget` bytes allow) into staging buffers, after what
    /// `encoder` holds; `find` gives a request's resource. Call `submitted`
    /// after submitting the encoder.
    pub fn issue(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        requests: &[Request],
        tag: u64,
        budget: u64,
        find: &dyn Fn(&Source) -> Option<Target>,
    ) {
        self.frame += 1;
        let mut spent = 0u64;
        for request in requests {
            let view = crate::view::current();
            if !self.wants(request, tag)
                || self
                    .flights
                    .iter()
                    .any(|f| f.view == view && f.key == request.key)
            {
                continue;
            }
            let Some(target) = find(&request.source) else {
                continue;
            };
            let tag = request.tag.unwrap_or(tag);
            match target {
                Target::Texture { texture, level } => self.copy_texture(
                    device,
                    encoder,
                    request,
                    (texture, level),
                    (tag, view),
                    (&mut spent, budget),
                ),
                Target::Buffer(buffer) => {
                    if !buffer.usage().contains(wgpu::BufferUsages::COPY_SRC) {
                        continue;
                    }
                    let Some(region) = request.region.clip_bytes(buffer.size()) else {
                        continue;
                    };
                    let bytes = region.size[0] as u64;
                    if spent > 0 && spent + bytes > budget {
                        continue;
                    }
                    spent += bytes;
                    let staging = self.staging(device, bytes);
                    encoder.copy_buffer_to_buffer(
                        &buffer,
                        region.origin[0] as u64,
                        &staging,
                        0,
                        bytes,
                    );
                    let extent = [buffer.size().min(u32::MAX as u64) as u32, 1, 1];
                    self.flights.push(Flight {
                        view,
                        key: request.key.clone(),
                        source: request.source.clone(),
                        asked: request.region,
                        region,
                        extent,
                        format: None,
                        row: region.size[0],
                        pitch: region.size[0],
                        rows: 1,
                        tag,
                        frame: self.frame,
                        staging,
                        state: Arc::new(AtomicU8::new(0)),
                    });
                }
            }
        }
    }

    /// A request's region of level `level` of `texture` copied into a staging buffer, while the budget allows
    /// (`spent` bytes so far of `budget`): a flight that lands later. Nothing for a texture that cannot be copied.
    fn copy_texture(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        request: &Request,
        (texture, level): (wgpu::Texture, u32),
        (tag, view): (u64, crate::view::ViewId),
        (spent, budget): (&mut u64, u64),
    ) {
        if level >= texture.mip_level_count()
            || !texture.usage().contains(wgpu::TextureUsages::COPY_SRC)
            || texture.sample_count() != 1
        {
            return;
        }
        let format = texture.format();
        let Some(texel) = format.block_copy_size(None) else {
            return;
        };
        let layers = if texture.dimension() == wgpu::TextureDimension::D3 {
            (texture.depth_or_array_layers() >> level).max(1)
        } else {
            texture.depth_or_array_layers()
        };
        let extent = [
            (texture.width() >> level).max(1),
            (texture.height() >> level).max(1),
            layers,
        ];
        let Some(region) = request.region.clip(extent) else {
            return;
        };
        let row = region.size[0] * texel;
        let pitch =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let rows = region.size[1] * region.size[2];
        let bytes = pitch as u64 * rows as u64;
        if *spent > 0 && *spent + bytes > budget {
            return;
        }
        *spent += bytes;
        let staging = self.staging(device, bytes);
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level,
                origin: wgpu::Origin3d {
                    x: region.origin[0],
                    y: region.origin[1],
                    z: region.origin[2],
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(pitch),
                    rows_per_image: Some(region.size[1]),
                },
            },
            wgpu::Extent3d {
                width: region.size[0],
                height: region.size[1],
                depth_or_array_layers: region.size[2],
            },
        );
        self.flights.push(Flight {
            view,
            key: request.key.clone(),
            source: request.source.clone(),
            asked: request.region,
            region,
            extent,
            format: Some(format),
            row,
            pitch,
            rows,
            tag,
            frame: self.frame,
            staging,
            state: Arc::new(AtomicU8::new(0)),
        });
    }

    /// Map the staging buffers copied into by the encoder just submitted.
    pub fn submitted(&mut self) {
        for f in self
            .flights
            .iter()
            .filter(|f| f.state.load(Ordering::Acquire) == 0)
        {
            f.state.store(1, Ordering::Release);
            let state = f.state.clone();
            f.staging
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |r| {
                    state.store(
                        if r.is_ok() {
                            2
                        } else {
                            3
                        },
                        Ordering::Release,
                    )
                });
        }
    }

    /// Store the reads that have landed (polling the device without waiting,
    /// or waiting for all of them with `wait`).
    pub fn collect(&mut self, device: &wgpu::Device, wait: bool) {
        if self.flights.is_empty() {
            return;
        }
        let _ = device.poll(if wait {
            wgpu::PollType::wait_indefinitely()
        } else {
            wgpu::PollType::Poll
        });
        let mut kept = Vec::new();
        for f in std::mem::take(&mut self.flights) {
            match f.state.load(Ordering::Acquire) {
                2 => {
                    let mut bytes = Vec::with_capacity((f.row * f.rows) as usize);
                    let read = match f.staging.slice(..).get_mapped_range() {
                        Ok(mapped) => {
                            for r in 0..f.rows as usize {
                                bytes.extend_from_slice(
                                    &mapped[r * f.pitch as usize
                                        ..r * f.pitch as usize + f.row as usize],
                                );
                            }
                            true
                        }
                        Err(_) => false,
                    };
                    f.staging.unmap();
                    if !read {
                        continue;
                    }
                    let (floats, channels) =
                        match f.format.and_then(|fmt| channels_of(fmt).map(|c| (fmt, c))) {
                            Some((fmt, c)) => (decode(fmt, &bytes), c),
                            None => (Vec::new(), 0),
                        };
                    let fetched = Fetched {
                        source: f.source,
                        region: f.region,
                        extent: f.extent,
                        format: f.format,
                        bytes,
                        floats,
                        channels,
                        tag: f.tag,
                        frame: f.frame,
                        asked: f.asked,
                    };
                    store()
                        .lock()
                        .expect("fetch store")
                        .results
                        .insert((f.view, f.key), Arc::new(fetched));
                    self.pool.push(f.staging);
                }
                3 => {}
                _ => kept.push(f),
            }
        }
        self.flights = kept;
        // A few staging buffers are kept for reuse; the largest first.
        self.pool.sort_by_key(|b| std::cmp::Reverse(b.size()));
        self.pool.truncate(8);
    }

    /// A staging buffer of at least `bytes`, from the pool when one fits.
    fn staging(&mut self, device: &wgpu::Device, bytes: u64) -> wgpu::Buffer {
        let bytes =
            bytes.max(4).div_ceil(wgpu::COPY_BUFFER_ALIGNMENT) * wgpu::COPY_BUFFER_ALIGNMENT;
        // The smallest pooled buffer that holds it.
        let fit = self
            .pool
            .iter()
            .enumerate()
            .filter(|(_, b)| b.size() >= bytes && b.size() <= bytes * 4)
            .min_by_key(|(_, b)| b.size())
            .map(|(i, _)| i);
        match fit {
            Some(i) => self.pool.swap_remove(i),
            None => device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("fetch staging"),
                size: bytes,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_clip_to_the_resource() {
        assert_eq!(
            Region::ALL.clip([8, 4, 1]),
            Some(Region {
                origin: [0; 3],
                size: [8, 4, 1]
            })
        );
        assert_eq!(
            Region::rect(6, 2, 5, 5).clip([8, 4, 1]),
            Some(Region::rect(6, 2, 2, 2))
        );
        assert_eq!(Region::rect(8, 0, 1, 1).clip([8, 4, 1]), None);
        assert_eq!(Region::around([0, 0], 2), Region::rect(0, 0, 3, 3));
        assert_eq!(Region::around([5, 5], 1), Region::rect(4, 4, 3, 3));
        assert_eq!(
            Region::bytes(6, 5).clip_bytes(64),
            Some(Region::bytes(4, 8))
        );
        assert_eq!(
            Region::bytes(60, 10).clip_bytes(64),
            Some(Region::bytes(60, 4))
        );
        assert_eq!(Region::bytes(64, 4).clip_bytes(64), None);
    }

    #[test]
    fn formats_decode() {
        assert_eq!(f16_to_f32(0x3c00), 1.0);
        assert_eq!(f16_to_f32(0xc000), -2.0);
        assert_eq!(f16_to_f32(0x3555), 0.333_251_95);
        assert_eq!(
            decode(wgpu::TextureFormat::Rgba8Unorm, &[0, 255, 51, 102]),
            vec![0.0, 1.0, 0.2, 0.4]
        );
        assert_eq!(
            decode(wgpu::TextureFormat::R32Float, &1.5f32.to_le_bytes()),
            vec![1.5]
        );
        assert_eq!(
            decode(wgpu::TextureFormat::Bgra8Unorm, &[255, 0, 0, 255]),
            vec![0.0, 0.0, 1.0, 1.0]
        );
    }

    #[test]
    fn texels_are_found_in_their_region() {
        let f = Fetched {
            source: Source::image("x", 0),
            region: Region::rect(2, 3, 2, 2),
            extent: [8, 8, 1],
            format: Some(wgpu::TextureFormat::R32Float),
            bytes: Vec::new(),
            floats: vec![1.0, 2.0, 3.0, 4.0],
            channels: 1,
            tag: 7,
            frame: 1,
            asked: Region::rect(2, 3, 2, 2),
        };
        assert_eq!(f.texel([3, 4]).map(|v| v.x), Some(4.0));
        assert_eq!(f.texel([2, 3]).map(|v| v.x), Some(1.0));
        assert_eq!(f.texel([4, 3]), None);
        assert!(f.answers(&Source::image("x", 0), &Region::rect(2, 3, 2, 2), 7));
        assert!(!f.answers(&Source::image("x", 0), &Region::rect(2, 3, 2, 2), 8));
    }
}
