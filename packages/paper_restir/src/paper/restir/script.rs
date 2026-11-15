//! What the investigation's scripts (script/*.lam, lab/src/script/runtime.rs) read from its code: the numbers of
//! the worked example of RIS (demos/toy.rs) and of the math scenes' example world (examples.rs), which its math
//! scenes draw (lab/src/math/), so no number on the canvas is typed in.
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
//! | `exampleLights which phase intensity` | the math scenes' example world (examples.rs): what each light brings to P (0) or Q (1), and its share of the power |
//! | `exampleTrace seed count`, `exampleMerge seed count` | a stream of that world's candidates through the lab's reservoir, its steps and its merge |
//! | `exampleAlias` | an alias table over that world's shares (the lab's illustration of the paper's): each column's own part and alias |
//! | `examplePieces` | that table's pieces, each a part of a light's share in a column |
//! | `exampleCombine seed count` | P's reservoir and its neighbours' in that world, each of `count` candidates, combined at P as the lab's spatial pass does |
//! | `exampleReach k radius rounds seed` | for each number of rounds of spatial reuse, the paths down the paper's tree (k to the n) and the distinct pixels they end on, with the lab's neighbour pattern |
//! | `exampleHistory current clamp frames` | a pixel's M over frames of temporal reuse (the lab's count, its clamp; 0 none), and each frame's share of the last M |
//! | `biasCurves m weight nearZero` | the 1D example's curves (demos/bias.rs, Fig. 7): per bin, the true inverse PDF of RIS's output and the mean of W, and the runs behind them |
//! | `biasPdf i x nearZero`, `biasTarget x` | that example's PDFs (0 the flat one, 1 the step) and its target at x |
//! | `exampleSearch u` | the lab's binary search over the running sum of the shares for `u`: each halving, then the light found |

use super::demos::ris::Trace;
use super::demos::toy::{self, Drop};
use super::examples;
use lab::script::lamlet::embed::{Error, NativeRegistry};

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

lab::lamlet_record! {
    /// One column of the alias table (examples.rs `AliasColumn`).
    pub struct AliasRecord {
        pub own: f64 = "own",
        pub alias: i64 = "alias",
    }
}

lab::lamlet_record! {
    /// A piece of the alias table (examples.rs `AliasPiece`).
    pub struct PieceRecord {
        pub light: i64 = "light",
        pub column: i64 = "column",
        pub lo: f64 = "lo",
        pub hi: f64 = "hi",
        pub start: f64 = "start",
        pub length: f64 = "length",
    }
}

lab::lamlet_record! {
    /// One halving of the lab's binary search: the range, its middle, and the running sum there.
    pub struct SearchStep {
        pub lo: i64 = "lo",
        pub hi: i64 = "hi",
        pub mid: i64 = "mid",
        pub sum: f64 = "sum",
    }
}

lab::lamlet_record! {
    /// The lab's binary search for a number: its halvings and the light it finds.
    pub struct SearchRecord {
        pub steps: Vec<SearchStep> = "steps",
        pub found: i64 = "found",
    }
}

lab::lamlet_record! {
    /// One reservoir combined at P (examples.rs `Source`): its pixel and record, its sample's target at P, its
    /// weight there, the update's number, whether it replaced the kept sample, and the sum so far.
    pub struct SourceRecord {
        pub pixel: i64 = "pixel",
        pub light: i64 = "light",
        pub p_hat: f64 = "pHat",
        pub wsum: f64 = "wsum",
        pub m: f64 = "m",
        pub w: f64 = "w",
        pub p_hat_p: f64 = "pHatP",
        pub weight: f64 = "weight",
        pub u: f64 = "u",
        pub taken: bool = "taken",
        pub running: f64 = "running",
    }
}

lab::lamlet_record! {
    /// P's reservoir combined with its neighbours' (examples.rs `Combined`).
    pub struct CombineRecord {
        pub sources: Vec<SourceRecord> = "sources",
        pub kept: i64 = "kept",
        pub wsum: f64 = "wsum",
        pub m: f64 = "m",
        pub w: f64 = "w",
    }
}

lab::lamlet_record! {
    /// How far P's candidates reach after `n` rounds (examples.rs `reach`).
    pub struct ReachRecord {
        pub n: i64 = "n",
        pub paths: f64 = "paths",
        pub pixels: i64 = "pixels",
    }
}

lab::lamlet_record! {
    /// A pixel's M over frames of temporal reuse, and each frame's share of the last one (examples.rs `history`).
    pub struct HistoryRecord {
        pub m: Vec<f64> = "m",
        pub shares: Vec<f64> = "shares",
    }
}

lab::lamlet_record! {
    /// The 1D example's curves (demos/bias.rs `Curves`), and the runs of RIS behind them.
    pub struct BiasRecord {
        pub truth: Vec<f64> = "truth",
        pub mean_w: Vec<f64> = "meanW",
        pub runs: i64 = "runs",
    }
}

