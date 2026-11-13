# ReSTIR example: from a recited ratio to a necessary ratio

This is a **design example**, not a claim that the current ReSTIR narration or the paper's mathematics is wrong. It draws on the existing Render Lab ReSTIR story: its Related work introduces the RIS weight `w = p̂(x)/p(x)` and its Streaming RIS chapter introduces `W = (sum w)/(M p̂(y))`. The lesson here is the missing *reasoning bridge* between the words and the equations.

Source to verify before editing: Bitterli et al. (2020), *Spatiotemporal reservoir resampling for real-time ray tracing with dynamic direct lighting*, §2.1, Eqs. (5)–(7) and Algorithm 1 for RIS; §2.2 and §3.1 for reservoir implementation. Consult the recorded PDF edition, source index, and exact derivation under `new-story-paper`'s rules. The numbers below are **toy discrete values**, not experimental measurements or claimed parameters of the paper.

## Formula 1: a candidate's weight

\[
w(x) = \frac{\hat p(x)}{p(x)}
\]

**Question at pixel P.** We can afford to propose light candidates using a cheap rule `p`, but we would prefer to keep candidates favored by a target score `p̂`. If we pick directly by the target score, why might the cheap proposal still influence which candidate wins?

**Create the missing need.** A frequently proposed type of candidate occurs more often in the candidate list, *before* the target score is applied. To express desired importance relative to the mechanism that produced the candidate, compare the target with the proposal: `p̂/p`. For a continuous domain, `p` is a probability **density**; the discrete example below uses probability masses for easier arithmetic.

**Two candidates on screen.** In a deliberately simple world with two possible candidates A and B:

| Candidate | Proposal probability `p` | Unnormalized target `p̂` | Ratio `w = p̂/p` |
| --- | ---: | ---: | ---: |
| A | 0.75 | 1 | 4/3 |
| B | 0.25 | 3 | 12 |

The target scores alone favor B over A 3:1. But A is proposed three times as often as B. The ratio therefore favors B more strongly *among the candidates that actually appeared*. If the drawn list happens to contain one A and one B, weighted resampling keeps B with probability `12/(12 + 4/3) = 0.9`.

**Caution:** The conditional selection probability for this one candidate list is **not** the unconditional probability that RIS emits B. With a *finite* number of drawn candidates, RIS generally does not reproduce the normalized target exactly; its estimator includes a correction. Do not say otherwise. The target `p̂` here is an **unnormalized score**, not by itself a PDF.

**Visual handoff.** Show the same P and two labeled light candidates, with proposal-frequency bars, target bars, the two ratios, and a sampled selection. Then ask: "We have changed which candidate is likely to be kept. How do we still estimate the light correctly from one kept candidate?" This question opens Eq. (6) and the definition of `W` rather than jumping to them.

## Formula 2: why a final correction is needed

For one kept RIS candidate `y` from `M` proposals, the paper's estimator has the form

\[
\widehat{L} = \frac{f(y)}{\hat p(y)}\left(\frac{1}{M}\sum_{i=1}^M \frac{\hat p(x_i)}{p(x_i)}\right)
= f(y)\,W,\qquad
W = \frac{\sum_i w_i}{M\,\hat p(y)}.
\]

Here `f` is the actual integrand; `p̂` controls resampling and need not equal `f`; `W` is the estimator multiplier used *after* selecting `y`. The sum accounts for all proposed candidates, division by `M` takes their average, and division by `p̂(y)` compensates for the selected candidate's target score. Establish this from the paper's estimator rather than suggesting that every factor is a mere convention.

**Build a tiny consistency check.** Let the discrete space be A and B with `p(A)=p(B)=1/2`, `p̂(A)=f(A)=1`, and `p̂(B)=f(B)=3`. Use `M=2` independent proposals. Then `w(A)=2`, `w(B)=6`. The correct discrete total is `L=f(A)+f(B)=4`.

- If the proposals are **A,A** (probability 1/4): the average weight is 2, `f(y)/p̂(y)=1`, and the estimate is 2.
- If they are **A,B or B,A** (probability 1/2 combined): the average weight is 4, and the estimate is 4 regardless of which candidate is kept, because we deliberately chose `f=p̂` for this illustration.
- If they are **B,B** (probability 1/4): the average weight is 6 and the estimate is 6.

The average *over the random proposal lists* is `(1/4)×2 + (1/2)×4 + (1/4)×6 = 4`. An individual estimate may be 2 or 6; unbiasedness is a statement about the expectation, not every realized estimate. This simplified arithmetic is a sanity check, **not** the general proof and not evidence about ReSTIR's reuse/bias modes. Check the paper's positivity and support conditions before a general unbiasedness claim.

**Counterfactual worth showing.** If we throw away the average weight in this toy example and report only `f(y)/p̂(y)`, the output is always 1, even though the quantity to estimate is 4. The missing correction therefore changes the estimator's scale; it is not just decoration on the formula.

**Handoff.** "The reservoir needs to retain one candidate and enough accumulated weight to compute this multiplier. That is why the next step tracks a kept sample, a count, and a running weight sum." Now the reservoir fields have a reason to exist.

## How to use this example in the storyboard

- Reuse the current RIS demonstration and pixel P where possible. Check whether it exposes the exact proposal, target score, per-candidate weight, running sum, selected candidate, and final estimator multiplier. If not, plan the missing readouts and step values before writing additional narration; do not claim an unverified view exists.
- Keep the source-index callouts for Eq. (5), Eq. (6), and Algorithm 1, and separate paper claims from the lab's toy numbers.
- Use `formulaSaid` for the complete rule once the components have been earned. The earlier moments should show two candidate bars or one intermediate equality instead of a wall of TeX.
- Respect the existing English/zh-CN one-paragraph-per-sentence mapping and `new-story`'s listener/fact-check rules. A translation should preserve the distinction among proposal density, unnormalized target, candidate weight, and estimator multiplier.
