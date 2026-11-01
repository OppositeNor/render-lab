"""Report a research run's results (`render-lab … experiment <file> --out DIR`, lab/src/research.rs).

    python3 tools/experiments/report.py DIR [--out REPORT_DIR]

Reads DIR's results.json, manifest.json, and convergence.csv (if any), and
writes into REPORT_DIR (default DIR/report):

- summary.md: what ran (experiment, investigation, commit, GPU, resolution)
  and a table of every run: the values it varies, then each metric's mean ±
  sample standard deviation over its measured frames;
- sweep-<key>.png: each metric against a swept value, for the runs that vary
  that value alone (error bars: one standard deviation);
- pareto.png: quality against GPU time per run (ꟻLIP, else PSNR), the
  runs on the Pareto front joined;
- passes.png: each run's GPU time per pass, stacked;
- convergence.png: the image metrics frame by frame (with `convergence`);
- grid.png: the reference, each run's image, and its ꟻLIP map;
- report.html: all of the above in one self-contained page.
"""
from __future__ import annotations

import argparse
import base64
import csv
import html
import json
import math
import sys
from dataclasses import dataclass, field
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

# Metrics where lower is better (the others: higher is better).
LOWER_IS_BETTER: set[str] = {"mse", "rmse", "flip", "relmse", "gpu_ms", "cpu_ms"}
# How each metric is labelled.
LABELS: dict[str, str] = {
    "gpu_ms": "GPU time (ms)",
    "cpu_ms": "frame time (ms)",
    "mse": "MSE",
    "rmse": "RMSE",
    "psnr": "PSNR (dB)",
    "ssim": "SSIM",
    "flip": "ꟻLIP",
    "relmse": "relMSE",
    "luminance": "mean luminance",
}


@dataclass
class Stat:
    mean: float
    sd: float
    p95: float


@dataclass
class RunResult:
    name: str
    varied: dict[str, object]
    summary: dict[str, Stat]
    image: str | None
    flip: str | None


@dataclass
class Results:
    directory: Path
    experiment: str
    description: str
    investigation: str
    reference: str | None
    varied: list[str]
    metrics: list[str]
    passes: list[str]
    runs: list[RunResult]
    manifest: dict[str, object] = field(default_factory=dict)


def load(directory: Path) -> Results:
    """The results of a research run's directory."""
    data = json.loads((directory / "results.json").read_text())
    manifest_path = directory / "manifest.json"
    manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else {}
    runs: list[RunResult] = []
    for r in data["runs"]:
        summary = {k: Stat(float(v["mean"]), float(v["sd"]), float(v["p95"])) for k, v in r["summary"].items() if v["mean"] is not None}
        runs.append(RunResult(r["name"], dict(r["varied"]), summary, r.get("image"), r.get("flip")))
    return Results(directory, data["experiment"], data.get("description", ""), data["investigation"], data.get("reference"), list(data["varied"]), list(data["metrics"]), list(data["passes"]), runs, manifest)


def number(value: object) -> float | None:
    """A varied value as a number for a plot's axis (a flag as 0 or 1), or None."""
    if isinstance(value, bool):
        return 1.0 if value else 0.0
    if isinstance(value, (int, float)):
        return float(value)
    return None


def fmt(x: float) -> str:
    if math.isinf(x) or x >= 999.0:
        return "∞"
    if math.isnan(x):
        return "–"
    if x != 0 and abs(x) < 1e-3:
        return f"{x:.2e}"
    return f"{x:.4g}"


