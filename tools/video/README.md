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
| `--skip-render` | Reuses the rendered `video.mp4` |
| `--verify-only` | Only the checks of an existing build |
| `--tts-device auto/cuda/cpu` | Where new narration is synthesised (`auto`: the GPU when the CUDA packages are installed) |
| `--listing-style blocks/text` | How the code panel draws pseudo-code listings (`blocks`, the default, whatever the user's settings say) |
| `--investigation-dir DIR` | The directory of its `video/video.json`, when it is not its package in `packages/` |

A full build of the 16-minute O3DE video takes about 15 minutes on an RTX 4050, almost all of it rendering. Run one GPU
job at a time.

A change to a storyboard's form or to the pipeline should change nothing a video says or shows. `compare.py` checks it:
save every edition's prepared outputs before the change, prepare them again after, and compare (timelines by value,
captions and documents byte for byte, the narration track by hash, and no clip synthesized anew):

```sh
python3 tools/video/compare.py save /tmp/before        # after build.py --all --prepare-only
python3 tools/video/compare.py /tmp/before /tmp/after   # after the change, the same build, and a save to /tmp/after
```

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
  (`motion`), motions as a JSON String (`lab/src/animation.rs`), each driving its channels until a later key sets
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
  then runs until its sentence (or beat) ends. A `demo` key resets every channel to the demonstration's state, so it
  cuts a running motion. Chapters are stretched to their `target` with extra pauses; keep the budget near 1.2 times
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
| `{motion {' {cameraPath {' {' 18.0 16.0 6.2} {' 8.0 22.0 5.8}}}}}`, `{motion {' {path {' lightX lightY} {' {' 0.0 1.0}} sentence} {sway cameraYaw 6.0 24.0}}}` | Motions a key starts (`animate`, lab/src/animation.rs): a `path` of channels (the vocabulary's own functions) through waypoints, an `orbit` about a centre by degrees, or a `sway` (a sine wave) of one channel, for the cue's `beat`, its `sentence`, or `{seconds 4.0}`. Wrappers add the rest: `eased easingSine`, `looping`, `pingPonging`, `closed` (a path), `around`, `from` (an orbit), `centered`, `phased`, `lasting`, `fading` (a wave). `cameraPath` and `cameraTurn` are the camera's yaw, pitch, and distance (or yaw and pitch) over the beat |
| `{inset "floodSeeds"} {imageCornerTopLeft} {imageTitle "demos.seeds"}`, `{inset ""}` | An image in an inset, by key: one the investigation publishes (`render-lab images`; its own title and axes), or a PNG resource of the package (`test/globe`) or one an imported package exports (`render_lab.<key>`), at its own size. It fades in and out over the key's `blend`; `imageCorner` is `top-left`, `top-right` (the default), `bottom-left`, or `bottom-right`; `imageTitle` is a text key of the package. The lab checks the key when it loads the timeline |
| `{inset "maps"} {imageFrom "probeMap"} {imageProbe {' 7.0 7.0 8.0}}`, `{imageHide {' "maps"}}` | A GPU image: an instance, named by the story, of an image kind the package's shaders define (`lab/src/views.rs`; `render-lab images` lists the kinds and their parameters, `image.<param>`, written `imageParam…` where a field has the name). The lab runs the kind's compute entry point into the instance's own texture every frame and shows it live. A later `inset` of the name sets its parameters (blended over `blend`), title, or corner; several images show at once; `imageHide` fades some out and `{inset ""}` all. The lab checks the kind and each parameter when it loads the timeline |
| `{inset "lights"} {imageGraph "o3deLightCullingCS.tileData"} {imageShowCount} {attr "image.range" "0,16"}` | A resource of the frame's graph as it stood right after a node (`lab/src/graph.rs`; `render-lab graph` lists the nodes and what each reads and writes): the lab draws it right after that node, whatever later nodes write over it, with the visualizer `imageShow` names (`color`, `unorm`, `normal`, `depth`, `ramp`, `signed`, `count`, `float`, `hash`; the format's default without one) and its parameters (`exposure`, `channel`, `encoding`, `average`, `range`, `slice`, `level`, `axis`, `word`; written with `attr`, as the vocabulary has only image kinds' parameters). The lab checks the node, the resource, the visualizer, and each parameter when it loads the timeline |
| `{inset "t0"} {mark 0.0 0.0 0.5 0.5} {imageMarkText "views.near"} {blend 1.0}`, `unmark` | A region of a shown image pointed out while a sentence explains it: `u0 v0 u1 v1` are fractions of the picture from its top left; the region is outlined in the accent colour and the rest of the picture dimmed, with an optional label (`imageMarkText`, a text key of the package; `""` for none). It fades in over `blend` where it points, moves to a later `mark` of the same image, and `unmark` fades it out |
| `{arrow "toWall"} {arrowFrom {at -2.2 0.3 -2.2}} {arrowTo {point "field.seed"}} {arrowColor "#50d7ff"} {arrowWidth 3.0}`, `{label "hot"} {labelAt {at 0.0 1.0 0.0}} {labelText "demos.key"}`, `{arrows "probe.rays"}` | Arrows and labels the story declares (`packages/render_lab/src/annotations.rs`), drawn over the frame: a point is `{at x y z}` (blended over `blend` when a later key moves it) or `{point "key"}`, one the package publishes (`render-lab images` lists them), so an arrow can follow what it computes; `arrows` shows an arrow set the package publishes. A later `arrow`/`label` of the name changes it; they fade over `blend`; `{arrowHide {' "a"}}`, `{labelHide {' "l"}}` hide some, `{arrow ""}` and `{label ""}` all. The lab checks every point, set, colour, and text key when it loads the timeline |
| `{inset "map"} {imageClick "pickCell"}` | A click on the image's picture, in the viewer while the timeline is paused, runs the package's click handler (`#[click]`, `lab/src/actions.rs`; `render-lab actions` lists them) with the point clicked. The lab checks the handler when it loads the timeline |
| `${code "LightCullingTilePrepare"}`, `${say {code "r.Example.MaxIntensity"} "max ray intensity"}` | A code name (not a cue): the caption (and any on-screen text) draws it in the monospace font on a soft chip, the voice says it as written (or as `say` gives it), and documents and subtitles show its text. A translation carries the same names. Wrap every function, struct, file, and console variable the narration names |
| `{run "jumpFlood"} {runSeconds 0.5}`, `{stop "jumpFlood"}` | An action of the package (`lab/src/actions.rs`; `render-lab actions` lists them): `run` starts it with its arguments (`run.<arg>`; a later `run` of it restarts it), `stop` stops it, and a `demo` switch stops every one. While it runs, it sets the shot's values from the seconds since it started, after every key; the lab checks its name and arguments when it loads the timeline |
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
