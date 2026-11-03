# Migrating the storyboards to Lamlet presentations (plan)

Today a storyboard (`.story`) is prose with `{@ …}` tags, read by `tools/video/story.py` and `markup.py`. This
plan replaces it with a Lamlet *presentation*, the document format of the Lamlet extension `lamlet-present`
(`lamlet present …`). Headings are chapters and beats, paragraphs are sentences, and cues are typed values. A
misspelled key or a wrong value type is then an error at the storyboard's line, reported before any speech is
synthesized. Today it only shows up when the lab loads the finished timeline.

`lamlet-present` names no host. The lab-specific part, what the cues mean here, stays in this repository
(`tools/video`). The lab gains no Cargo dependency: `tools/video` runs the installed commands, as `lamlet.py`
runs `lamlet` today.

It is independent of [lamlet-in-process.md](lamlet-in-process.md) and
[lamlet-live-logic.md](lamlet-live-logic.md): nothing here runs in the viewer.

Status: plan (2026-11-02). The facts below come from a survey of the tracked tools and storyboards on that
date. Nothing in the lab is changed.

## The approach: replace the reader, keep the timing

The pipeline today:

```text
.story ─story.py─▶ board ─narration.py (Piper, marks)─▶ durations ─timeline.py─▶ timeline.json, captions, audio
          markup.parse(sentence text) is called again in narration.py, timeline.py, zh.py, outputs.py, build.py
```

Every downstream step works from one parsed sentence: `markup.parse(text) → Sentence` (`caption`, `spoken`,
`pieces`, `tags`, `formulas`).
- `narration.py` turns tag pieces into word marks.
- `timeline.py` turns tags into keys.
- Both cut captions from that parsed sentence.

So the migration replaces only the *reader*. A new loader builds the same board, and each sentence carries the
parsed `Sentence` it would have had. The timing rules stay exactly as they are: `LEAD`, `GAP`, `TAIL`, padding
to a chapter's target, beat keys at the beat's start, `"sentence"`/`"beat"` motion lengths, and range blending.

```text
storyboard.lam ─lamlet present deck─▶ deck.json ─presentation.py─▶ the same board (sentences already parsed)
                                                  ─▶ narration.py, timeline.py, zh.py, outputs.py (unchanged)
```

This makes the check strong. For each storyboard, the timeline from the presentation must be **byte-identical**
to the timeline from the `.story` file. And because the spoken text is the same, the narration cache is hit and
nothing is synthesized again.

`lamlet present schedule` (its own timing) is not used. The lab's timing rules are specific to the lab, and
they stay here.

## What the corpus needs

Counts are from the tracked storyboards (English):

| | sdfgi | voxelgi | o3de | total |
|---|---|---|---|---|
| chapters / beats / sentences | 11 / 64 / 136 | 9 / 80 / 219 | 10 / 96 / 414 | 30 / 240 / 769 |
| tags (`{@…}` and `{<…}`) | 209 | 787 | 889 | 1,885 |
| point cues (of them in beat-key paragraphs) | 167 (56) | 257 (119) | 737 (177) | 1,161 (352) |
| ranges `{@n> k=a..b}…{<n}` | 21 | 265 | 76 | 362 |
| `animate=[…]` motions | 8 | 59 | 92 | 159 |
| formulas / code spans | 0 / 0 | 0 / 0 | 1 / 2 | 1 / 2 |

There are 197 distinct attribute keys:
- 167 shot parameters, such as `cameraDrift` (187 uses), `voxelgiMix` and `octreeLevel`
- 24 tag fields, such as `callout` (555), `image` (255), `panel` (207) and `demo` (191)
- 5 `camera.*`, and one `param.formula`
- dotted keys: `image.<p>`, `run.<arg>` and `freeCamera.*`

**Not used anywhere:** named cues, `{@define}`, `pause=`, `=b` values, `record=`, escapes, `$` regions,
mid-word tags, ranges crossing sentences, and tag-only paragraphs after a beat's first sentence. The converter
handles only what is used.

**What is used and needs care:**
- nested ranges, up to 3 deep (voxelgi, 51 sentences)
- point cues inside ranges (82)
- several tags before the first word
- beat-key paragraphs over two lines with `animate` JSON
- empty values (`callout=""`, 300 uses)
- quoted values with spaces
- `param.formula` (a parameter, not a formula tag)

**Translations** (zh-CN): same chapter numbers, beat ids and paragraph counts as English. They have no cues;
only o3de has 2 code spans and 1 formula, without `say`.

## The presentation format for this lab