def summary_markdown(r: Results) -> str:
    m = r.manifest
    gpu = m.get("gpu") if isinstance(m.get("gpu"), dict) else {}
    lines = [f"# {r.experiment}", ""]
    if r.description:
        lines += [r.description, ""]
    resolution = m.get("resolution", ["?", "?"])
    lines += [
        f"- Investigation: `{r.investigation}`",
        f"- Commit: `{m.get('describe') or m.get('commit') or 'unknown'}`",
        f"- GPU: {gpu.get('name', '?')} ({gpu.get('backend', '?')}, driver {gpu.get('driverInfo') or gpu.get('driver') or '?'})",
        f"- Resolution: {resolution[0]}×{resolution[1]}; frames {m.get('frames', '?')} + warm-up {m.get('warmup', '?')} + measured {m.get('repeats', '?')} per run",
        f"- GPU timestamps: {'yes' if m.get('timestamps') else 'no'}",
        f"- Experiment SHA-256: `{m.get('sha256', '?')}`",
        "",
    ]
    columns = r.metrics
    lines.append("| run | " + " | ".join(r.varied + [LABELS.get(c, c) for c in columns]) + " |")
    lines.append("|" + "---|" * (1 + len(r.varied) + len(columns)))
    for run in r.runs:
        cells = [run.name] + [str(run.varied.get(k, "")) for k in r.varied]
        for c in columns:
            s = run.summary.get(c)
            cells.append(f"{fmt(s.mean)} ± {fmt(s.sd)}" if s else "")
        lines.append("| " + " | ".join(cells) + " |")
    lines.append("")
    best: list[str] = []
    for c in [c for c in columns if c != "luminance"]:
        scored = [(run.summary[c].mean, run.name) for run in r.runs if c in run.summary and not math.isnan(run.summary[c].mean)]
        if not scored:
            continue
        value, name = min(scored) if c in LOWER_IS_BETTER else max(scored)
        best.append(f"- Best {LABELS.get(c, c)}: **{name}** ({fmt(value)})")
    if best:
        lines += ["Best per metric:", ""] + best + [""]
    lines.append("Times are the mean ± sample standard deviation over each run's measured frames; image metrics are of its last frame.")
    return "\n".join(lines) + "\n"


def sweep_plots(r: Results, out: Path) -> list[Path]:
    """Each metric against each numeric swept value, for the runs varying that value alone."""
    written: list[Path] = []
    for key in r.varied:
        points = [(number(run.varied[key]), run) for run in r.runs if list(run.varied) == [key] and number(run.varied[key]) is not None]
        if len(points) < 2:
            continue
        points.sort(key=lambda p: p[0] or 0.0)
        metrics = [c for c in r.metrics if c != "luminance" and all(c in p[1].summary for p in points)]
        if not metrics:
            continue
        fig, axes = plt.subplots(1, len(metrics), figsize=(3.4 * len(metrics), 3.0), squeeze=False)
        for ax, c in zip(axes[0], metrics):
            xs = [p[0] for p in points]
            ys = [p[1].summary[c].mean for p in points]
            es = [p[1].summary[c].sd for p in points]
            ax.errorbar(xs, ys, yerr=es, marker="o", capsize=3, color="#2a6fdb")
            ax.set_xlabel(key)
            ax.set_ylabel(LABELS.get(c, c))
            ax.grid(alpha=0.3)
        fig.suptitle(f"{r.experiment}: {key}")
        fig.tight_layout()
        path = out / f"sweep-{key}.png"
        fig.savefig(path, dpi=130)
        plt.close(fig)
        written.append(path)
    return written


