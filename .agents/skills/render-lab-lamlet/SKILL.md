---
name: render-lab-lamlet
description: Work with Lamlet in Render Lab - the storyboards, which are Lamlet presentations (`packages/<package>/video/storyboard.lam` and `storyboard.zh-CN.lam`, read by `tools/video/presentation.py`), their cue vocabulary (`render-lab story keys`), the lab's own Lamlet package (`packages/render_lab/language/render_lab.lam`), and the lab's pinned Lamlet and its `render-lab lamlet` commands. Use when writing or editing a storyboard, `render_lab.lam`, or the lab's Lamlet code, together with the `lamlet` and `lamlet-present` skills for the language and the presentation format.
---

# Lamlet in Render Lab

Lamlet is a typed functional language embedded in text documents; lamlet-present is its extension for narrated
presentations. Render Lab's storyboards are Lamlet presentations. This skill is what is specific to the lab: how a
storyboard is written and read here, its vocabulary, the lab's own Lamlet package, and the rules the lab imposes.
`AGENTS.md`'s rules apply to everything below; `tools/video/README.md` has every cue attribute.

## 1. The language skills

Read these before writing Lamlet. They are versioned with the Lamlet the lab pins, so read the copies at the
pinned version:

| Skill | Pinned copy (after `cargo fetch`, in the project-local CARGO_HOME) | Source |
|---|---|---|
| `lamlet` | `.cache/cargo/git/checkouts/lamlet-<hash>/<rev>/.agents/skills/lamlet/SKILL.md` | github.com/lamlet/lamlet |
| `lamlet-present` | `.cache/cargo/git/checkouts/lamlet-present-<hash>/<rev>/.agents/skills/lamlet-present/SKILL.md` | github.com/lamlet/lamlet-present |

The pinned versions are the tags in `lab/Cargo.toml` (and the exact commits in `Cargo.lock`). Nothing in the lab
uses a `lamlet` installed on the system: `tools/video/lamlet.py` runs the lab's own `render-lab lamlet …`.

## 2. A storyboard

```text
$[Import* std]$[Import* content]$[Import* present]$[Import* render_lab]$[Import* godot_voxelgi]$[Import* godot_voxelgi_story]
# How VoxelGI Lights a Room {#video subtitle="..." fps=30 width=1920 height=1080}

## Where it runs {#c1 number=1 target=185 summary="..." sources="frame-order, settings"}

### The headline {#c1-room kind=3d}

${cue "key" {opening demoRoom panelHidden 1.0 {' {cameraYaw 24.0}}}}
${cue "key" {' {motion {' {cameraPath {' {' 14.0 16.0 6.8} {' 26.0 20.0 5.2}}}}}}}

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
time it reads the storyboard, so it always matches the investigation.

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
- **Typed attributes**, each writing the String the lab reads (tools/video/markup.py): motions (`{motion {' …}}` of
  `path`, `orbit`, `sway`, with a `Length`, `beat`, `sentence`, or `{seconds 4.0}`, and the wrappers `eased`,
  `looping`, `pingPonging`, `closed`, `around`, `from`, `centered`, `phased`, `lasting`, `fading`; `cameraPath`,
  `cameraTurn`), points (`{at x y z}`, `{point "key"}` for `labelAt`, `arrowFrom`, `arrowTo`), marks (`{mark u0 v0 u1
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

## 6. Rules

- **Builds** stay `--locked --offline` and clean (`AGENTS.md`). The Lamlet crates are pinned by git tag; never
  depend on a path outside the repository. `vendor/` is regenerated by `setup.sh`, never edited.
- **The lab names no investigation** in its Lamlet code; investigations reach it through the `Investigation`
  trait, as everywhere else.
- **Ids and names in documents** use `-` (`c1-room`, range names); Lamlet package names use `_`.
- **Read only tracked files** (`git ls-files`), and never name anything outside git.
