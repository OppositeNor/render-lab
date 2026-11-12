//! Deterministic recording: evaluate the timeline by frame index, render
//! each frame into the shared RGBA target, read it back through a ring of
//! buffered GPU transfers, and stream the frames in order to FFmpeg.

use crate::frame::Statics;
use crate::gpu::{self, Gpu};
use crate::investigation;
use crate::renderer::{self, FrameRequest, Renderer};
use crate::shot::{SceneType, Timeline};
use crate::ui::{Chrome, UiReport};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
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
    /// Frames rendered before the range's first and not written (`--warmup`): the history a whole render has there.
    pub warmup: u64,
    /// Width and height of the frames; None for the timeline's.
    pub resolution: Option<[u32; 2]>,
    pub snapshots: Vec<u64>,
    pub snapshot_dir: Option<PathBuf>,
    pub report: Option<PathBuf>,
    pub frame_stamp: bool,
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
    // Two beats of one diagram (or one frame graph scene) in a chapter run on
    // without a fade: the diagram moves to the next beat's focus instead.
    let differs = |a: &crate::shot::TimelineBeat, b: &crate::shot::TimelineBeat| {
        a.kind != b.kind
            || a.chapter != b.chapter
            || (a.kind != SceneType::ThreeD && !a.same_diagram(b))
    };
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

