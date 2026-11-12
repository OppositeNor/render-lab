---
name: new-story
description: The process and standard every Render Lab story shares, whatever it explains: an investigation's narrated video (its storyboard and the demonstrations it needs), from the outline and the knowledge level through demonstrations, the storyboard's sentences and cues, per-chapter reviews from offscreen stills, the fact check, the quiz, and both editions' build. Scales from adjusting one cue or sentence to a whole new story. Use when asked to create or plan a story or video, outline or restructure chapters, write, adjust, or review narration, cues, animations, or depth, or act on feedback about a built video; load with it the variant for what the story explains, `new-story-engine` (an engine's feature, read from its source) or `new-story-paper` (a paper's method, read from the paper). For the storyboard's Lamlet syntax use `render-lab-lamlet`; for the package, port, and source index use `new-investigation`.
---

# A story for an investigation

A story is an investigation's narrated video: research into what it explains, an outline, the demonstrations it
needs, and a storyboard (`packages/<package>/video/storyboard.lam`, a Lamlet presentation). This skill is the process
and the standard every story shares. What it explains decides its research, its precision rules, and the shape of its
chapters, which are a variant skill's (**required**, load one):

- `new-story-engine`: a game engine's feature, read from the engine's source at a recorded commit.
- `new-story-paper`: a paper's method, read from the paper (and from its authors' code, when the user provides it).

Other skills own the rest:

- `new-investigation` (**required**): the package, the port, its CPU mirror and selftest, the source index, and the
  video's inputs, for Phase 4 and when a demonstration needs code; its "Precision" section is the standard every
  engine sentence meets, and `new-story-paper`'s "Precision for a paper" the one every paper sentence meets.
- `storytelling` (**required when writing or revising narration**, from one sentence to a chapter, and when a review
  flags one): how each sentence is built (its known and new parts, its job), how sentences chain, how a passage
  develops an idea, and the sentence map that finds and fixes what is hard to follow.
- `render-lab-lamlet` (**required when writing cues**): the storyboard's syntax, its vocabulary, the translation's
  form.
- `knowledge-graph` (**useful**): the graph, the audience's commands, and the Prerequisites tab.
- `experiments` (**useful**; required for a paper's Experimentation chapter): measured runs and the Experiments tab.

## Choose the depth

AGENTS.md, "How deep a task goes", sets the levels; for a story they are:

| Task | Do | Load |
|---|---|---|
| **Local**: a cue, a sentence's wording, a camera key, a callout, a timing | Edit it; `--prepare-only` (both languages when a sentence changed; the build too if code changed); render stills across the beat (`references/storyboard.md`, "Check each chapter") and look at them. A changed or split English sentence changes its zh-CN paragraph too (a cue-only change leaves the translation alone). A changed **fact** is verified against its primary source before it is narrated (the variant's fact check, for that sentence). A changed sentence is mapped with its neighbours (`storytelling`). | `references/storyboard.md`, `storytelling` |
| **Chapter**: a chapter rewritten, a beat or demonstration added, a chapter's depth or audience changed | Phases 5 to 7 for that chapter (with Phase 3 for a new demonstration), its neighbours' bridges, its animation count, the knowledge check, then Phase 8's checks except the full build (the selftest with `--timeline` for labels, text sizes, and fit; both languages' `--prepare-only`) | the references of those phases, `storytelling`, the variant |
| **Story**: a new story, or a redesign of an existing one | Every phase, in order | each phase's reference as you reach it, the variant |
| **Feedback** on a built story | `references/review.md`, "Revising a finished story" | it, then what the corrections need |
| **Quiz**: writing or revising a story's quiz | `references/quiz.md`: the quiz agent, then the quiz file and its checks | `references/quiz.md` |

## What is required at every depth

These do not relax for a small change:

- Every claim is read from its primary source and cited (the engine's checkout at its recorded commit, by file,
  symbol, and line; the paper, by section, page, and equation, figure, or algorithm line), under the variant's
  precision rules; the narration never states as the source's what the lab does differently, and says where the lab
  differs.
- Every factual sentence is backed by a callout or its chapter's `sources`; numbers about the lab's scene come from the
  lab's code, never typed in.
- Each sentence has a visual that shows it; a listing highlight changes the scene; a view is explained when it appears.
- The narration is clear and in-depth: written for the ear (`references/storyboard.md`, "Written for the ear"), built
  and checked sentence by sentence with the `storytelling` skill's map, and
  each changed chapter passes the cold-listener review and the native-language review, one reviewer per language,
  briefed in its own language (`references/review.md`).
- The translation keeps one paragraph per English sentence, the same headings and formulas.
- What is published respects the source's license and the optional module's boundary (AGENTS.md): research, outlines,
  and reviews of a local package's story stay in that package.
- Visual checks are offscreen renders you look at; one build or GPU job at a time.

In the references, **required** marks a correctness rule; the other rules are the standard a story is reviewed
against, and an exception needs a reason a reviewer would accept. Wording like "about", "prefer", or "where it fits"
marks a heuristic.

## Working with agents

The main agent designs, decides, edits, and checks. Research and review go to subagents when the environment has them
(on a cheaper model when one can be chosen: AGENTS.md), with a brief that carries everything the agent needs, since it
knows only its brief. Run independent briefs in parallel where possible; send follow-ups to the agent that did the work
when your environment can message it, or else start a fresh agent with its brief and report. Without subagents, do each
brief yourself, one scope at a time, and keep its findings in the phase's working file before starting the next, so
nothing depends on what you remember. The references say this per phase.

