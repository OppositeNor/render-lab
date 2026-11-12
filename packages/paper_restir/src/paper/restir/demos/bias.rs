//! The paper's 1D example of RIS's bias (148:8 §4.2-4.3, Fig. 7), redrawn by
//! the lab: a linear target p̂(x) = 2 − 2x sampled with half the candidates from
//! a constant PDF p₁ = 1 and half from a step p₂ = 2H(1/2 − x) (or, nearly zero,
//! p₂ ∝ max(H(1/2 − x), 10⁻⁴)). Many runs of RIS give the true inverse PDF of
//! their output (from its histogram, the thick line) and the mean of W where it
//! lands (the thin line); they agree only when W is unbiased. The weight m is
//! the biased 1/M, the naive 1/|Z| (Eq. 20-21), or the balance heuristic
//! (Eq. 22).

use crate::paper::restir::text::tr;
use lab::frame::FrameContent;
use lab::utility::inset::{Corner, Picture, PictureInset};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// The bins of the histogram, across [0, 1).
pub const BINS: usize = 50;

/// m, the weight W uses in place of 1/M.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weight {
    Biased,
    Naive,
    Balance,
}

impl Weight {
    pub fn of(v: f32) -> Weight {
        match v.round() as i32 {
            0 => Weight::Biased,
            1 => Weight::Naive,
            _ => Weight::Balance,
        }
    }
}

fn target(x: f64) -> f64 {
    2.0 - 2.0 * x
}

/// The candidates' PDFs at x: p₁ for even candidates, p₂ for odd ones.
fn pdf(i: usize, x: f64, near_zero: bool) -> f64 {
    if i.is_multiple_of(2) {
        1.0
    } else if near_zero {
        // max(H(1/2 − x), 1e-4) normalised: its integral is 1/2 + 1e-4/2.
        let raw = if x < 0.5 {
            1.0
        } else {
            1e-4
        };
        raw / (0.5 + 0.5e-4)
    } else if x < 0.5 {
        2.0
    } else {
        0.0
    }
}

fn draw_from(i: usize, u: f64, v: f64, near_zero: bool) -> f64 {
    if i.is_multiple_of(2) {
        u
    } else if near_zero {
        // The step's half with nearly all its mass, else the long tail.
        let tail = 0.5e-4 / (0.5 + 0.5e-4);
        if v < tail {
            0.5 + 0.5 * u
        } else {
            0.5 * u
        }
    } else {
        0.5 * u
    }
}

/// The curves for one setting: per bin, the true inverse PDF of RIS's output (from its histogram) and the mean of W.
pub struct Curves {
    pub truth: Vec<f64>,
    pub mean_w: Vec<f64>,
}

/// Runs RIS `runs` times with `m` candidates (half from each PDF) and the weight `weight` (a simple LCG, seeded,
/// so the curves are the same every time).
pub fn run(m: usize, weight: Weight, near_zero: bool, runs: usize) -> Curves {
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut rand = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((state >> 11) as f64) / ((1u64 << 53) as f64)
    };
    let mut count = vec![0u64; BINS];
    let mut sum_w = vec![0.0f64; BINS];
    let mut xs = vec![0.0f64; m];
    let mut ws = vec![0.0f64; m];
    for _ in 0..runs {
        // One run: m candidates and their weights, one picked in proportion, then W with the chosen m.
        let mut total = 0.0;
        for i in 0..m {
            let (u, v) = (rand(), rand());
            xs[i] = draw_from(i, u, v, near_zero);
            ws[i] = target(xs[i]) / pdf(i, xs[i], near_zero);
            total += ws[i];
        }
        if total <= 0.0 {
            continue;
        }
        let mut pick = rand() * total;
        let mut z = m - 1;
        for i in 0..m {
            if pick < ws[i] {
                z = i;
                break;
            }
            pick -= ws[i];
        }
        let y = xs[z];
        let mw = match weight {
            Weight::Biased => 1.0 / m as f64,
            Weight::Naive => 1.0 / (0..m).filter(|&i| pdf(i, y, near_zero) > 0.0).count() as f64,
            Weight::Balance => {
                pdf(z, y, near_zero) / (0..m).map(|i| pdf(i, y, near_zero)).sum::<f64>()
            }
        };
        let w = mw * total / target(y);
        let bin = ((y * BINS as f64) as usize).min(BINS - 1);
        count[bin] += 1;
        sum_w[bin] += w;
    }
    let width = 1.0 / BINS as f64;
    Curves {
        truth: count
            .iter()
            .map(|&c| {
                if c > 0 {
                    runs as f64 * width / c as f64
                } else {
                    0.0
                }
            })
            .collect(),
        mean_w: count
            .iter()
            .zip(&sum_w)
            .map(|(&c, &s)| {
                if c > 0 {
                    s / c as f64
                } else {
                    0.0
                }
            })
            .collect(),
    }
}

