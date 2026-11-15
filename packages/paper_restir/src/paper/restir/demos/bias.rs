//! The paper's 1D example of RIS's bias (148:8 §4.2-4.3, Fig. 7), redrawn by
//! the lab: a linear target p̂(x) = 2 − 2x sampled with half the candidates from
//! a constant PDF p₁ = 1 and half from a step p₂ = 2H(1/2 − x) (or, nearly zero,
//! p₂ ∝ max(H(1/2 − x), 10⁻⁴)). Many runs of RIS give the true inverse PDF of
//! their output (from its histogram, the thick line) and the mean of W where it
//! lands (the thin line); they agree only when W is unbiased. The weight m is
//! the biased 1/M, the naive 1/|Z| (Eq. 20-21), or the balance heuristic
//! (Eq. 22). The math scene `biasMath` (script/bias.lam) draws the curves on the canvas through the natives
//! (script.rs).

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

/// The runs of RIS each setting's curves average.
pub const RUNS: usize = 400_000;

/// The target p̂(x) = 2 − 2x.
pub fn target(x: f64) -> f64 {
    2.0 - 2.0 * x
}

/// The candidates' PDFs at x: p₁ for even candidates, p₂ for odd ones.
pub fn pdf(i: usize, x: f64, near_zero: bool) -> f64 {
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
    let c = Arc::new(run(m, weight, near_zero, RUNS));
    cache
        .lock()
        .unwrap()
        .insert((m, weight, near_zero), c.clone());
    c
}
