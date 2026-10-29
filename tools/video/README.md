# Narrated video builder

Builds an investigation's narrated video from its storyboard: Piper narration measured sentence by sentence, a timeline
that `render-lab render` records frame by frame, the muxed video with chapters, captions, a script, a source-reading
guide, review stills, and a verification report.

```sh
bash tools/video/setup.sh [--gpu]                                    # once: the local environment under .cache/
python3 tools/video/build.py --investigation o3de/lighting           # the English edition
python3 tools/video/build.py --investigation o3de/lighting --lang zh-CN   # Chinese text and subtitles, English narration
```

| Option | Does |
|---|---|
| `--prepare-only` | Narration, timeline, captions, and documents (about 15 s once the narration is cached) |
| `--snapshots-only` | Also the diagrams and one still per beat in the package's `build/video/<lang>/snapshots/` |
| `--frames A:B` | Renders only those frames (testing; no mux or verification) |
| `--skip-render` | Reuses the rendered `video.mp4` |
| `--verify-only` | Only the checks of an existing build |
| `--tts-device auto/cuda/cpu` | Where new narration is synthesised (`auto`: the GPU when the CUDA packages are installed) |
| `--investigation-dir DIR` | The directory of its `video/video.json`, when it is not its package in `packages/` |

A full build of the 16-minute O3DE video takes about 15 minutes on an RTX 4050, almost all of it rendering. Run one GPU
job at a time.

## Inputs

An investigation keeps its video in its package, `packages/<package>/video/` (the builder finds the `video.json` whose
`investigation` names it):

- **`video.json`**: paths (storyboard, translation, diagrams, source index, pseudo-code, timeline directories, and
  optionally `build` per language and `audioCache`), output names, the engine label, the duration the verification expects, the reading path, `pronunciations` (how words are
  spoken; captions keep the written form), and the documents' texts per language.
