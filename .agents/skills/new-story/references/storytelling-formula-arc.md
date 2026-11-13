# Formula arc: plan from what the listener can already reason about

Load for a **core** formula, or a bridge formula whose one added term is surprisingly important. The order is the order of dependencies in the listener's mind, not necessarily the order of the paper's printed equations.

## The formula brief

Write a compact brief *before* scripting. Put it in the relevant section of the package's `story/research.md` and its teaching beats in `story/outline.md`:

| Field | Question |
| --- | --- |
| **Source** | Exact paper edition, section, page, equation, and derivation; or engine file/symbol/line |
| **Audience already knows** | Which prerequisite can be used without re-teaching it? |
| **Concrete anchor** | Which one pixel, sample, ray, probe, or record remains visible from start to end? |
| **Need** | What result is impossible, wasteful, or wrong with the current rule? |
| **Naive attempt and failure** | What would the listener try first? What *specific* wrong result or cost does it cause? |
| **Construction** | Which previous rule is extended; what motivates each **new** term? |
| **Claim type** | For each crucial step: stated / derived / lab choice; what supports it? |
| **Worked instance** | Small values, displayed intermediate steps, predicted result, actual result |
| **Counterfactual and boundary** | Which one removed/changed term exposes its purpose? What assumptions make the rule valid? |
| **Handoff** | Which method step consumes the equation's result? |

Do not write the narration as a list of these fields. They are the *underlying logic* for a natural, continuous explanation.

## Build the audience's mental model

### 1. Begin before the formula

A useful opening is a question about a visible operation: "We drew the light candidates cheaply. How do we stop our cheap drawing rule from deciding which ones look important?" Do not open with a wall of TeX or an abstract term list.

Contrast the desired result with the affordable or existing result. A failure must be concrete: too many rays, overrepresented samples, a weight with the wrong scale, a biased boundary, or a missing probability mass. Avoid inventing a failure the paper does not assert.

### 2. Build with a single anchor

Use one continuously named record. If the formula reweights a sample `y` for the receiving pixel `P`, keep `P` and `y` on screen and give the other sample a distinct name. Every displayed quantity should have a caption explaining what its number represents—not merely its variable name.

Prefer a two-element discrete example for a probability or estimator, one pixel and its neighbor for reuse, or one vector and one angle for geometry. Only generalize to all pixels after the one instance works. Do not slip from discrete probability masses to continuous densities without saying the example is a discrete analogue.

### 3. Make the algebra follow the necessity

Explain a term with the pattern:

- **Without it:** "If we used only the target score, candidates seen more often from the cheap proposal could receive too much total influence."
- **Needed correction:** "We therefore divide by the probability of having proposed that candidate."
- **Consequence:** "The score now measures how important the candidate is relative to how often the proposal produces it."

This structure generalizes: if a sum accumulates candidates, why those terms rather than the selected term alone? If a count `M` appears, is it a mean, an effective count, or a weight of several represented candidates? If a factor uses the *receiving* pixel, what goes wrong if it uses the *source* pixel? If a normalization appears, which invariant does it preserve?

**Be mathematically precise:** `p(x)` can be a density, not a probability of the exact continuous sample; `\hat p(x)` may be an unnormalized target; some correction factors are only unbiased under specific support assumptions; some approximations deliberately introduce bias. State the form that the paper actually uses.

### 4. Give the whole equation only after the pieces have a job

When the complete expression finally appears, highlight the parts in the order they were motivated. `formulaSaid` means *say an interpretable sentence about the formula*, not simply a list of operators. A formula is allowed earlier as an orientation card **only** when it is explicitly a preview, not counted as having taught it.

For spoken narration, break explanations across sentences and beats. Keep symbols on screen while the narration talks about their role; the viewer cannot hold six unfamiliar meanings in working memory at once. Use the sentence map (`references/storytelling.md`) after the mathematical arc is correct.

### 5. Work the numbers and check the claim

Work the *actual equation*, not an unrelated analogy. Use values simple enough to verify. The output should be interpreted in its correct sense: conditional selection probability, estimator for an integral, expected value over random trials, per-pixel record weight, or approximation error. If the quantity is an expectation, one numerical trial cannot prove or disprove unbiasedness.

Animate one intermediate at a time with values the viewer can inspect. If the representation cannot display the term, plan a small readout, diagram, plot, or scripted step before writing the narration. The visuals should be driven by the same inputs as the algorithm, with a CPU mirror/selftest where the parent skills require it.

### 6. Remove a term—carefully

A *counterfactual* is a teaching device, not a claim the paper evaluated that ablation. Label it as the lab's illustration if the paper did not test it. Calculate or simulate what breaks, and identify the claim it demonstrates. Examples: sample frequency goes uncorrected, weights no longer normalize, dimensions do not match, the estimator loses the intended expectation, or a receiving pixel evaluates the wrong target. Do not claim that one changed image proves unbiasedness or correctness.

### 7. Land and bridge

Finish with a short, formula-free paraphrase: "We rate each proposed candidate by its desired importance *relative to* how often the cheap sampler proposes it." Then explicitly say which step uses that number next: selection, reservoir update, shading, or neighborhood reuse. Do not end a core arc on its last symbol.

## Scope control

Full arcs are for **core** equations, not every notation change. A bridge can be two or three well-anchored sentences plus a changed readout; a reference can be recalled in one sentence. Conversely, a core estimator can take multiple beats even if its printed formula occupies one line. Prefer fewer formulas genuinely understood to more formulas recited. If runtime is tight, trim optional context, choose a simpler numeric case, or ask to increase the chapter budget under the parent story rules. Never silently drop the failure, worked instance, and correction that make the method intelligible.
