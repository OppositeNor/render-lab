# Experiment reports

`report.py` turns the output directory of a research run into tables, plots, and an image grid. A research run is an
experiment file rendered headless; the format is in `lab/src/experiment.rs`, and the runner is in
`lab/src/research.rs`.

```sh
render-lab --investigation godot/voxelgi experiment godot_voxelgi.experiment/cones-and-bias --out runs/vgi
python3 tools/experiments/report.py runs/vgi           # runs/vgi/report/report.html
python3 -m unittest tools/experiments/test_report.py
```

It needs Python 3.10+ and matplotlib (numpy for the tests).

The report writes:

- `summary.md`: what ran (commit, GPU, resolution), and every run's metrics as mean ± standard deviation.
- `sweep-<key>.png`: each metric against a swept value.
- `pareto.png`: quality against GPU time.
- `passes.png`: GPU time per pass.
- `convergence.png`: with `convergence = true`.
- `grid.png`: the reference, each run, and its ꟻLIP map.
- `report.html`: all of it in one page.

The `experiments` skill (`.agents/skills/experiments/SKILL.md`) describes how to design an experiment: the reference,
the controls, the repeats, and the checks.
