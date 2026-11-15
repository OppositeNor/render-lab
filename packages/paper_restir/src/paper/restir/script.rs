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
    Ok(())
}
