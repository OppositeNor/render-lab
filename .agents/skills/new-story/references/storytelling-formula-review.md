# Formula comprehension review

Use **alongside** the known/new sentence map (`references/storytelling.md`) and the paper/engine fact check. A perfectly flowing sentence map can still hide a mathematical leap. Conversely, full mathematical detail can still be impossible to follow aloud. Both reviews are required for core formulas.

## Review a formula as a listener, not as the author

For each core equation, watch/listen from the beginning of its first teaching beat until its handoff. Hide the formula card, then try to answer:

| Gate | A passing explanation makes this possible | Failure symptom |
| --- | --- | --- |
| **Need** | Name the calculation's purpose and why the baseline fails | "They introduced W" without explaining the problem |
| **Causal structure** | Explain each surprising term's purpose and what changes if it is absent | Denominator/sum/count described only as a symbolic operation |
| **Executable example** | Follow one identifiable input and compute its intermediate and final values | Eight random-looking arrows but no traceable candidate or arithmetic |
| **Validity** | Distinguish PDF vs unnormalized target, estimate vs expectation, condition vs approximation | "Unbiased" asserted after a single sample, or finite-M output called an exact target draw |
| **Handoff** | State who consumes the result next | Formula disappears and an unrelated algorithm begins |
| **Source fidelity** | Locate the paper/source evidence for all factual claims; identify lab-only illustrations | Invented motivation narrated as the authors' finding |

For a bridge formula, check the **one change and its necessity**, plus its destination. For a reference formula, check only the prerequisite and relevance, not the full arc.

## Failure patterns and targeted repairs

| Anti-pattern | Why it fails | Repair |
| --- | --- | --- |
| **TeX → dictionary → next equation** | The listener knows the vocabulary but not the construction | Put a question and naive attempt before the TeX; add a worked case and a non-obvious-term counterfactual |
| **"This corrects the bias"** | Describes an outcome without showing the needed factor or the assumption | Show which imbalance exists, the correcting algebra, and when the claim holds |
| **"Take target divided by proposal"** | Tells the operation, not why division is required | Compare how often a candidate is proposed with how much influence is desired |
| **"This is an unbiased estimate"** | Confuses a property of expectation with a property of one trial | Show the expectation argument, or cite it explicitly and clarify that individual trials vary |
| **Everything is a full lesson** | Spends time on known notation and hides the main contribution | Classify equations core/bridge/reference and devote full arcs only to core |
| **Visual highlights but no change in data** | A coloured term is not a worked calculation | Bind each new step to input, intermediate number, state, or plot that visibly changes |
| **A toy example sold as a paper result** | Conflates illustration with evidence | Say "in this two-candidate example"; keep the actual paper's experiments separate |
| **Algebra skips the proof-critical step** | The listener cannot derive the next term, even with perfect wording | Restore the exact identity/assumption, or explicitly separate intuitive motivation from a cited proof |
| **Overconfident physical analogy** | Creates a mental model inconsistent with a density, probability, or estimator | State where the analogy ends and return to the real mathematical objects |

## Suggested review record

A table in the investigation's `story/reviews/` is enough. Do not create a new mandatory artifact type:

`Equation and source | Kind | Question | Missing stage | Mathematically checked? | Visual checked? | Fix | Verdict`

For each **core** formula the verdict is **TAUGHT** only when all five checks in `references/storytelling-formula.md` pass; otherwise **MENTIONED** and the specific missing stage is recorded. Re-run the review after revision. Also run the existing native-language, sentence-chain, timing, layout, and source verification checks. Do not inflate an explanation merely to satisfy an arbitrary minimum sentence or animation count.