/// A trace's steps as the scripts read them.
fn steps_of(t: &Trace) -> Vec<RisStep> {
    t.steps
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
        .collect()
}

/// A trace's merge as the scripts read it.
fn merge_of(t: &Trace) -> RisMerge {
    RisMerge {
        split: t.split as i64,
        u: f64::from(t.merge_u),
        kept: t.merged.map_or(-1, |k| k as i64),
    }
}

/// Lights as the scripts read them.
fn lights_of(lights: &[examples::LightAt]) -> Vec<LightAtRecord> {
    lights
        .iter()
        .map(|l| LightAtRecord {
            brings: f64::from(l.brings),
            share: f64::from(l.share),
        })
        .collect()
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
        .function(
            "paper_restir/example-lights",
            |_, (which, phase, intensity): (i64, f64, f64)| {
                Ok(lights_of(&examples::lights_at(which, phase as f32, intensity as f32)))
            },
        )
        .doc("Fn([Int, Float, Float], List({brings: Float, share: Float})). The math scenes' example world (examples.rs, the lab's illustration): what each of its 24 lights brings to P (0) or Q (1) at the lights' phase and intensity, and its share of the power.")
        .register()?;
    natives
        .function("paper_restir/example-trace", |_, (seed, count): (i64, i64)| {
            Ok(steps_of(&examples::stream(seed as u64, count.clamp(1, 64) as usize)))
        })
        .doc("Fn([Int, Int], List({kept: Int, light: Int, p: Float, pHat: Float, taken: Bool, u: Float, w: Float, wsum: Float})). P's stream in the example world: `count` candidates drawn by power with the numbers of `seed`, through the lab's reservoir (the first eight through one, the rest through a second), ")
        .register()?;
    natives
        .function("paper_restir/example-merge", |_, (seed, count): (i64, i64)| {
            Ok(merge_of(&examples::stream(seed as u64, count.clamp(1, 64) as usize)))
        })
        .doc("Fn([Int, Int], {kept: Int, split: Int, u: Float}). The merge of that example stream's two reservoirs.")
        .register()?;
    register_alias(natives)?;
    register_reuse(natives)?;
    register_bias(natives)
}

/// The natives of the 1D example of RIS's bias (demos/bias.rs).
fn register_bias(natives: &mut NativeRegistry) -> Result<(), Error> {
    use super::demos::bias;
    natives
        .function(
            "paper_restir/bias-curves",
            |_, (m, weight, near_zero): (i64, f64, f64)| {
                let m = (m.clamp(2, 20) as usize) & !1;
                let c = bias::curves(m, bias::Weight::of(weight as f32), near_zero > 0.5);
                Ok(BiasRecord {
                    truth: c.truth.clone(),
                    mean_w: c.mean_w.clone(),
                    runs: bias::RUNS as i64,
                })
            },
        )
        .doc("Fn([Int, Float, Float], {meanW: List(Float), runs: Int, truth: List(Float)}). The 1D example's curves (Fig. 7) for `m` candidates (even, 2 to 20), the weight (0 biased 1/M, 1 naive 1/|Z|, 2 the balance heuristic), and whether the step is nearly zero: per bin across [0, 1), the true inverse PDF of RIS's output and the mean of W there, over `runs` runs.")
        .register()?;
    natives
        .function("paper_restir/bias-pdf", |_, (i, x, near_zero): (i64, f64, f64)| {
            Ok(bias::pdf(i.max(0) as usize, x, near_zero > 0.5))
        })
        .doc("Fn([Int, Float, Float], Float). The 1D example's PDF of candidate kind `i` (0 the flat one, 1 the step) at x, the step nearly zero past one half when `nearZero` is 1.")
        .register()?;
    natives
        .function("paper_restir/bias-target", |_, (x,): (f64,)| {
            Ok(bias::target(x))
        })
        .doc("Fn([Float], Float). The 1D example's target, 2 - 2x.")
        .register()?;
    Ok(())
}

