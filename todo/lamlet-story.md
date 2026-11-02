# The storyboard as a Lamlet source file (plan)

Today a storyboard (`.story`) is prose with `{@ …}` tags, and Lamlet regions are optional extras that
`tools/video/lamlet.py` expands into text before `story.py` reads the file. This plan makes the storyboard
itself a Lamlet document: the prose stays prose, and the tags become typed regions that return *cue values*.
A misspelled key or a wrong value type is then reported at the storyboard's line before any speech is
synthesized, instead of when the lab loads the finished timeline.

It builds on [lamlet-integration.md](lamlet-integration.md) (how a storyboard uses Lamlet) and is independent of
[lamlet-in-process.md](lamlet-in-process.md) and [lamlet-live-logic.md](lamlet-live-logic.md): nothing here runs
in the lab, and the lab gains no Cargo dependency.

Status: plan (2026-11-02). A prototype rewrote the first beat of `godot_voxelgi`'s storyboard with typed
regions; `story.py` read the original and the generated storyboard as identical boards (beat keys, ranges, and
sentences). The reader is built in `lamlet-present` (milestone 1). Nothing in the lab is changed.

## What was checked

- **Every tracked storyboard is already a valid Lamlet document.** None of the six contains a `$`, and all six
  pass `lamlet check` unchanged: Lamlet reads headings, properties, and `{@ …}` tags as prose. So the switch
  can be made one beat at a time; old tags and typed cues mix in one file.
- **Typed cues give the same board.** In the prototype,

  ```text
  {@m0> voxelgiMix=0..0.15}This is the lab's study room, … flat light{<m0}.
  ```

  became

  ```text
  ${open "m0" {' {voxelgiMixRange 0.0 0.15}}}This is the lab's study room, … flat light${close "m0"}.
  ```

  and the reader wrote the first form back from the second. `story.py` gave equal boards.
- **Errors point at the storyboard:**

  ```text
  typo.story:21:17: name error: unknown name `voxelgiMxRange`
  type.story:16:102: type error: argument 1: type mismatch: `Float` versus `String`
  ```

## How it works

```text
typed .story ──lamlet-present story──▶ canonical .story ──story.py──▶ board ──timeline.py──▶ timeline
               (check, evaluate,        (the tags story.py
                write each cue as        reads today)
                its tag)
```

- **Prose stays prose.** Headings, `key: value` properties, `summary:`, paragraphs, and narration are unchanged.
- **A tag becomes a region whose value is a cue.** The `cues` package (in `lamlet-present`) defines the types:
  - `Val`: `text`, `num`, `int`, `flag`, `vec`, a `range` `a..b`, a `toward` `=b`, or `json` (motions)
  - `Attr`: a key and a `Val`, made by `setText`, `setNum`, `setInt`, `setFlag`, `setVec`, `setRange`,
    `setToward`, or `setJson`
  - `Cue`: `point` (`{@ k=v …}`), `named` (`{@intro}`), `open`/`close` (a range), or `around` (a range around
    a String of narration)
  - The names differ from rlstd's `story.lam` (`at`, `num`, `span`, `cue`, …) so a storyboard can import both.
- **The reader writes each cue back as its tag**, with `markup.py`'s quoting. A region giving a String or a
  number is written as text, as `lamlet.py` does today, so rlstd's `story.lam` keeps working unchanged.
- **Everything after the reader is unchanged:** speech, word timing, captions, the board, and the timeline.
- **Translations need nothing.** A zh-CN file carries only formulas and code spans, and is already a valid
  Lamlet document.

### Typed keys: the values package

The keys a storyboard may set are the investigation's shot parameters. The lab writes them out, and
`lamlet-present values` turns them into a package of typed setters, one per parameter:

| Kind (example name) | Setters |
|---|---|
| float (`voxelgiMix`) | `{voxelgiMix 0.4}`, `{voxelgiMixRange 0.0 1.0}`, `{voxelgiMixToward 1.0}` |
| int (`steps`) | `{steps 3}` |
| bool (`showProbes`) | `{showProbes true}` |
| vector (`tint`) | `{tint {' 1.0 0.8 0.6}}` |
| text with options (`mode`: `albedo`, `light-only`) | `{mode "albedo"}`, and `modeAlbedo`, `modeLightOnly` |
| float (`camera.yaw`) | `{cameraYaw 24.0}` (dots and dashes become camel case) |

The list is JSON, `[{"name": "camera.yaw", "kind": "float"}, …]`. A shot parameter whose name the tag format
reserves (`blend`, `image`, …) gets the key `param.<name>` (setter `paramBlend`), as `markup.py`'s `RESERVED`
requires. The tag format's own fields (`demo`, `callout`, `panel`, `image`, …) are listed with
`"field": true` and keep their names, for example `{demo "room"}` and `panelHidden`. The package is generated, never edited, and it is regenerated when the
parameters change; a stale package shows up as an unknown name when a storyboard is checked.

### What Lamlet adds to a storyboard

- typed keys and values, checked before synthesis
- named, reusable cues: `$[# warm [% {point {' {voxelgiMix 1.0} {cameraYaw 24.0}}}]]` once, then `$warm`
- loops and conditions for repetitive sequences, and computed numbers (spoken as words through rlstd)
- comments (`$;{…}`) that are neither shown nor spoken
- cue modules shared between storyboards