def pareto_plot(r: Results, out: Path) -> Path | None:
    """Quality (ꟻLIP, else PSNR) against GPU time per run, the Pareto front joined."""
    cost = "gpu_ms" if any("gpu_ms" in run.summary for run in r.runs) else "cpu_ms"
    quality = next((q for q in ("flip", "psnr", "ssim") if any(q in run.summary for run in r.runs)), None)
    runs = [run for run in r.runs if cost in run.summary and quality and quality in run.summary]
    if quality is None or len(runs) < 2:
        return None
    lower = quality in LOWER_IS_BETTER
    pts = [(run.summary[cost].mean, run.summary[quality].mean, run) for run in runs]
    # Infinite PSNR (identical to the reference) plots at the top.
    finite = [y for _, y, _ in pts if not (math.isinf(y) or y >= 999.0)]
    cap = (max(finite) + 3.0) if finite else 100.0
    pts = [(x, min(y, cap) if not lower else y, run) for x, y, run in pts]
    front: list[tuple[float, float, RunResult]] = []
    for p in sorted(pts, key=lambda p: (p[0], p[1] if lower else -p[1])):
        if not front or (p[1] < front[-1][1] if lower else p[1] > front[-1][1]):
            front.append(p)
    fig, ax = plt.subplots(figsize=(6.0, 4.2))
    ax.errorbar([p[0] for p in pts], [p[1] for p in pts], xerr=[p[2].summary[cost].sd for p in pts], fmt="o", color="#2a6fdb", capsize=3)
    ax.plot([p[0] for p in front], [p[1] for p in front], color="#e07a1f", lw=1.5, label="Pareto front")
    for x, y, run in pts:
        ax.annotate(run.name, (x, y), textcoords="offset points", xytext=(4, 4), fontsize=7)
    ax.set_xlabel(LABELS.get(cost, cost))
    ax.set_ylabel(LABELS.get(quality, quality) + (" (lower is better)" if lower else " (higher is better)"))
    ax.grid(alpha=0.3)
    ax.legend(fontsize=8)
    ax.set_title(f"{r.experiment}: quality against cost")
    fig.tight_layout()
    path = out / "pareto.png"
    fig.savefig(path, dpi=130)
    plt.close(fig)
    return path


def passes_plot(r: Results, out: Path) -> Path | None:
    """Each run's mean GPU time per pass, stacked."""
    passes = [p for p in r.passes if any(f"gpu:{p}" in run.summary for run in r.runs)]
    if not passes:
        return None
    fig, ax = plt.subplots(figsize=(max(6.0, 0.6 * len(r.runs) + 2.0), 4.2))
    bottoms = [0.0] * len(r.runs)
    cmap = plt.get_cmap("tab20")
    for i, p in enumerate(passes):
        values = [run.summary[f"gpu:{p}"].mean if f"gpu:{p}" in run.summary else 0.0 for run in r.runs]
        ax.bar(range(len(r.runs)), values, bottom=bottoms, label=p, color=cmap(i % 20))
        bottoms = [b + v for b, v in zip(bottoms, values)]
    ax.set_xticks(range(len(r.runs)), [run.name for run in r.runs], rotation=40, ha="right", fontsize=7)
    ax.set_ylabel("GPU time (ms)")
    ax.legend(fontsize=7, ncol=2)
    ax.grid(axis="y", alpha=0.3)
    ax.set_title(f"{r.experiment}: GPU time per pass")
    fig.tight_layout()
    path = out / "passes.png"
    fig.savefig(path, dpi=130)
    plt.close(fig)
    return path


def convergence_plot(r: Results, out: Path) -> Path | None:
    """The image metrics frame by frame, one line per run."""
    path_in = r.directory / "convergence.csv"
    if not path_in.exists():
        return None
    rows: list[dict[str, str]] = list(csv.DictReader(path_in.open()))
    if not rows:
        return None
    metrics = [k for k in rows[0] if k not in ("run", "frame")]
    fig, axes = plt.subplots(1, len(metrics), figsize=(3.6 * len(metrics), 3.2), squeeze=False)
    names = list(dict.fromkeys(row["run"] for row in rows))
    for ax, c in zip(axes[0], metrics):
        for name in names:
            pts = [(int(row["frame"]), float(row[c])) for row in rows if row["run"] == name and row[c]]
            ax.plot([p[0] for p in pts], [min(p[1], 999.0) for p in pts], label=name, lw=1)
        ax.set_xlabel("frame")
        ax.set_ylabel(LABELS.get(c, c))
        ax.grid(alpha=0.3)
    axes[0][0].legend(fontsize=6)
    fig.tight_layout()
    path = out / "convergence.png"
    fig.savefig(path, dpi=130)
    plt.close(fig)
    return path