/// The natives of the example's rounds of spatial reuse and its count over frames.
fn register_reuse(natives: &mut NativeRegistry) -> Result<(), Error> {
    natives
        .function(
            "paper_restir/example-history",
            |_, (current, clamp, frames): (f64, f64, i64)| {
                let (m, shares) = examples::history(current as f32, clamp as f32, frames.clamp(1, 200) as usize);
                Ok(HistoryRecord {
                    m: m.into_iter().map(f64::from).collect(),
                    shares: shares.into_iter().map(f64::from).collect(),
                })
            },
        )
        .doc("Fn([Float, Float, Int], {m: List(Float), shares: List(Float)}). A pixel's count over `frames` frames of temporal reuse alone, each drawing `current` candidates and combining them with last frame's reservoir as the lab's temporal pass counts (last frame's M clamped to `clamp` times this frame's; 0 no limit): M after each frame, and each frame's share of the last M, the oldest first.")
        .register()?;
    natives
        .function(
            "paper_restir/example-reach",
            |_, (k, radius, rounds, seed): (i64, f64, i64, i64)| {
                Ok(examples::reach(k.clamp(1, 16) as u32, radius as f32, rounds.clamp(0, 10) as u32, seed as u64)
                    .into_iter()
                    .map(|(n, paths, pixels)| ReachRecord {
                        n: i64::from(n),
                        paths,
                        pixels: pixels as i64,
                    })
                    .collect::<Vec<_>>())
            },
        )
        .doc("Fn([Int, Float, Int, Int], List({n: Int, paths: Float, pixels: Int})). How far P's candidates reach after each number of rounds of spatial reuse, 0 to `rounds`, in a 1920 by 1080 image: the paths down the paper's tree (k to the n: each round combines k neighbours' reservoirs of the round before) and the distinct pixels they end on, with the lab's neighbour pattern (R2 points within `radius`, rotated per pixel and round from `seed`).")
        .register()?;
    Ok(())
}

/// The natives of the example's alias table and binary search, and of its reservoirs combined.
fn register_alias(natives: &mut NativeRegistry) -> Result<(), Error> {
    natives
        .function("paper_restir/example-combine", |_, (seed, count): (i64, i64)| {
            let c = examples::combine(seed as u64, count.clamp(1, 256) as usize);
            Ok(CombineRecord {
                sources: c
                    .sources
                    .iter()
                    .map(|s| SourceRecord {
                        pixel: s.pixel as i64,
                        light: s.kept.light as i64,
                        p_hat: f64::from(s.kept.p_hat),
                        wsum: f64::from(s.kept.wsum),
                        m: f64::from(s.kept.m),
                        w: f64::from(s.kept.w),
                        p_hat_p: f64::from(s.p_hat_p),
                        weight: f64::from(s.weight),
                        u: f64::from(s.u),
                        taken: s.taken,
                        running: f64::from(s.running),
                    })
                    .collect(),
                kept: c.kept as i64,
                wsum: f64::from(c.wsum),
                m: f64::from(c.m),
                w: f64::from(c.w),
            })
        })
        .doc("Fn([Int, Int], {kept: Int, m: Float, sources: List({light: Int, m: Float, pHat: Float, pHatP: Float, pixel: Int, running: Float, taken: Bool, u: Float, w: Float, weight: Float, wsum: Float}), w: Float, wsum: Float}). P's reservoir and its three neighbours' in the example world, each of `count` candidates (`seed`'s numbers), combined at P as the lab's spatial pass does: each source's pixel (0 P), its kept light, that light's target at its pixel, its sum of weights, M, and W; the target at P, its weight there (target at P times W times M), the update's number, whether it replaced the kept sample, and the sum so far; then the source kept, the sum, the count, and W.")
        .register()?;
    natives
        .function("paper_restir/example-alias", |_, ()| {
            Ok(examples::alias_table(&examples::shares())
                .iter()
                .map(|c| AliasRecord {
                    own: f64::from(c.own),
                    alias: c.alias as i64,
                })
                .collect::<Vec<_>>())
        })
        .doc("Fn([], List({alias: Int, own: Float})). An alias table over the example's shares (Vose's method; the lab's illustration of the paper's): each column's own part (of 1) and the light that has the rest.")
        .register()?;
    natives
        .function("paper_restir/example-pieces", |_, ()| {
            let shares = examples::shares();
            Ok(examples::alias_pieces(&shares, &examples::alias_table(&shares))
                .iter()
                .map(|p| PieceRecord {
                    light: p.light as i64,
                    column: p.column as i64,
                    lo: f64::from(p.lo),
                    hi: f64::from(p.hi),
                    start: f64::from(p.start),
                    length: f64::from(p.length),
                })
                .collect::<Vec<_>>())
        })
        .doc("Fn([], List({column: Int, hi: Float, length: Float, light: Int, lo: Float, start: Float})). The alias table's pieces, each light's in turn: its column and its span there (a column 1 high), and where it lies along the shares laid end to end.")
        .register()?;
    natives
        .function("paper_restir/example-search", |_, (u,): (f64,)| {
            let (steps, found) = examples::search(u as f32);
            Ok(SearchRecord {
                steps: steps
                    .iter()
                    .map(|&(lo, hi, mid, sum)| SearchStep {
                        lo: lo as i64,
                        hi: hi as i64,
                        mid: mid as i64,
                        sum: f64::from(sum),
                    })
                    .collect(),
                found: found as i64,
            })
        })
        .doc("Fn([Float], {found: Int, steps: List({hi: Int, lo: Int, mid: Int, sum: Float})}). The lab's pick for the number `u` over the example's shares: each halving of the binary search over their running sum (its range, middle, and the sum there), and the light found.")
        .register()?;
    Ok(())
}
