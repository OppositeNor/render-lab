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

use super::demos::toy::{self, Drop};
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
    Ok(())
}
