---
name: new-story
description: Plan, research, write, review, and revise a Render Lab investigation's narrated story (its storyboard and the demonstrations it needs): engine-source research, the knowledge level, a chapter outline pairing each claim with what shows it, demonstrations that mirror the engine's storage, the storyboard's sentences and cues, per-chapter reviews from offscreen stills, a fact check, and both editions' build. Scales from adjusting one cue or sentence to a whole new story. Use when asked to create or plan a story or video, research a feature for one, outline or restructure chapters, write, adjust, or review narration, cues, animations, or depth, or act on feedback about a built video. For the storyboard's Lamlet syntax use `render-lab-lamlet`; for the package, port, and source index use `new-investigation`.
---

# A story for an investigation

A story is an investigation's narrated video: research into the engine's source, an outline, the demonstrations it
needs, and a storyboard (`packages/<package>/video/storyboard.lam`, a Lamlet presentation). This skill is the process
and the standard of what a story says and shows. Other skills own the rest:

- `new-investigation` (**required**): its "Precision" section is the standard every sentence meets; its references hold
  the package, the port, the source index, and the video's inputs, for Phase 4 and when a demonstration needs code.
- `render-lab-lamlet` (**required when writing cues**): the storyboard's syntax, its vocabulary, the translation's
  form.
- `knowledge-graph` (**useful**): the graph, the audience's commands, and the Prerequisites tab.

## Choose the depth

AGENTS.md, "How deep a task goes", sets the levels; for a story they are:

| Task | Do | Load |
|---|---|---|
| **Local**: a cue, a sentence's wording, a camera key, a callout, a timing | Edit it; `--prepare-only`; render stills across the beat (`references/storyboard.md`, "Check each chapter") and look at them. A changed or split English sentence changes its zh-CN paragraph too (a cue-only change leaves the translation alone). A changed **fact** is verified against the checkout before it is narrated (the fact check, for that sentence). | `references/storyboard.md` |
| **Chapter**: a chapter rewritten, a beat or demonstration added, a chapter's depth or audience changed | Phases 5 to 7 for that chapter (with Phase 3 for a new demonstration), its neighbours' bridges, its animation count, the knowledge check, then Phase 8's checks | the references of those phases |
| **Story**: a new story, or a redesign of an existing one | Every phase, in order | each phase's reference as you reach it |
| **Feedback** on a built story | `references/review.md`, "Revising a finished story" | it, then what the corrections need |

## What is required at every depth

These do not relax for a small change:

- Every claim about the engine is read from the checkout at its recorded commit and cited by file, symbol, and line
  (`new-investigation`, "Precision"); the narration never states as the engine's what the lab does differently, and
  says where the lab differs.
- Every factual sentence is backed by a callout or its chapter's `sources`; numbers about the lab's scene come from the
  lab's code, never typed in.
- Each sentence has a visual that shows it; a listing highlight changes the scene; a view is explained when it appears.
- The translation keeps one paragraph per English sentence, the same headings and formulas.
- What is published respects the engine's license and the optional module's boundary (AGENTS.md): research, outlines,
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

They are published with a public package, like the video that cites the same lines: cite by file, symbol, and line,
and keep quotes short (none at all for an engine whose license forbids them). For a local package they stay local, as
the package does. The working files are deleted once the story is finished.

Keep a guide for the investigation beside its code (`packages/<package>/src/<engine>/<feature>/AGENTS.md`) while the
story is being made: the checkout's version and commit (never its location), the user's preferences for this story (the
subject object, what to hide in close-ups, a camera style), decisions the user made and ones an agent made that are open
to change, the current phase, and pending requests with what doing them would take. Read it before each phase, and add
to it whenever the user decides something. Git ignores it (`/packages/**/AGENTS.md`), so it is never published.
**When the story is finished** (the user has watched the built video and has nothing left to adjust), delete the guide
and the working files, and tell the user about any pending request in the guide that was not done, so none is lost.

## Gates

Phases 1, 2, and 3 end at a gate: the user agrees to the research's scope, the chapters and their budgets, and the
demonstrations, before the next phase builds on them. If the user has asked you to work through the phases without
stopping, make each gate's call yourself, record it and its reasons in the guide, and continue; the user reviews the
recorded calls at the end. Never skip the work a gate reviews.

## The phases

| Phase | Output | Load | Done when |
|---|---|---|---|
| 1. Research | `story/research.md` | `references/research.md` | every subtopic cited, open questions answered or listed, lab differences noted; gate |
| 2. Outline | `story/outline.md`, `[audience]` in `knowledge.toml` | `references/outline.md` | every key claim has a source and a visual, every chapter a budget, the level checked; gate |
| 3. Demonstrations | `story/demonstrations.md`, the views and their interactions | `references/demonstrations.md` | storage mirrored, every view interactive, every taught topic demonstrated, pick round-trip in the selftest; gate |
| 4. The base | index entries, port, mirror, selftest, listings, formulas, step parameters | `new-investigation` (procedure steps 3 to 6) | below |
| 5. Storyboard | `storyboard.lam`, chapter by chapter | `references/storyboard.md` | each chapter prepared and its stills looked at |
| 6. Showing the data | animations, interactions, views | `references/storyboard.md`, Phase 6 | each chapter's animations counted (at least 32, each demonstrating something), its per-beat checks passed |
| 7. Review | `story/reviews/`, revisions | `references/review.md` | every review row fixed or answered, every fact verified or a stated simplification |
| 8. Translate and build | `storyboard.zh-CN.lam`, both editions | below | the checks below pass, the user asked to watch |

### Phase 4: The base

Before the storyboard, build what it stands on (`new-investigation`, procedure steps 3 to 6):

- the source-index entries for every claim in the outline, with their hashes from the checkout;
- the port, its CPU mirror, and the selftest's comparisons and invariants;
- the pseudo-code listings and formulas;
- a step parameter for every intermediate step the outline shows, each defaulting to "no change", with a slider in the
  dock.

Done when the selftest passes with the new comparisons and `render-lab --investigation <id> values` lists every step
parameter.

### Phase 8: Translate and build

- Write `storyboard.zh-CN.lam`: the same headings, one paragraph per English sentence, the same formulas, no cues
  (`render-lab-lamlet`, "The translation"). Technical terms stay English; keep its `glossary` of the terms'
  translations consistent. The Chinese edition keeps the English narration, timed by the English sentences.
- Check (one build or GPU job at a time):
  - `cargo build --release --locked --offline`, and each local package's workspace, with no warnings;
  - `--prepare-only` for both languages;
  - the selftest with `--timeline`;
  - the stills of every beat (`--snapshots-only`), looked at;
  - after a change to a storyboard's form or to `tools/video`, every edition's prepared outputs unchanged
    (`tools/video/compare.py`);
  - the full build in the background, English first, then `--lang zh-CN`.
- Report the checks and the corrections, and ask the user to watch the result.

A local change runs `--prepare-only` (both languages when a sentence changed) and the stills of the beats it changed;
the build too if code changed. A chapter change adds the selftest with `--timeline` (labels, text sizes, fit) and
both languages' `--prepare-only`. Render the full video only when
the user asks for it or the story is being delivered.
