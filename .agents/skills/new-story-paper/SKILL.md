---
name: new-story-paper
description: The paper variant of a Render Lab story, loaded with `new-story`, for an investigation that demonstrates a published paper's method (a package, packages/paper_<short>/, investigation id paper/<short>). Holds how a paper is read (abstract, introduction, and conclusion for the contributions; the related work and what each does; the method in detail; the whole paper), asking the user for the authors' source code, checking the code against the paper step by step and recording each divergence, the precision standard for claims about a paper, its citation index and APA references, and the story's chapters: Introduction, Related work, Method, Divergence (only with diverging code), Experimentation (measured on this machine, no numbers spoken), and Conclusion with the references. Use when asked to investigate, demonstrate, or tell the story of a paper, or to revise one; for an engine's feature use `new-story-engine`.
---

# A paper's story

This skill is loaded with `new-story`, which holds the process every story shares (the outline, demonstrations,
storyboard, reviews, translation, and build); this one holds what is particular to a story that demonstrates a paper's
method. The package, the port, and the index are `new-investigation`'s, with this skill's precision rules in place of
its engine rules (its step 0 and "Precision" say where).

The paper is the source. The lab implements the method **as the paper describes it**; the authors' code, when the user
provides it, is checked against the paper and its divergences are recorded and narrated, never silently adopted.

## Where it lives

