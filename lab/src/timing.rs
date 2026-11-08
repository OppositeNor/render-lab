//! GPU time per pass, for experiments (src/experiment.rs): when the device
//! has timestamp queries, the renderer's live frame writes a pair around
//! each of its passes (the depth views, the pre-pass, each compute pass of
//! the investigation's chain, the scene pass, the overlay, the composite).
//! The pairs are resolved into a buffer at the end of the frame and read a
//! few frames later without waiting (`submitted`), so the viewer never
//! stalls; a research run waits for them (`finish`).

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Pairs a frame can time.
const PAIRS: u32 = 64;
/// Frames whose timestamps may be on their way back at once.
const RING: usize = 3;
/// Frames each pass's statistics cover.
const HISTORY: usize = 120;

/// A pass's two timestamps (the start and the end of a pass).
pub struct Span {
    set: wgpu::QuerySet,
    begin: u32,
}

impl Span {
    pub fn writes(&self) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(self.begin),
            end_of_pass_write_index: Some(self.begin + 1),
        }
    }

    /// For one pass of several the span covers: the start in the first, the end in the last.
    pub fn span(&self, first: bool, last: bool) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: first.then_some(self.begin),
            end_of_pass_write_index: last.then_some(self.begin + 1),
        }
    }

    pub fn compute(&self) -> wgpu::ComputePassTimestampWrites<'_> {
        wgpu::ComputePassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(self.begin),
            end_of_pass_write_index: Some(self.begin + 1),
        }
    }
}

/// One frame's timestamps on their way back: its labels and the buffer they are read into.
struct InFlight {
    buffer: wgpu::Buffer,
    labels: Vec<String>,
    mapped: Arc<AtomicBool>,
    waiting: bool,
}

pub struct Timing {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    /// Nanoseconds per timestamp tick.
    period: f64,
    /// This frame's labels, a pair each, in query order.
    labels: Vec<String>,
    ring: Vec<InFlight>,
    next: usize,
    /// Each pass's recent durations in milliseconds, in the order passes first appeared.
    history: Vec<(String, VecDeque<f64>)>,
    /// The last complete frame's durations.
    last: Vec<(String, f64)>,
    /// Each recent frame's GPU time, all its passes together, oldest first.
    totals: VecDeque<f64>,
}