## Working files

The phases write into the package's `story/`:

```
packages/<package>/story/research.md        phase 1: the merged report               (stays with the package)
                         demonstrations.md  phase 3: how each structure is stored, shown, and explored   (stays)
                         outline.md         phase 2: the chapters, and what shows each claim   (working file)
                         reviews/           phase 7: one review per chapter, and the fact check (working files)
```

The variant may add files (a paper's `story/divergences.md`). They are published with a public package, like the video
that cites the same sources: cite as the variant says, and keep quotes short (none at all for a source whose license
forbids them). For a local package they stay local, as the package does. The working files are deleted once the story
is finished.

Keep a guide for the investigation beside its code (`packages/<package>/src/<group>/<topic>/AGENTS.md`) while the story
is being made: the source's version (the checkout's commit, or the paper's edition; never a location on disk), the
user's preferences for this story (the subject object, what to hide in close-ups, a camera style), decisions the user
made and ones an agent made that are open to change, the current phase, and pending requests with what doing them would
take. Read it before each phase, and add to it whenever the user decides something. Git ignores it
(`/packages/**/AGENTS.md`), so it is never published. **When the story is finished** (the user has watched the built
video and has nothing left to adjust), delete the guide and the working files, and tell the user about any pending
request in the guide that was not done, so none is lost.

## Gates

Phases 1, 2, and 3 end at a gate: the user agrees to the research's scope, the chapters and their budgets, and the
demonstrations, before the next phase builds on them. If the user has asked you to work through the phases without
stopping, make each gate's call yourself, record it and its reasons in the guide, and continue; the user reviews the
recorded calls at the end. Never skip the work a gate reviews. A question only the user can answer (a paper's source
code, say) is asked when the phase reaches it, not saved for a gate.

## The phases

| Phase | Output | Load | Done when |
|---|---|---|---|
| 1. Research | `story/research.md` | the variant's research reference | the variant's research is done; gate |
| 2. Outline | `story/outline.md`, `[audience]` in `knowledge.toml` | `references/outline.md`, the variant's story structure | every key claim has a source and a visual, every chapter a budget, the level checked; gate |
| 3. Demonstrations | `story/demonstrations.md`, the views and their interactions | `references/demonstrations.md`, the variant's notes on data | the data mirrored as its source keeps it, every view interactive, the interaction review answered, every taught topic demonstrated, pick round-trip in the selftest; gate |
| 4. The base | index entries, port, mirror, selftest, listings, formulas, step parameters | `new-investigation` (procedure steps 3 to 6) | below |
| 5. Storyboard | `storyboard.lam`, chapter by chapter | `references/storyboard.md`, `storytelling` | each chapter's sentences mapped with no fault left, the chapter prepared, its stills looked at |
| 6. Showing the data | animations, interactions, views | `references/storyboard.md`, Phase 6 | each chapter's supporting animations at least ⌈T / 6 s⌉, no gap over about 12 s, every named mechanism shown (a replay only with a reason the narration gives), its per-beat checks passed |
| 7. Review | `story/reviews/`, revisions | `references/review.md`, `storytelling`, the variant's fact check | every review row (the interaction review's included) fixed or answered, every fact verified or a stated simplification |
| 8. Translate and build | `storyboard.zh-CN.lam`, both editions | below | the checks below pass, the user asked to watch |

### Phase 4: The base

Before the storyboard, build what it stands on (`new-investigation`, procedure steps 3 to 6):

- the source-index entries for every claim in the outline, written by the package's index script from the source
  (the checkout's lines and their hashes, or the paper's passages and theirs);
- the port, its CPU mirror, and the selftest's comparisons and invariants;
- the pseudo-code listings and formulas;
- a step parameter for every intermediate step the outline shows, each defaulting to "no change", with a slider in the
  dock.

Done when the selftest passes with the new comparisons and `render-lab --investigation <id> values` lists every step
parameter.

### Phase 8: Translate and build

- Write `storyboard.zh-CN.lam`: the same headings, one paragraph per English sentence, the same formulas, no cues
  (`render-lab-lamlet`, "The translation"), each paragraph written as Chinese rather than mirrored from the English
  (`references/storyboard.md`, "Written for the ear"). Technical terms stay English; keep its `glossary` of the terms'
  translations consistent. The Chinese edition keeps the English narration, timed by the English sentences.
- Check (one build or GPU job at a time):
  - `cargo build --release --locked --offline`, and each local package's workspace, with no warnings;
  - `--prepare-only` for both languages;
  - the selftest with `--timeline`;
  - the stills of every beat (`--snapshots-only`), looked at;
  - after a change to a storyboard's form or to `tools/video`, every edition's prepared outputs unchanged
    (`tools/video/compare.py`);
  - the full build in the background, English first, then `--lang zh-CN`, only when the user asks for it or the story
    is being delivered.
- Report the checks and the corrections, and ask the user to watch the result.

### The quiz

A story can end with a quiz: multiple-choice questions on how its algorithm works, each asked over a live view of a
demonstration (the homepage's Quiz button). Write one when the story is delivered, or when the user asks:
`references/quiz.md` (a dedicated quiz agent proposes the questions; every question about the algorithm, never its
configuration).