- **`storyboard.story`**: the chapters, beats, and narration as text, with the shots as tags (`story.py` reads it;
  LaTeX is written as is). `# 11 · Title · 150 s` starts a chapter (its `summary:` and `sources:` lines follow);
  `## beat-id · scene · Headline` a beat (`kind`: `scene`, `mermaid`, or `sources`; `diagram:`, `focus:`,
  `sources:`, `hold:` lines). Before the first sentence, a line of tags sets the beat's keys, and
  `{@define name …}` defines a named cue. Then each paragraph is one sentence, with its tags (below); a tag before
  its first word fires at the sentence's start, and `{@ pause=0.5}` at its end adds silence. A tag's attributes:
  shot parameters (a name a tag reserves, such as `formula`, is written `param.formula`; `render-lab --investigation
  <path> values` lists every key, the shaders' tagged values and their components included; a value is a number,
  `true` or `false`, a vector as numbers separated by commas, or a choice's option by name, `viewMode=ndotl`, which the
  lab checks against the value's declaration when it loads the timeline), `demo`,
  `camera.target=x,y,z`, `camera.yaw` (`pitch`, `distance`, `fov`; `camera=continuous` keeps the camera through a
  `demo` switch), `record=probe:67` (or `ray`), `highlight=a,b`, `panel`, `code`, `callout`, `run=action` with
  `run.<arg>=value` and `stop=a,b` (the package's actions, `render-lab actions`), `blend`, `delay` (seconds),
  `ease` (`smooth` by default, `linear`, `in`, `out`, `inOut`, or `sine`), and `animate=[…]`, motions as JSON
  (`lab/src/animation.rs`), each driving its channels until a later key sets them; a channel is a shot parameter or
  `camera.x`, `camera.y`, `camera.z` (the target), `camera.yaw`, `camera.pitch`, `camera.distance`, or `camera.fov`:
  - `{"path": {"params": [...], "through": [[...], ...], "seconds": ...}}` runs from the channels' values through the
    waypoints on a smooth curve, without stopping at each one (`ease` paces the whole curve; `closed` returns to the
    start);
  - `{"orbit": {"params": [x, y] or [x, y, z], "center": [...], "degrees": ..., "seconds": ...}}` turns a point about
    `center` (two channels from the first towards the second, three about `axis`, +y by default), from `start` or
    the channels' values;
  - `{"wave": {"param": ..., "amplitude": ..., "period": ...}}` adds a sine about `center` (the channel's value by
    default), with `phase` in degrees, and `seconds` and `fade` to stop it smoothly.

  Each takes `ease` and `repeat` (`once`, `loop`, `pingPong`). A motion's `seconds` may be `"sentence"` or `"beat"`: it
  then runs until its sentence (or beat) ends (`"range"` in a range: until the range closes). A `demo` key resets every channel to the demonstration's state, so it
  cuts a running motion. Chapters are stretched to `targetSeconds` with extra pauses; keep the budget near 1.2 times
  the narration.
- **`diagrams/*.mmd`**: the Mermaid diagrams (`focus` names the lit nodes).
- **`storyboard.zh-CN.story`**: the translation, with the same headings (`# 11 · 标题`, `## beat-id · 标题`; header
  lines `title:`, `subtitle:`, `locale:`, `glossary:`) and one paragraph per English sentence, with its formulas and no
  cues. (A storyboard or translation may still be JSON, as `story.py` writes them from; `.story` is read by its suffix.)

### Text-driven shots: tags in a sentence

A sentence's text may hold tags, `{@ name=value …}` (a value with spaces or braces is quoted, `"…"`, where only `\"` is
an escape, so LaTeX keeps its backslashes). They are cut out of what is said and shown, and each takes effect on the
frame where the word after it starts. The voice's model gives every phoneme's duration on the way to the audio, so
word times are exact to the sample (`narration.py`; each wav has a `<digest>.json` with its word starts). In a JSON
storyboard they work alongside `keys`, `sequence`, and `pause`; `markup.py` parses them (tests: `python3 -m unittest tools/video/test_markup.py`).

| Tag | Does |
|---|---|
| `{@ opacity=1 callout=source-key}` | A point cue: shot parameters, `callout`, `code`, `panel`, `demo`, `highlight=a,b`, `record=card:3`, `ease`, `blend=0.5` (seconds), `term=n` |
| `{@ run=jumpFlood run.seconds=0.5}`, `{@ stop=jumpFlood}` | An action of the package (`lab/src/actions.rs`; `render-lab actions` lists them): `run` starts it with its arguments (a later `run` of it restarts it), `stop` stops it, and a `demo` switch stops every one. While it runs, it sets the shot's values from the seconds since it started, after every key; the lab checks its name and arguments when it loads the timeline |
| `{@wide}` | The named cue `wide` from the sentence's or the beat's `cues` (a full key: cameras, records, motions) |
| `{@grow> radius=0..2}` … `{<grow}` | A range, inside one sentence: `a..b` jumps to `a` where it opens and blends to `b` exactly where it closes (`=b` blends from the current value). `{@name>}` alone blends the cue `name` over the range; its motions with `"seconds": "range"` run for it |
| `{@ formula="\frac{1}{1+\lit{1}{n}}" say="one over one plus n"}` | A formula: the caption shows it, typeset; the voice says `say`; the formula card in the viewport opens on it and closes with the beat (or at `{@ formula=off}`). `display=yes` sets it in display style on the card, `colors="#rrggbb,…"` colours its `\lit{n}{…}` terms, `card=no` keeps it to the caption |
| `{@ term=n}` | Lights term `n` of the card (it fades in over `blend`, 0.4 s by default; `term=0` unlights it) |

Captions are cut at word boundaries, never inside a formula, and each one shows from its first word; the `.srt` and
`.vtt` files and the script write formulas as `$LaTeX$`. A translation's sentence carries the same formula tags
(`formula`, `display`, `colors`; the same LaTeX, as many times) and no cues. The build typesets every formula of the
timeline with `render-lab formulas --check` (glyphs of at least 24 px, display formulas at most 546 px wide).

The lab draws formulas itself (lab/src/formula.rs: MiTeX translates the LaTeX into Typst math, and Typst typesets it),
so the same tag works in its text tables, `{@ formula="…" display=yes colors="…"}`, as O3DE's readout formulas do.

## Outputs

- the package's `build/timelines/<lang>/`: `timeline.json` and the diagram frames, which the homepage's Watch
  and `render-lab render` play. They are generated, so git ignores them: in a fresh checkout, `--prepare-only` builds
  them (the homepage shows "Not built yet" until then).
- the package's `build/video/<lang>/`: the video, `narration.wav`, captions, `chapters.txt`, `script.md`,
  `source-reading-guide.md`, `preview.html`, `contact-sheet.png`, `poster.png`, `snapshots/`, logs, and
  `verification-report.{json,md}` (git-ignored). The narration cache is shared by the editions in `audio/`.

## Environment

`setup.sh` creates a Conda prefix (Python 3.11, Node 22, FFmpeg) in `.cache/video-env`, installs Piper 1.3.0, ONNX (for the voice's
phoneme durations), Mermaid, and Puppeteer, and downloads the `en_US-lessac-medium` voice into `.cache/video-voices`; `--gpu` adds the CUDA build of
ONNX Runtime and cuDNN in `.cache/video-tts-gpu` (the CUDA 13 runtime comes from the system). To reuse an existing
install, point `RENDER_LAB_VIDEO_ENV`, `RENDER_LAB_VOICES`, `RENDER_LAB_NODE_MODULES`, `RENDER_LAB_TTS_GPU`, and
`RENDER_LAB_PUPPETEER_CACHE` at it. `build.py` re-runs itself under the environment's Python.

## Verification

The build checks the file (30 fps H.264 at the build's `--resolution`, 1080p by default; BT.709, AAC), its duration against the timeline and the expected range, the
frame count, each chapter within 10% of its budget, a full decode, audio levels, caption line lengths, the render
report (text at least 24 px, labels inside the viewport and apart), diagram labels, every cited line against the engine
checkout and its commit, every relative link in the documents, `render-lab selftest --timeline` (every scene beat
readable), and the Rust and WGSL layouts. A zh-CN build also checks that it shares the English narration and frames and
leaves the English edition unchanged.
