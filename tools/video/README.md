# Narrated video builder

Builds an investigation's narrated video from its storyboard: Piper narration measured sentence by sentence, a timeline
that `render-lab render` records frame by frame, the muxed video with chapters, captions, a script, a source-reading
guide, review stills, and a verification report.

```sh
bash tools/video/setup.sh [--gpu]                                    # once: the local environment under .cache/
python3 tools/video/build.py --investigation o3de/lighting           # the English edition
python3 tools/video/build.py --investigation o3de/lighting --lang zh-CN   # Chinese text and subtitles, English narration
python3 tools/video/build.py --all                                  # every package's video, each in all its languages, in turn
```

| Option | Does |
|---|---|
| `--prepare-only` | Narration, timeline, captions, and documents (about 15 s once the narration is cached) |
| `--snapshots-only` | Also the diagrams and one still per beat in the package's `build/video/<lang>/snapshots/` |
| `--frames A:B` | Renders only those frames (testing; no mux or verification) |
| `--jobs N` | Renders the video in N parallel frame ranges, each after a second of warm-up frames it does not write, and joins them without re-encoding: the same frames as one render, faster where one render leaves the GPU idle. Each range takes about 1.1 GB of RAM, 1.4 GB for its FFmpeg, and 1.7 GB of video memory; 2 suits a 6 GB GPU |
| `--skip-render` | Reuses the rendered `video.mp4` |
| `--verify-only` | Only the checks of an existing build |
| `--tts-device auto/cuda/cpu` | Where new narration is synthesised (`auto`: the GPU when the CUDA packages are installed) |
| `--listing-style blocks/text` | How the code panel draws pseudo-code listings (`blocks`, the default, whatever the user's settings say) |
| `--investigation-dir DIR` | The directory of its `video/video.json`, when it is not its package in `packages/` |

A full build of the 16-minute O3DE video takes about 15 minutes on an RTX 4050, almost all of it rendering. Run one GPU
job at a time.

## Checking without speech

Use an already-built host and its matching package libraries to check a story
with the system Python. This command needs no video environment, voice model,
audio cache, FFmpeg, or GPU:

```sh
python3 tools/video/check.py --investigation godot/ssr --binary target/release/render-lab
python3 tools/video/check.py --investigation godot/ssr --lang zh-CN --json
```

The check reads the real Lamlet document and vocabulary, checks its sources,
listings and diagram focus, and asks the host to check values, actions, image
references and motion shapes using the same rules as playback. The translated
edition also checks its alignment with English. Motion lengths such as
`sentence` and `beat` stay symbolic: this does not measure speech or establish
exact word times, frame boundaries, or chapter budgets.

One beat at a time:

```sh
python3 tools/video/check.py --investigation godot/ssr --explain c1-adds [--json]
python3 tools/video/build.py --investigation godot/ssr --preview c1-adds --cached-audio-only
```

`--explain` (tools/video/explain.py) lists the beat's sentences, as shown and as spoken, then its cues in source
order: where each is, the attributes it lowers to, the names it refers to, and the channels it writes (a value, or a
motion and its length), each marked with the cue it takes the channel over from. It warns where two cues set one
channel on the same word, so the first never shows. Once the edition is prepared it adds each cue's frames and
seconds; before that it says the timing is not measured, and synthesizes nothing. `--preview` prepares the edition
and renders that beat's stills into `build/video/<lang>/preview/<beat>/`: its start and end, each cue and the frame
before it, where each motion and blend ends, and the midpoint, with `index.json` saying why each was taken.

Highlights are checked in the inherited demo, panel, listing and record context.
Use an explicit namespace for new cues:

```lamlet
${cue "key" {' panelGraph {highlightPasses {' "ssrTrace"}}}}
${cue "key" {' panelCode {highlightCode "trace" {' "edge"}}}}
${cue "key" {' panelValues recordCamera {highlightFields "CameraData" {' "worldToView"}}}}
```

`highlightCode` also selects its listing. The other helpers leave panel and record
selection to the cue. Empty lists clear the highlight. Graph, DAG and resource
panels use graph node names; Performance uses the original timing names (for
example `depth pre-pass`), before their display labels are translated. Code tags
belong to the selected listing; record fields belong to `ObjectData` or
`CameraData`. Switching the listing or panel checks any inherited highlight again.
A demo cue first resets this context to that demo's defaults, then applies its
explicit fields, matching playback.

This validates the requested panel/listing/record context, not whether a frame
actually displays that panel. A field helper checks the named WGSL schema and
the `record` selection; the investigation must also supply `FrameContent.panel_record`
for it to appear. Current public demos supply readouts or no record panel, so
field membership and runtime names are tested, but their visible field highlight
has no public demo fixture. Two-dimensional beats can also retain panel settings
while displaying a diagram or source card instead.

Legacy `highlight` lists still work. A legacy list with no active consumer
(Hidden, Knowledge, or a readout without a record) emits
`story.highlight.ambiguous`; membership is checked when a later cue activates a
consumer. Explicit targets are checked even while hidden. JSON reports retain
warnings with `ok: true`, and the text checker and builder print them.

String helpers, generated constants and raw `attr` values reach the same host
checks. Invalid options and references report their category, context and nearby
spellings. Raw source and text keys are checked too. Image names may be published
images, PNG resources, or instances created by `imageFrom`/`imageGraph`.
`stop` accepts a known action even if it has not run. A hide may refer to a
same-category instance declared later in the document; repeated cleanup while
inactive is valid. Unknown cleanup names fail. This forward-reference rule applies
only to cleanup: an image parameter update still needs an earlier instance.
Arrow and label instances share runtime names. Changing an instance's kind needs
the new kind's complete geometry (and text for a label); one cue cannot define
the same note twice. Hiding an old kind leaves a replacement of another kind alone.

Heading contracts are checked before converting attributes or translated IDs
to dictionaries. Chapter IDs and beat IDs must each be unique throughout an
edition. Chapter numbers are unique positive whole numbers; they need not be
consecutive. A translation may omit chapter numbers, or repeat the English
number. Empty stories, chapters and beats, and English paragraphs without any
spoken letters or digits, fail before speech preparation.

| Heading | Functional attributes |
|---|---|
| English video | `title`, `subtitle`, `fps`, `width`, `height`, `locale`, `captionWidth`, and the pacing attributes below |
| English chapter | `number`, `target`, `summary`, `sources` |
| English beat | `kind`, `diagram`, `graph`, `focus`, `sources`, `hold` |
| Translated video | `title`, `subtitle`, `locale`, `glossary`, `captionWidth` |
| Translated chapter | `number`, `summary` |
| Translated beat | Text and ID; visual settings come from English |

The English video declares `fps`, `width` and `height`. FPS is a positive whole
number; dimensions use the CLI's even-size limits, 2 through 8192. All numeric
input must be finite; `locale` is `en` or `zh-CN`. `target`, `hold` and pacing values are nonnegative; zero
target adds no budget pauses, and zero hold adds no extra hold. Seconds must fit
the timeline's 64-bit frame representation at its FPS. Cue term numbers and
record indices are nonnegative whole numbers, without fractional truncation.
These checks do not clamp investigation values to dock slider ranges.

`captionWidth` is a whole number, at least 4 English characters or 3 Chinese
display units, within the platform's integer index range. The checker also
rejects widths that cannot fit an indivisible token from the actual captions,
before caption splitting or audio preparation can fail or stall.

Repeated heading attributes are errors, including metadata. Unknown names get
a spelling suggestion where available. The archival fields `engineVersion`,
`commit` and `date` remain registered metadata; custom annotations use `meta.*`
(for example `meta.review=approved`). They are retained on the parsed heading
record and do not affect timing. Cue attributes keep their existing source
order: a later baseline override still replaces an earlier value.

`--json` writes a versioned `renderLabStoryCheck: 1` report on stdout and exits
nonzero on an error. Diagnostics include a stable code, severity, field and
message, beat/paragraph/piece where known, and the complete available source
range. A container without its own IR location reports an explicit fallback;
missing columns are left unknown. The checker stops at the first failure.
Duplicate declarations also include the first location in `related`; heading
locations refer to the positioned heading text because IR has no attribute spans.
Older host compilation failures may supply only a rendered diagnostic; the
report then preserves that message and identifies its location as a fallback.

Every normal build runs semantic preflight before selecting a narration device
or synthesizing speech. After timing, it runs the host's complete timeline
validation before writing prepared outputs, including for `--prepare-only`.
The timeline's existing `source` remains its beat ID; the separate, versioned
`timeline.provenance.json` beside it maps keys to their source ranges without
changing playback data. Validate a directly supplied timeline without rendering:

```sh
target/release/render-lab --investigation godot/ssr story check --timeline path/to/timeline.json
```

The machine interface `story check FILE` accepts `{renderLabStory: 1, cues: [...]}`
from `preflight.normalized`, or `-` for JSON on stdin. With `--timeline` it accepts
a raw timeline or `{timeline: ..., provenance: [...]}`; the latter supplies source
locations for failures. Neither form executes script actions or playback effects.

A change to a storyboard's form or to the pipeline should change nothing a video says or shows. `compare.py` checks it:
save every edition's prepared outputs before the change, prepare them again after, and compare (timelines by value,
captions and documents byte for byte, the narration track by hash, and no clip synthesized anew):

