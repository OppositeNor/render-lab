---
name: render-lab-lamlet
description: Work with Lamlet in Render Lab: storyboards as Lamlet presentations (`packages/<package>/video/storyboard.lam`, `storyboard.zh-CN.lam`, read by tools/video/presentation.py) and their cue vocabulary (`render-lab story keys`); the lab's Lamlet package (`render_lab.lam`); a package's live scripts (`script/*.lam`: overlays, readouts, click handlers, actions, animations), the runtime library (`runtime.lam`), and `render-lab script check|stubs`; Lamlet content files; the pinned Lamlet and `render-lab lamlet` commands. Use when writing or editing a storyboard's syntax or cues, a script or the runtime library, `render_lab.lam`, the story reader, or the lab's Lamlet code, with the pinned `lamlet` and `lamlet-present` skills for the language. What a story should say and show is `new-story`'s.
---

# Lamlet in Render Lab

Lamlet is a typed functional language embedded in text documents; lamlet-present is its extension for narrated
presentations. Render Lab's storyboards are Lamlet presentations. This skill is what is specific to the lab: how a
storyboard is written and read here, its vocabulary, the lab's own Lamlet package, and the rules the lab imposes.
`AGENTS.md`'s rules apply to everything below; `tools/video/README.md` has every cue attribute.

## 1. The language skills

Read these before writing Lamlet beyond a change that copies a form already in the file (a cue's value, a
sentence). They are versioned with the Lamlet the lab pins, so read the copies at the pinned version:

