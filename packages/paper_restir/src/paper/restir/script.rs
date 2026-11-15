//! What the investigation's scripts (script/*.lam, lab/src/script/runtime.rs) read from its code: the numbers of
//! the worked example of RIS (demos/toy.rs), which its math scene draws (script/math.lam, lab/src/math/), so no
//! number on the canvas is typed in.
//!
//! | Native | Returns |
//! |---|---|
//! | `toyQuarters i` | the cheap choice's chance of sample i (0 A, 1 B), in quarters |
//! | `toyBrings i` | what sample i brings, f, which is also its target p̂ |
//! | `toyWeight i drop` | its weight, p̂ / p (`drop` 1: p̂ alone) |
//! | `toyLists` | the lists of two candidates: their samples and chance in sixteenths |
//! | `toyEstimate list drop` | RIS's estimate from list `list`, averaged over the candidate it keeps |
//! | `toyAverage drop` | the estimate averaged over the lists' chances |
//! | `toyKept i` | the chance RIS keeps sample i, and W's mean when it does |
//! | `risTrace` | P's stream through weighted reservoir sampling in this frame (demos/ris.rs `trace`): each candidate's light, weight, number, weight sum, and kept one; none before P's surface is read back |
//! | `risMerge` | where the trace's second reservoir starts, the merge's number, and the candidate the merge keeps |
//! | `lightsAt which` | what each light brings to P (0) or a second pixel Q (1), and its share of the power |

use super::demos::ris::Trace;
use super::demos::toy::{self, Drop};
use lab::script::lamlet::embed::{Error, NativeRegistry};
use lab::script::runtime::frame_data;

lab::lamlet_record! {
    /// A list of two candidates (the order left out): its samples, and its chance in sixteenths.
    pub struct ToyList {
        pub a: i64 = "a",
        pub b: i64 = "b",
        pub sixteenths: i64 = "sixteenths",
    }
}

lab::lamlet_record! {
    /// How often RIS keeps a sample, and W's mean when it does.
    pub struct ToyKept {
        pub chance: f64 = "chance",
        pub w: f64 = "w",
    }
}

lab::lamlet_record! {
    /// One candidate's update in P's stream (demos/ris.rs `TraceStep`).
    pub struct RisStep {
        pub light: i64 = "light",
        pub p_hat: f64 = "pHat",
        pub p: f64 = "p",
        pub w: f64 = "w",
        pub u: f64 = "u",
        pub wsum: f64 = "wsum",
        pub kept: i64 = "kept",
        pub taken: bool = "taken",
    }
}

lab::lamlet_record! {
    /// What a light brings to a pixel, and its share of the power (demos/ris.rs `LightAt`).
    pub struct LightAtRecord {
        pub brings: f64 = "brings",
        pub share: f64 = "share",
    }
}

lab::lamlet_record! {
    /// The merge of P's two reservoirs: where the second starts, the merge's number, and what it keeps.
    pub struct RisMerge {
        pub split: i64 = "split",
        pub u: f64 = "u",
        pub kept: i64 = "kept",
    }
}

/// The frame's trace of P's stream, or an empty one before P's surface is read back (or on another demonstration).
fn trace() -> std::sync::Arc<Trace> {
    frame_data::<Trace>().unwrap_or_default()
}

/// `toyDrop`'s value as the part of W it removes (0 none, 1 the division by p, 2 the average).
fn drop_of(drop: i64) -> Drop {
    Drop::of(drop as f32)
}

/// Sample `i` of the example, checked.
fn sample(i: i64) -> Result<usize, String> {
    usize::try_from(i)
        .ok()
        .filter(|&i| i < 2)
        .ok_or_else(|| format!("the worked example has samples 0 (A) and 1 (B), not {i}"))
}

