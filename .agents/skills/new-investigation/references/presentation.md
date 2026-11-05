# Presentation: Play, actions, images, clicks, and the narrated video

Load this when giving an investigation its Play animation, actions, published images, annotations, click handlers, or
GPU image kinds, or when setting up or building its narrated video. What the story says and shows is the `new-story`
skill's; the storyboard's syntax and cue vocabulary are the `render-lab-lamlet` skill's (which also covers the same
hooks written as Lamlet scripts instead of Rust).

## The Play animation

A demonstration's Play animation is a `Clip` (`lab/src/animation.rs`) of looping motions: a `Wave` (a sine about a
centre), an `Orbit` (a point turned about a centre; a `pingPong` of `Ease::Sine` swings it back and forth), or a `Path`
through waypoints (`closed` with `Repeat::Loop` goes round). `play_clip(demo)` builds it, and
`demo_animation(demo, t) -> Vec<(param, value)>` samples it `t` seconds after Play for the trait's `animation`
(`lab::utility::demo::animation(demos(), &play_clip(demo), t)`). The viewer adds the camera's slow sway, the sliders
follow, and stopping leaves the values in them. Sweep the parameter the demonstration is about (O3DE: the light circles
the room; roughness sweeps through the cubemap's mips). `backdrop(t)` can reuse it for the homepage's scene. A
demonstration's file may also name a script animation (`animation = "playSurface"`), applied after the code's.

## Actions

A story drives the package through actions. Write one as a script (`render-lab-lamlet`: a package's `script/<name>.lam`,
a binding of type `{Action Args}`, `Args` a struct of its arguments, optional ones `Option`), as Godot SDFGI's
`jumpFlood` (`packages/godot_sdfgi/script/logic.lam`) and VoxelGI's `marchCone` are: it sets the shot's values from
`elapsed`, the seconds since the story started it, so any frame still renders on its own (required: no state between
frames). Only work too heavy for a script stays native (`lab/src/actions.rs`: `#[lab::actions]` on an `impl` block of
the investigation's type, one `#[action]` function each, its arguments' kinds read from their types, returned from
`Investigation::actions`). Test either through a timeline in the selftest (SDFGI's `jumpFlood` check). The story's cues
write `{run "jumpFlood"} {runSeconds 0.5}` and `{stop "jumpFlood"}` (`render-lab actions` lists them).

## Images and annotations

A picture the story shows is an image by key (`packages/render_lab/src/inset.rs`). Declare the ones the package
computes in `Investigation::images` (`ImageSpec { key, doc }`), and publish each in `FrameContent::images` as a
`PictureInset` (its pixels, title, axes, marker) only in the frames whose state shows it (`state.shows_image(key)`);
Godot's `field_demo` publishes its slices so. A PNG needs no code: a `file` resource of the package is an image by its
key. The story's cues write `{inset "key"} {imageCornerTopLeft}` and `{imageHide {' "key"}}`, and choose the corner and
the title.

A story's arrows and labels name points the package publishes (`lab/src/frame.rs`, `FrameContent::points`) and arrow
sets it computes (`FrameContent::arrow_sets`, only while `state.shows_arrows(key)`), declared in
`Investigation::annotations` (`AnnotationSpec`): publish what an arrow would follow (a hit, a seed, a probe) rather than
drawing a fixed annotation in Rust. Godot's `field_demo` publishes `field.worked` and `field.seed`. `render-lab images`
lists the images, image kinds, points, and sets.

## Clicks on images

A click on an inset never reaches the scene behind it. What a click on an image's picture does is a click handler, a
`#[click]` function in the `#[lab::actions]` block, `fn pick_cell(frame: &mut ClickFrame) -> bool` (the state on
screen, the state it sets, and `frame.uv`, the point clicked, 0..1 with y down), or a script's click handler. The story
gives an image one, `{inset "map"} {imageClick "pickCell"}`; an inset the package draws names one in
`PictureInset::click`. A click happens while no timeline drives the state (a demonstration, or a timeline paused).

## GPU image kinds

