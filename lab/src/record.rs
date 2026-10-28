//! Deterministic recording: evaluate the timeline by frame index, render
//! each frame into the shared RGBA target, read it back through a ring of
//! buffered GPU transfers, and stream the frames in order to FFmpeg.

use crate::frame::Statics;
use crate::gpu::{self, Gpu};
use crate::renderer::{self, FrameRequest, Renderer};
use crate::shot::{BeatKind, Timeline};
use crate::ui::{Chrome, UiReport};
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;

pub use crate::readback::write_png;

pub const RING: usize = 4;
pub const FADE_FRAMES: u64 = 9;

#[derive(Clone, Debug)]
pub struct RecordOptions {
    pub timeline: PathBuf,
    pub output: Option<PathBuf>,
    pub ffmpeg: PathBuf,
    pub codec: String,
    pub crf: u32,
    pub preset: String,
    pub frames: Option<(u64, u64)>,
    /// Width and height of the frames; None for the timeline's.
    pub resolution: Option<[u32; 2]>,
    pub snapshots: Vec<u64>,
    pub snapshot_dir: Option<PathBuf>,
    pub report: Option<PathBuf>,
    pub frame_stamp: bool,
}

/// Mermaid frames referenced by the timeline, loaded on demand as egui textures.
pub struct ImageCache {
    base: PathBuf,
    images: HashMap<String, (egui::TextureHandle, egui::Vec2)>,
}

impl ImageCache {
    pub fn new(base: &Path) -> Self {
        ImageCache { base: base.to_path_buf(), images: HashMap::new() }
    }

    pub fn get(&mut self, ctx: &egui::Context, path: &str) -> Result<(egui::TextureId, egui::Vec2)> {
        if !self.images.contains_key(path) {
            let full = self.base.join(path);
            let file = std::io::BufReader::new(std::fs::File::open(&full).with_context(|| format!("open {}", full.display()))?);
            let image = decode_png(file).with_context(|| format!("decode {}", full.display()))?;
            let size = egui::vec2(image.size[0] as f32, image.size[1] as f32);
            let handle = ctx.load_texture(path, image, egui::TextureOptions::LINEAR);
            self.images.insert(path.to_string(), (handle, size));
        }
        let (h, s) = &self.images[path];
        Ok((h.id(), *s))
    }

    /// Keep only the images for the given paths (frees GPU memory).
    pub fn retain(&mut self, keep: &[&str]) {
        self.images.retain(|k, _| keep.contains(&k.as_str()));
    }
}

/// An RGB or RGBA PNG as an egui image.
pub fn decode_png(data: impl std::io::BufRead + std::io::Seek) -> Result<egui::ColorImage> {
    let mut reader = png::Decoder::new(data).read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().context("png size")?];
    let info = reader.next_frame(&mut buf)?;
    let size = [info.width as usize, info.height as usize];
    let pixels = &buf[..info.buffer_size()];
    Ok(match info.color_type {
        png::ColorType::Rgba => egui::ColorImage::from_rgba_unmultiplied(size, pixels),
        png::ColorType::Rgb => egui::ColorImage::from_rgb(size, pixels),
        other => bail!("unsupported PNG colour type {other:?}"),
    })
}

/// Fade toward the background at changes of beat kind or chapter.
pub fn beat_fade(timeline: &Timeline, frame: u64) -> f32 {
    let (i, beat) = timeline.beat_at(frame);
    let local = frame - beat.start_frame;
    let mut fade = 0.0f32;
    let differs = |a: &crate::shot::TimelineBeat, b: &crate::shot::TimelineBeat| a.kind != b.kind || a.chapter != b.chapter || a.kind != BeatKind::Scene;
    if i > 0 && differs(&timeline.beats[i - 1], beat) && local < FADE_FRAMES {
        fade = fade.max(1.0 - (local as f32 + 1.0) / (FADE_FRAMES as f32 + 1.0));
    }
    if let Some(next) = timeline.beats.get(i + 1) {
        let remaining = beat.frames - local;
        if differs(beat, next) && remaining <= FADE_FRAMES {
            fade = fade.max(1.0 - (remaining as f32) / (FADE_FRAMES as f32 + 1.0));
        }
    }
    fade
}