/// Registers the natives (`Investigation::script_natives`).
pub fn register(natives: &mut NativeRegistry) -> Result<(), Error> {
    natives
        .function("paper_restir/toy-quarters", |_, (i,): (i64,)| {
            Ok(i64::from(toy::QUARTERS[sample(i)?]))
        })
        .doc("Fn([Int], Int). The cheap choice's chance of sample i (0 A, 1 B), in quarters.")
        .register()?;
    natives
        .function("paper_restir/toy-brings", |_, (i,): (i64,)| {
            Ok(toy::BRINGS[sample(i)?])
        })
        .doc("Fn([Int], Float). What sample i brings, f, which is also its target p-hat.")
        .register()?;
    natives
        .function("paper_restir/toy-weight", |_, (i, drop): (i64, i64)| {
            Ok(toy::weight(sample(i)?, drop_of(drop)))
        })
        .doc("Fn([Int, Int], Float). Sample i's weight, p-hat over p; with `drop` 1, p-hat alone.")
        .register()?;
    natives
        .function("paper_restir/toy-lists", |_, ()| {
            Ok(toy::lists()
                .iter()
                .map(|l| ToyList {
                    a: l.samples[0] as i64,
                    b: l.samples[1] as i64,
                    sixteenths: i64::from(l.sixteenths),
                })
                .collect::<Vec<_>>())
        })
        .doc("Fn([], List({a: Int, b: Int, sixteenths: Int})). The lists of two candidates (A A, A B, B B): their samples and their chance in sixteenths.")
        .register()?;
    natives
        .function("paper_restir/toy-estimate", |_, (list, drop): (i64, i64)| {
            let lists = toy::lists();
            let l = usize::try_from(list)
                .ok()
                .and_then(|k| lists.get(k))
                .ok_or_else(|| format!("no list {list} of candidates (0 to 2)"))?;
            Ok(toy::estimate(l, drop_of(drop)))
        })
        .doc("Fn([Int, Int], Float). RIS's estimate from list `list`, averaged over the candidate it keeps, with `drop` removing a part of W (0 none, 1 the division by p, 2 the average of the weights).")
        .register()?;
    natives
        .function("paper_restir/toy-average", |_, (drop,): (i64,)| {
            Ok(toy::average(drop_of(drop)).0)
        })
        .doc("Fn([Int], Float). The estimate averaged over the lists' chances, with `drop` removing a part of W.")
        .register()?;
    natives
        .function("paper_restir/toy-kept", |_, (i,): (i64,)| {
            let (chance, w) = toy::kept()[sample(i)?];
            Ok(ToyKept { chance, w })
        })
        .doc("Fn([Int], {chance: Float, w: Float}). The chance RIS keeps sample i, and W's mean when it does.")
        .register()?;
    natives
        .function("paper_restir/ris-trace", |_, ()| {
            Ok(trace()
                .steps
                .iter()
                .map(|s| RisStep {
                    light: i64::from(s.light),
                    p_hat: f64::from(s.p_hat),
                    p: f64::from(s.p),
                    w: f64::from(s.w),
                    u: f64::from(s.u),
                    wsum: f64::from(s.wsum),
                    kept: s.kept.map_or(-1, |k| k as i64),
                    taken: s.taken,
                })
                .collect::<Vec<_>>())
        })
        .doc("Fn([], List({kept: Int, light: Int, p: Float, pHat: Float, taken: Bool, u: Float, w: Float, wsum: Float})). P's candidates in this frame as weighted reservoir sampling streams them (Alg. 2): each one's light, weight, the uniform number its update drew, its reservoir's weight sum after it and the candidate kept then (-1 while none weighs anything), and whether it replaced the kept one. The first eight go through one reservoir, the rest through a second. Empty before P's surface is read back.")
        .register()?;
    natives
        .function("paper_restir/ris-merge", |_, ()| {
            let t = trace();
            Ok(RisMerge {
                split: t.split as i64,
                u: f64::from(t.merge_u),
                kept: t.merged.map_or(-1, |k| k as i64),
            })
        })
        .doc("Fn([], {kept: Int, split: Int, u: Float}). The merge of P's two reservoirs: the candidate where the second starts, the merge's uniform number, and the candidate the merged reservoir keeps (-1 for none).")
        .register()?;
    natives
        .function("paper_restir/lights-at", |_, (which,): (i64,)| {
            let t = trace();
            let lights = if which == 0 { &t.at_p } else { &t.at_q };
            Ok(lights
                .iter()
                .map(|l| LightAtRecord {
                    brings: f64::from(l.brings),
                    share: f64::from(l.share),
                })
                .collect::<Vec<_>>())
        })
        .doc("Fn([Int], List({brings: Float, share: Float})). Each light's contribution to a pixel in this frame (0: P, 1: Q, a quarter of the view to P's left): what it brings (its target over the whole sphere, no shadows) and its share of the power, the chance the cheap choice picks it. Empty before the pixel's surface is read back.")
        .register()?;
    Ok(())
}