```sh
python3 tools/video/compare.py save /tmp/before        # after build.py --all --prepare-only
python3 tools/video/compare.py /tmp/before /tmp/after   # after the change, the same build, and a save to /tmp/after
```

### Lamlet consumer regression

`regression.py` makes that comparison a strict gate for the public SSR, SDFGI, VoxelGI and O3DE Lighting
stories, English and every declared translation. It runs the production preparation pipeline on cached speech
and measured word timings, checks each investigation's live scripts, and saves the document IR as well as every
prepared output. Missing editions, files, audio or word timings fail the capture; a partial capture cannot pass
comparison. Cached WAV and timing sidecars are hashed, so changing their contents under the same name is detected.
Every capture prepares into a fresh directory, so old outputs cannot hide a missing new artifact, and existing
video and timeline builds are preserved.

Build the pinned host and its package libraries, and prepare the narration caches once with the normal builder
(`--prepare-only --tts-device cpu`, English before zh-CN for each investigation). The gate itself never synthesizes
speech or renders frames. It needs the video Python environment and voice's phonemizer from `setup.sh`, even
with complete caches. Then, from the same content checkout:

```sh
python3 tools/video/regression.py save /tmp/lamlet-before
python3 tools/video/regression.py save /tmp/lamlet-after --binary /path/to/candidate/target/release/render-lab
python3 tools/video/regression.py compare /tmp/lamlet-before /tmp/lamlet-after
```