pub fn spawn_ffmpeg(ffmpeg: &Path, width: u32, height: u32, fps: u32, codec: &str, crf: u32, preset: &str, output: &Path) -> Result<Child> {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-f", "rawvideo", "-pix_fmt", "rgba"]);
    cmd.args(["-s", &format!("{width}x{height}"), "-r", &fps.to_string(), "-i", "-", "-an"]);
    match codec {
        "h264" => {
            // setparams tags the frames themselves: raw RGBA input carries no colour
            // properties, and unset frame properties override the encoder options.
            cmd.args(["-vf", "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p,setparams=colorspace=bt709:color_primaries=bt709:color_trc=bt709:range=tv"]);
            cmd.args(["-c:v", "libx264", "-preset", preset, "-crf", &crf.to_string(), "-pix_fmt", "yuv420p"]);
            cmd.args(["-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-color_range", "tv", "-movflags", "+faststart"]);
        }
        "ffv1" => {
            cmd.args(["-c:v", "ffv1", "-pix_fmt", "rgba"]);
        }
        "rawvideo" => {
            cmd.args(["-c:v", "rawvideo", "-pix_fmt", "rgba"]);
        }
        other => bail!("unknown codec {other}"),
    }
    cmd.arg(output);
    cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::inherit());
    cmd.spawn().with_context(|| format!("start {}", ffmpeg.display()))
}

#[derive(Debug, Default, serde::Serialize)]
pub struct RecordSummary {
    pub frames: u64,
    pub first_frame: u64,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub ring_buffers: usize,
    pub min_text_pt: f32,
    pub max_label_overlaps: usize,
    pub max_labels_outside: usize,
    pub frames_with_label_issues: Vec<u64>,
    pub frame_sha256: Vec<(u64, String)>,
    pub seconds: f64,
    pub gpu: serde_json::Value,
    pub output: Option<String>,
}

/// Read the 24-bit frame index that `--frame-stamp` draws in the top-left
/// corner (one 8x8 block per bit: red = 1, green = 0).
pub fn read_stamp(frame: &[u8], width: u32) -> u64 {
    (0..24u32).fold(0u64, |acc, bit| {
        let px = ((4 * width + bit * 8 + 4) * 4) as usize;
        if frame[px] > 127 { acc | (1 << bit) } else { acc }
    })
}

