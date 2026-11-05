# Globalization backend (2026-11-05: stages 1–5, 7, 8 and part of 6 done)

The lab's on-screen text moves to Project Fluent: each package's text tables become `text/<locale>.ftl`, read through
`fluent-bundle`, and the lab's rich text (formulas and code names inside a string) becomes Fluent function
placeables, which the video builder writes into captions too. Languages become data, and every other piece of
translated data moves into the tables. Neither English nor Chinese output changes.

## Where it stood (at cb757d1)

- **Two languages, fixed in code.** `Locale` (`lab/src/i18n.rs:19`) has two variants; `Table` holds `[HashMap; 2]`;
  `lab/check/src/lib.rs:13` lists `LOCALES = ["en", "zh-CN"]`. `i18n::zh()` switches `sources.rs:94`,
  `pseudocode.rs:156,164`, and two caches keyed by `(String, bool)` (`knowledge.rs:692`, `graph/scene.rs:83`).
- **Keys** are `<namespace>.<name>` (1,945 keys with dots; 259 also have `-`). Rust names them in 767 `tr`, 78
  `tr_with`, 19 `package_text` calls and about 28 `format!`s; scripts in `textKey "…"`; content in `demos/*.toml`,
  `knowledge.toml`, scenes, and storyboards. Fluent ids forbid dots.
- **Placeholders** `{name}`, filled by `i18n::fill`, which passes `{@…}` tags through.
- **Legacy `{@…}` rich-text tags** (`lab/src/rich_text.rs`): `formula` (with `display`, `colors`), `codespan`,
  `symbol`; others dropped. In tables only 15 `formula.*` messages (o3de_lighting, each a whole formula, equal in
  both languages) and 4 `notice.credit` (`{@ symbol=copyright}`). The `.lam` files hold none; `tools/video` writes
  `formula` and `codespan` tags into timeline captions and the `formula` key (`markup.py`, `presentation.py`,
  `timeline.py`, `build.py:188`; `config.py:217` writes symbols for the notices). WGSL `/// {@ …}` doc tags
  (`shader_data::parse_tags`) share only the tag scanner, not the rich text.
- **Other translated data:** `inspect`/`inspectZh`, `note`/`noteZh` in source indexes; `# title: English | 中文` in
  listings; `edition = "English" | "中文"` in manifests; `object.*` keys prefix-matched by `object_name`.
- **Fluent's licenses** (crates.io, 2026-11-05): fluent-bundle, fluent-syntax, fluent-langneg, intl-memoizer,
  intl_pluralrules, type-map: MIT or Apache-2.0 (taken as MIT). `self_cell` (already vendored) is Apache-2.0 OR
  GPL-2.0-only, taken as Apache-2.0. All go in `THIRD_PARTY.md`.

## Decisions

1. **Keys renamed everywhere**: `.` → `-` (`viewer.chapter` → `viewer-chapter`), in tables, Rust, scripts, content,
   storyboards, Python, and docs. Checked: no two keys of a package collide.
2. **Tables are Fluent**: `text/en.ftl`, `text/zh-CN.ftl`, placeholders `{ $name }`, read by `fluent-bundle` with
   isolation marks off. `tr` / `tr_with` / `args!` stay the call API.
3. **Rich text is text with Fluent function placeables**: `{ FORMULA("LaTeX", display: "yes", colors: "#rrggbb,…") }`
   and `{ CODE("Name") }`. A table message may hold them (the `formula-*` messages); the bundle's `FORMULA` and `CODE`
   functions write themselves back out, so a resolved message is rich text as written. Captions use the same form:
   the video builder's backend (`markup.py`) writes it instead of `{@…}`. `rich_text` parses these placeables with
   `fluent-syntax`; any other text is literal.
4. **No symbol tags**: `©` is written as the character. `rich_text::symbols`, `SYMBOLS`, and `config.py`'s copy go.
5. **Locale registry as data**: `packages/render_lab/locales.toml` lists each language's `code`, `name`, `fallback`,
   and `status` (`complete` | `draft`). `Locale` is an index into it; `lab/check` and `tools/video` read it.
6. **Fallback**: a key missing from a language falls back along its chain; `lab/check` fails on a key missing from a
   `complete` language and on variables that differ between languages.
7. **One home for translated data**: source-index notes, listing titles, and edition names become table keys;
   `zh()` and its caches become `Locale`.
8. **Report**: `render-lab i18n` lists, per language and package, missing keys, fallbacks, and characters the bundled
   fonts lack (moving the glyph check out of one selftest).

Left out (nothing uses it): our own plural and number syntax, a CLDR rule parser, locale number separators, plural
rewording of existing strings. Fluent's selectors are there when a message first needs one.

## Status (2026-11-05)

Done: the keys renamed (`f895fe9`); the tables on Fluent, rich text on `{ FORMULA(…) }` / `{ CODE(…) }` placeables in
tables and captions, `©` written out, `{@…}` left only in the WGSL doc tags (`shader_data::parse_tag`); the locale
registry (`packages/render_lab/locales.toml`, `Locale` an index, fallback chains, draft locales); listing titles and
dispatch lines as text keys (`# title-text:`, `# dispatch-text:`); edition names from the timeline's locale (the
manifests' `edition` field removed); `render-lab i18n [--check]`; `render-lab text PACKAGE --locale L`, which the video
tools read tables through.

Checked: both builds clean, the local package's workspace too; unit and video-tool tests; the homepage autotest; every
selftest (O3DE fails the same two checks at cb757d1: a listing cut in `diffuse`, and the narration's measured
numbers); every edition's prepared outputs equal to cb757d1's after the key rename and the placeable syntax; 619 stills
of cb757d1 and the change equal, or different only where two runs of cb757d1 differ too (GPU and CPU timings, viewport
noise, the config page's checkout path).

Left: the source indexes' own translations (`inspectZh`, `noteZh`, `engineNameZh`, `exactZh`, `labLayoutZh`, written
by each investigation's `source_index.py` from the engine's checkout). Moving them into the tables changes those
generators and needs each engine's source to regenerate the indexes; `i18n::zh()` remains for them alone.

## Stages (each a local commit; never pushed)

Each stage: both builds clean (the public tree with `-j 4`), the local packages' workspaces rebuilt, `cargo test`,
the homepage autotest, every selftest, `tools/video` unit tests and `compare.py` over every edition when it touches
what they read, and the frame identity check against cb757d1 (`../render-lab-baseline`; decoded-pixel hashes of beat
midpoints of every timeline and the homepage pages; a mismatch re-rendered alone with both binaries).

1. **Plan** (this file).
2. **Keys renamed** (`.` → `-`), still JSON. Output identical.
3. **Fluent tables**: vendor the crates; `.json` → `.ftl`; `i18n` on `fluent-bundle`; `lab/check` on `fluent-syntax`;
   Python's readers of tables. `©` written as the character. Output identical.
4. **Rich text on Fluent placeables**: `rich_text`, `formula --check`, `tools/video`'s writers and readers, the
   `formula-*` messages; `{@…}` gone from rich text (the WGSL tag scanner moves to `shader_data`). Output identical.
5. **Registry and `Locale` as data**; callers off `zh()`. Output identical.
6. **Translated data into the tables**, one kind per commit. Output identical.
7. **Fallback and report.**
8. **Docs**: `AGENTS.md`, `README.md`, the skills (`python3 tools/check_skills.py`), `tools/video/README.md`.
