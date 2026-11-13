//! RIS worked by hand (the lab's example, not the paper's): a world of two
//! light samples, A and B, which the cheap choice draws with p = 3/4 and 1/4,
//! and which bring f = 1 and 3, the target p̂ equal to f. RIS draws M = 2
//! candidates; every list of candidates is counted with its chance and the
//! estimate f(y) · W it gives (Eq. 6), and their average is set against the
//! true total f(A) + f(B). `toyDrop` removes one part of W to show what it is
//! for: the division by p in the weights, or the average of the weights.

use crate::paper::restir::text::{tr, tr_with};
use lab::frame::{FrameContent, Readout};
use lab::palette;
use lab::utility::inset::{Corner, Picture, PictureInset};
use std::sync::Arc;

/// The cheap choice's chance of each sample, in quarters: A 3/4, B 1/4.
const QUARTERS: [u32; 2] = [3, 1];
/// What each sample brings, f, which is also the target p̂.
const BRINGS: [f64; 2] = [1.0, 3.0];
/// The samples' names, as the narration says them.
const NAMES: [&str; 2] = ["A", "B"];

/// Which part of W the example removes (`toyDrop`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drop {
    /// RIS as the paper writes it (Eq. 5-6).
    None,
    /// The weights without the division by p: w = p̂.
    DivideByP,
    /// W without the average of the weights: f(y) / p̂(y) alone.
    Average,
}

impl Drop {
    pub fn of(v: f32) -> Drop {
        match v.round() as i32 {
            0 => Drop::None,
            1 => Drop::DivideByP,
            _ => Drop::Average,
        }
    }
}

fn p(i: usize) -> f64 {
    f64::from(QUARTERS[i]) / 4.0
}

/// A candidate's weight: p̂ / p, or p̂ alone without the division.
pub fn weight(i: usize, drop: Drop) -> f64 {
    if drop == Drop::DivideByP {
        BRINGS[i]
    } else {
        BRINGS[i] / p(i)
    }
}

/// One list of two candidates (the order left out): the samples in it, and its chance in sixteenths.
pub struct List {
    pub samples: [usize; 2],
    pub sixteenths: u32,
}

pub fn lists() -> [List; 3] {
    let both = |a: usize, b: usize| {
        QUARTERS[a]
            * QUARTERS[b]
            * if a == b {
                1
            } else {
                2
            }
    };
    [
        List {
            samples: [0, 0],
            sixteenths: both(0, 0),
        },
        List {
            samples: [0, 1],
            sixteenths: both(0, 1),
        },
        List {
            samples: [1, 1],
            sixteenths: both(1, 1),
        },
    ]
}

/// RIS's estimate from one list, averaged over which candidate it keeps (each with its weight's share):
/// f(y) / p̂(y) times the average weight, or f(y) / p̂(y) alone without the average.
pub fn estimate(list: &List, drop: Drop) -> f64 {
    let w = list.samples.map(|i| weight(i, drop));
    let total = w[0] + w[1];
    let mean = total / 2.0;
    list.samples
        .iter()
        .zip(w)
        .map(|(&i, wi)| {
            // The chance this candidate is kept, times f(y) / p̂(y) (one here, as p̂ = f).
            let (f, target) = (BRINGS[i], BRINGS[i]);
            let shade = f / target;
            wi / total
                * shade
                * if drop == Drop::Average {
                    1.0
                } else {
                    mean
                }
        })
        .sum()
}

/// The estimate averaged over every list's chance, and the true total it should equal.
pub fn average(drop: Drop) -> (f64, f64) {
    let mean = lists()
        .iter()
        .map(|l| f64::from(l.sixteenths) / 16.0 * estimate(l, drop))
        .sum();
    (mean, BRINGS.iter().sum())
}

/// For each sample, the chance that RIS keeps it, and the mean of W when it does (Eq. 6's W, the paper's form):
/// W averages one over that chance.
pub fn kept() -> [(f64, f64); 2] {
    let mut chance = [0.0; 2];
    let mut w_sum = [0.0; 2];
    for list in lists() {
        // Each candidate of the list kept with its weight's share; W is the average weight over its target.
        let w = list.samples.map(|i| weight(i, Drop::None));
        let total = w[0] + w[1];
        for (&i, wi) in list.samples.iter().zip(w) {
            let p_list = f64::from(list.sixteenths) / 16.0 * wi / total;
            chance[i] += p_list;
            w_sum[i] += p_list * (total / 2.0) / BRINGS[i];
        }
    }
    [0, 1].map(|i| (chance[i], w_sum[i] / chance[i]))
}