Build the candidate and **all its matching public package libraries** in a disposable source checkout. To test a
local Lamlet revision, patch both `lamlet` and `lamlet-doc` from `https://github.com/lamlet/lamlet` to that revision
only in that disposable workspace, resolve its temporary lock offline, then build with `--release --locked --offline`.
Keep this checkout's manifests, lock and vendor unchanged. A candidate is a host build, not a standalone `lamlet`
executable: the host's natives, conversions and presentation extension are part of the contract being checked.

Capture directories must be new. `--investigation godot/ssr` (repeatable) selects a smaller corpus; both captures
must select the same cases and languages. Each capture has `outputs/`, per-command `logs/`, and a completion
`manifest.json` recording the corpus, artifact hashes, host hashes and lock hash. Input hashes detect content
changes between captures; changing the host is expected. Comparison exits nonzero on any missing or changed output.
Run the gate in CI where its builds and narration cache are available. Test the gate itself with:

```sh
python3 -m unittest tools/video/test_markup.py tools/video/test_presentation.py tools/video/test_compare.py tools/video/test_regression.py tools/video/test_preflight.py tools/video/test_structure.py
```

This gate proves real-document compilation and preparation compatibility, including script hook discovery. It
does not execute live hooks or prove their determinism, failure recovery, final GPU appearance or render timing;
those need runtime and rendering tests separately.

## Inputs

An investigation keeps its video in its package, `packages/<package>/video/` (the builder finds the `video.json` whose
`investigation` names it):

- **`video.json`**: paths (storyboard, translation, diagrams, source index, pseudo-code, timeline directories, and
  optionally `build` per language and `audioCache`), output names, the engine label, the duration the verification expects, the reading path, `pronunciations` (how words are
  spoken; captions keep the written form), and the documents' texts per language.