impl Timing {
    /// None when the device has no timestamp queries.
    pub fn new(gpu: &crate::gpu::Gpu) -> Option<Timing> {
        if !gpu
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            return None;
        }
        let device = &gpu.device;
        let bytes = PAIRS as u64 * 2 * 8;
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("pass timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: PAIRS * 2,
        });
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timestamp resolve"),
            size: bytes,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let ring = (0..RING)
            .map(|_| InFlight {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("timestamp readback"),
                    size: bytes,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                labels: Vec::new(),
                mapped: Arc::new(AtomicBool::new(false)),
                waiting: false,
            })
            .collect();
        Some(Timing {
            set,
            resolve,
            period: gpu.queue.get_timestamp_period() as f64,
            labels: Vec::new(),
            ring,
            next: 0,
            history: Vec::new(),
            last: Vec::new(),
            totals: VecDeque::new(),
        })
    }

    /// A pair for a pass labelled `label`; None once the frame's pairs are used,
    /// or while the slot this frame resolves into is still on its way back.
    fn pair(&mut self, label: &str) -> Option<Span> {
        if self.labels.len() as u32 >= PAIRS || self.ring[self.next].waiting {
            return None;
        }
        let begin = self.labels.len() as u32 * 2;
        self.labels.push(label.to_string());
        Some(Span {
            set: self.set.clone(),
            begin,
        })
    }

    pub fn render_writes(&mut self, label: &str) -> Option<Span> {
        self.pair(label)
    }

    pub fn compute_writes(&mut self, label: &str) -> Option<Span> {
        self.pair(label)
    }

    /// At the end of the frame's encoder: resolve its pairs for reading.
    pub fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if self.labels.is_empty() || self.ring[self.next].waiting {
            return;
        }
        let count = self.labels.len() as u32 * 2;
        encoder.resolve_query_set(&self.set, 0..count, &self.resolve, 0);
        let slot = &mut self.ring[self.next];
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &slot.buffer, 0, count as u64 * 8);
        slot.labels = std::mem::take(&mut self.labels);
    }

    /// After the frame's submission: start reading its slot, and take in
    /// any earlier slot that has come back, without waiting.
    pub fn submitted(&mut self, device: &wgpu::Device) {
        let slot = &mut self.ring[self.next];
        if !slot.labels.is_empty() && !slot.waiting {
            slot.waiting = true;
            slot.mapped.store(false, Ordering::Release);
            let flag = slot.mapped.clone();
            slot.buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |r| {
                    flag.store(r.is_ok(), Ordering::Release)
                });
            self.next = (self.next + 1) % RING;
        }
        self.labels.clear();
        let _ = device.poll(wgpu::PollType::Poll);
        self.collect();
    }

    /// Wait for every slot on its way back (a research run's measured frame).
    pub fn finish(&mut self, device: &wgpu::Device) {
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        self.collect();
    }

    fn collect(&mut self) {
        for i in 0..RING {
            let slot = &mut self.ring[i];
            if !slot.waiting || !slot.mapped.load(Ordering::Acquire) {
                continue;
            }
            let ticks: Vec<u64> = match slot.buffer.slice(..).get_mapped_range() {
                Ok(data) => data
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .take(slot.labels.len() * 2)
                    .map(|c| u64::from_le_bytes(*c))
                    .collect(),
                Err(_) => Vec::new(),
            };
            slot.buffer.unmap();
            slot.waiting = false;
            let labels = std::mem::take(&mut slot.labels);
            if ticks.len() < labels.len() * 2 {
                continue;
            }
            let mut frame: BTreeMap<usize, (String, f64)> = BTreeMap::new();
            for (k, label) in labels.into_iter().enumerate() {
                let (a, b) = (ticks[2 * k], ticks[2 * k + 1]);
                let ms = b.saturating_sub(a) as f64 * self.period / 1e6;
                // Passes with one label (a chain run twice) add up.
                match frame.values_mut().find(|(l, _)| *l == label) {
                    Some((_, total)) => *total += ms,
                    None => {
                        frame.insert(k, (label, ms));
                    }
                }
            }
            self.last = frame.into_values().collect();
            self.totals.push_back(self.last.iter().map(|p| p.1).sum());
            if self.totals.len() > HISTORY {
                self.totals.pop_front();
            }
            for (label, ms) in &self.last {
                match self.history.iter_mut().find(|(l, _)| l == label) {
                    Some((_, h)) => {
                        h.push_back(*ms);
                        if h.len() > HISTORY {
                            h.pop_front();
                        }
                    }
                    None => self.history.push((label.clone(), VecDeque::from([*ms]))),
                }
            }
        }
    }

    /// Each pass's mean and 95th percentile over the recent frames, in milliseconds.
    pub fn summary(&self) -> Vec<(String, f64, f64)> {
        self.summary_last(HISTORY)
    }

    /// `summary` over the last `frames` measured frames of each pass (the
    /// performance panel's rows, which follow a change within a second).
    pub fn summary_last(&self, frames: usize) -> Vec<(String, f64, f64)> {
        self.history
            .iter()
            .map(|(label, h)| {
                let xs: Vec<f64> = h
                    .iter()
                    .skip(h.len().saturating_sub(frames))
                    .copied()
                    .collect();
                let (mean, _, p95) = crate::metrics::summary(&xs);
                (label.clone(), mean, p95)
            })
            .collect()
    }

    /// The last frame read back: each pass's milliseconds.
    pub fn last(&self) -> &[(String, f64)] {
        &self.last
    }

    /// Each recent frame's total GPU time in milliseconds, oldest first.
    pub fn totals(&self) -> Vec<f64> {
        self.totals.iter().copied().collect()
    }

    /// Forget what was measured (a research run's next variant).
    pub fn clear(&mut self) {
        self.history.clear();
        self.last.clear();
        self.totals.clear();
    }
}
