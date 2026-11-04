# Sources, pseudo-code, and formulas

Load this when writing or changing the source index, a pseudo-code listing, or a readout formula. What a citation must
establish is SKILL.md's "Precision"; this file is their formats and checks.

## The source index (`source-index.json`)

- `engine` (name, version, commit, license) and `entries` (`path`, `line`, `symbol`, `lineHash`, `inspect`,
  `inspectZh`, `details`). `lineHash` is the first 16 hex digits of the SHA-256 of the cited line, trimmed.
- Every entry has `inspect` and `inspectZh` saying precisely what the line does, plus `details` for the other lines a
  claim depends on (each with its own hash, note, and noteZh).
- `chapters`: a reading table per chapter; `mappings`: each record or readout the lab shows, the engine structure it
  stands for, whether the relationship is exact or conceptual, the engine's exact layout (`exact`), and the lab's
  (`labLayout`); `passes`: the engine pass each render-graph node mirrors (the `render-graph` skill).
- Write the index with a script that reads the checkout (O3DE: `source_index.py`; it reads the engine's variable, e.g.
  `O3DE_SOURCE_DIR`, or asks where the source is), so lines and hashes never drift by hand (required). Even for a
  published engine, keep quotes short.

Updating one entry (a moved line, a corrected `inspect`) is a local change, but not a hand edit: change the script's
entry, rerun it against the checkout at the recorded commit, read the cited line and the function around it, and
check that every narration sentence and callout that uses the key still says what the line does. Tell the two apart by comparing the index's `engine.commit` with the checkout's (`git -C <checkout> rev-parse
HEAD`): the same commit with a different line means the index was wrong; a different commit is a chapter-level change: every entry is re-read, and the narration's version claims with it.

## Pseudo-code (`pseudocode/*.py`)

- A header `# title: English | 中文`, `# source: <index key>`, `# demo: <demo>`, and optionally `# dispatch: English |
  中文`, how its first function runs ("for each probe, in parallel", "on the CPU, each frame"; check it in the engine's
  dispatch code).
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

- A formula tag in the text tables, `{@ formula="LaTeX" display=yes colors="#rrggbb,…"}`, with `\lit{n}{…}` marking term
  n and `colors` giving each term its readout row's colour (the same LaTeX in every language; `build.rs` checks it).
- Name the tag's text in `Readout::formula` and the lit term in `Readout::formula_lit`; the lab typesets it
  (`lab/src/formula.rs`; glyphs at least 24 px, at most 546 px wide, which `render-lab formulas --check` and the
  selftest enforce). A `formula` parameter can pick the lit term.
- Narration can show formulas too, with a formula cue (`formulaSaid`; the `render-lab-lamlet` skill).