- **`storyboard.lam`**: the chapters, beats, and narration as a Lamlet presentation (the document format of
  lamlet-present, read with the Lamlet the lab pins: `render-lab lamlet ir`, `lamlet.py`; `presentation.py` turns it
  into the board). Its first line imports `std`, `content`, `present`, and the investigation's cue vocabulary, named
  after its package (`$[Import* godot_voxelgi]`; also `render_lab`, for numbers said in words). The level-1 heading
  is the video, its text the title and its attributes the rest of the header: `# How VoxelGI Lights a Room {#video
  subtitle="…" fps=30 width=1920 height=1080}`. `## Title {#c11 number=11 target=150 summary="…" sources="a, b"}`
  starts a chapter (`target` in seconds), `### Headline {#beat-id kind=3d}` a beat, its scene's type `3d` (the lab's
  3D frame) or `2d` (a page in its place: the image `diagram=` names, the frame graph `graph=` names, with its
  `focus="A, B"` nodes, or without either the sources card of its `sources=`), with `diagram`, `graph`, `focus`,
  `sources`, `hold` attributes (a quoted value may hold spaces, not quotes or braces). Only `3d` and `2d` are
  accepted as scene types in storyboards and timelines; a paused timeline keeps a 2D scene on screen. Before the
  first sentence, a paragraph of cues sets the beat's keys, one per cue. Then each paragraph is one sentence: its
  prose is CommonMark (`$` is written `$$`), with its cues (below); a cue before its first word fires at the
  sentence's start. A cue's attributes are the investigation's vocabulary: `render-lab --investigation <path> story
  keys` lists them, and the builder writes them as typed functions to the package's `build/<package>.lam` before it
  reads the storyboard, so a misspelled key is an unknown name and a wrong value a type error, at the storyboard's
  line. Each is named after its key (`camera.yaw` is `{cameraYaw 24.0}`), but `code` is `listing`, `image` is
  `inset`, `animate` is `motion`, and `ease` is `easing`; a key with options has one function per option too
  (`panelCode`, `demoRoom`, `imageCornerTopLeft`), and `{attr "key" "value"}` writes any key as text. The keys:
  shot parameters (a name a cue reserves, such as `formula`, is `param.formula`, `{paramFormula 1.0}`; `render-lab
  --investigation <path> values` lists every key, the shaders' tagged values and their components included; a value
  is a number, written as a Float whatever its type, since the timeline blends every value, a vector,
  `{lightPosition {' 0.4 1.9 0.8}}`, or a choice's option by name, `{viewModeNdotl}`, which the lab checks against
  the value's declaration when it loads the timeline), `demo`, `camera.target` (a vector), `camera.yaw` (`pitch`,
  `distance`, `fov`; `{cameraContinuous}` keeps the camera through a `demo` switch), `freeCamera.on` (`{freeCameraOn
  1.0}`: the scene seen from the free camera, while the demonstration keeps following the main camera, drawn with its
  view cone and with its own view in a corner) placed by `freeCamera.target`, `freeCamera.yaw` (`pitch`, `distance`,
  `fov`; `cameraDrift` sways it instead of the main camera while it is on, and a `demo` switch turns it off),
  `record` (`"object:3"` or `"camera"`), `highlight` (`"a,b"`), `panel` (`declaration`, `values`, `memory`, `code`,
  `performance`: the GPU time per pass, measured as the frame renders, with `highlight` naming the passes to call
  out, `hidden`), `code` (`listing`), `callout`, `run` with `run.<arg>` and `stop` (the package's actions,
  `render-lab actions`), `image` (`inset`) with `imageTitle`, `imageCorner`, `imageFrom` and `image.<param>`,
  `imageGraph` and `imageShow`, and `imageHide` (images, `render-lab images`, `render-lab graph`), `blend`, `delay`
  (seconds), `ease` (`easing`: `smooth` by default, `linear`, `in`, `out`, `inOut`, or `sine`), and `animate`
  (`motion`), motions as Data (render_lab's `motion`, read by `markup.motion_of` into the timeline's form below; a raw
  `{attr "animate" "…"}` may still give them as a JSON String; `lab/src/animation.rs`), each driving its channels until a later key sets
  them; a channel is a shot parameter or `camera.x`, `camera.y`, `camera.z` (the target), `camera.yaw`,
  `camera.pitch`, `camera.distance`, or `camera.fov` (or the same of `freeCamera`):
  - `{"path": {"params": [...], "through": [[...], ...], "seconds": ...}}` runs from the channels' values through the
    waypoints on a smooth curve, without stopping at each one (`ease` paces the whole curve; `closed` returns to the
    start);
  - `{"orbit": {"params": [x, y] or [x, y, z], "center": [...], "degrees": ..., "seconds": ...}}` turns a point about
    `center` (two channels from the first towards the second, three about `axis`, +y by default), from `start` or
    the channels' values;
  - `{"wave": {"param": ..., "amplitude": ..., "period": ...}}` adds a sine about `center` (the channel's value by
    default), with `phase` in degrees, and `seconds` and `fade` to stop it smoothly.

  Each takes `ease` and `repeat` (`once`, `loop`, `pingPong`). A motion's `seconds` may be `"sentence"` or `"beat"`: it
  then runs until its sentence (or beat) ends, as "Where `sentence` and `beat` end" below says. A `demo` key resets
  every channel to the demonstration's state, so it cuts a running motion. Two motions of one key may not drive the
  same channel (`story.motion.duplicate`), nor one motion name a channel twice; a later key taking a channel over is
  ordinary. A motion's numbers must be finite (`story.motion`). Chapters are stretched to their `target` with extra pauses (the build reports each chapter's length, budget, natural length, and any narration over budget as soon as speech is measured, and stops before writing the timeline when a chapter is more than 10% from its budget, which the video's verification would refuse); keep the budget near 1.2 times
  the narration.
- **`diagrams/*.toml`**: the diagrams (`tools/video/diagrams.py` has the format): boxes, with rows if any, dashed or
  not, an image (`image = "<png key>"`: a beat that shows a picture is a diagram of one node), and edges, labelled or
  dotted; their text is keys of the package's text, so one file serves every edition. The
  lab draws them with its shared graph view (`lab/src/graph_view.rs`); `focus` names the lit boxes or rows. Two beats
  in a row of one diagram (in a chapter) run on without a fade: the diagram moves from the first's focus to the
  second's, its boxes gliding and its new pictures growing in; a diagram's first beat builds it up, layer by layer,
  each edge drawn along its path.
- **`graphs/frame.toml`** (in the package; a beat's `graph=frame`): the frame graph as a 2D scene, which the lab draws from its live render
  graph (`tools/video/frame_graph.py` has the format): sections of the frame's passes (or resources), each a box a
  beat's `focus` names (or one of its passes or resources, whose row lights), edges where one section's passes write
  what another's read; dashed, a section the lab's frame does not run (built offline, once, or on the CPU). Titles are
  keys of the package's text (a newline breaks one). The lab renders the beat's 3D frame unseen, and a lit box holds
  a picture of what it makes (its last pass's result, or the section's `picture = "<pass>.<resource>"`), the view
  kept at the lab's minimum text size on the lit boxes when the pictures do not fit. The lab checks it when it loads
  the timeline: every pass in a section, and the passes, resources, and pictures it names.
- **`story.lam`** (optional): the package's story library, the Lamlet package `<package>_story`, which the storyboard
  imports: the cue groups it repeats (a beat's `opening` keys, a lighting baseline, camera waypoints), bound once. It
  imports the vocabulary and `render_lab` as the storyboard does.
- **`storyboard.zh-CN.lam`**: the translation, a presentation with the same headings by id (its own title, chapter
  titles, summaries, and headlines; the video's `subtitle`, `locale`, `glossary`, comma-separated, and `captionWidth`;
  a chapter's number is the English's) and one paragraph per English sentence, with its formulas (`${math "…"}`) and
  code names (`${code "…"}`) and no cues. It imports only `std` and `content`.
- **Pacing**: the video heading's `lead` (0.4 s of silence before a beat's first sentence), `gap` (0.32 s between
  sentences), `tail` (0.7 s after the last), `maxExtraPause` (2.2 s, the most a chapter's budget lengthens one pause),
  `taggedWeight` (2.0, how much more of it a sentence with cues takes), `termFade` (0.4 s), `captionBridge` (0.6 s,
  a caption stays up through a gap this short), and `captionWidth` (58 characters a line; the translation's heading
  gives the zh-CN edition's, 64 display units by default), each with that default when left out (`timeline.Pacing`).

### Where `sentence` and `beat` end

A sentence is one paragraph of the beat (tools/video/timeline.py `build_timeline`). Its cues start on the frame
their word starts; a `"sentence"` motion runs from there to the **next sentence's start**, which is not where the
voice stops:

| Boundary | Frame |
|---|---|
| The sentence's speech ends | its start + the measured speech |
| A `"sentence"` motion ends (not the last sentence) | the next sentence's start: speech end + `gap` + the extra pause the chapter's budget gives it |
| A `"sentence"` motion ends (the last sentence) | the beat's end |
| A `"beat"` motion ends, and the beat ends | the last speech end + `tail` + the beat's `hold` (and its share of the chapter's leftover budget, for a 3D beat) |

So changing the pacing or a chapter's `target` changes how long a `"sentence"` motion runs; a motion that must end
with the voice gives its `seconds`. The keys of a paragraph of cues alone (the beat's `keys`) start at the beat's
start and know only `"beat"`.

### What a key resets and carries

A key sets only what it names. A `demo` key first starts the demonstration's state, then applies the rest of the key
(lab/src/shot.rs `Timeline::state_at`):

| State | A `demo` key | Cleared by |
|---|---|---|
| Parameters, running motions | reset to the demonstration's | a later key |
| Camera | reset to the demonstration's, unless the key moves the camera: then it blends from where it was | a camera cue |
| Panel, listing, record, highlight | reset to the demonstration's | `panel…`, `code`, `record…`, `highlight…` |
| Free camera, running actions | off, stopped | `freeCameraOn`, `{stop "…"}` |
| Callout | **carried** | `{callout ""}` |
| Images (insets) and their marks | **carried** | `{inset ""}` (all), `imageHide` (one) |
| Arrows and labels | **carried** | `arrowHide`, `labelHide` (by name) |
| Formula card | **carried**; closed at the end of the beat that showed it | a formula cue with `formula=off` |

A story's `opening` (its `video/story.lam`) is its own recipe on top of this; render_lab's `narrationCleared` clears
the three carried items that can be cleared at once (callout, insets, formula card), to append where a beat should
start clean.

### Text-driven shots: cues in a sentence

A sentence holds cues, `present`'s functions with the vocabulary's attributes and render_lab's typed ones
(`packages/render_lab/language/render_lab.lam`; `presentation.py` reads them): a point cue, `${cue "key" {' {opacity
1.0} {callout "source-key"}}}`, and a range over words, `${overText {' {radiusRange 0.0 2.0}} "step by step"}` (or
`over` with content: `{cat {' {text "a "} {cue "key" …} …}}`, for cues and ranges inside it; `present`'s `during` and
`duringText` name the range, which nothing needs). render_lab checks the names the lab knows when the storyboard is
read, through the lab's natives (`rl`, `lab/src/script/natives.rs`, with `render-lab --investigation <id> lamlet ir`):
a callout's source-index key, a text key (`imageTitle`, `imageMarkText`, `labelText`), a published point, and a
formula's LaTeX, so a wrong one fails at its line before any speech. A package's code can give its narration numbers
(`Investigation::script_natives`, the package `<package>_natives`): `${spoken {dispatches}}`. They are cut out of what is said and shown, and each takes effect on the frame where the word
after it starts. The voice's model gives every phoneme's duration on the way to the audio, so word times are exact to
the sample (`narration.py`; each wav has a `<digest>.json` with its word starts). The cues' attributes become keys as
`markup.py` reads them (tests: `python3 -m unittest tools/video/test_markup.py tools/video/test_presentation.py`).

| Cue attributes | Do |
|---|---|
| `{opacity 1.0} {callout "source-key"}` | A point cue: shot parameters, `callout`, `listing` (`code`), `panel`, `demo`, `{highlight {' "tag" "pass"}}` (`{highlight {'}}` clears it), `recordCamera`, `{recordObject 3}`, `recordNone`, `easing` (`ease`), `blend` (seconds), `term` |
| `{motion {' {cameraPath {' {' 18.0 16.0 6.2} {' 8.0 22.0 5.8}}}}}`, `{motion {' {path {' lightX lightY} {' {' 0.0 1.0}} sentence} {sway cameraYaw 6.0 24.0}}}` | Motions a key starts (`animate`, lab/src/animation.rs): a `path` of channels (the vocabulary's own functions) through waypoints, an `orbit` about a centre by degrees, or a `sway` (a sine wave) of one channel, for the cue's `beat`, its `sentence`, or `{seconds 4.0}`. Wrappers add the rest: `eased easingSine`, `looping`, `pingPonging`, `closed` (a path), `around`, `from` (an orbit), `centered`, `phased`, `lasting`, `fading` (a wave). `cameraPath` and `cameraTurn` are the camera's yaw, pitch, and distance (or yaw and pitch) over the beat, `cameraPathOver` and `cameraTurnOver` over a `Length`; their waypoints may be named, `{pose 18.0 16.0 6.2}` and `{aim 18.0 16.0}`. The commonest motion, one channel to one value, is a whole attribute: `{glide cameraYaw 24.0 sentence}` |
| `{inset "floodSeeds"} {imageCornerTopLeft} {imageTitle "demos.seeds"}`, `{inset ""}` | An image in an inset, by key: one the investigation publishes (`render-lab images`; its own title and axes), or a PNG resource of the package (`test/globe`) or one an imported package exports (`render_lab.<key>`), at its own size. It fades in and out over the key's `blend`; `imageCorner` is `top-left`, `top-right` (the default), `bottom-left`, or `bottom-right`; `imageTitle` is a text key of the package. The lab checks the key when it loads the timeline |
| `{inset "maps"} {imageFrom "probeMap"} {imageProbe {' 7.0 7.0 8.0}}`, `{imageHide {' "maps"}}` | A GPU image: an instance, named by the story, of an image kind the package's shaders define (`lab/src/views.rs`; `render-lab images` lists the kinds and their parameters, `image.<param>`, written `imageParam…` where a field has the name). The lab runs the kind's compute entry point into the instance's own texture every frame and shows it live. A later `inset` of the name sets its parameters (blended over `blend`), title, or corner; several images show at once; `imageHide` fades some out and `{inset ""}` all. The lab checks the kind and each parameter when it loads the timeline |
| `{inset "lights"} {imageGraph "o3deLightCullingCS.tileData"} {imageShowCount} {attr "image.range" "0,16"}` | A resource of the frame's graph as it stood right after a node (`lab/src/graph.rs`; `render-lab graph` lists the nodes and what each reads and writes): the lab draws it right after that node, whatever later nodes write over it, with the visualizer `imageShow` names (`color`, `unorm`, `normal`, `depth`, `ramp`, `signed`, `count`, `float`, `hash`; the format's default without one) and its parameters (`exposure`, `channel`, `encoding`, `average`, `range`, `slice`, `level`, `axis`, `word`; written with `attr`, as the vocabulary has only image kinds' parameters). The lab checks the node, the resource, the visualizer, and each parameter when it loads the timeline |
| `{inset "t0"} {mark 0.0 0.0 0.5 0.5} {imageMarkText "views.near"} {blend 1.0}`, `unmark` | A region of a shown image pointed out while a sentence explains it: `u0 v0 u1 v1` are fractions of the picture from its top left; the region is outlined in the accent colour and the rest of the picture dimmed, with an optional label (`imageMarkText`, a text key of the package; `""` for none). It fades in over `blend` where it points, moves to a later `mark` of the same image, and `unmark` fades it out |
| `{arrow "toWall"} {arrowFrom {at -2.2 0.3 -2.2}} {arrowTo {point "field.seed"}} {arrowColor "#50d7ff"} {arrowWidth 3.0}`, `{label "hot"} {labelAt {at 0.0 1.0 0.0}} {labelText "demos.key"}`, `{arrows "probe.rays"}` | Arrows and labels the story declares (`packages/render_lab/src/annotations.rs`), drawn over the frame: a point is `{at x y z}` (blended over `blend` when a later key moves it) or `{point "key"}`, one the package publishes (`render-lab images` lists them), so an arrow can follow what it computes; `arrows` shows an arrow set the package publishes. A later `arrow`/`label` of the name changes it; they fade over `blend`; `{arrowHide {' "a"}}`, `{labelHide {' "l"}}` hide some, `{arrow ""}` and `{label ""}` all. The lab checks every point, set, colour, and text key when it loads the timeline |
| `{inset "map"} {imageClick "pickCell"}` | A click on the image's picture, in the viewer while the timeline is paused, runs the package's click handler (a script's, or `#[click]` in `lab/src/actions.rs`; `render-lab actions` lists them) with the point clicked. The lab checks the handler when it loads the timeline |
| `${code "LightCullingTilePrepare"}`, `${say {code "r.Example.MaxIntensity"} "max ray intensity"}` | A code name (not a cue): the caption (and any on-screen text) draws it in the monospace font on a soft chip, the voice says it as written (or as `say` gives it), and documents and subtitles show its text. A translation carries the same names. Wrap every function, struct, file, and console variable the narration names |
| `{run "jumpFlood"} {runSeconds 0.5}`, `{stop "jumpFlood"}` | An action of the package (a script's, or `#[action]` in `lab/src/actions.rs`; `render-lab actions` lists them): `run` starts it with its arguments (`run.<arg>`; a later `run` of it restarts it), `stop` stops it, and a `demo` switch stops every one. While it runs, it sets the shot's values from the seconds since it started, after every key; the lab checks its name and arguments when it loads the timeline |
| `${overText {' {radiusRange 0.0 2.0}} "…"}`, `${overCallout {' {radiusRange 0.0 2.0}} "source-key" "…"}` | A range, inside one sentence (`overCallout`: its words start with a callout): `xRange a b` jumps to `a` where it opens and blends to `b` exactly where it closes (a plain value blends from the current one) |
| `${formulaSaid "\\frac{1}{1+\\lit{1}{n}}" "one over one plus n" {'}}` | A formula: the cue opens the formula card in the viewport on it (it closes with the beat, or at `{formulaOff}`); the caption shows what `say` shows, typeset, and the voice says its words. `{display "yes"}` sets it in display style on the card, `{colors "#rrggbb,…"}` colours its `\lit{n}{…}` terms, `{card "no"}` keeps it to the caption (each in the last argument's list); in a String, LaTeX's backslashes are doubled. It is `${cue "formula" {' {formula "…"} …}}${say {math "…"} "…"}`, the LaTeX written once |
| `{term 1}` | Lights term `n` of the card (it fades in over `blend`, 0.4 s by default; `{term 0}` unlights it) |

Captions are cut at word boundaries, never inside a formula, and each one shows from its first word; the `.srt` and
`.vtt` files and the script write formulas as `$LaTeX$`. A translation's sentence carries the same formulas (the same
LaTeX, as many times) and no cues. The build typesets every formula of the timeline with `render-lab formulas
--check` (glyphs of at least 24 px, display formulas at most 546 px wide).

The lab draws formulas itself (lab/src/formula.rs: MiTeX translates the LaTeX into Typst math, and Typst typesets it),
in the captions and the formula card, and in its text tables, as the rich-text tag `{@ formula="…" display=yes
colors="…"}`, as O3DE's readout formulas do.

## Outputs

- the package's `build/timelines/<lang>/`: `timeline.json` and the diagram frames, which the homepage's Watch
  and `render-lab render` play. They are generated, so git ignores them: in a fresh checkout, `--prepare-only` builds
  them (the homepage shows "Not built yet" until then).
- the package's `build/video/<lang>/`: the video, `narration.wav`, captions, `chapters.txt`, `script.md`,
  `source-reading-guide.md`, `preview.html`, `NOTICE.txt` (the package's `[[notice]]`s and each one's license
  text), `contact-sheet.png`, `poster.png`, `snapshots/`, logs, and `verification-report.{json,md}` (git-ignored).
  A package with notices gets a closing notices card (a silent 2D beat, `timeline.py` `NOTICES` seconds) at the end of
  its last chapter, left out of that chapter's budget. The narration cache is shared by the editions in `audio/`.

## Environment

`setup.sh` creates a Conda prefix (Python 3.11, FFmpeg) in `.cache/video-env`, installs Piper 1.3.0 and ONNX (for the voice's
phoneme durations), and downloads the `en_US-lessac-medium` voice into `.cache/video-voices`; `--gpu` adds the CUDA build of
ONNX Runtime and cuDNN in `.cache/video-tts-gpu` (the CUDA 13 runtime comes from the system). To reuse an existing
install, point `RENDER_LAB_VIDEO_ENV`, `RENDER_LAB_VOICES`, and `RENDER_LAB_TTS_GPU` at it. `build.py` re-runs itself under the environment's Python.

## Verification

The build checks the file (30 fps H.264 at the build's `--resolution`, 1080p by default; BT.709, AAC), its duration against the timeline and the expected range, the
frame count, each chapter within 10% of its budget, a full decode, audio levels, caption line lengths, the render
report (text at least 24 px, labels inside the viewport and apart), diagram labels, the engine checkout's commit, every relative link in the documents, `render-lab selftest --timeline` (every scene beat
readable), and the Rust and WGSL layouts. A zh-CN build also checks that it shares the English narration and frames and
leaves the English edition unchanged.