pub fn record(options: &RecordOptions) -> Result<RecordSummary> {
    let started = std::time::Instant::now();
    let timeline = Timeline::load(&options.timeline)?;
    crate::i18n::set(timeline.locale);
    let base = options.timeline.parent().unwrap_or(Path::new(".")).to_path_buf();
    let gpu = Gpu::new(gpu::instance(None), None)?;
    let gpu_info = renderer::gpu_summary(&gpu);
    eprintln!("GPU: {}", gpu.info.name);
    let statics = Statics::prepare(crate::investigation::current().first_scene())?;
    let [width, height] = options.resolution.unwrap_or([timeline.width, timeline.height]);
    let mut renderer = Renderer::new(gpu, statics, width, height)?;
    let (first, last) = options.frames.unwrap_or((0, timeline.total_frames));
    let last = last.min(timeline.total_frames);
    if first >= last {
        bail!("empty frame range {first}..{last}");
    }
    let mut child = match &options.output {
        Some(path) => Some(spawn_ffmpeg(&options.ffmpeg, width, height, timeline.fps, &options.codec, options.crf, &options.preset, path)?),
        None => None,
    };
    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(RING);
    let writer = child.as_mut().map(|c| {
        let mut stdin = c.stdin.take().expect("ffmpeg stdin");
        std::thread::spawn(move || -> std::io::Result<u64> {
            let mut n = 0;
            for frame in rx {
                stdin.write_all(&frame)?;
                n += 1;
            }
            Ok(n)
        })
    });
    let device = renderer.device().clone();
    let buffers: Vec<wgpu::Buffer> = (0..RING).map(|_| renderer::readback_buffer(&device, width, height)).collect();
    let mut pending: VecDeque<(u64, usize, mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>)> = VecDeque::new();
    let mut images = ImageCache::new(&base);
    let mut summary = RecordSummary { first_frame: first, width, height, fps: timeline.fps, ring_buffers: RING, min_text_pt: f32::INFINITY, gpu: gpu_info, output: options.output.as_ref().map(|p| p.display().to_string()), ..Default::default() };
    if let Some(dir) = &options.snapshot_dir {
        std::fs::create_dir_all(dir)?;
    }
    let finish = |frame_index: u64, slot: usize, summary: &mut RecordSummary| -> Result<()> {
        let buffer = &buffers[slot];
        let data = buffer.slice(..).get_mapped_range().context("mapped readback")?.to_vec();
        buffer.unmap();
        let frame = renderer::unpad_rows(&data, width, height);
        if options.snapshots.contains(&frame_index) {
            if let Some(dir) = &options.snapshot_dir {
                write_png(&dir.join(format!("frame-{frame_index:06}.png")), width, height, &frame)?;
            }
            summary.frame_sha256.push((frame_index, hex(&frame)));
        }
        if writer.is_some() {
            tx.send(frame).context("FFmpeg writer stopped")?;
        }
        Ok(())
    };
    let progress_every = (timeline.fps as u64) * 10;
    // Without a video output only the requested snapshots are rendered: each
    // frame depends only on its index, so skipping the others changes nothing.
    let frame_list: Vec<u64> = if options.output.is_none() {
        let mut v: Vec<u64> = options.snapshots.iter().copied().filter(|f| (first..last).contains(f)).collect();
        v.sort_unstable();
        v.dedup();
        v
    } else {
        (first..last).collect()
    };
    for (n, f) in frame_list.into_iter().enumerate() {
        // Slots are used round-robin in render order, so the slot needed next
        // is always the oldest one in flight: frames leave in the order they entered.
        let slot = n % RING;
        while pending.len() >= RING || pending.iter().any(|p| p.1 == slot) {
            let (index, s, done) = pending.pop_front().unwrap();
            device.poll(wgpu::PollType::wait_indefinitely()).ok();
            done.recv().context("map callback")?.context("map readback buffer")?;
            finish(index, s, &mut summary)?;
        }
        let state = timeline.state_at(f);
        let (_, beat) = timeline.beat_at(f);
        let image = match (&beat.kind, &beat.image) {
            (BeatKind::Mermaid, Some(path)) => {
                images.retain(&[path.as_str()]);
                Some(images.get(&renderer.egui_ctx, path)?)
            }
            _ => None,
        };
        let request = FrameRequest {
            state: &state,
            beat_kind: beat.kind,
            image,
            beat_sources: &beat.sources,
            chapter_number: beat.chapter,
            fade: beat_fade(&timeline, f),
            time: f as f32 / timeline.fps as f32,
            raw_input: egui::RawInput { time: Some(f as f64 / timeline.fps as f64), ..Default::default() },
            chrome: Chrome::Recording { timeline: &timeline, frame: f },
            stamp: options.frame_stamp.then_some(f),
        };
        let result = renderer.render_frame(request, Some(&buffers[slot]));
        track(&mut summary, f, &result.report, beat.kind);
        let (done_tx, done_rx) = mpsc::channel();
        buffers[slot].slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = done_tx.send(r);
        });
        pending.push_back((f, slot, done_rx));
        device.poll(wgpu::PollType::Poll).ok();
        summary.frames += 1;
        if options.output.is_some() && (f - first) % progress_every == 0 {
            eprintln!("frame {f} / {last} ({:.0}%)", 100.0 * (f - first) as f64 / (last - first) as f64);
        }
    }
    while let Some((index, s, done)) = pending.pop_front() {
        device.poll(wgpu::PollType::wait_indefinitely()).ok();
        done.recv().context("map callback")?.context("map readback buffer")?;
        finish(index, s, &mut summary)?;
    }
    drop(tx);
    if let Some(w) = writer {
        let written = w.join().expect("writer thread").context("write frames to FFmpeg")?;
        if written != summary.frames {
            bail!("wrote {written} frames, rendered {}", summary.frames);
        }
    }
    if let Some(mut c) = child {
        let status = c.wait()?;
        if !status.success() {
            bail!("FFmpeg exited with {status}");
        }
    }
    summary.seconds = started.elapsed().as_secs_f64();
    if let Some(path) = &options.report {
        std::fs::write(path, serde_json::to_string_pretty(&summary)?)?;
    }
    Ok(summary)
}

fn track(summary: &mut RecordSummary, frame: u64, report: &UiReport, kind: BeatKind) {
    summary.min_text_pt = summary.min_text_pt.min(report.min_text_pt);
    if kind == BeatKind::Scene {
        summary.max_label_overlaps = summary.max_label_overlaps.max(report.label_overlaps);
        summary.max_labels_outside = summary.max_labels_outside.max(report.labels_outside);
        if (report.label_overlaps > 0 || report.labels_outside > 0) && summary.frames_with_label_issues.len() < 200 {
            summary.frames_with_label_issues.push(frame);
        }
    }
}

pub fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// FFmpeg: `FFMPEG`, the video build's Conda environment, or PATH.
pub fn default_ffmpeg(manifest_dir: &Path) -> PathBuf {
    if let Some(p) = std::env::var_os("FFMPEG") {
        return PathBuf::from(p);
    }
    let local = manifest_dir.join("video/.conda-env/bin/ffmpeg");
    if local.is_file() { local } else { PathBuf::from("ffmpeg") }
}