## Costs and limits

- **Verbosity.** `${open "m0" {' {voxelgiMixRange 0.0 0.15}}}` is about twice as long as the tag it replaces.
  Named cues help, and so does `around` for a short range. A region cannot wrap prose, so a long range stays an
  `open`/`close` pair.
- **Conversion.** The three English storyboards have about 1,900 tags (813, 188, and 522). They need a
  mechanical converter, checked by comparing boards per storyboard, as the prototype did.
- **Literal dollars.** A `$` in narration must be written `$$`. None appears today.
- **Errors raised inside std.** When a cue helper fails through std's `error` in tail position, Lamlet reports
  the error at `std.lam` instead of at the storyboard. That is a Lamlet fix (keeping the frames tail calls
  replace), planned there.

## Milestones

1. **The reader** (`lamlet-present`, outside this repository). `lamlet-present story FILE --package
   name=path …` checks a typed storyboard and prints the canonical storyboard; diagnostics go to stderr with
   the storyboard's line and column. `lamlet-present values NAME PARAMS.json` writes the values package and
   checks it against `std`, `cues`, and rlstd for name collisions. Status: done, with tests; the first beat of
   `godot_voxelgi`'s storyboard expands to byte-identical text and an equal board.
2. **The lab writes its parameters.** A command (for example `render-lab --investigation X story params`)
   prints the investigation's shot parameters, their kinds, and the tag format's fields as JSON, for
   `lamlet-present values`.
3. **`lamlet.py` uses the reader.** When a storyboard imports `cues`, `lamlet.py` runs `lamlet-present story`
   (named by a `LAMLET_PRESENT` variable, like `LAMLET`) with rlstd's `story.lam` and the generated values
   package, instead of `lamlet prepare`/`eval`. Storyboards without regions are read as today.
4. **Convert one storyboard.** A converter rewrites `godot_voxelgi`'s storyboard. Done when `story.py` gives
   an identical board from the original and the converted file, and the video's timeline is byte-identical.
5. **Convert the rest**, one storyboard at a time, with the same check.

## Later: the board from Lamlet's document IR

Milestones 1 to 5 write typed cues back as tag text, which `story.py` then parses. Lamlet plans a document IR
(Lamlet's `plans/formats.md`): a format-neutral tree with speech nodes (`Say` for shown versus spoken text,
`Anchor` between words, `Pause`, `Span` for ranges) and a speech writer that gives one utterance per paragraph,
with the spoken text cut into pieces at each anchor. With it, the board can come from values rather than from
text.

How the parts divide:

| Where | What |
|---|---|
| Lamlet (`lamlet-doc`) | The IR, the speech nodes, the speech writer. Anchor and span attributes are opaque there |
| `lamlet-present` | A *board writer*: IR to the board `story.py` builds (chapters, beats, `keys`, `cues`, `narration`), interpreting the attributes by this lab's tag rules |
| The lab | Piper, word times, captions, and the timeline: `narration.py` and `timeline.py`, unchanged at first |

What it takes, as of today's code:

- **Sentences still hold tags.** `story.py`'s board keeps each sentence as text with its tags, and
  `narration.py` (`spoken`, `synthesize`) and `timeline.py` (`tag_keys`, `caption_cues`) call `markup.parse` on
  it. So the board writer works in two steps:
  1. **The board with tag text.** It writes the board as JSON, with each sentence's text in canonical tags.
     This skips `story.py`'s section and heading parsing. Done when the JSON equals `story.load` of the same
     storyboard.
  2. **Parsed sentences.** `narration.py` and `timeline.py` accept a sentence already parsed (`caption`,
     `spoken`, `pieces`, `tags`: the fields of `markup.Sentence`), and the board writer fills them from the
     utterances. Tags are then never written or parsed. This changes the lab's tools, so it is a change here,
     reviewed as one.
- **Pieces, not words.** The speech writer gives anchors as piece indices, as `markup.Sentence.pieces` does.
  `narration.py` keeps mapping pieces to words with its phonemizer (`marks`).
- **Headings.** Until Lamlet parses prose (its phase 7), chapter and beat headings stay prose. The board writer
  reads them from the IR's `Raw` text, as `story.py` reads them now.
- **Translations.** A zh-CN storyboard lowers to its own IR. Lamlet's `--align` check (matching sections and
  paragraph counts, and no anchors) replaces `load_translation`'s matching.

The check is the same as for milestones 4 and 5: an equal board and a byte-identical timeline for every
converted storyboard.

## Open questions

- **Where the cue helpers live.** The `Cue` type and the reader are in `lamlet-present`; rlstd's `story.lam`
  could move its cue functions (`cue`, `sweep`, `callout`, `panel`) onto typed cues, or stay a String library.
- **Motions.** A camera path is a `setJson "animate" "…"` today, JSON in a String. Typed motion constructors
  (paths, orbits) would check those too.
- **Pinning.** `lamlet-present` depends on Lamlet's `v0.2.0` tag; the reader is installed with
  `cargo install`, so the lab's `--locked --offline` build is not involved.
