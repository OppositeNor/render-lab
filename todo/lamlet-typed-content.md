# Typed content in Lamlet (plan)

The storyboards are Lamlet presentations (the in-process plan's milestone 1; that plan is done and removed), but most of
what they say is still Strings and repetition: motions are JSON inside a String, points and marks are
comma-separated numbers, names the lab knows (source keys, text keys, listing tags) are free Strings, and a beat's
opening cue repeats the same reset in nearly every beat. This plan lists everything that could move into Lamlet's
types, functions, and bindings, in the storyboards, in the pipeline's policy, and in the lab's other content
formats, and orders it. It also says what should not move, and why.

Status (2026-11-03; rechecked 2026-11-05, unchanged since): phases 0, 1, 2, and 5 are done; 3, 4, and 6 in part
(below, under each phase). The survey's counts ("What there is to migrate") are of the three storyboards before the
migration; Godot SSR's storyboard, added since, was written in the typed forms and has its own `video/story.lam`. Every step
left every edition's prepared outputs equal to those before it (`tools/video/compare.py`: timelines by value,
captions, documents, and narration unchanged, nothing synthesized anew). The plan was written from three surveys of
the tracked files (the storyboards; the video pipeline and the lab's timeline code; the lab's other content formats). Runtime logic
(actions, click handlers, the Play animation, live readouts) is [lamlet-live-logic.md](lamlet-live-logic.md)'s, and
the natives that check names against the lab are the in-process plan's milestone 2; this plan says where it needs
them.

## What there is to migrate

### In the storyboards