/// Starts FFmpeg encoding RGBA frames `width` x `height` at `fps` into `output`, with
/// the encoder `options` name (its binary, codec, CRF, and preset).
pub fn spawn_ffmpeg(
    options: &RecordOptions,
    [width, height]: [u32; 2],
    fps: u32,
    output: &Path,
) -> Result<Child> {
    let (ffmpeg, codec, crf, preset) = (
        &options.ffmpeg,
        options.codec.as_str(),
        options.crf,
        options.preset.as_str(),
    );
    let mut cmd = Command::new(ffmpeg);
    cmd.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-y",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
    ]);
    cmd.args([
        "-s",
        &format!("{width}x{height}"),
        "-r",
        &fps.to_string(),
        "-i",
        "-",
        "-an",
    ]);
    match codec {
        "h264" => {
            // setparams tags the frames themselves: raw RGBA input carries no colour
            // properties, and unset frame properties override the encoder options.
            cmd.args(["-vf", "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p,setparams=colorspace=bt709:color_primaries=bt709:color_trc=bt709:range=tv"]);
            cmd.args([
                "-c:v",
                "libx264",
                "-preset",
                preset,
                "-crf",
                &crf.to_string(),
                "-pix_fmt",
                "yuv420p",
            ]);
            cmd.args([
                "-colorspace",
                "bt709",
                "-color_primaries",
                "bt709",
                "-color_trc",
                "bt709",
                "-color_range",
                "tv",
                "-movflags",
                "+faststart",
            ]);
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
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    cmd.spawn()
        .with_context(|| format!("start {}", ffmpeg.display()))
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

/// A frame in flight: its index, its read-back buffer's slot, and the buffer's mapping.
type InFlight = (
    u64,
    usize,
    mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
);

/// A recording under way: what it renders and how, its read-back ring and the frames in it, where finished
/// frames go (the FFmpeg writer's channel, when it writes a video), and its summary so far.
struct Recording<'a> {
    options: &'a RecordOptions,
    timeline: &'a Timeline,
    renderer: Renderer,
    device: wgpu::Device,
    buffers: Vec<wgpu::Buffer>,
    pending: VecDeque<InFlight>,
    tx: Option<mpsc::SyncSender<Vec<u8>>>,
    /// The first frame kept (those before it warm up), the end of the range, and the frame's size.
    range: (u64, u64),
    size: [u32; 2],
    record_history: bool,
    last_drawn: Option<(u64, usize)>,
    summary: RecordSummary,
}

pub fn record(options: &RecordOptions) -> Result<RecordSummary> {
    let started = std::time::Instant::now();
    let timeline = Timeline::load(&options.timeline)?;
    crate::i18n::set(timeline.locale);
    let gpu = Gpu::new(gpu::instance(None), None)?;
    let gpu_info = renderer::gpu_summary(&gpu);
    eprintln!("GPU: {}", gpu.info.name);
    let statics = Statics::prepare(crate::investigation::current().first_scene())?;
    let [width, height] = options
        .resolution
        .unwrap_or([timeline.width, timeline.height]);
    let renderer = Renderer::new(gpu, statics, width, height)?;
    let (first, last) = options.frames.unwrap_or((0, timeline.total_frames));
    let last = last.min(timeline.total_frames);
    if first >= last {
        bail!("empty frame range {first}..{last}");
    }
    let mut child = match &options.output {
        Some(path) => Some(spawn_ffmpeg(options, [width, height], timeline.fps, path)?),
        None => None,
    };
    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(RING);
    let writer = child.as_mut().map(|c| spawn_writer(c, rx));
    if let Some(dir) = &options.snapshot_dir {
        std::fs::create_dir_all(dir)?;
    }
    let device = renderer.device().clone();
    let mut recording = Recording {
        options,
        timeline: &timeline,
        buffers: (0..RING)
            .map(|_| renderer::readback_buffer(&device, width, height))
            .collect(),
        renderer,
        device,
        pending: VecDeque::new(),
        tx: writer.is_some().then_some(tx),
        range: (first, last),
        size: [width, height],
        // A program that records with history (`ShadingProgram::record_history`):
        // each frame of a beat builds on the one before it, rendered just before;
        // a beat's first frame, and a frame after a gap, stand alone.
        record_history: options.output.is_some()
            && investigation::current()
                .scene_shading()
                .and_then(|s| s.program)
                .is_some_and(|p| p.record_history),
        last_drawn: None,
        summary: RecordSummary {
            first_frame: first,
            width,
            height,
            fps: timeline.fps,
            ring_buffers: RING,
            min_text_pt: f32::INFINITY,
            gpu: gpu_info,
            output: options.output.as_ref().map(|p| p.display().to_string()),
            ..Default::default()
        },
    };
    // The rate so far, from the first frame on, for the progress line's estimate.
    let rendering = std::time::Instant::now();
    let progress_every = (timeline.fps as u64) * 10;
    for (n, f) in frames_to_render(options, first, last)
        .into_iter()
        .enumerate()
    {
        recording.render(n, f)?;
        if f >= first && options.output.is_some() && (f - first) % progress_every == 0 {
            print_progress(f, (first, last), rendering.elapsed().as_secs_f64());
        }
    }
    while !recording.pending.is_empty() {
        recording.finish_oldest()?;
    }
    let mut summary = std::mem::take(&mut recording.summary);
    drop(recording);
    if let Some(w) = writer {
        let written = w
            .join()
            .expect("writer thread")
            .context("write frames to FFmpeg")?;
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

/// The thread that writes the frames `rx` brings into FFmpeg's input: how many it wrote.
fn spawn_writer(
    ffmpeg: &mut Child,
    rx: mpsc::Receiver<Vec<u8>>,
) -> std::thread::JoinHandle<std::io::Result<u64>> {
    let mut stdin = ffmpeg.stdin.take().expect("ffmpeg stdin");
    std::thread::spawn(move || -> std::io::Result<u64> {
        let mut n = 0;
        for frame in rx {
            stdin.write_all(&frame)?;
            n += 1;
        }
        Ok(n)
    })
}

/// The frames rendered: without a video output only the requested snapshots (each frame depends only on its
/// index, so skipping the others changes nothing); with one, the range and the warm-up before it.
fn frames_to_render(options: &RecordOptions, first: u64, last: u64) -> Vec<u64> {
    if options.output.is_none() {
        let mut v: Vec<u64> = options
            .snapshots
            .iter()
            .copied()
            .filter(|f| (first..last).contains(f))
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    } else {
        (first.saturating_sub(options.warmup)..last).collect()
    }
}

/// The progress line at frame `f` of `first..last`, `elapsed` seconds in: the share done, the rate, and when it
/// is done.
fn print_progress(f: u64, (first, last): (u64, u64), elapsed: f64) {
    let done = f - first;
    let estimate = if done > 0 && elapsed > 0.0 {
        let fps = done as f64 / elapsed;
        let left = ((last - f) as f64 / fps).round() as u64;
        format!(
            " · {fps:.1} fps · {} left, done about {}",
            duration_text(left),
            crate::readback::local_clock_after(left)
        )
    } else {
        String::new()
    };
    eprintln!(
        "frame {f} / {last} ({:.0}%){estimate}",
        100.0 * done as f64 / (last - first) as f64
    );
}

impl Recording<'_> {
    /// Frame `f`, the `n`th rendered, drawn into its read-back slot and its read-back started; the oldest frames
    /// in flight finished first while the slot is in use (slots are used round-robin in render order, so the
    /// slot needed next is always the oldest one in flight: frames leave in the order they entered).
    fn render(&mut self, n: usize, f: u64) -> Result<()> {
        let slot = n % RING;
        while self.pending.len() >= RING || self.pending.iter().any(|p| p.1 == slot) {
            self.finish_oldest()?;
        }
        let timeline = self.timeline;
        let state = timeline.state_at(f);
        let (beat_index, beat) = timeline.beat_at(f);
        let history =
            self.record_history && self.last_drawn == Some((f.wrapping_sub(1), beat_index));
        self.last_drawn = Some((f, beat_index));
        let request = FrameRequest {
            state: &state,
            scene_type: beat.kind,
            diagram: timeline.diagram_at(f),
            beat_sources: &beat.sources,
            notices: beat.notices,
            references: beat.references,
            chapter_number: beat.chapter,
            fade: beat_fade(timeline, f),
            time: f as f32 / timeline.fps as f32,
            raw_input: egui::RawInput {
                time: Some(f as f64 / timeline.fps as f64),
                ..Default::default()
            },
            chrome: Chrome::Recording { timeline, frame: f },
            stamp: self.options.frame_stamp.then_some(f),
            focus: None,
            history,
            wait: true,
        };
        let result = self
            .renderer
            .render_frame(request, Some(&self.buffers[slot]));
        let warming = f < self.range.0;
        if !warming {
            track(&mut self.summary, f, &result.report, beat.kind);
        }
        let (done_tx, done_rx) = mpsc::channel();
        self.buffers[slot]
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                let _ = done_tx.send(r);
            });
        self.pending.push_back((f, slot, done_rx));
        self.device.poll(wgpu::PollType::Poll).ok();
        if !warming {
            self.summary.frames += 1;
        }
        Ok(())
    }

    /// The oldest frame in flight read back: saved when it is a snapshot (and hashed), sent to FFmpeg when
    /// there is a video; a warm-up frame dropped.
    fn finish_oldest(&mut self) -> Result<()> {
        let (frame_index, slot, done) = self.pending.pop_front().expect("a frame in flight");
        self.device.poll(wgpu::PollType::wait_indefinitely()).ok();
        done.recv()
            .context("map callback")?
            .context("map readback buffer")?;
        let buffer = &self.buffers[slot];
        let data = buffer
            .slice(..)
            .get_mapped_range()
            .context("mapped readback")?
            .to_vec();
        buffer.unmap();
        if frame_index < self.range.0 {
            return Ok(());
        }
        let [width, height] = self.size;
        let frame = renderer::unpad_rows(&data, width, height);
        if self.options.snapshots.contains(&frame_index) {
            if let Some(dir) = &self.options.snapshot_dir {
                write_png(
                    &dir.join(format!("frame-{frame_index:06}.png")),
                    width,
                    height,
                    &frame,
                )?;
            }
            self.summary.frame_sha256.push((frame_index, hex(&frame)));
        }
        if let Some(tx) = &self.tx {
            tx.send(frame).context("FFmpeg writer stopped")?;
        }
        Ok(())
    }
}

