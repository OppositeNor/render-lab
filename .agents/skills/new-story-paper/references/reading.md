# Phase 1: Reading the paper

Load this for a paper story's research, or when a revision needs the paper read again. The precision rules every
brief carries are SKILL.md's "Precision for a paper"; this file is how the reading is organised. The steps run in
this order (required): each one reads what the step before decided matters.

## Who does it

With subagents: the main agent does steps 1 and 2 itself (they decide the scope), then sends one brief per method
section (step 3) and the code comparison's briefs (step 6) in parallel when the environment allows, on a cheaper model
(AGENTS.md). Without them, do each brief yourself, one scope at a time, writing its findings into its section of
`story/research.md` before starting the next. Either way, a finding is a claim with its section, page, and equation,
figure, or algorithm line; verify a sample of each report against the PDF yourself before merging it.

Extract the text with page breaks to search it (`pdftotext -layout <pdf> <scratch>/paper.txt`; each form feed is a
page), but read equations, figures, and algorithms from the rendered pages (the Read tool's `pages`), since text
extraction mangles them.

## Steps

1. **Record the edition.** Title, authors, venue, year, volume and issue or article number, DOI, the edition the PDF
   is (journal, conference, author's version, arXiv `vN`), its supplementary material, and the PDF's SHA-256. These
   go into the index's `paper` (`new-investigation`'s `references/sources.md`).
2. **Abstract, introduction, and conclusion.** Read the three, and write down:
   - **the problem**: what goes wrong without the method, in terms the lab can show;
   - **the core contributions**: each one the paper claims (its list of contributions when it has one), in plain
     words, with section and page, and the method section that delivers it;
   - **the claimed results**: what it says it achieves, against what, on what setup (cited; these are the paper's
     claims, not facts the lab measured);
   - **the limitations and future work** the conclusion names.
3. **The related work.** List every work the related-work section (and the background the method section recalls)
   discusses: `| Work (APA key) | What it does | How this paper relates to it (builds on, improves on, competes with, is
   an alternative to) | Depth in the story (foundation: taught in depth; context: a sentence) | Where this paper discusses
   it |`. A **foundation** is a work the method is made of or derives from (for ReSTIR: resampled importance sampling,
   weighted reservoir sampling); find it (the user's library, `~/Documents/papers` and `~/Documents/books`, or open
   access) and read the parts the story teaches, citing them there. Write each work's APA reference (7th edition) as
   you list it, from the work itself or its publisher's record, for the index's `references`.
4. **The method, in detail.** One brief per section (or per step) of the method, each returning:
   - every equation, its terms, what each term is, and the derivation that leads to it;
   - every algorithm, line by line: its inputs, its state, its outputs, what runs per pixel, per frame, or once;
   - every parameter with the value the paper uses and where it says so;
   - the assumptions (what must hold for an estimator to be unbiased, a weight to be valid) and what breaks without
     them;
   - what the section leaves unspecified, which the lab will have to choose;
   - the paper's figures of the step (what each shows), as references for the demonstrations.
5. **The whole paper.** Read the rest: the results and their setups, the discussion, the limitations, the appendices,
   the supplementary material. Write down what is unclear, contradicts another section, or depends on a choice the
   paper does not state, and answer it from the paper (or list it as open) in one or two rounds.
6. **The source code.** If the user has not given the authors' implementation, ask for it now (where it is, and its
   version), offering "there is none" as an answer; record their answer in the guide. With code, see "The code against
   the paper" below.
7. **Merge** into `story/research.md`: the edition, the contributions, the related-work table, one section per method
   step with its claims cited, the open questions, and a column (or note) for **where the lab differs or chooses**:
   what the paper leaves open and what the lab will do, a smaller scene, a CPU stand-in.
8. **Gate**: the contributions, the related works and which are taught in depth, the method's steps, the divergences,
   and what is left out (`new-story`, "Gates").

## The code against the paper

With the authors' code (required when it is given):

1. Record its repository, version, and commit, and read its license (SKILL.md, "Where it lives"). These go into the
   index's `engine` block (name, version, commit), so the build checks the checkout.
2. Find the code's entry points for the method (the passes, kernels, or functions), as for an engine
   (`new-story-engine`'s `references/research.md`, "Overview"), following what runs.
3. For **every step of the paper's procedure** (each algorithm line, each equation the method evaluates, each stated
   parameter), one by one, find the code that does it and compare them closely: the formula term by term, the order of
   operations, the constants, the clamps and thresholds, what is reused and when it is discarded. Delegate groups of
   steps to briefs, each with the paper's steps (cited) and the checkout, returning one row per step.
4. Write `story/divergences.md`: `| Step | The paper (section, page, equation or line) | The code (file, symbol, line)
   | Verdict (matches, diverges, missing in the code, extra in the code) | What the difference changes | Evidence |`.
   Verify every row that is not "matches" yourself, reading the paper's passage and the code's function around the
   line.
5. The lab's port and the narration follow the paper. A divergence that changes what the method computes becomes a
   beat of the Divergence chapter (SKILL.md), with an index entry for each side; where it is cheap, the lab offers the
   code's variant as a choice beside the paper's, so the chapter can show what it changes.

Done when every method step in `story/research.md` has its claims cited, every related work its row and APA
reference, every open question an answer or a listing as open, and, with code, every step of the procedure a row in
`story/divergences.md`.
