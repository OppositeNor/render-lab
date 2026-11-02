---
name: experiments
description: Run experiments on a Render Lab investigation's algorithm, interactively (the viewer's Experiments tab, the homepage's "Experiment with the algorithm") or as research (an experiment file run headless by `render-lab experiment`, reported by tools/experiments/report.py). Covers the experiment file format (base, reference, sweeps, variants, metrics, frames, warm-up, repeats, convergence), choosing a reference and controls, the metrics (GPU time per pass, PSNR, SSIM, ꟻLIP, relMSE), the output directory and its manifest, reading the report, and exposing an engine's settings to experiments (`engine=` on a value, `Investigation::scene_data` for settings baked into scene data). Use when asked to compare settings, sweep a parameter, ablate a feature, measure quality or cost, reproduce a result, ship an experiment with a package, or make an engine setting tunable.
---

# Experiments

Every investigation has an **Experiments** mode: the homepage's **Experiment with the algorithm** button, or the
viewer's **Experiments** dock tab. The same experiment can be rerun **headless** as research, with numbers you can put
in a table. One TOML format (`lab/src/experiment.rs`, documented at its top) serves both.

## 1. Interactively (the viewer)

The Experiments tab lists **every value** of the investigation, whichever demonstration is open, in groups:

- **Engine settings**: values with `engine=`, grouped by label; hover a name to see the engine property.
- **Scene**: labelled values without `engine`.
- **Presentation**: overlays and steps, hidden until shown.
- **Camera**.

Its tools:

- **Filter**.
- **Changed from the defaults**, with a ↺ per value and **Reset all**. In experiments mode, changed values carry over to
  the next demonstration.
- **A/B compare**:
  - **Set reference** makes the current values A. Change values, and the live image is B.
  - Views: **Wipe**, **Side by side**, **Difference** (B − A in displayed luminance, orange brighter, blue darker,
    ×gain), **A only** (flicker by toggling), **Swap**.
  - Both views keep their own history, so temporal methods converge in each. The camera is shared.
- **Measure B against A**: PSNR, SSIM, ꟻLIP, MSE, and relMSE of the scene alone (no overlays).
- **Performance**: GPU time per pass from timestamp queries, plus the frame time. **Show in the panel** opens the
  performance panel on the right: the frame's GPU and frame time, their history against the 120/60/30 fps budgets,
  and a row per pass by cost. A story shows the same panel with `panel=performance` and calls out passes with
  `highlight=<pass>` (the pass names are the chain's entry points, `scene pass`, `depth pre-pass`, `overlay`,
  `composite`; `--page viewer-performance` lists them). A recording measures as it renders, after a few timed warm-up
  renders of its first shot, so the numbers are this machine's, not fixed. **Open the frame graph** switches to the
  dock's Frame graph tab (the `render-graph` skill): the frame's passes in the panel, each one's result and what it
  does.
- **An intermediate**: an experiment file's `[inspect]` table (`of = "<node>.<resource>"`, `show`, `values`) measures
  that resource right after its node, as the inspector draws it, instead of the frame (the `render-graph` skill).
- **Experiments**: **Save** writes an experiment file of the current values (and the reference) to
  `~/.config/render-lab/experiments/<engine>/<feature>/`. The list loads it again, or one a package ships.

Offscreen checks: `home --snapshot X.png --page viewer-experiments[:<demo>]`, and `--page viewer-compare[:<demo>]` (a
reference, one engine setting moved, a wipe, a measurement printed).

## 2. As research (headless)

```sh
render-lab --investigation godot/voxelgi experiment godot_voxelgi.experiment/cones-and-bias --dry-run   # the runs
render-lab --investigation godot/voxelgi experiment <file.toml or package key> --out <dir>             # run it
python3 tools/experiments/report.py <dir>                                                              # <dir>/report/
```

