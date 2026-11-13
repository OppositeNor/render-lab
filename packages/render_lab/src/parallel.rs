//! A map over a range of indices on every core, for the CPU mirrors.

use std::ops::Range;

/// `f(i)` for every `i` of `range`, on every core, in order: the range split
/// into one run of consecutive indices per thread.
pub fn par_map<T: Send + Clone + Default>(
    range: Range<usize>,
    f: impl Fn(usize) -> T + Sync,
) -> Vec<T> {
    let n = range.len();
    let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
    let chunk = n.div_ceil(threads).max(1);
    let mut out = vec![T::default(); n];
    std::thread::scope(|s| {
        for (k, part) in out.chunks_mut(chunk).enumerate() {
            // Each thread fills its run, from the index its run starts at.
            let f = &f;
            let start = range.start + k * chunk;
            s.spawn(move || {
                for (i, o) in part.iter_mut().enumerate() {
                    *o = f(start + i);
                }
            });
        }
    });
    out
}
