# Formulas in on-screen text (superseded)

Done, with a different syntax: formulas are tags, `{@ formula="…"}`, typeset by Typst through MiTeX at run time
(`src/lab/formula.rs`, `src/lab/rich_text.rs`), and storyboards use the same tags with word-timed shot cues
(`tools/video/README.md`). What follows is the original plan, kept for its measurements.

An investigation's readout formulas are LaTeX in `src/<engine>/<feature>/formulas/formulas.json`. `tools/video/formulas.py`
renders them with KaTeX in headless Chrome (`formulas.mjs`, Node) into coloured PNGs, which are committed with
`manifest.json`; `build.rs` (`FORMULAS`, `check_formulas`) only checks that they are not stale. This plan lets a formula
be written inside on-screen text as `$...$`, rendered at build time without Node, and coloured when the lab draws it.

## Where it stands

- **Colour baked in.** Every term a demonstration can light is its own RGBA image: 55 PNGs for O3DE's 13 formulas,
  1.6 MB. `draw_readout` (`src/lab/ui.rs`) already draws them with a tint, which is white today.
- **Node after every edit.** Changing a formula means running `formulas.py` and committing the images; the lab builds
  offline only because the images are committed.
- **Formulas live apart from the text.** The readout names a formula by key (`Readout::formula`); labels, notes, and
  rows cannot contain math.
- **No `$` in any text table today**, so `$...$` is free as the delimiter (`\$` for a literal dollar sign).

## Storage: coverage masks, coloured at draw time

Measured on the 55 committed images (their alpha):

| Storage | All 55 images |
|---|---|
| Today (RGBA, colour baked in) | 1566 KiB |
| 8-bit coverage mask | 580 KiB |
| 1-bit mask (cut at half coverage) | 126 KiB |

- **8-bit masks (chosen).** They look the same as today's images. The lab loads a mask as white with the mask as alpha
  and tints it with a `palette.rs` colour, so colours stop being baked in.
- **1-bit masks.** Stair-stepped curves and broken thin strokes (root signs, fraction bars) at 24 px; visible in
  motion. Only worth it rendered at 2x or 4x and box-filtered on load (an estimated 0.3 to 0.6 MB, not measured).
- **Signed distance fields.** Not worth it: formulas are drawn at or near 1:1, and one channel rounds corners.
- **Lit terms.** One mask of the whole formula plus one per term (at most four, so they pack into one RGBA image): draw
  the formula dim, then the lit term in its readout colour. The highlight can then fade with an eased tint alpha
  (`src/lab/animation.rs`) instead of switching images.

## Rendering without Node: Typst and MiTeX

Rust options (from a survey; versions and maintenance not checked):

- **Typst** (`typst`, `typst-render`): an actively developed typesetter with OpenType MATH layout, rasterized with
  tiny-skia (already vendored). **MiTeX** (`mitex`) translates LaTeX math into Typst's syntax. The best fit, but a large
  dependency tree.
- **ReX**: a small TeX-style math layout engine, light enough to run at runtime; sporadic development, less LaTeX
  coverage, and worse output than KaTeX.
- Not useful here: `pulldown-latex` and `latex2mathml` (MathML only, nothing draws it), the `katex` crate (HTML, still
  needs a browser), Tectonic (a full TeX engine producing PDF, with C dependencies).

## Decisions before starting

1. **Vendoring** `typst`, `typst-render`, and `mitex` as build dependencies, and updating `Cargo.lock`: the only change
   to `vendor/`, and a large one (many crates, slower clean builds).
2. **Fonts:** bundle New Computer Modern Math (OFL) under `modules/render_lab/assets/fonts/`, plus a text font for `\text{}` and
   `\mathrm{}`.
3. **Fallback** if the trial fails: keep KaTeX for rendering, and do only the inline `$...$` text and the masks
   (phases 2 to 5 without phase 1).

## Phases

0. **Trial (go or no-go).** Render the 13 formulas of `src/o3de/lighting/formulas/formulas.json` with Typst and MiTeX in
   a scratch program, and compare them with the KaTeX images at the same size. Check that MiTeX handles every construct
   they use (`aligned`, `\\[3pt]`, `\dfrac`, `\big`, `\left...\right`, `\textstyle\sum`, `\lfloor`, `\widehat`,
   `\mathrel{+}=`, `\text`, `\mathrm`, spacing), that `\lit{n}{#rrggbb}{...}` can be expanded before translation, that
   glyphs stay at least 24 px and widths at most 546 px, and that the output is the same bytes from run to run. Measure
   the build time and the vendored size.
1. **Build-time rendering.** `build.rs` collects every `$...$` and `$$...$$` from the text tables (`TEXT_TABLES`),
   translates and typesets each at the readout's size, and writes one 8-bit mask per formula plus the term masks, with
   their width, height, and ascent above the baseline, into `OUT_DIR` as a generated table keyed by a hash of the
   LaTeX (cached, so unchanged formulas are not typeset again). It fails on an unmatched `$`, LaTeX that does not
   translate, a glyph under 24 px or a display formula wider than 546 px, a formula that differs between `en` and
   `zh-CN` for the same key (formulas stay English), and a `{name}` value inside a formula (values cannot be
   pre-rendered; keep them outside the `$...$`).
2. **Rich text in the lab.** A new `src/lab/rich_text.rs` splits a string into text and formula runs and lays them out
   with egui's fonts (egui cannot put images inline): formulas on the baseline, lines wrapped between runs but never
   inside a formula, each formula tinted with its run's colour, `$$...$$` on its own line. Textures load at first use,
   as `formula_texture` in `src/lab/renderer.rs` does now. `src/lab` reaches investigations' formulas only through the
   generated table or the `Investigation` trait. Readout titles, rows and notes, labels, and dock text use it; strings
   without `$` keep today's path.
3. **Readout formulas move into the text.** Each becomes a text key holding a `$$...$$` formula, which
   `Readout::formula` names; the `formula` parameter picks the lit term. Delete `formulas.json`, the PNGs,
   `manifest.json`, `tools/video/formulas.py` and `formulas.mjs`, `FORMULAS` and `check_formulas` in `build.rs`, and
   the KaTeX and Puppeteer parts of `tools/video/setup.sh`.
4. **Documentation.** `AGENTS.md`'s validation list, `tools/video/README.md`'s Formulas section, the
   `new-investigation` skill, the O3DE README, and `THIRD_PARTY.md` rows for Typst, MiTeX, and the fonts.
5. **Verification.** Both builds without warnings, the public-only tree rendering its formulas offline; O3DE's
   `selftest --timeline`; stills of every readout demonstration against the KaTeX versions; frames outside the readout
   panel unchanged, and other investigations' frames byte-identical (re-render a mismatching O3DE frame on its own
   first: batch renders of O3DE vary); repeated builds giving identical formula images.

## Risks

- Typst's dependency tree: compile time and vendored size.
- MiTeX coverage: an unsupported construct would show up in the trial.
- Typst's math looks different from KaTeX's, so the video stills change.