| `.story` | Presentation |
|---|---|
| header lines (`title:`, `subtitle:`, `engineVersion:`, `commit:`, `date:`, `fps:`, `width:`, `height:`) | attributes of the level-1 heading: `# How VoxelGI Lights a Room {#video subtitle="…" fps=30 width=1920 …}` |
| `# 1 \| Where it runs \| 185 s` with `summary:` and `sources:` | `## Where it runs {#c1 number=1 target=185 summary="…" sources="a, b"}`. No summary has a quote or brace; the longest is 284 characters. |
| `## c1-room \| 3d \| Headline` with `graph:`, `focus:`, `sources:`, `diagram:` | `### Headline {#c1-room kind=3d graph=frame focus="UPD, VOX" sources="…"}` |
| a tag-only paragraph before the first sentence (beat keys) | a paragraph of cues only, before the first sentence (the same rule) |
| a sentence | a paragraph |
| `{@ k=v …}` | `${cue "k" {' {kAttr v} …}}` with typed attributes from the lab's vocabulary |
| `{@n> k=a..b}…{<n}` | `${during "n" {' {kRange a b}} {text "…"}}`, nested as the ranges are |
| `animate=[…]` | `{animate "[…]"}`: the JSON as a String for now (typed motions are an open question) |
| `{@ formula="…" say="…"}` | `${cue "formula" {' {formula "…"}}}${say {math "…"} "…"}`: the cue gives the key, `say` gives what is shown and spoken |
| `{@ codespan="…"}` | `${say {code "…"} "…"}`, or `${code "…"}` when said as written |

The level-1 heading is the video, level 2 a chapter, level 3 a beat. The zh-CN presentation has the same
headings (same ids, its own titles and `summary`) and the same paragraphs, with no cues. Its formulas and code
spans are `${math "…"}` and `${code "…"}`.

**Files.** `packages/<pkg>/video/storyboard.lam` and `storyboard.zh-CN.lam` replace the two `.story` files of
each package.

### The vocabulary

The keys a cue may set come from the lab. A new lab command, `render-lab --investigation X story keys`, prints
them as JSON for `lamlet present vocab`.
- **Shot parameters,** from `values::list()` (as `values --json` has them):
  - `f32` is a float
  - `i32` and `u32` are ints
  - `bool` is a bool
  - `vecN<f32>` is a vector
  - a `u32` with `choices` is text with those options
- **Camera channels:** `camera.*` and `freeCamera.*`.
- **Key fields,** with their options where the lab knows them:
  - `demo`: the demo keys
  - `panel`: the 10 `PanelMode`s
  - `code`: the listing names
  - `image`: the published images, resources and kinds
  - `run`, `stop`, `imageClick`: the actions and click handlers
  - `ease`, `imageCorner`, `imageShow`: their enums
  - free text: `callout`, `imageTitle`, `labelText`, …
- **Names a tag reserves** get `"key"`: `{"name": "paramFormula", "key": "param.formula", …}`, likewise
  `image.<p>` and `run.<arg>`.

`build.py` writes the vocabulary, a package named after the investigation (`godot_voxelgi`), to
`build/godot_voxelgi.lam` before reading the storyboard, so it always matches the
investigation's current parameters. That means the lab binary is built first.

`{attr "key" "value"}` remains as an escape hatch for a key the vocabulary does not have yet.

Attribute values stay text, and `markup.cue_key` and `param_value` read them as today. So `{voxelgiMix 0.15}`
(written `0.15`) and `0.15` in a tag give the same key.

## Changes, by repository

### Lamlet (general, small)

1. **The speech backend keeps a sentence's inlines.** `Utterance` gains its source inlines, so a host can draw
   what is shown with structure (formulas, code), not only as plain text.

### lamlet-present (general)

2. **`lamlet present deck FILE`:** the deck as JSON, with no timing.
   - sections: `id`, `level`, `title`, `attrs`, `path`
   - sentences: `section`, `spoken`, `pieces`, `anchors` (`kind`, `name`, `attrs`, `piece`), `inlines` (lamlet-doc's
     IR JSON) and `at`

   This is what a host needs to apply timing rules of its own.
3. A Lamlet release (a tag with `lamlet-doc`), with lamlet-present pinned to it.

### render-lab

4. **`render-lab story keys`** (lab/src/cli.rs, next to `values`, `actions` and `images`): the vocabulary JSON.
5. **`tools/video/presentation.py`, the loader.**
   - It runs `lamlet present deck` with the vocabulary package, and builds the board `story.load` returns:
     - header properties from the level-1 heading
     - chapters from level 2
     - beats from level 3, with `keys` from the beat's first cue-only paragraph
     - narration from its sentences
   - Each sentence gets `parsed`, a `markup.Sentence` built from the deck:
     - `tags` from the anchors: open, close, point, and formula for a `formula` cue. Their attribute pairs are
       the same strings `markup.parse` would give.
     - `pieces` from the deck's pieces.
     - `spoken` as `markup.tidy(' '.join(pieces))`, exactly as `markup.parse` computes it.
     - `caption` and `formulas` from the inlines, with formulas and code spans as the lab's `{@ formula="…"}` and
       `{@ codespan="…"}` rich text (which the lab still draws).
   - Each sentence also gets a stable `text` key for `Durations`.