A GPU image is an image kind a shader of the package defines (`lab/src/views.rs`, Godot's `shaders/sdfgi_views.wgsl`
in `packages/godot_sdfgi`): a `texture_storage_2d<rgba8unorm, write>` at `@group(3) @binding(0)` tagged
`/// {@ view=probeMap entry=<compute entry> size=w,h text=<title key>}`, and its parameters' uniform at
`@group(3) @binding(1)` tagged `/// {@ view_params=probeMap}`, its members tagged `{@ key=probe default=7,7,8
range=0,16}`. The entry point runs with the frame (group 0) and the scene shading (group 1) bound, one invocation per
texel, and writes display colours. A kind that pictures the screen adds `aspect=viewport`: its texture then takes the
viewport's shape (the longer side of `size` kept), so a tall viewport gives a tall picture. No Rust: the story makes
instances and drives their parameters (`{inset "near"} {imageFrom "probeMap"} {imageProbe {' 7.0 7.0 8.0}}`). Keep
`//` comments away from just above a tagged declaration: naga then drops its `///` lines.

## The narrated video

Inputs in `packages/<package>/video/` (tools/video finds the `video.json` whose `investigation` names it):

- **`video.json`**: `investigation`, the paths (storyboard, translations, diagrams, source index, pseudo-code,
  timelines), output names, `engineLabel`, `checkoutEnv`, `durationMinutes`, `readingPath` (index keys for the
  source-reading guide), `pronunciations`, and the documents' texts per language (intro, demo descriptions,
  simplifications). Copy O3DE's and adapt it.
- **`storyboard.lam`** and **`storyboard.zh-CN.lam`**: Lamlet presentations, their headings, cues, ranges, motions,
  formulas, and the translation's form in the `render-lab-lamlet` skill; the cue vocabulary from `render-lab
  --investigation <id> story keys`. Groups the storyboard repeats go in the package's story library, `video/story.lam`.
- **`diagrams/*.toml`**: diagrams for `diagram=` beats, boxes and edges whose text is keys of the package's text
  (`tools/video/diagrams.py` has the format; the lab draws them, `lab/src/diagram.rs`; `focus` lights boxes or rows).
  The frame's own map is the frame graph's scene (a beat's `graph=frame`, the package's `graphs/frame.toml`; the
  `render-graph` skill).

What the video does with these (sentences, cues, animation, reviews) is the `new-story` skill's. The mechanics it
relies on:

- A step parameter is reset with an instant key (no `blend`) at the next beat of the same demonstration; a `demo` key
  resets every channel, camera included unless the key sets the camera. To keep a motion continuous across beats of one
  demonstration, reset only the parameters that need it.
- Float the camera on a path of `camera.yaw`, `camera.pitch`, and `camera.distance` for 3D views, with `cameraDrift` on
  top, and glide it (a camera key with a `blend`) to a front view when a step is clearer in 2D. Lights move on paths
  and orbits too.
- Budget each chapter (its heading's `target=`) near 1.2 times its narration; the builder fills the rest with pauses.

Register the timelines as demos of its package (`[[provides]] type = "demo"`: key, title, edition, and the `built`
resources of its timeline and narration; the `packages` skill) so the homepage's Watch plays them, and let its selftest
honour `options.timeline` by rendering each scene beat and checking that text is at least 24 px, labels stay inside and
apart, and listings and readouts fit.

Build (one GPU job at a time; the full render runs in the background):

```sh
bash tools/video/setup.sh --gpu                                                   # once
python3 tools/video/build.py --investigation <engine>/<feature> --prepare-only    # narration, timeline, captions, documents
python3 tools/video/build.py --investigation <engine>/<feature> --snapshots-only  # one still per beat: look at them
python3 tools/video/build.py --investigation <engine>/<feature>                   # the full video and its verification
python3 tools/video/build.py --investigation <engine>/<feature> --lang zh-CN      # after the English edition
```

Render a few stills across a beat (`render --timeline … --snapshot A,B,C --snapshot-dir D`) to see an animation's
steps, not just its midpoint. Outputs go to the package's `build/video/<lang>/` and the timelines to its
`build/timelines/<lang>/` (both ignored by git: generated files are never tracked). `tools/video/README.md` has the
details.
