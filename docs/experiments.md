# Experiments and measurements

[Render Lab](../README.md) · [Build](../BUILD.md) · [简体中文](experiments.zh-CN.md)

Run the commands below from the repository root.

Each feature on the selection page also has **Experiment with the algorithm**. It opens the viewer on its
**Experiments** tab, which Explore has too. The tab has:

- **Every value** of the investigation, whichever demonstration is open:
  - the engine's own settings, each naming the property it mirrors (`render-lab values` marks them `engine …`);
  - the scene's values;
  - the demonstrations' presentation.
- **Changes from the defaults**, marked and resettable. In experiments mode they carry over to the next
  demonstration.
- **A/B comparison** against a saved reference: a wipe, side by side, a difference, or the reference alone, each view
  keeping its own history.
- **Measurements** of the difference: PSNR, SSIM, ꟻLIP, MSE, and relMSE.
- **GPU time per pass**, and a **performance panel** on the right (the frame's GPU time, its history, and a bar per
  pass) that a narrated story can show too (`panel=performance`, `highlight=<pass>`).
- **The frame graph** (the dock's Frame graph tab; `panel=dag`, `panel=graph`): every pass in the order it runs, the resources each reads and writes, their
  lifetimes, and the engine pass each mirrors. A click on a pass's resource shows it as it stood right after that
  pass (`imageGraph=<pass>.<resource>` in a story, `[inspect]` in an experiment). As a **DAG** (`panel=dag`, or full
  screen), each pass is a card holding its result right after it, with arrows for the resources between them; drag
  the view or a card to rearrange it. **Resources** shows every resource as a card, as the frame leaves it.
  Full screen, a pass's **Show shader source** shows the WGSL it is compiled from beside the graph, and the viewer
  reloads shaders live: an edit to a WGSL file recompiles the pipelines while it runs.
- **Experiment files** to save and load.

## Headless research experiments

The same file runs headless as a research experiment:

- **Runs:** a reference, sweeps, and ablations.
- **Timing:** warm-up frames, then repeated timed frames, each timed per pass on the GPU.
- **Output:** a directory of CSV and JSON results, PNG and PFM images, ꟻLIP maps, and a manifest of what ran (the
  experiment, the commit, the GPU and driver).

[`tools/experiments`](../tools/experiments/README.md) turns the directory into tables, plots, and an image grid:

```bash
cargo run --release -- --investigation godot/voxelgi experiment godot_voxelgi.experiment/cones-and-bias --out runs/vgi
python3 tools/experiments/report.py runs/vgi        # runs/vgi/report/report.html
```

The format is in `lab/src/experiment.rs`, and the method (references, controls, repeats) is in the `experiments`
skill.