- A package `packages/paper_<short>/` (`paper_restir`), its investigation id `paper/<short>` (the `Engine` it returns
  is the shared group `paper`, "Papers"; `new-investigation`'s `references/code.md`).
- Implementing a method from a paper's text is the lab's own code (MIT, the lab's author), published; the paper is
  cited, never copied: quotes stay short (a phrase the index needs to locate a passage), and its figures are redrawn
  as the lab's demonstrations, never reproduced.
- The authors' code has its own license: read it first. It is **read**, to compare, not ported. Porting from it is
  allowed only when its license allows publishing derived code (then it keeps its copyright, a row in
  `THIRD_PARTY.md`, and a `[[notice]]`); with a license that does not, nothing derived from it leaves the user's
  machine (AGENTS.md's rules for the optional module), and the divergences are described in the lab's own words,
  cited by file and line.
- Ask the user where the PDF and the code are (or read the variables the package's video names); never write a
  location into a file. Keep both read-only.

## Precision for a paper (required)

- **Cited and checked.** Every claim about the paper names its section, page, and the equation, figure, table, or
  algorithm line it rests on, through the source index (`new-investigation`'s `references/sources.md`, "A paper's
  entries"), read from the PDF at the recorded edition. Never narrate from memory, from the abstract alone, from a
  blog, a talk, or a later paper's summary: read the passage and the derivation around it.
- **Said, derived, or the lab's.** Every step the story explains is one of three, and the narration keeps them apart:
  **stated** by the paper (cited); **derived** from what it states (the derivation shown, each step cited); or a
  **choice the lab makes** where the paper leaves a gap (a buffer layout, a random-number generator, a tie-break),
  said as the lab's ("the paper does not say how …; the lab …") and listed in the README's "Simplified" list. A gap is
  never filled with how such systems usually work and narrated as the paper's.
- **This edition.** Claims are about the recorded edition (journal, conference, arXiv version, author's version) and
  its supplementary material; a later erratum or version is recorded with its own citation, and the story says which
  it follows.
- **Numbers as the paper has them.** Candidate counts, neighbour counts, radii, thresholds, clamps, history caps, and
  the order of operations, with the configuration the paper gives them for ("thirty-two candidates in the paper's
  main results"). Results the paper reports are its claims about its own setup (its hardware, scenes, and times).
- **The lab's numbers are measured**, as `new-investigation` says, and the Experimentation chapter speaks none.
- **Mirror, port, and test.** The method is ported (WGSL) from the paper's equations and algorithms and mirrored on
  the CPU; the selftest compares them term by term, plus the properties the paper proves or promises (an estimator's
  expectation, weights that sum to one, a reservoir that keeps each candidate with its stated probability), checked
  statistically where they hold in expectation.
- **Divergences are findings.** Where the authors' code does something the paper does not say, or says otherwise, it
  is recorded with both citations (`references/reading.md`, "The code against the paper"); the lab follows the paper.
- **Related work is cited at its source.** A claim about a related work is read from that work when it is at hand
  (the user's library, `~/Documents/papers` and `~/Documents/books`, or open access) and cited there; otherwise it is
  what this paper says of it, cited to this paper and narrated as such ("the paper describes …").
- **Corrections are reported**, as `new-investigation` says: when the paper contradicts a request, an earlier
  statement, or a common belief, tell the user with the evidence.

## Phase 1: Reading the paper

**Formula comprehension.** Load `new-story`'s `references/storytelling-formula.md` when the method's core equations are researched, outlined, narrated, or revised. Classify each equation as core, bridge, or reference, then give core equations the complete causal arc and an executable example before the story advances. The paper's stated/derived/lab-choice distinction and citation requirements continue to govern every step.

`references/reading.md`, in order (required; the user's process):

1. The abstract, the introduction, and the conclusion: the problem, the core contributions, the claimed results.
2. The related work: every work it discusses and what each does, and which ones the method builds on.
3. The method, in detail: every equation, algorithm, and parameter.
4. The whole paper (results, limitations, appendices, supplementary material).
5. The source code: if the user has not given it, ask for it (with "there is none" as an answer); with code, check
   every step of the paper's procedure against it, one by one, and record each divergence.

Done when `story/research.md` holds the contributions, the related-work table, the method's steps all cited, the open
questions answered or listed, and the lab's choices noted; and, with code, `story/divergences.md` covers every step.
Then the gate.

## The chapters of a paper's story

Six kinds of chapter, in this order (required); the shared standard (`new-story`'s `references/outline.md`, "The shape
of a story", and every later phase) holds in each.

1. **Introduction** (one chapter). Its first sentences name the paper's authors and title, shown on screen as the
   paper's title card (title, authors, venue, year). Then the problem the paper solves, shown in the lab as a
   comparison the viewer can see (the scene rendered the way the problem leaves it, beside the paper's result at the
   same camera), how the paper solves it in one or two sentences, and its main contributions as a short list on
   screen, each in plain words. It also says what the story teaches and what the viewer should already know (the
   shared rule), and shows the architecture map of the method.
2. **Related work** (one chapter). Each related work the paper discusses, in a sentence or two: what it does and how
   this paper relates to it (builds on it, improves on it, competes with it), its citation on screen. The works the
   method **rests on** are taught in depth, by "Teaching a topic" (`new-story`'s `references/outline.md`), each with a
   demonstration of its own (for ReSTIR: resampled importance sampling and weighted reservoir sampling, the two
   pieces its reservoirs are made of). The rest stay brief; group them by approach rather than listing papers one by
   one.
3. **Method** (any number of chapters). The paper's method, explained deeply and in detail, with interactive
   demonstrations, to the shared standard: one mechanism per chapter, in the order the method builds its data, each
   equation said in words and then shown, each algorithm worked on one element before many, each parameter swept.
4. **Divergence** (one chapter, only when the authors' code was provided and diverges from the paper). Each
   divergence that changes what the method computes: the paper's step and the code's, both cited on screen, what the
   difference changes (shown where the lab can show it, as a choice beside the paper's path), and that the lab and the
   story follow the paper. Divergences that change nothing on screen are listed briefly.
5. **Experimentation** (one chapter). Experiments run on **this machine**, at the time the video is recorded: the
   comparisons the paper makes (its method against its baselines at equal time or equal samples, its ablations),
   rebuilt as the package's experiments and shown live (the performance panel, A/B wipes and differences, the
   measurement against a reference; the `experiments` skill). The narration says so plainly ("These are the
   experimentation data run on this current machine.") and **speaks no numbers** (no times, no errors, no ratios,
   neither the lab's nor the paper's): it says what each comparison sets side by side, how to read it, and what to
   look for, and the figures on screen are measured as the frames render. Nothing measured is typed into text.
6. **Conclusion** (one chapter), as `new-story` ends a story: a recap that walks the method again in order, each
   mechanism's contribution shown, the paper's own conclusion (what it claims, the limitations and future work it
   names), and three takeaways. It ends on the **references**: every work the story cites, the paper included, in APA
   style (7th edition), shown on screen from the index's `references` (`new-investigation`'s `references/sources.md`,
   "References").

Each chapter ends with its sources beat as every story does; for a paper, the sources card lists its sections and
pages.

## Demonstrations of a paper's method

For `new-story`'s Phase 3 (`references/demonstrations.md` there), steps 1 and 2: each structure is shown as the paper
defines it (a reservoir's fields; a candidate list; a neighbour set), with the paper's names and the values it keeps.
The paper's figures say what its authors found worth seeing: take them as references and draw them as the lab's own,
live and interactive, never as copies. A structure the paper defines on paper only (a probability distribution, an
estimator's weight) gets a view that shows its value per element (a histogram, a plot, a colour per pixel). Where the
authors' code stores a structure differently, the demonstration follows the paper, and the Divergence chapter shows
the code's.

## The fact check

`new-story`'s fact check (`references/review.md` there), with the paper as the source: every factual sentence and
every `inspect` against the PDF at the recorded edition, with section, page, and passage as evidence, in
`new-investigation`'s classes (verified, correct but simplified, potentially misleading, incorrect, needs
investigation), plus one more check: each sentence that explains a step is classed as stated, derived, or the lab's
choice, and narrated as that. Divergence claims are checked against the code too, with file:line evidence. The quiz's
right answers are checked the same way.

## Check

The checks are `new-story`'s for the depth of the change, with the paper's:

```sh
python3 packages/paper_<short>/src/paper/<short>/source_index.py      # rewrite the index from the PDF (asks where it is)
cargo run --release -- --investigation paper/<short> selftest --report <scratch>/selftest.json
cargo run --release -- --investigation paper/<short> experiment paper_<short>.experiment/<name> --dry-run
python3 tools/video/build.py --investigation paper/<short> --prepare-only
python3 tools/video/build.py --investigation paper/<short> --snapshots-only   # the stills of every beat; look at them
```

The full build's verification (`tools/video/verify.py`) checks the PDF's SHA-256 against the index's `paper.sha256`,
and the code's commit against `engine.commit` when the index records the authors' code.