| What | Today | Uses (sdfgi / voxelgi / o3de) | As Lamlet |
|---|---|---|---|
| Motions | `{motion "[{\"path\": …}]"}`: JSON in a String; channels are Strings | 8 / 59 / 92 (path 159, orbit 6, wave 1); `seconds` always `"beat"` or `"sentence"` | `Motion` values from typed constructors; channels are the vocabulary's own functions; a `Length` variant |
| Points | `{labelAt "1.8,2.4,-2.95"}`, `arrowFrom`, `arrowTo`: three numbers or a point's name, in a String | 0 / 0 / 2 | `{at 1.8 2.4 -2.95}` or `{point "probe.top"}`, a `Point` variant |
| Image marks | `{imageMark "0.28,0.42,0.62,0.98"}`, `""` to clear | 0 / 0 / 60 (20 clears) | `{mark u0 v0 u1 v1}`, `unmark` |
| Name lists | `{imageHide "t0,t1"}`, `labelHide`, `arrowHide`, `highlight` (comma lists) | 0 / 1 / 5 (lists); `highlight` 46 / 54 / 22 | `{imageHide {' "t0" "t1"}}`: a List of Strings |
| Record | `{record "object:3"}` | unused | `{recordObject 3}`, `recordCamera` |
| A formula | `${cue "formula" {' {formula "TeX"}}}${say {math "TeX"} "said"}`: the LaTeX twice | 0 / 0 / 1 | one function, `${formulaSaid "TeX" "said" {'}}`, giving the cue and the `say` together |
| Names the lab knows | `callout` (source-index keys), `imageTitle`, `imageMarkText`, `labelText` (text keys), `highlight` (listing tags, pass names), inset instance names | callouts 83 / 151 / 321; text keys 0 / 0 / 89 | checked at the storyboard's line: closed option sets or natives (phase 3) |
| A beat's opening reset | `demoX {callout ""} {inset ""} panelY {cameraDrift n} …` | 140 of the 176 beats of voxelgi and o3de open with `{callout ""}`; 44 / 59 / 77 distinct opening cues | a function, `{opening demoRoom panelHidden 1.0}`, plus the beat's own attributes |
| Repeated groups | o3de's baseline lighting (6 values) ×14, its pick point (`inspectX/Y/Z/NX/NY/NZ`) ×17, an image binding (`inset`, `imageFromChainTarget`, `imageTarget N`, corner, title) ×21, `{delay 0.4}` ×93 | o3de | bindings in a package's story library: `baseLight`, `pickPoint`, `{showTarget "t0" 2 imageCornerTopLeft}` |
| Repeated camera paths | 82 of o3de's 92 motions start near distance 3; several identical (`[[0,6,3.1],[-10,4,3.3],[6,8,3.0]]` ×6) | o3de, voxelgi | named waypoints and a `{cameraPath …}` helper |
| A range wrapping a callout | `${during "x" {' …} {cat {' {text " "} {cue "key" {' {callout "k"}}} {text " prose"}}}}` | 64 of o3de's 67 `during` | `${duringCallout "x" {' …} "k" "prose"}` |
| Numbers in the narration | spelled by hand ("one hundred twenty-eight cells"), often beside the same number in a cue (`{cascadesShownRange 0.0 4.0}` and "four cascades") | 157 / 200 / 368 spelled numbers | `${spoken cells}` from a binding, or from package code through natives (phase 4) |
| Throwaway range names | `"m0"`, `"vm"` | 193 distinct names over voxelgi's 214 ranges | optional names (a range needs one only when another refers to it; nothing does) |
| The translation's headings | each id, `number`, and title repeated by hand, kept in step with English | 76 / 90 / 107 headings | ids only; `number` taken from the English by id |

The storyboards have no `attr` escapes, so the vocabulary already covers every key they set.

### In the pipeline (tools/video, lab/src)

| What | Today | As Lamlet, or not |
|---|---|---|
| Pacing: `LEAD` 0.40, `GAP` 0.32, `TAIL` 0.70, `MAX_EXTRA_PAUSE` 2.2, `TERM_FADE` 0.4, the caption bridge 0.6 s, the tagged-sentence weight 2.0 | constants in timeline.py | per-video attributes of the video heading (`{#video lead=0.4 gap=0.32 …}`), the same defaults; the arithmetic stays in Python |
| How a chapter is padded to its target (weights, which beats absorb the leftover) | `plan_chapter` | not now: it would need Lamlet called with the measured durations, for one rule nobody has asked to change |
| The formula card closing at the beat's end | a hidden key timeline.py adds | stays, documented; a storyboard can close it earlier with `formulaOff` |
| Pronunciations | `video.json`'s `pronunciations`, a word table applied to spoken text | stays data; a storyboard that needs a word said otherwise uses `say` |
| Caption widths (58 English, 64 zh-CN display units) | constants | video heading attributes, as pacing; the wrapping algorithms stay in Python |
| The attribute rules | three copies: `markup.cue_key` (Strings), `presentation.attrs_key` (typed), and field lists in markup.py, presentation.py, and keys.rs; the camera fields in two | one reader in markup.py; the field lists from `render-lab story keys` |
| Enumerations in keys.rs (`PANELS`, `EASES`, `CORNERS`, `RESERVED`) | written by hand beside the Rust enums | generated from the enums |
| `EASES` in timeline.py | lacks `sine`, which the lab and the vocabulary accept: `{easingSine}` passes the vocabulary and fails in `make_key` | a bug: read the list from the vocabulary |
| What a `demo` switch keeps (callout, formula, images, notes; the camera only when the key moves it) | `shot.rs` `state_at` | stays in Rust: it is what the timeline means, not content |
| Callouts | checked only by build.py's `validate`; the lab stores the key and shows nothing for a wrong one | checked at the storyboard's line (phase 3) |

### In the lab's other content formats

| Format | Files | What it repeats or computes by hand | Fit |
|---|---|---|---|
| Experiments (`experiments/*.toml`, lab/src/experiment.rs) | 3 | sweeps, zips, and variants are a small expansion language of their own; values keyed by Strings | best: typed values, sweeps as functions; but the Experiments tab saves presets as TOML, so TOML stays readable |
| Scenes (`packages/render_lab/scenes/*.toml`, lab/src/scene.rs) | 3 | `study-room-grounds` is `study-room` with 13 lines changed; wall transforms written out from the room's half size | good: one room function with parameters; walls derived |
| Demos and presets (`demos/*.toml`, lab/src/demo.rs) | 37 demos, 16 presets | `room-camera.toml` byte-identical in three packages; `hint-text = "investigation.demo_hint"` on every demo; `inspectX…NZ` written as six numbers for a point and a normal | good: shared presets imported, typed params against the declared values |
| knowledge.toml (lab/src/knowledge.rs) | 4 (render_lab's 1,208 lines) | `title-text`/`summary-text` always `topic.<key>.title`/`.summary`; `known` repeats `render_lab.knowledge#` 20 times | fair: key conventions as functions; the graph checks stay in Rust |
| Diagrams (`video/diagrams/*.toml`) | 1 | a chain of seven nodes written as seven nodes and five edges | small: a `chain` function; text keys from ids |
| Frame graphs (`graphs/frame.toml`) | 3 | pass names must match the Rust render graph | small: pass names as natives |
| video.json | 3 | documents' texts per language; numbers also in the text tables and the WGSL ("52 samples") | small: shared numbers from package values (phase 4) |
| Text tables (`text/*.json`) | 8 | 36 of o3de's keys identical in both languages (all 15 `formula.*`, with the same LaTeX and colours) | keep flat JSON (the lab's i18n and its build check read it); shared entries could be generated |
| Source-index generators (`source_index.py`) | 3 | `ask_checkout`, `line_hash`, `find`, and `main` copied three times | keep Python (they read an engine checkout, which Lamlet's core cannot); share the helpers in one module |

### What should not move

- **WGSL value tags** (`/// {@ key=…}`): tied to struct layout through naga's reflection, and per-frame.
- **Pseudo-code listings** (`pseudocode/*.py`): shown as source text; their `# @tag` trailers are line anchors.
- **source-index.json and timeline.json**: generated contracts read by Rust and Python.
- **Manifests and collections** (`manifest.toml`, `resources.toml`, `files.toml`): build inputs, read by the build
  script and the cook before any runtime exists.
- **The timeline's evaluation** (`state_at`, blends, motions, channels), narration (Piper, marks), caption
  wrapping, and frame bookkeeping: the engine, with frame identity guarantees.
- **The lab's type checks** (`values::resolve`, action arguments, image and note rules): the authority; Lamlet
  calls them through natives instead of copying them.

## Design

**Where types live.** The lab's types go in render_lab's package (`render_lab.lam`): motions, points, marks,
name lists, records, and helpers built from them. lamlet-present names no host and stays general. The vocabulary
(`render-lab story keys`) stops listing the fields render_lab types (`animate`, `arrowFrom`, `arrowTo`, `labelAt`,
`imageMark`, `imageHide`, `arrowHide`, `labelHide`, `highlight`, `record`), so the two never define one name twice,
and the vocabulary's check (which imports render_lab) catches a clash.

**Attributes stay Strings at the boundary.** lamlet-doc's attributes are pairs of Strings, so a typed constructor
writes the String the lab reads today (a motion's JSON, `"0.28,0.42,0.62,0.98"`), and the loader and the lab are
unchanged. The types are checked where the storyboard is written; the lab keeps checking what it loads.

**Channels are the vocabulary's functions.** `{cameraYaw 24.0}` is an `Attr` whose key is `camera.yaw`, so a
motion takes the functions themselves: `{path {' cameraYaw cameraPitch} …}`, and render_lab reads each key by
calling it (`{.key {f 0.0}}`, checked to work with Lamlet v0.3.1). A misspelled channel is an unknown name, and a
non-numeric key (a vector, a choice) is a type error. A vector's component whose name a value already has
(`camera.x` beside `cameraX`) is not in the vocabulary; no storyboard animates one, and `{attr …}` remains.

**Motions.**

```text
$[# Length [% <beat: Unit, sentence: Unit, seconds: Float>]]
{path channels waypoints length}          path through waypoints (a List of Lists of Floats)
{orbit channels center degrees length}    about center, two or three channels
{wave channel amplitude period}           a sine about the channel's value
{eased easing m}  {closed m}  {repeating loop m}  {around axis m}  {from start m}  {phased degrees m}  {fading seconds m}
{motion {' m1 m2}}                        the cue attribute (`animate`)
```

The optional fields are wrappers, as Lamlet has no optional arguments. `beat`, `sentence`, and `{seconds 4.0}`
are a `Length`, so `"seconds": "bea"` is a type error.

```text
before: ${cue "key" {' {motion "[{\"path\": {\"params\": [\"camera.yaw\", \"camera.pitch\"], \"through\": [[18, 16], [8, 22]], \"seconds\": \"beat\"}}]"}}}
after:  ${cue "key" {' {motion {' {path {' cameraYaw cameraPitch} {' {' 18.0 16.0} {' 8.0 22.0}} beat}}}}}
```

**A story library per package.** Repeated groups (an opening reset, a lighting baseline, a pick point, an image
binding, waypoints) are bindings in the package's `video/story.lam`, a Lamlet package the storyboard imports
(named `<package>_story`). It is content like the storyboard. (The vocabulary check cannot import it, since it
imports the vocabulary; it is checked where the storyboard is read.) A cue's
attributes are a List, so a helper's list joins the beat's own with `flatten`:
`${cue "key" {flatten {' baseLight {' {sunYaw 60.0}}}}}` (checked to work).

**Checks against what the lab knows** (phase 3) come in two forms:
- **Closed option sets**, for small sets the lab enumerates (demos, panels, listings, image kinds, corners, eases,
  click handlers, actions): only the option constants, no free String function, so a wrong name is an unknown
  name. This needs a general lamlet-present change: a vocabulary key marked `"closed": true`.
- **Natives**, for large sets that change with content (source-index keys, text keys, listing tags, published points
  and images): the in-process plan's milestone 2 (`rl.source`, `rl.text`, `rl.point`, `rl.image`), plus
  `rl.listingTag` and `rl.formula` (a formula typesets within the lab's limits, today `render-lab formulas --check`,
  run only after the timeline is built).

**Checks of every step.** Each step that rewrites storyboards must leave every board and timeline equal by value
(numbers compared as numbers: Lamlet writes `18` as `18.0`) and the narration unchanged (no new synthesis). This
needs a tracked tool, `tools/video/compare.py OLD NEW`, comparing two `--prepare-only` outputs: timelines by value,
captions and documents byte for byte, narration audio by hash. The migration used a scratch version of it.

## Phases

0. **Cleanups, no new language** (independent of the rest). Done: `markup.cue_key` is the one reader (fields from
   the key list, `"field": true`; values typed once, a range a `markup.Range`; easings from the vocabulary, so
   `sine` works), keys.rs's lists from the enums' `ALL` (and `RESERVED` from its own fields), `compare.py` and
   `test_compare.py`.
   - `EASES` in timeline.py from the vocabulary (fixes `sine`).
   - One attribute reader: `presentation.attrs_key` folded into `markup.cue_key`, typing values once; Python's field
     lists (`STRING_FIELDS`, the camera fields) read from the vocabulary's key list (`build/<package>.keys.json`).
   - keys.rs's `PANELS`, `EASES`, `CORNERS` generated from the enums (an `ALL` list beside each, as `graph::Show`
     has).
   - `tools/video/compare.py`, and a test for it.
   - Check: every edition's outputs unchanged (compare.py, byte for byte).
1. **Typed attributes in render_lab** (the design above). Done; `wave` is `sway` (`present` has `wave`), the optional
   parts are wrappers (`eased`, `looping`, `pingPonging`, `closed`, `around`, `from`, `centered`, `phased`,
   `lasting`, `fading`), `highlight` takes a list (`{highlight {'}}` clears it), and the typed fields stay in the key
   list marked `"typed": true` (keys::TYPED), which `render-lab lamlet vocab` leaves out. The converter was a scratch
   script. motions, points, marks, name lists, records,
   `formulaSaid`; the vocabulary drops those fields; a converter rewrites the 159 motions, 60 marks, 2 points, and 6
   lists of the three storyboards (and any local package's). Check: every timeline equal by value, narration
   unchanged; a test of each constructor's String against the lab's reader (`Motion` JSON parsed by the lab's
   `animation.rs`).
2. **Story libraries.** Done: a library per package (sdfgi and voxelgi an `opening`; o3de `opening`, `baseLight`,
   `teapotPoint`, `chainTarget`, `settle` for `{delay 0.4}` (`pause` is `content`'s), and three named waypoint lists;
   the local package its own); render_lab's `over`, `overText`, `overCallout`, `cameraPath`, `cameraTurn`; unnamed
   ranges named by presentation.py. Rewritten by a scratch converter, checked equal. `video/story.lam` per package, with the repeated groups the survey found (an opening reset,
   o3de's lighting baseline and pick point, its image binding, `duringCallout`, named waypoints, a `pause` for
   `{delay 0.4}`); the storyboards rewritten to use them, by hand, beat by beat, with an agent's help. Optional range
   names. Check: as phase 1.
3. **Checks at the storyboard's line.** Done in the lab: the natives `rl::source`, `rl::text`, `rl::point`,
   `rl::formula` (lab/src/script/natives.rs; `render-lab --investigation <id> lamlet ir|deck`), called by render_lab's
   `callout`, `imageTitle`, `imageMarkText`, `labelText`, `point`, and `formulaSaid` (those fields left the
   vocabulary); build.py's callout check removed; test_presentation's wrong names fail at their line. Open: closed
   option sets need the lamlet-present change (`"closed": true`), released and pinned, which is the user's call;
   `rl.listingTag` is not done (a highlight names listing tags, passes, or struct fields, by panel); and
   `lab/check`'s textual harvest stays: the build script cannot evaluate a storyboard (its vocabulary comes from
   running the lab), and the text check needs the keys it uses at build time. Closed option sets (the lamlet-present change, released and pinned);
   natives (the in-process plan milestone 2, with `rl.listingTag` and `rl.formula`); build.py's `validate` and
   `lab/check`'s textual harvest of cue attributes removed once the natives cover them. Check: a test storyboard
   with each kind of wrong name fails at its line, before speech; every edition unchanged.
   Since (2026-11-04): `story check` and `tools/video/check.py` ([render-lab-storyboard-plan.md](render-lab-storyboard-plan.md),
   PR 1 to 3) check a storyboard's names in Rust before speech (`lab/src/story_refs.rs`: highlights by
   namespace, sources, text keys, actions, images); the typed fields grew to `highlightPasses`, `highlightCode`, and
   `highlightFields` (`TYPED`, `lab/src/script/keys.rs`). Closed option sets, `rl.listingTag`, and the harvest are
   still open.
4. **Computed narration.** In part: `Investigation::script_natives` (ids `<package>/<name>`, the package
   `<package>_natives`); VoxelGI's chapter 8 numbers from cost.rs (leaves, dispatches, cone steps) and its cone
   counts bound once in its story library, each said and cued from the one value; SDFGI's cascade count and size from
   its code. Open: the other spelled numbers, sentence by sentence, and video.json's and the text tables' shared
   numbers. Numbers from bindings and package values (the in-process plan milestone 3): the cell
   counts, cascade counts, sizes the narration spells by hand, said with render_lab's `spoken`, from the same values
   the cues use; shared numbers in video.json and the text tables from the same source. Check: the spoken text of
   every sentence unchanged (no new synthesis).
5. **Pacing and translation.** Done (`timeline.Pacing`; the translations' chapters by id, without `number`). Pacing and caption widths as video heading attributes (defaults unchanged); the
   translation's headings reduced to ids and titles, its chapter numbers taken from the English. Check: every
   edition unchanged.
6. **Other content formats.** In part: the data path is `lab/src/script/data.rs`, which writes a `.lam` resource's
   binding as the TOML document the reader takes (so every reader stays as it is; `render-lab lamlet toml`), and the
   study room and its grounds are one Lamlet room function (`scenes/study-room.lam`; parsed equal to the TOML, the
   viewer's study room drawn from it). Open: experiments, demos and presets, knowledge.toml, diagrams. One at a time, each read through the lab's host as data: a `.lam` resource whose
   exported binding has a type the lab declares (a stub package), converted to the struct the TOML parser makes
   today, so the rest of the lab is unchanged. In order: experiments (the TOML reader stays for saved presets),
   scenes, demos and presets, knowledge.toml's key conventions, diagrams. Each needs the host's data path
   (`Document::binding` to `Data` to the struct) first. Check: the parsed structs equal to the TOML's, and every
   selftest and default frame unchanged.

## What is left, in stages (2026-11-05)

Each stage is one commit, checked as its phase says (every edition's prepared outputs equal, `tools/video/compare.py`,
or the parsed structs equal to the TOML's).

1. **Computed narration (phase 4), one package per commit.** The numbers a storyboard spells by hand that its
   package's code computes (cell counts, sizes, steps), said with render_lab's `spoken` from a package native, as
   VoxelGI's chapter 8 is; the spoken text of every sentence unchanged (no new synthesis): (a) SDFGI, (b) O3DE,
   (c) SSR.
2. **Experiments as Lamlet (phase 6).** An experiment file may be a `.lam` resource whose binding the lab reads
   through `lab/src/script/data.rs`, as the study room is; the TOML reader stays. One experiment converted, parsed
   equal.
3. **Shared demo presets (phase 6).** `demos/presets/room-camera.toml` is byte-identical in three public packages:
   it becomes one render_lab preset the packages import; every demo's parsed state unchanged.
4. **knowledge.toml and diagrams (phase 6):** only if a stage above shows the data path pays for itself; otherwise
   they stay TOML, and this plan ends.

Not in this repository: the vocabulary's closed option sets (phase 3) need a lamlet-present release with
`"closed": true`, released and pinned upstream (the user's call). Until then the story check
(`lab/src/story_refs.rs`) checks those names before speech.

## Open questions

- **Closed sets or natives for listings and demos.** Both work; closed sets need no runtime, natives give better
  messages. Recommended: closed sets for what `story keys` enumerates, natives for the rest.
- **A story library per package, or helpers in the storyboard itself.** The library keeps the storyboard to the
  story, and its helpers can be tested; the storyboard's own `$[# …]` bindings are simpler for one-off groups. Both
  are allowed; the plan's phase 2 uses the library for what repeats across chapters.
- **Typed channels for vector components** (`camera.x`): a vocabulary function per component, named apart from
  values, if a storyboard needs one.
