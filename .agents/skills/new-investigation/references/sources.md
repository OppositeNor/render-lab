# Sources, pseudo-code, and formulas

Load this when writing or changing the source index, a pseudo-code listing, or a readout formula. What a citation must
establish is SKILL.md's "Precision"; this file is their formats and checks.

## The source index (`source-index.json`)

- `engine` (name, version, commit, license) and `entries` (`path`, `line`, `symbol`, `lineHash`, `inspect`,
  `details`). `lineHash` is the first 16 hex digits of the SHA-256 of the cited line, trimmed.
- Every entry has `inspect` saying precisely what the line does, plus `details` for the other lines a claim depends on
  (each with its own hash and note). The index is English; another language's text is the package's source table,
  `text/sources/<locale>.ftl` (the resource `sources/<locale in lower case>`), keyed `entry-<key>-inspect`,
  `entry-<key>-detail-<n>` (from 1), `chapter-<n>-title`, and `mapping-<lab>-engine-name`, `-exact`, `-lab-layout`
  (the lab name with anything but letters, digits, `_`, `-` as `-`). The build fails when a complete language's
  table lacks a key or has one the index does not.
- `chapters`: a reading table per chapter; `mappings`: each record or readout the lab shows, the engine structure it
  stands for, whether the relationship is exact or conceptual, the engine's exact layout (`exact`), and the lab's
  (`labLayout`); `passes`: the engine pass each render-graph node mirrors (the `render-graph` skill).
- Write the index with a script that reads the checkout (O3DE: `source_index.py`; it reads the engine's variable, e.g.
  `O3DE_SOURCE_DIR`, or asks where the source is), so lines and hashes never drift by hand (required). Even for a
  published engine, keep quotes short.

### A paper's entries

An investigation of a paper (`new-story-paper`) cites the paper instead of a checkout (`lab/src/sources.rs`):

- `paper` instead of `engine`: `key` (its key in `references`), `short` ("Bitterli et al. 2020", how the chrome names
  it), `title`, `venue` ("ACM TOG 39(4)"), `doi`, and `sha256`, the PDF's, which the video's verification checks
  (`paperEnv` in `video.json` names the variable that says where the PDF is; otherwise it asks). An `engine` block is
  added only when the authors' code was read too (its name, version, and commit), and its entries are the code's.
- Each paper entry has `section` ("4.4"), `page` (the printed page, "148:9"), `symbol` (the equation, algorithm line,
  or figure: "Alg. 6, line 10", "Eq. 22", "Fig. 7"), `quote` (a phrase of the passage, short, that locates it) and
  `quoteHash` (the first 16 hex digits of the SHA-256 of the quote with its whitespace collapsed), `inspect`, and
  `details` with a `page` and a `note`. The chrome shows it as `§4.4, p. 148:9` where an engine's shows `path:line`.
- Write the index with the package's script, which reads the PDF's text (`pdftotext -layout`), finds each quote on
  its page, and fills the hashes, so a cited passage never drifts by hand (required), as an engine's script reads the
  checkout.

### References

`references`: the works a story cites, each `key` and `apa`, its reference in APA style (7th edition), from the work
itself or its publisher's record; the paper's own is among them. A 2D beat with `references=true` shows them on the
references card (`lab/src/ui/panel.rs`), the paper's in the accent colour, in the order listed (alphabetical, as APA
lists them). A paper's story ends on it.

Updating one entry (a moved line, a corrected `inspect`) is a local change, but not a hand edit: change the script's
entry, rerun it against the checkout at the recorded commit, read the cited line and the function around it, and
check that every narration sentence and callout that uses the key still says what the line does. Tell the two apart by comparing the index's `engine.commit` with the checkout's (`git -C <checkout> rev-parse
HEAD`): the same commit with a different line means the index was wrong; a different commit is a chapter-level change: every entry is re-read, and the narration's version claims with it.

## Pseudo-code (`pseudocode/*.py`)

- A header `# title-text: listing-<name>-title`, `# source: <index key>`, `# demo: <demo>`, and optionally
  `# dispatch-text: listing-<name>-dispatch`, how its first function runs ("for each probe, in parallel", "on the CPU,
  each frame"; check it in the engine's dispatch code); both texts are keys of the package's text tables.
- A line ending in `# @tag` lights when a shot highlights `tag` with the code panel. A line may also end in a mark of
  what it does, `@@read` or `@@write` (memory: a texture, a buffer) or `@@math`, in the same trailer (`# @tag @@read`)
  or its own (`# @@read`).
- The code panel draws a listing as Scratch-like blocks (the default) or as text (the Config page's listing style,
  `--listing-style`): loops and branches wrap their bodies, each line coloured by its mark (control flow by its
  keyword), the dispatch on the first function's hat.
- It must compile as Python and fit the code panel at 24 px in both styles (`home --snapshot … --page
  viewer-code:<listing>`, with and without `--listing-style text`).
- Mirror the engine function's real steps and names; one listing per step.

## Formulas (optional)

- A formula in the text tables, `{ FORMULA("LaTeX", display: "yes", colors: "#rrggbb,…") }` (a Fluent string literal:
  backslashes doubled), with `\lit{n}{…}` marking term
  n and `colors` giving each term its readout row's colour (the same LaTeX in every language; `build.rs` checks it).
- Name the formula's text in `Readout::formula` and the lit term in `Readout::formula_lit`; the lab typesets it
  (`lab/src/formula.rs`; glyphs at least 24 px, at most 546 px wide, which `render-lab formulas --check` and the
  selftest enforce). A `formula` parameter can pick the lit term.
- Narration can show formulas too, with a formula cue (`formulaSaid`; the `render-lab-lamlet` skill).
