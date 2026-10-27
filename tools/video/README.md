# Narrated video builder

Builds an investigation's narrated video from its storyboard: Piper narration measured sentence by sentence, a timeline
that `render-lab render` records frame by frame, the muxed video with chapters, captions, a script, a source-reading
guide, review stills, and a verification report.

```sh
bash tools/video/setup.sh [--gpu]                                    # once: the local environment under .cache/
python3 tools/video/build.py --investigation o3de/lighting           # the English edition
python3 tools/video/build.py --investigation o3de/lighting --lang zh-CN   # Chinese text and subtitles, English narration
python3 tools/video/formulas.py --investigation o3de/lighting      # the readout's formula images (build.py runs it too)
```

| Option | Does |
|---|---|
| `--prepare-only` | Narration, timeline, captions, and documents (about 15 s once the narration is cached) |
| `--snapshots-only` | Also the diagrams and one still per beat in `build/video/<engine>/<feature>/<lang>/snapshots/` |
| `--frames A:B` | Renders only those frames (testing; no mux or verification) |
| `--skip-render` | Reuses the rendered `video.mp4` |
| `--verify-only` | Only the checks of an existing build |
| `--tts-device auto/cuda/cpu` | Where new narration is synthesised (`auto`: the GPU when the CUDA packages are installed) |

A full build of the 16-minute O3DE video takes about 15 minutes on an RTX 4050, almost all of it rendering. Run one GPU
job at a time.

## Inputs

An investigation keeps its video in `src/<engine>/<feature>/video/`:

- **`video.json`**: paths (storyboard, translation, diagrams, source index, pseudo-code, timeline directories), output
  names, the engine label, the duration the verification expects, the reading path, `pronunciations` (how words are
  spoken; captions keep the written form), and the documents' texts per language.
- **`storyboard.json`**: `chapters` (`number`, `title`, `targetSeconds`, `summary`, `sources`) of `beats` (`id`,
  `kind`: `scene`, `mermaid`, or `sources`, `headline`, `narration`, `keys`). A sentence is a string or
  `{text, keys?, sequence?, pause?}`. Keys apply at the sentence's start; `sequence` keys are spread over it; a key
  sets `demo`, `camera`, `record`, `highlight`, `panel`, `params`, `callout`, or `code`, with `delay` and `blend` in
  seconds, and `ease` for the blend (`smooth` by default, `linear`, `in`, `out`, `inOut`, or `sine`). A key's `animate`
  starts motions (`src/lab/animation.rs`), each driving its channels until a later key sets them; a channel is a shot
  parameter or `camera.x`, `camera.y`, `camera.z` (the target), `camera.yaw`, `camera.pitch`, `camera.distance`, or
  `camera.fov`:
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
  cuts a running motion. Chapters are stretched to `targetSeconds` with extra pauses; keep the budget near 1.2 times
  the narration.
- **`diagrams/*.mmd`**: the Mermaid diagrams (`focus` names the lit nodes).
- **`zh-CN/translation.json`**: one Chinese sentence per English sentence, per beat, plus titles and a glossary.

### Formulas

An investigation may list LaTeX formulas for its readout panel in `src/<engine>/<feature>/formulas/formulas.json`, as
`{key: latex}`. `\lit{n}{#rrggbb}{...}` marks term n: `formulas.py` writes `<key>.png` with no term lit and
`<key>-<n>.png` with the formula dimmed and term n in that colour (use the colour of the term's readout row).
`formulas.mjs` renders them with KaTeX in headless Chrome as transparent PNGs, one pixel per point at 1080p, and fails
on a glyph under 24 px (scripts included, so avoid nested scripts) or a formula wider than the panel (546 px; break it
over more lines with `aligned`). The PNGs and `manifest.json` (each image's source hash and its own hash) are
committed, so the lab builds without Node; only stale images are rendered again, `--check` fails on a stale one, and
the lab's `build.rs` fails when a formula's LaTeX or a PNG no longer matches the manifest. The investigation embeds the
PNGs (`include_bytes!`), returns them from `Investigation::formula`, and names one in `Readout::formula`; the zh-CN
edition uses the same images.

## Outputs

- `src/<engine>/<feature>/timelines/<lang>/`: `timeline.json` and the diagram frames. They are tracked, so the viewer
  plays them from any checkout.
- `build/video/<engine>/<feature>/<lang>/`: the video, `narration.wav`, captions, `chapters.txt`, `script.md`,
  `source-reading-guide.md`, `preview.html`, `contact-sheet.png`, `poster.png`, `snapshots/`, logs, and
  `verification-report.{json,md}` (git-ignored). The narration cache is shared by the editions in `audio/`.

## Environment

`setup.sh` creates a Conda prefix (Python 3.11, Node 22, FFmpeg) in `.cache/video-env`, installs Piper 1.3.0, Mermaid,
KaTeX, and Puppeteer, and downloads the `en_US-lessac-medium` voice into `.cache/video-voices`; `--gpu` adds the CUDA build of
ONNX Runtime and cuDNN in `.cache/video-tts-gpu` (the CUDA 13 runtime comes from the system). To reuse an existing
install, point `RENDER_LAB_VIDEO_ENV`, `RENDER_LAB_VOICES`, `RENDER_LAB_NODE_MODULES`, `RENDER_LAB_TTS_GPU`, and
`RENDER_LAB_PUPPETEER_CACHE` at it. `build.py` re-runs itself under the environment's Python.

## Verification

The build checks the file (1080p30 H.264, BT.709, AAC), its duration against the timeline and the expected range, the
frame count, each chapter within 10% of its budget, a full decode, audio levels, caption line lengths, the render
report (text at least 24 px, labels inside the viewport and apart), diagram labels, the formula images (current, glyphs
at least 24 px), every cited line against the engine
checkout and its commit, every relative link in the documents, `render-lab selftest --timeline` (every scene beat
readable), and the Rust and WGSL layouts. A zh-CN build also checks that it shares the English narration and frames and
leaves the English edition unchanged.
