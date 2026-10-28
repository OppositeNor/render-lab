# Globalization backend (plan, not started)

The lab's on-screen text is keyed: each layer keeps its text in `text/en.json` and `text/zh-CN.json`, read with
`tr("key")` or `tr_with("key", args!(name = value))` through `crate::lab::i18n::Table`, and `build.rs` checks the
tables. This plan turns that into a globalization backend: languages are data, messages handle plurals and number
formats, and every translated piece of data shares one system.

## Where it stands

- **Two languages, fixed in code.** `Locale` (`src/lab/i18n.rs`) has two variants, `Table` holds exactly two texts,
  and `build.rs`'s `LOCALES` lists `en` and `zh-CN`. A third language means editing all three.
- **Plurals by hand.** English plurals are passed in as values (`clusters_s = "s"`), so `build.rs` allows a language
  to use only some of English's placeholders.
- **Numbers formatted in code.** Values are formatted before they reach the text (`format!("{x:.1}")`), so every
  language gets `0.91`, never `0,91`.
- **Other translated data, each its own way:**
  - source-index notes: `inspect` / `inspectZh`, `note` / `noteZh` (`src/lab/sources.rs`)
  - pseudo-code titles: the `# title: English | 中文` header (`src/lab/pseudocode.rs`)
  - timeline editions: `TimelineInfo::edition` ("English", "中文") and the per-edition titles
  - scene object names: `object.*` keys, prefix-matched by `object_name`
  - the video's narration translation: an investigation's `video/zh-CN/translation.json`
    (`modules/o3de_lighting/video/zh-CN/translation.json`), read by `tools/video`
- **Fonts by hand.** Source Han Sans CN is bundled for Chinese (`src/lab/text.rs`). The check that every character
  has a glyph runs in only one investigation's selftest.

## Design

1. **Locale registry as data.** A `locales.json` beside `src/lab/i18n.rs` would list, for each language: its code
   (`zh-CN`), its own name ("中文"), its fallback chain (`zh-CN` → `en`), and the fonts it needs. `Locale` becomes an
   index into the registry. Tables load every listed language, and `build.rs` reads the registry instead of
   `LOCALES`. Adding a language is then data only: an entry in the registry and a file per table.
2. **Fallback while a translation is in progress.** A key missing from a language falls back along the chain. The
   build keeps failing for a missing key in a *complete* language (en, zh-CN). A language marked `draft` in the
   registry may miss keys, and the report below lists them.
3. **Messages with plurals and selections: Project Fluent.** Adopt `fluent-bundle` (vendored like every crate)
   instead of growing our own syntax.
   - Plurals: English `{ $count -> [one] cluster *[other] clusters }`, Chinese `{ $count } 个 cluster`. The
     `_s` values go, and `build.rs`'s placeholder rule becomes "the same variables in every language".
   - Numbers: `NUMBER($d, maximumFractionDigits: 2)` formats by locale, so values reach the text unformatted.
     Identifiers, formulas, and record values that must read the same everywhere stay formatted in code.
   - Fluent ids can't contain dots: `viewer.chapter` becomes `viewer-chapter`, a mechanical rename.
   - Python has a Fluent runtime (`fluent.runtime`), so `tools/video` could read the same files.
   - Keep `tr` / `tr_with` / `args!` as the call API, so call sites change only in their keys.
4. **One home per layer for translated data.** These become keys in the layer's tables, with English kept only once:
   - source-index notes (`inspectZh`, `noteZh`)
   - pseudo-code titles
   - timeline edition names
   - object names
   `source_index.py` and the pseudo-code parser write or read keys instead of `…Zh` fields. The narration's
   `translation.json` stays as it is: it holds sentence-by-sentence timing for the video, not interface text.
5. **Report.** A `render-lab i18n --report REPORT` command (or a `build.rs` output) lists, per language and table:
   - missing keys and keys that fall back
   - keys whose text equals English (often fine: technical terms)
   - characters the language's registered fonts lack
   It moves the glyph check out of one investigation's selftest so it covers every table.

## Steps

1. **Baselines.** Render every beat of every timeline in every edition, the selftests' stills, and the homepage
   pages, and hash them, as for the move to text tables. Render O3DE frames that mismatch on their own with both
   binaries before calling them a regression: O3DE's batch renders vary from run to run inside the 3D viewport.
2. **Vendor Fluent.** Add `fluent-bundle` (and `unic-langid`) to `Cargo.toml` and vendor them with `setup.sh`. Check
   both builds, and a fresh clone without `vendor/`.
3. **Registry.** Add `locales.json` and make `Locale` an index into it. Teach `Table` and `build.rs` the registry.
   Output must not change.
4. **Convert the tables to Fluent.** A script turns each layer's `text/<locale>.json` into
   `text/<locale>.ftl`, renaming keys (`.` → `-`) in the tables and the code, and placeholders `{name}` → `{ $name }`.
   Output must not change.
5. **Plurals and numbers.** Replace the `_s` plural values with Fluent selectors, and number formatting in code with
   `NUMBER()` where the text, not the record, owns the number. English and Chinese output must not change.
6. **Fold in the other translated data** (design point 4), one kind at a time, each with its baseline check.
7. **Report and fallback** (design points 2 and 5).
8. **Docs.** Update `AGENTS.md`, `README.md`, the `new-investigation` and demo skills, and the investigations' guides.

## Verification

- `cargo build --release --locked --offline` with no warnings, with and without the optional `commercial` module, and a fresh clone
  building with plain `cargo build`.
- `cargo test` for the backend: lookup, fallback, plurals, number formats, and switching locale while running.
- `build.rs` refuses: a key missing from a complete language, variables that differ between languages, an unknown
  key in code, and an unused key.
- Every selftest and the homepage autotest pass, and the report shows no missing keys or glyphs for en and zh-CN.
- The baselines from step 1 are byte-identical after every step, except where a step deliberately changes text
  (none of steps 3–5 should).

## Open questions

- Fluent, or our own format with a registry and a small plural syntax? Fluent is the standard and covers plurals,
  selections, and numbers, but adds crates. Our own format keeps the dependency list short.
- Should a draft language be offered on the homepage, or only through a command-line option?
- Should `tools/video` read the Fluent tables, or keep its own `translation.json` for everything it shows?