Keep the output out of the repository (a scratch folder, or a package's git-ignored `build/`). Run one GPU job at a time.

### Writing the experiment

```toml
name = "probe-rays"
investigation = "godot/sdfgi"   # checked against --investigation
demo = "room"                   # the starting demonstration and camera
resolution = [1280, 720]
frames = 4                      # frames before measuring (a history converges)
warmup = 4                      # untimed frames before the timed ones
repeats = 30                    # timed frames, back to back
convergence = false             # true: image metrics after each of the first `frames`
metrics = ["gpu_ms", "psnr", "ssim", "flip", "relmse"]
[camera]                        # optional: target, yaw, pitch, distance, fov
[base.values]                   # every run's values, and the reference's
[reference]
frames = 64                     # a converged ground truth may need more frames
[reference.values]
giRayCount = 6                  # the expensive, high-quality setting
[[sweep]]
values = { giRayCount = [0, 1, 2, 3, 4] }   # each combination; `zip = true`: in step
[[variant]]
name = "no-sky"
values = { giReadSkyLight = false }         # an ablation
```

The runs are the base, then each sweep's combinations, then each variant. A value is written as a story writes it: a
number, `true`/`false`, an option's name for a choice, or an array for a vector. `render-lab values` lists the keys
(`engine …` marks a setting). An unknown key fails before anything renders, and the error lists the valid ones.

### Method, as papers do it

- **Reference.** Use the engine's own most expensive setting (more rays, more cones, a longer history), or a feature
  turned off for an ablation of what it adds. Say which in `description`.
- **Controls.** Fix the camera (the demonstration's or `[camera]`), the resolution, and every value you are not
  varying (`[base.values]`). Choose the final image, not a debug view: set a demonstration's `viewMode` back to `final`
  if it uses another.
- **Repeats and warm-up.** GPU times vary with clocks and power. Warm up, then time 20 to 30 frames back to back, and
  report mean ± standard deviation (the report does). Image metrics are of the last frame. Compare runs only within one
  output directory: one GPU, one driver, one build.
- **Temporal methods.** Use `convergence = true` to plot quality frame by frame, and give the reference enough frames.
- **Check the setup.** A run equal to the base, such as a sweep point at the default, must give the base's numbers.
  A variant equal to the reference must measure as identical (PSNR ∞, ꟻLIP 0).
- **Reproducibility.** `manifest.json` records the experiment (verbatim and its SHA-256), the commit (`describe
  --dirty`), the GPU and driver, the resolution, and the times. Keep the directory with any result you quote.

### The output

The directory holds:
- `manifest.json` and `experiment.toml`;
- `results.csv`: a row per run and timed frame, with the varied values, the metrics, and `gpu:<pass>` columns;
- `results.json`: per run, its values, its samples, and each metric's mean, sd, and p95;
- `convergence.csv`;
- `images/`: PNG for display, PFM for linear HDR;
- `flip/`: ꟻLIP maps in a magma-like ramp.

The report writes:
- `summary.md`: a table and the best run per metric;
- `sweep-<key>.png`: metric against value, with error bars;
- `pareto.png`: quality against GPU time, with the front joined;
- `passes.png`: GPU time per pass, stacked;
- `convergence.png`, `grid.png`;
- `report.html`: self-contained.

## 3. Shipping an experiment with a package

Put the file in the package's `experiments/<name>.toml` and declare it in its `resources.toml`:

```toml
[[resource]]
id = "experiment/<name>"
kind = "file"
source = "experiments/<name>.toml"
license = "MIT"
```

The Experiments tab lists it, and `render-lab experiment <package>.experiment/<name>` runs it.

## 4. Exposing an engine's settings

- **A value that is an engine setting** gets `engine = "<the engine's property>"`:
  - on a `#[value(…)]` field, `engine = "VoxelGIData.bias"`;
  - in a WGSL tag, `engine=Environment.sdfgi_energy`.
  - Cite the property's definition, default, and range in its doc comment (file and line in the checkout). Use the
    engine's range where it has one.
  - Its default is the engine's, and **must change nothing** in existing frames.
- **Do not repurpose a demonstration's value** that a story animates for the scene's setting. Add a scene value (the
  Godot packages' `gi*` values) so the timelines keep rendering the same.
- **A hard-coded constant** becomes a value read where the shader read the constant:
  - a tagged uniform member, or a free lane of the investigation's frame record (O3DE's shadow blur, read from
    `FRAME_SUN + 3`);
  - with the same default, so frames do not change.
- **A setting baked into the scene shading's data** (a bake's resolution, a converged probe history) needs
  `Investigation::scene_data(state)`:
  - return `None` at the settings `SceneShading::data` was built with;
  - otherwise return a `SceneData { key, build }`. The key hashes the baked settings. The build is a `Send` closure
    that returns the data.
  - The renderer keeps the last few builds by key. It builds on a worker thread in the viewer (the Performance group
    shows "Rebuilding…"), waits in recordings and research runs, and restarts the history when the data swaps.
  - See `godot_sdfgi`'s reconverged probe maps.
- **A setting whose GPU work is redone when it changes** (a relit cache, a light texture) is better as an update
  (`SceneShading::update`, below) than as rebuilt data: its plan's key hashes the setting, and the update passes run
  only when the key changes, as the engine relights. See `godot_voxelgi`'s light passes (update.rs).
- A setting that changes an array's size or a texture's dimensions is a larger change (the layout and the passes); note
  it as not yet tunable rather than half-supporting it.
- Then check:
  - `render-lab values` shows the property;
  - the selftest still passes;
  - default-state frames are unchanged (the frame identity check);
  - a sweep through the new value moves the metrics the way the engine's documentation says it should.