| Skill | Pinned copy (after `cargo fetch`, in the project-local CARGO_HOME) | Source |
|---|---|---|
| `lamlet` | `.cache/cargo/git/checkouts/lamlet-<hash>/<rev>/.agents/skills/lamlet/SKILL.md` | github.com/lamlet/lamlet |
| `lamlet-present` | `.cache/cargo/git/checkouts/lamlet-present-<hash>/<rev>/.agents/skills/lamlet-present/SKILL.md` | github.com/lamlet/lamlet-present |
| `lamlet-embedding` (for the lab's Rust side: natives, the runtime) | `.cache/cargo/git/checkouts/lamlet-<hash>/<rev>/.agents/skills/lamlet-embedding/SKILL.md` | github.com/lamlet/lamlet |

The pinned versions are the tags in `lab/Cargo.toml`, and the exact commits in `Cargo.lock` (`grep -A2 'name =
"lamlet' Cargo.lock`): `<rev>` is the first seven hex digits of that commit. The cache may hold other revisions
beside it; read only the pinned one. Nothing in the lab uses a `lamlet` installed on the system:
`tools/video/lamlet.py` runs the lab's own `render-lab lamlet …`.

## 2. A storyboard

```text
$[Import* std]$[Import* content]$[Import* present]$[Import* render_lab]$[Import* godot_voxelgi]$[Import* godot_voxelgi_story]
# How VoxelGI Lights a Room {#video subtitle="..." fps=30 width=1920 height=1080}

## Where it runs {#c1 number=1 target=185 summary="..." sources="frame-order, settings"}

### The headline {#c1-room kind=3d}

${cue "key" {opening demoRoom panelHidden 1.0 {' {cameraYaw 24.0}}}}
${cue "key" {' {motion {' {cameraPath {' {pose 14.0 16.0 6.8} {pose 26.0 20.0 5.2}}}}}}}

The first sentence, ${cue "key" {' {callout "settings"}}}with a cue on a word, and
${overText {' {voxelgiMixRange 0.0 1.0}} "a range over words"}.
```

- **Headings:** `#` the video (its text the title, its attributes the header), `##` a chapter (`number`, `target`
  seconds, `summary`, `sources`), `###` a beat (its id, `kind` `3d` or `2d`, `diagram`, `graph`, `focus`, `sources`,
  `hold`). Quoted attribute values may hold spaces, not quotes or braces.
- **Paragraphs:** a paragraph of cues before a beat's first sentence sets its keys, one per cue; every other
  paragraph is one sentence. Prose is CommonMark: `$` is `$$`, and `* _ [ ] < \ `` ` `` are markup (escape them with
  `\`, or put the text in a region).
- **Cues** are `present`'s: `${cue "key" ATTRS}` at a point, and render_lab's unnamed ranges, `${overText ATTRS
  "words"}` over words, or `${over ATTRS {cat {' {text "a "} {cue "key" ...} {text "b"}}}}` with cues or ranges
  inside (`present`'s `during`/`duringText` take a name nothing needs). A point cue is always named `"key"`. ATTRS is
  a list of attributes, or an expression giving one (`{opening …}`, `{append baseLight {' …}}`).
- **Formulas:** `${formulaSaid "TeX" "what the voice says" {' {colors "#ffd166"}}}` (backslashes doubled in
  Strings; it is the formula cue and its `say`); `${cue "formula" {' {formulaOff}}}` closes the card. **Code names:**
  `${code "Name"}`, or `${say {code "Name"} "how it is said"}`.
- **The translation** (`storyboard.zh-CN.lam`) imports `std` and `content` only: the same headings by id (its own
  titles, summaries, headlines, with no chapter `number`; the video's `subtitle`, `locale`, `glossary`
  comma-separated, `captionWidth`), one paragraph per English sentence, formulas as `${math "TeX"}`, code names as
  `${code "Name"}`, no cues.
- **Pacing** is the video heading's (`lead`, `gap`, `tail`, `maxExtraPause`, `taggedWeight`, `termFade`,
  `captionBridge`, `captionWidth`; tools/video/README.md), each with its default when left out.
- **The story library** (`video/story.lam`, the package `<package>_story`, optional): what the storyboard repeats,
  bound once (a beat's `opening`, a baseline of values, camera waypoints, chapter numbers it shows and says). It
  imports what the storyboard does. Write a text key a helper sets at the call site (`{imageTitle "views.…"}` in the
  helper's `more` list), where the text check finds it.

## 3. The vocabulary

`render-lab --investigation <path> story keys` (`lab/src/script/keys.rs`) lists what a cue may set: the
investigation's values (and their vector components), and a timeline key's fields with their options where the lab
knows them (demos, panels, listings, images, image kinds, actions, click handlers, eases, corners). `build.py` turns
it into a package named after the investigation's package, `build/<package>.lam` (`render-lab lamlet vocab`), every
time it reads the storyboard, so it always matches the investigation. `render-lab --investigation <path> story schema`
adds what writing each key needs (a value's type, unit, range, default, and doc; each key's attribute and function)
and lists what the vocabulary leaves out (an option that makes no Lamlet name, a key whose function name another
has), each with the `{attr "…" "…"}` that still writes it.

- A function is its key in camel case (`camera.yaw` is `cameraYaw`), except `code` (`listing`), `image` (`inset`),
  and `ease` (`easing`), whose names `content` and `present` already use. The fields render_lab types or checks
  (`animate`, points, `imageMark`, the hide lists, `highlight`, `record`, `callout`, and the text keys `imageTitle`,
  `imageMarkText`, `labelText`) are in the key list marked `"typed": true` and have no vocabulary function
  (`keys::TYPED`). A
  value a cue reserves is `param.<name>` (`paramFormula`); a dotted key whose name a field has is `imageParam…` or
  `runArg…`.
- Every numeric value is a Float (`{octreeLevelRange 7.0 1.0}`), whatever its type in the lab: the timeline blends
  every value, toggles too. A float key has a `…Range a b` function for ranges. Vectors are lists
  (`{cameraTarget {' 1.0 0.5 0.0}}`); a key with options has a constant per option (`panelCode`, `demoRoom`) beside
  the text function.
- `{attr "key" "value"}` writes a key the vocabulary does not have (a graph visualizer's `image.range`).
- A misspelled key is an unknown name and a wrong value a type error, reported with the storyboard's line before any
  speech is synthesized.

## 4. The lab's Lamlet package: `render_lab`

`packages/render_lab/language/render_lab.lam` is the package `render_lab` (the resource
`render_lab.script/render-lab`), which every storyboard imports:

- **Numbers said in words:** `spoken 42289`, `rounded 42289 2`, `spokenFloat 3.2`, `capitalized`.
- **Typed attributes**, each writing what the lab reads (tools/video/markup.py), a String or, for motions, Data: motions (`{motion {' …}}` of
  `path`, `orbit`, `sway`, with a `Length`, `beat`, `sentence`, or `{seconds 4.0}`, and the wrappers `eased`,
  `looping`, `pingPonging`, `closed`, `around`, `from`, `centered`, `phased`, `lasting`, `fading`; `cameraPath`,
  `cameraTurn` over the beat, `cameraPathOver`, `cameraTurnOver` over a `Length`, their waypoints `{pose yaw pitch
  distance}` and `{aim yaw pitch}`; and the commonest, one channel to one value, as a whole attribute:
  `{glide cameraYaw 24.0 sentence}`. Write those short forms; the general `path` is the fallback), points (`{at x y z}`, `{point "key"}` for `labelAt`, `arrowFrom`, `arrowTo`), marks (`{mark u0 v0 u1
  v1}`, `unmark`), lists of names (`imageHide`, `arrowHide`, `labelHide`, `highlight`), records (`recordNone`,
  `recordCamera`, `{recordObject n}`), and `formulaSaid`.
- **Checked names**, through the lab's natives (the package `rl`, `lab/src/script/natives.rs`): `callout` (a
  source-index key), `imageTitle`, `imageMarkText`, `labelText` (text keys), `point`, and `formulaSaid`'s LaTeX
  (typeset within the lab's limits). They check against the investigation `render-lab --investigation <id> lamlet ir`
  names (the video builder passes it); without one they fail, but an empty name (clearing) needs none.
- **Ranges** without names: `over`, `overText`, `overCallout`.
- **Another view's values:** `{view "field" {' {fieldMorph 1.0}}}` renames each attribute `view.field.<key>`, so a
  cue or a range sets the values (and the own camera) of a view the investigation stacks over the main one
  (lab/src/view.rs) rather than the main view's; join it to other attributes with `append`
  (`{append {' {traceStep 6.0}} {view "profile" {' {profileShow 1.0}}}}`).

Write it in Lamlet 0.4's style (the lab pins Lamlet v0.4.0 and lamlet-present v0.1.4, lab/Cargo.toml): templates
`f"{...}"` (the bracket `[f "..."]` and `[latex "..."]` forms are gone), an attribute's value typed data (`Data`,
compared but not read: `eased` finds its easing's name by equality), function definitions `[# {f x} body]`, ascriptions without `[% ]`, private helpers
with a leading `_`, definitions before their uses (source order).

A package's code can give its storyboard numbers: `Investigation::script_natives` registers `Pure` natives with ids
`<package>/<name>`, which the storyboard imports as the package `<package>_natives` (`${spoken {dispatches}}`).

A content file can be Lamlet too: `render-lab lamlet toml FILE NAME` writes a static binding as the TOML document the
lab's readers take (`lab/src/script/data.rs`; a `.lam` resource is read as its binding named after the resource's key,
as `scenes/study-room.lam` is).

## 5. How a storyboard is read

`presentation.py` runs `render-lab lamlet ir` (the document's IR, lamlet-doc's JSON, every region evaluated) with the
vocabulary, and builds the board the builder reads: each sentence carries a parsed `markup.Sentence` (its caption,
spoken pieces cut at each cue, tags, formulas), which `narration.py`, `timeline.py`, `zh.py`, and `outputs.py` use
through `markup.sentence` and `markup.shown`. The lab's own timing rules (`timeline.py`) stay; lamlet-present's
`schedule` is not used. The lab's text check (`lab/check`) finds the text keys a storyboard names
(`imageTitle`, `imageMarkText`, `labelText`).

Tests: `python3 -m unittest tools/video/test_markup.py tools/video/test_presentation.py tools/video/test_compare.py`
(the second skips until the lab is built). A change to a storyboard's form or to the reader leaves every edition's
prepared outputs equal (`tools/video/compare.py`, tools/video/README.md).

## 6. Scripts: Lamlet the lab runs

A package's **scripts** are Lamlet the lab runs while the viewer plays and while a video records
(`lab/src/script/runtime.rs`; the plan and its decisions are `todo/lamlet-live-logic.md`). The split: what is heavy
or engine math stays the package's native code (passes, CPU mirrors, a ray into the scene, octrees), and a script
says what to do with it: where an overlay's arrows, lobes, and labels go, what a click sets, how a value moves
over time, what a readout row says.

- **Where.** Each resource `script/<name>` of the investigation's package (`[[resource]] id = "script/overlay"`,
  `source = "script/overlay.lam"`) is a script; the package's other scripts import it as `<package>_<name>`.
- **Imports.** `std`; render_lab's runtime library `render_lab_runtime`
  (`packages/render_lab/language/runtime.lam`: the hooks' types, `Vec3` and vector functions, shapes, eases and
  curves, `text`, `demo`, colours); the investigation's **values package** `<package>_values`, which the lab
  writes from its values: a reader per key (`{vectorsShown}`, the shot the hook reads) and a setter
  (`{setVectorsShown 1.0}`, a `Write`), a choice as a variant (`{setViewMode {#final unit}}`); its natives
  `<package>_natives`; and the lab's `rl_runtime` (`trace`, `viewport`, the readers' natives). A key whose name std, the
  library, or the natives already have is left out of the values package (`script stubs` lists them).
- **Hooks**, found by their type (ascribe it; a name starting with `_` is never one):

  | Hook | Type | The lab |
  |---|---|---|
  | overlay | `Overlay` = `Fn([Float], List(Shape))` | draws its shapes over every frame (`shader_data::build_frame`), at `FrameContent::overlay_at` when the code marks its place |
  | readout | `Readout` = `Fn([Float], List(Row))` | adds its rows to the frame's readout |
  | click handler | `ClickHandler` = `Fn([Click], List(Write))` | runs it for an image's `imageClick` of its name; the one named `scene` takes a click on the scene, before `Investigation::click` |
  | action | `{Action Args}` = `Fn([Float, Args], List(Write))` | runs it from a story's `{run "name"}`, with `run.<arg>` as the record `Args` (its fields' types are the arguments' kinds) |
  | animation | `Animation` = `Fn([Float], List(Write))` | the viewer's Play animation of a demonstration whose file names it (`animation = "playSurface"`), after the code's |

- **No state.** A hook is a function of what it is passed (the time, a click, its arguments) and what it reads;
  it keeps nothing between calls, so seeking and recording stay exact. A sequence is a function of time (`seq`,
  `span`, `steps`); a hook returns a description, which the lab checks whole and then applies (writes all or none).
  Lamlet computes in f64 and the lab in f32: count steps with `steps`, which counts a step from its frame.
- **Natives.** `Investigation::script_natives` registers the package's (`<package>/<name>`). One that reads the
  frame takes what `build_frame` left in `FrameContent::script` (`lab::script::runtime::frame_data::<T>()`), so it
  computes nothing twice; `state`, `frame`, and `statics` read the call's context. A native works on a whole list
  (all of a lobe's directions in one call), and `lab::lamlet_record!` declares a record it returns. Its name must not
  be a value's key (the key would leave the values package).
- **Text.** A label's or a row's text is a text key: `{labelText at {textKey "demos.observer"} color}`, or
  `{text {textKey "…"}}` for the string; the text check (`lab/check`) finds `textKey`.
- **Errors.** A script that does not check fails when its package loads, with its line (`render-lab --investigation
  <id> script check` lists the hooks, or the diagnostic). At run time a failing hook fails a recording or a selftest
  (`set_strict`); in the viewer it keeps its last good shapes, sets nothing, and shows its message as a note until
  it succeeds again. A script hook named like one of the code's (`#[click]`, `#[action]`) is an error.
- **Examples.** `packages/o3de_lighting/script/` (overlay, readout, picks, animations; natives in `script.rs`) and
  `packages/godot_voxelgi/script/logic.lam` (`marchCone`, `pickSliceCell`).

## 7. Rules

- **Builds** stay `--locked --offline` and clean (`AGENTS.md`). The Lamlet crates are pinned by git tag; never
  depend on a path outside the repository. `vendor/` is regenerated by `setup.sh`, never edited.
- **The lab names no investigation** in its Lamlet code (the runtime library included); investigations reach it
  through the `Investigation` trait, as everywhere else.
- **Ids and names in documents** use `-` (`c1-room`, range names); Lamlet package names use `_`.
- **Name only tracked files.** What you write (storyboards, scripts, the lab's Lamlet) names and imports only files
  and packages `git ls-files` lists, never anything outside git (a local package included). The pinned language
  skills under `.cache/` are read as references; nothing written names them.

## Checks, by change

| Change | Check |
|---|---|
| A cue or sentence of a storyboard | `render-lab --investigation <id> story keys` when unsure of a key; `python3 tools/video/build.py --investigation <id> --prepare-only` (unknown names and type errors fail here, with the storyboard's line); stills of the beat (`new-story`, `references/storyboard.md`), looked at |
| The translation | `--prepare-only --lang zh-CN`; one paragraph per English sentence, the same heading ids |
| A package's script (`script/*.lam`) | `render-lab --investigation <id> script check` (and `script stubs` for what it may import); the selftest; an offscreen snapshot of a demonstration it draws or animates (`home --snapshot … --page viewer:<demo>`), looked at (a failing hook fails the selftest and a recording, but only shows a note in the viewer) |
| The runtime library, `render_lab.lam`, or the lab's Lamlet code | every investigation's `script check`, the unit tests below, `--prepare-only` for every edition and `tools/video/compare.py` (outputs unchanged), the frame identity check (AGENTS.md) when it changes what frames show |
| The story reader (`tools/video/presentation.py`, `markup.py`) or a storyboard's form | the unit tests below and `tools/video/compare.py` over every edition |
| A Lamlet content file | `render-lab lamlet toml FILE NAME`, compared with what the reader expected; a snapshot of what it feeds |