/// A setting of the example: the candidates, the weight, and whether the step is nearly zero.
type Setting = (usize, Weight, bool);

/// The curves of a setting, computed once.
pub fn curves(m: usize, weight: Weight, near_zero: bool) -> Arc<Curves> {
    static CACHE: OnceLock<Mutex<HashMap<Setting, Arc<Curves>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(c) = cache.lock().unwrap().get(&(m, weight, near_zero)) {
        return c.clone();
    }
    let c = Arc::new(run(m, weight, near_zero, 400_000));
    cache
        .lock()
        .unwrap()
        .insert((m, weight, near_zero), c.clone());
    c
}

/// The plot as pixels: the true inverse PDF (thick), the mean W (thin), on 0 to 4; a line where the step stops.
fn plot(c: &Curves, size: [u32; 2], show: Show) -> Vec<u8> {
    let (w, h) = (size[0] as usize, size[1] as usize);
    let mut rgba = vec![0u8; w * h * 4];
    let mark = (show.mark >= 0.0).then(|| ((show.mark * w as f32) as usize).min(w - 1));
    for (k, px) in rgba.chunks_mut(4).enumerate() {
        // The middle of the range drawn as a faint rule, the marker as a bright one.
        let color = if Some(k % w) == mark {
            [235, 235, 240, 255]
        } else if k % w == w / 2 {
            [70, 76, 88, 255]
        } else {
            [22, 26, 32, 255]
        };
        px.copy_from_slice(&color);
    }
    let mut put = |x: usize, y: f64, color: [u8; 3], thick: usize| {
        let row = h as f64 - 1.0 - (y / 4.0).clamp(0.0, 1.0) * (h as f64 - 1.0);
        for t in 0..thick {
            let r = (row as usize + t).saturating_sub(thick / 2).min(h - 1);
            let i = 4 * (r * w + x);
            rgba[i..i + 3].copy_from_slice(&color);
        }
    };
    for x in 0..w {
        // Each curve drawn as far across as the reveal has reached it: the target, the two PDFs, then the lines.
        let bin = (x * BINS / w).min(BINS - 1);
        let at = (x as f64 + 0.5) / w as f64;
        let reached = |n: f32| show.reveal - n >= x as f32 / w as f32;
        if reached(0.0) {
            put(x, target(at), TARGET, 2);
        }
        if reached(1.0) {
            put(x, pdf(0, at, false), PDFS, 1);
        }
        if reached(2.0) {
            put(x, pdf(1, at, false), PDFS, 1);
        }
        if reached(3.0) {
            put(x, c.truth[bin], [120, 200, 255], 5);
        }
        if reached(4.0) {
            put(x, c.mean_w[bin], [255, 196, 90], 2);
        }
    }
    rgba
}

/// The target ramp's colour and the two source PDFs' in the plot.
const TARGET: [u8; 3] = [205, 205, 210];
const PDFS: [u8; 3] = [130, 210, 150];

/// What the plot shows (`biasReveal`, `biasMark`): how far each curve is drawn, and the marker's place.
#[derive(Clone, Copy)]
struct Show {
    reveal: f32,
    mark: f32,
}

pub fn draw(state: &lab::shot::ShotState, content: &mut FrameContent) {
    let m = (state.p("biasCandidates").round() as usize).clamp(2, 20) & !1;
    let weight = Weight::of(state.p("biasWeight"));
    let near_zero = state.p("biasNearZero") > 0.5;
    let c = curves(m, weight, near_zero);
    let size: [u32; 2] = [480, 240];
    let show = Show {
        reveal: state.p("biasReveal").clamp(0.0, 5.0),
        mark: state.p("biasMark").min(1.0),
    };
    let key = format!(
        "restir-bias-{m}-{weight:?}-{near_zero}-{}-{}",
        (show.reveal * 100.0).round(),
        (show.mark * 200.0).round()
    );
    content.insets.push(PictureInset {
        corner: Corner::TopRight,
        x_axis: tr("demos-bias_axis").to_string(),
        lines: vec![
            (
                tr("demos-bias_target").to_string(),
                egui::Color32::from_rgb(TARGET[0], TARGET[1], TARGET[2]),
            ),
            (
                tr("demos-bias_pdfs").to_string(),
                egui::Color32::from_rgb(PDFS[0], PDFS[1], PDFS[2]),
            ),
            (
                tr("demos-bias_truth").to_string(),
                egui::Color32::from_rgb(120, 200, 255),
            ),
            (
                tr("demos-bias_mean").to_string(),
                egui::Color32::from_rgb(255, 196, 90),
            ),
        ],
        ..PictureInset::new(
            tr("demos-bias_title"),
            Picture::Pixels {
                key,
                size: [size[0] as usize, size[1] as usize],
                rgba: Arc::new(plot(&c, size, show)),
            },
            [480.0, 240.0],
        )
    });
}