/// The bars of the two samples: p, p̂, and w, each kind scaled to its larger value, shown as far as `step` reaches.
fn bars(step: f32, drop: Drop, size: [usize; 2]) -> Vec<u8> {
    let (w, h) = (size[0], size[1]);
    let mut rgba: Vec<u8> = [22u8, 26, 32, 255].repeat(w * h);
    let kinds: [(f32, [f64; 2], [u8; 3]); 3] = [
        (1.0, [p(0), p(1)], PDF),
        (2.0, BRINGS, TARGET),
        (3.0, [weight(0, drop), weight(1, drop)], WEIGHT),
    ];
    let group = w / 2;
    let bar = group / 5;
    for (k, (shown, values, color)) in kinds.iter().enumerate() {
        // Each kind's bar for A in the left half, for B in the right, once the step has reached it.
        if step < *shown {
            continue;
        }
        let top = values[0].max(values[1]);
        for (i, v) in values.iter().enumerate() {
            let height = ((v / top) * (h as f64 - 16.0)) as usize;
            let x0 = i * group + bar + k * (bar + bar / 4);
            for y in h - height..h {
                for x in x0..(x0 + bar).min(w) {
                    rgba[4 * (y * w + x)..4 * (y * w + x) + 3].copy_from_slice(color);
                }
            }
        }
    }
    rgba
}

const PDF: [u8; 3] = [130, 210, 150];
const TARGET: [u8; 3] = [205, 205, 210];
const WEIGHT: [u8; 3] = [120, 200, 255];

fn color(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

/// The samples' rows, as far as `step` reaches: p, then p̂ = f, then w.
fn sample_rows(step: f32, drop: Drop, rows: &mut Vec<(String, String, egui::Color32)>) {
    for (i, name) in NAMES.iter().enumerate() {
        let mut value = format!("p {}/4", QUARTERS[i]);
        if step >= 2.0 {
            value += &format!(" · p̂ = f {}", BRINGS[i]);
        }
        if step >= 3.0 {
            value += &format!(" · w {:.2}", weight(i, drop));
        }
        rows.push((
            tr_with("demos-toy_sample", lab::args!(name = *name)).to_string(),
            value,
            palette::INCIDENT,
        ));
    }
}

pub fn draw(state: &lab::shot::ShotState, content: &mut FrameContent) {
    let step = state.p("toyStep").clamp(0.0, 8.0);
    let drop = Drop::of(state.p("toyDrop"));
    let mut rows = Vec::new();
    if step >= 1.0 {
        sample_rows(step, drop, &mut rows);
    }
    let (mean, truth) = average(drop);
    if step >= 2.0 {
        rows.push((
            tr("demos-toy_truth").into(),
            format!("{truth}"),
            palette::SELECTED,
        ));
    }
    if step >= 4.0 {
        rows.push((
            tr("demos-toy_lists").into(),
            tr("demos-toy_columns").into(),
            egui::Color32::GRAY,
        ));
    }
    for (n, list) in lists().iter().enumerate() {
        // The lists one by one from step 4: their samples, chance, and estimate.
        if step < 4.0 + n as f32 {
            break;
        }
        let names = list.samples.map(|i| NAMES[i]).join(", ");
        rows.push((
            tr_with("demos-toy_list", lab::args!(list = names)).to_string(),
            tr_with(
                "demos-toy_outcome",
                lab::args!(
                    chance = format!("{}/16", list.sixteenths),
                    estimate = format!("{:.2}", estimate(list, drop))
                ),
            )
            .to_string(),
            palette::THEME_ACCENT,
        ));
    }
    if step >= 7.0 {
        let right = (mean - truth).abs() < 1e-9;
        rows.push((
            tr("demos-toy_average").into(),
            format!("{mean:.2}"),
            if right {
                palette::SELECTED
            } else {
                super::BLOCKED
            },
        ));
    }
    if step >= 8.0 && drop == Drop::None {
        for (i, (chance, mean_w)) in kept().iter().enumerate() {
            // The chance RIS keeps each sample, and W's mean then: one over that chance.
            rows.push((
                tr_with("demos-toy_kept", lab::args!(name = NAMES[i])).to_string(),
                tr_with(
                    "demos-toy_kept_value",
                    lab::args!(
                        chance = format!("{chance:.2}"),
                        mean = format!("{mean_w:.2}")
                    ),
                )
                .to_string(),
                palette::SELECTED,
            ));
        }
    }
    let subtitle = match drop {
        Drop::None => tr("demos-toy_paper"),
        Drop::DivideByP => tr("demos-toy_no_p"),
        Drop::Average => tr("demos-toy_no_mean"),
    };
    content.readout = Some(Readout {
        title: tr("demos-toy_title").into(),
        subtitle: subtitle.into(),
        formula: (step >= 7.0 && drop == Drop::None).then(|| tr("formula-ris").into()),
        formula_lit: None,
        rows,
    });
    let size = [360usize, 180];
    content.insets.push(PictureInset {
        corner: Corner::TopRight,
        x_axis: tr("demos-toy_axis").to_string(),
        lines: vec![
            (tr("demos-toy_p").to_string(), color(PDF)),
            (tr("demos-toy_phat").to_string(), color(TARGET)),
            (tr("demos-toy_w").to_string(), color(WEIGHT)),
        ],
        ..PictureInset::new(
            tr("demos-toy_bars"),
            Picture::Pixels {
                key: format!("restir-toy-{}-{drop:?}", step.floor()),
                size,
                rgba: Arc::new(bars(step, drop, size)),
            },
            [360.0, 180.0],
        )
    });
}