6. **One accessor, `markup.sentence(s)`.** It returns `s['parsed']` if present, else `markup.parse(s['text'])`. It
   replaces the direct `markup.parse` calls in narration.py (`spoken`, `marks`), timeline.py (`tag_keys`,
   `caption_cues`, `plan_chapter`'s `has_tags`), zh.py (the formula check), outputs.py (`script_text`,
   `beat_listings`) and build.py (`text_keys`). Then both formats run through the same code during the migration.
7. **`build.py`** reads `storyboard.lam` when it exists, else `storyboard.story`. The formula check
   (`'{@ formula=' in timeline text`) still works, because keys keep that rich text.
8. **`tools/video/convert_story.py`** (temporary): reads a `.story` with today's `story.py` and `markup.py`, and
   writes the presentation, using the vocabulary's function names. Deleted after the migration.

## Milestones

Each milestone ends with a check that can fail.

1. **Lamlet and lamlet-present:** changes 1 to 3. Check: their tests, and `lamlet present deck` on a fixture that
   has every construct in the table above.
2. **The vocabulary:** change 4. Check: for each investigation, the generated package checks with `std`,
   `content`, `present` and render_lab; and every key used in its `.story` is in the vocabulary (197 keys over the
   three).
3. **The loader beside the old reader:** changes 5 to 7. Check: a hand-written presentation of one beat gives a
   board equal to `story.load` of the same beat (sentence `parsed` compared field by field).
4. **Convert `godot_sdfgi`,** the smallest (209 tags, no formulas). Checks, in order:
   1. **Boards:** the same chapters, beats, properties and keys. Every sentence's `spoken`, `caption` and
      `tags` (kind, name, attributes, and the text after each tag) are equal.
   2. **Narration:** `build.py --prepare-only` synthesizes nothing new (every sentence hits the audio cache), and
      the marks are equal.
   3. **Timeline:** `timeline.json` is byte-identical to the one from the `.story`, and so are the captions
      (`srt`, `vtt`) and `chapter-plans.json`.
   4. **zh-CN:** the same three checks for the translation.
5. **Convert `godot_voxelgi`** (nested ranges, 787 tags) and **`o3de_lighting`** (the formula, code spans, images,
   labels, 889 tags), with the same checks.
6. **Retire the tag format.**
   - Delete the `.story` files, `story.py`, `convert_story.py`, the parsing half of `markup.py` (`parse`,
     `parse_tag`; `cue_key` and `param_value` stay, as they read attributes), the region expansion in
     `lamlet.py`, and the tag-emitting functions of render_lab's `render_lab.lam` (`render_lab` after [lamlet-naming.md](lamlet-naming.md)). Its spoken-number functions stay,
     since they return text.
   - Update `tools/video/README.md`, `test_story.py` and `test_markup.py`, and add a test for the loader.
   - The lab's `rich_text.rs` stays: captions, text tables, the formula card and WGSL declarations still use
     tags at runtime.

## Risks

- **Whitespace and punctuation.** If a sentence's `spoken` differs by even a space, it is synthesized again and
  its timing moves.
  - Check 4.2 catches it, and so does `spoken` being computed with `markup.tidy`.
  - lamlet-doc collapses whitespace the same way and removes a space before `,.;:!?)`, as `tidy` does.
- **Mark positions.** A tag's mark counts the phonemizer words *after* it. The deck's pieces must cut the text
  where the tags stood. A range open before the first word, and several tags in a row, give empty pieces, which
  must stay empty. Check 4.2 compares marks.
- **Beat keys.** They fire at the beat's start, before `LEAD`. The loader must take them from the cue-only
  paragraph, not from the first sentence's start.
- **The vocabulary drifts.** It is generated from the investigation on every build, so it cannot be stale. A
  missing key is a check error naming the key, and the escape hatch keeps a build going.
- **Two readers for a while.** Only until milestone 6. `markup.sentence` keeps the rest of the code shared.

## Open questions

- **Typed motions.** `animate` stays a JSON String. Typed constructors (`path`, `orbit`, `wave`, checked like
  keys) would catch motion mistakes too. That's worth a follow-up once the migration is done.
- **Chapter summaries as attributes.** They fit today. If summaries grow (with quotes or several sentences), a
  `group "summary"` block under the chapter heading would be the alternative.
- **The other Lamlet plans.**
  - [lamlet-render-lab.md](lamlet-render-lab.md)'s text and timeline modules overlap with this format. Its storyboard
    parts are superseded by this plan.
  - [lamlet-integration.md](lamlet-integration.md)'s validation gates still apply.