def grid_image(r: Results, out: Path) -> Path | None:
    """The reference, then each run's image with its ꟻLIP map beneath."""
    tiles: list[tuple[str, Path | None, Path | None]] = []
    if r.reference:
        tiles.append(("reference", r.directory / r.reference, None))
    for run in r.runs:
        tiles.append((run.name, r.directory / run.image if run.image else None, r.directory / run.flip if run.flip else None))
    tiles = [t for t in tiles if t[1] and t[1].exists()]
    if not tiles:
        return None
    columns = min(4, len(tiles))
    rows = math.ceil(len(tiles) / columns)
    with_flip = any(t[2] for t in tiles)
    fig, axes = plt.subplots(rows * (2 if with_flip else 1), columns, figsize=(4.0 * columns, 2.4 * rows * (2 if with_flip else 1)), squeeze=False)
    for ax in axes.flat:
        ax.axis("off")
    for i, (name, image, flip) in enumerate(tiles):
        row, col = divmod(i, columns)
        top = axes[row * (2 if with_flip else 1)][col]
        top.imshow(plt.imread(image))
        top.set_title(name, fontsize=8)
        if with_flip and flip and flip.exists():
            bottom = axes[row * 2 + 1][col]
            bottom.imshow(plt.imread(flip))
            bottom.set_title(f"{name}: ꟻLIP", fontsize=7)
    fig.tight_layout()
    path = out / "grid.png"
    fig.savefig(path, dpi=110)
    plt.close(fig)
    return path


def markdown_table_html(md: str) -> str:
    """The summary's Markdown (headings, lists, one table) as simple HTML."""
    out: list[str] = []
    table: list[list[str]] = []

    def flush() -> None:
        if table:
            head, *body = table
            out.append("<table><thead><tr>" + "".join(f"<th>{html.escape(c)}</th>" for c in head) + "</tr></thead><tbody>")
            out.extend("<tr>" + "".join(f"<td>{html.escape(c)}</td>" for c in row) + "</tr>" for row in body)
            out.append("</tbody></table>")
            table.clear()

    for line in md.splitlines():
        if line.startswith("|"):
            cells = [c.strip() for c in line.strip("|").split("|")]
            if not all(set(c) <= {"-"} for c in cells):
                table.append(cells)
            continue
        flush()
        if line.startswith("# "):
            out.append(f"<h1>{html.escape(line[2:])}</h1>")
        elif line.startswith("- "):
            text = html.escape(line[2:]).replace("**", "")
            out.append(f"<li>{text.replace('`', '')}</li>")
        elif line:
            out.append(f"<p>{html.escape(line)}</p>")
    flush()
    return "\n".join(out)


def write_html(md: str, images: list[Path], out: Path) -> Path:
    parts = [markdown_table_html(md)]
    for image in images:
        data = base64.b64encode(image.read_bytes()).decode()
        parts.append(f'<figure><img alt="{html.escape(image.stem)}" src="data:image/png;base64,{data}"><figcaption>{html.escape(image.stem)}</figcaption></figure>')
    page = (
        "<!doctype html><html><head><meta charset=utf-8><title>Experiment report</title>"
        "<style>body{font:14px/1.45 system-ui,sans-serif;max-width:1100px;margin:2em auto;padding:0 16px;color:#1d2330}"
        "table{border-collapse:collapse;font-size:12px}td,th{border:1px solid #ccd;padding:3px 7px;text-align:right}"
        "td:first-child,th:first-child{text-align:left}img{max-width:100%}figure{margin:1.5em 0}</style></head><body>"
        + "\n".join(parts)
        + "</body></html>"
    )
    path = out / "report.html"
    path.write_text(page)
    return path


def report(directory: Path, out: Path | None = None) -> Path:
    """Write the report of `directory` into `out` (default `directory/report`); the HTML page's path."""
    results = load(directory)
    out = out or directory / "report"
    out.mkdir(parents=True, exist_ok=True)
    md = summary_markdown(results)
    (out / "summary.md").write_text(md)
    images: list[Path] = []
    images += sweep_plots(results, out)
    for made in (pareto_plot(results, out), passes_plot(results, out), convergence_plot(results, out), grid_image(results, out)):
        if made:
            images.append(made)
    return write_html(md, images, out)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("directory", type=Path, help="a research run's output directory")
    parser.add_argument("--out", type=Path, default=None, help="where to write the report (default: DIRECTORY/report)")
    args = parser.parse_args(argv)
    page = report(args.directory, args.out)
    print(f"report: {page}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
