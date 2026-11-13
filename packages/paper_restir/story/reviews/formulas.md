# Formula comprehension review (`new-story`'s references/storytelling-formula-review.md), 2026-11-12

| Equation and source | Kind | Question | Missing stage (before) | Math checked | Visual checked | Fix | Verdict |
|---|---|---|---|---|---|---|---|
| w = p̂/p, Eq. 5 | core | why divide by p? | need, worked example, counterfactual | selftest `check_toy` | c2-ris readout, c2-toy bars | c2-ris: the head start; c2-toy: 4/3 and 12; without it 1.5 | TAUGHT |
| W, Eq. 6 | core | why the average, why over p̂(y)? | worked example, expectation vs one run, counterfactual | `check_toy` (enumerated and simulated) | c2-toy lists and average | c2-toy: 4/3, 20/3, 12 average 4; without the average 1 | TAUGHT |
| p̂_P(y)·W·M, Alg. 4, §4.1 | core | why W, why M, why P's target? | the construction from w, a worked case | cpu.rs `combine` (the GPU's stream, selftest) | c4-combine readout | c4-combine: from w to target × W, the readout factor by factor, M honest in round one | TAUGHT |
| Eq. 19 | core | why W is too small where a PDF is zero | the equation itself, a read value | `check_bias` | c6-1d and c6-fix readouts at the marker | c6-1d: Eq. 19 named, half at the marker; c6-fix: divides by 2 not 4 | TAUGHT |

Cold-listener pass (same day): W was MENTIONED (why the average; W as one over the chance RIS keeps y, which c4 and c6
recall) and the combine weight MENTIONED (why W stands for one over a chance; why M). Fixed: c2-toy says the average
estimates the target's total, and shows W averaging one over the keep chance (toy step 8, checked by `check_toy`);
c4-combine says the chance is the neighbour's reservoir keeping the sample, W is one over it on average, and times M
P's own reservoir gets back its sum of weights; c6-source says "keeps". "Unbiased" glossed at first use (c2-ris); the
target "not quite a probability" (c2-ris-question). Both verdicts now TAUGHT. Native zh review: 8 sentences revised.
