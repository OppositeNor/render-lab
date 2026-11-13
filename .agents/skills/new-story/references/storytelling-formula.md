# Storytelling a formula

Load this with `references/storytelling.md` whenever a passage teaches a core equation rather than only mentions one: researching, outlining, narrating, or revising it, or when a review or the user says an equation beat is hard to follow. It applies to engine and paper stories alike; a paper's equations are classified while it is read (`new-story-paper`, `references/reading.md`).

A formula is a compact record of a mechanism. Teaching it means **uncompressing the decisions that made its terms necessary**. A listener who can pronounce every symbol but cannot say why the denominator, average, weight, or correction appears has not yet learned the formula.

This file is **not** a replacement for `references/storytelling.md`: that file governs sentence-level known-to-new flow, spoken prose, and the cold-listener review. This file governs the larger *mathematical understanding arc*, from the motivating problem to the reason for each essential term. For a published paper, use it with `new-story-paper`'s precision rules and `references/reading.md`: the paper is the source, not the storyteller's intuition.

## When to load, and how much to teach

Classify each equation **before** drafting narration. The classification reflects the audience's stated prerequisites (the story's listener card), not typographic complexity.

| Kind | Meaning | Treatment |
| --- | --- | --- |
| **Core** | The method depends on this equation; understanding it is a learning objective; a non-obvious term determines correctness | Full formula arc below, usually across several beats, with an actual worked example and an interactive demonstration wherever the story can show one |
| **Bridge** | Connects two already-taught ideas or introduces one important new factor | A shortened arc: recall the known piece, isolate *what changed and why*, show the consequence on the running example, and state the handoff |
| **Reference** | A standard definition already in the declared prerequisite set, or a formula cited but not needed to follow the method | Recall in plain words, identify its role, cite it, and move on; do not manufacture a tutorial |

Escalate a bridge/reference to **core** if the listener needs an untaught concept, an unexpected denominator/normalization, or a proof-critical assumption to understand the next step. Several symbols on one card do not make several equations: split the *reasoning*, not arbitrarily the notation.

## Required formula arc (core equations)

Read `references/storytelling-formula-arc.md` when planning or writing a core formula. The stages below are a **dependency order**, not a rigid one-sentence-per-stage script. They may span beats or chapters; merge only when the audience still gets every needed reason.

1. **Need — the question.** Point at one concrete object (pixel P, sample y, one reservoir) and state what we want to calculate, choose, or preserve, and why the naive way does not suffice.
2. **Anchor — one small case.** Use two samples, a simple ray, or a tiny discrete distribution. Show the quantities *before* naming the full symbolic rule; carry the same objects and names all the way through.
3. **Construct — one justified change at a time.** Start from a rule the listener knows. Add the numerator, denominator, sum, count, normalization, or condition only when a visible or calculable failure makes its purpose clear. State where the term is evaluated and what its units/support mean when relevant.
4. **Name and show — the complete equation.** Only now present the formula, grouping and highlighting parts as they were built. Say its *meaning* rather than merely reading its symbols. Every new symbol must have a unique referent already introduced or explained immediately.
5. **Work — predict and compute.** Follow one actual input through the full formula, with small values, intermediate readouts, and a result the listener can interpret. Distinguish a single realization from an expectation or a limit.
6. **Stress — remove or vary a critical part.** Ask what would break without one term (or as a parameter changes), check the prediction with arithmetic, a limiting case, a dimensional argument, or a faithful experiment. Do not infer correctness solely from a pretty image.
7. **Land — the exit sentence.** Say in ordinary words what the whole formula achieves, under what assumptions, and exactly where its result enters the next step of the paper or renderer.

**Hard stop:** after introducing a *core* formula, do not immediately jump to the next independent equation. The listener must first see at least one fully worked case and hear the reason for the formula's non-obvious term(s). A formula card plus a term glossary is not completion.

## Rules that prevent a convincing but false story

- **Source of each claim.** For papers, tag factual statements as **stated by the paper** (with section/page/equation), **derived from it** (show the derivation), or **the lab's choice** (label clearly). A plausible-sounding motivation is not automatically an author's motivation. Cite the paper's actual conditions for unbiasedness, normalization, valid weights, and support.
- **No hidden algebra.** If moving from A to B requires a mathematical identity, a change of measure, a conditional probability, or an estimator correction, show that intermediate relation. Where a full proof is too long, say what is established, what is only intuition, and point to the proof.
- **Do not confuse different claims.** Being unbiased *in expectation* does not mean each sample is exact; a finite-sample resampler need not sample the target distribution exactly; an unnormalized target is not automatically a PDF; a candidate weight is not necessarily an estimator weight.
- **Check simplified examples against the real rule.** Toy inputs can be invented for pedagogy, but label them as toy values. Keep domains and conditions consistent with the source; never label a toy configuration as the paper's experiment.
- **One spoken idea per sentence.** Follow `references/storytelling.md` and `references/storyboard.md`. Never make the viewer decode a full expression while hearing a second new concept.
- **Show real causal changes.** Use the project's demonstrated values, steps, readouts, or plots: vary an input and expose its intermediate and output values. A glowing formula term, a camera sway, or a generic scene animation by itself does not count as demonstrating the mathematics.
- **If a term cannot be justified**, consult the primary source and its appendix first. Record the unresolved question rather than inventing a purpose. An honest limitation is better than a fictitious derivation.

## Where this fits in Render Lab

- **Reading (`new-story-paper`)**: when inspecting the method, classify equations and record for core ones: the question, prior rule, new terms, cited derivation, assumptions, a small case, and what downstream step consumes the result. Keep these notes in the investigation's `story/research.md`; do not create an unrelated claims database.
- **Outline (`new-story`)**: attach the formula arc to the beats that teach it, with a visual/readout and a time budget for the example and counterfactual. A core equation can require several beats. If the explanation cannot fit, adjust scope or budget—not the mathematics.
- **Storyboard**: explain the problem and work the small case before showing the final `formulaSaid` card. Use `render-lab-lamlet` for actual cue syntax; keep a term visible only when narration is discussing it. Use the same concrete pixel/sample throughout. For the zh-CN edition, follow the one-paragraph-per-English-sentence rule; preserve mathematical meaning, not English syntax.
- **Review**: apply `references/storytelling-formula-review.md` as a *separate formula comprehension gate* in addition to the existing sentence map, factual review, layout review, and cold-listener review. Use `references/storytelling-formula-restir.md` as a model anchored in the lab's ReSTIR story, not as a universal template.

## Check

For each **core** equation, the reviewer must answer **yes** to all five:

1. Could a listener name the *specific problem* this equation solves, without seeing its notation?
2. Can they explain why every **non-obvious** numerator, denominator, normalizer, sum, count, or correction is necessary—and what fails without it?
3. Can they reproduce a small numerical or geometric example with the formula's actual inputs, intermediate values, and output?
4. Are the mathematical claims and assumptions checked against the source, with toy examples and derived statements distinguished from the source's claims?
5. Did the narrative connect the result to the next mechanism before introducing another independent equation, with a demonstration that changes for mathematical reasons?

If any answer is no, the equation is **mentioned, not taught**. Revise the missing stage before polishing transitions. The rubric and failure patterns are in `references/storytelling-formula-review.md`. After edits, run the parent story's chapter checks and both languages' preparation as required there; no special build command is introduced by this file.