fn track(summary: &mut RecordSummary, frame: u64, report: &UiReport, kind: SceneType) {
    summary.min_text_pt = summary.min_text_pt.min(report.min_text_pt);
    if kind == SceneType::ThreeD {
        summary.max_label_overlaps = summary.max_label_overlaps.max(report.label_overlaps);
        summary.max_labels_outside = summary.max_labels_outside.max(report.labels_outside);
        if (report.label_overlaps > 0 || report.labels_outside > 0)
            && summary.frames_with_label_issues.len() < 200
        {
            summary.frames_with_label_issues.push(frame);
        }
    }
}

pub fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// FFmpeg: `FFMPEG`, the video build's Conda environment, or PATH.
pub fn default_ffmpeg(manifest_dir: &Path) -> PathBuf {
    if let Some(p) = std::env::var_os("FFMPEG") {
        return PathBuf::from(p);
    }
    let local = manifest_dir.join("video/.conda-env/bin/ffmpeg");
    if local.is_file() {
        local
    } else {
        PathBuf::from("ffmpeg")
    }
}

/// A duration as `1 h 05 min`, `12 min 30 s`, or `45 s`.
fn duration_text(seconds: u64) -> String {
    match seconds {
        s if s >= 3600 => format!("{} h {:02} min", s / 3600, s / 60 % 60),
        s if s >= 60 => format!("{} min {:02} s", s / 60, s % 60),
        s => format!("{s} s"),
    }
}
