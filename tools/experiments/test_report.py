"""Tests of the experiment report (report.py): `python3 -m unittest tools/experiments/test_report.py`."""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402

import report  # noqa: E402


def stat(mean: float, sd: float = 0.1) -> dict[str, float]:
    return {"mean": mean, "sd": sd, "p95": mean + sd}


def make_results(directory: Path) -> None:
    """A research run's directory as lab/src/research.rs writes it: three swept runs and a variant."""
    (directory / "images").mkdir(parents=True)
    (directory / "flip").mkdir()
    picture = np.zeros((18, 32, 3), dtype=np.float32)
    plt.imsave(directory / "images" / "reference.png", picture)
    runs = []
    for i, (name, varied, flip, gpu) in enumerate([("base", {}, 0.10, 4.0), ("bias=1", {"bias": 1.0}, 0.05, 4.2), ("bias=2", {"bias": 2.0}, 0.08, 3.9), ("off", {"bounces": False}, 0.2, 3.0)]):
        slug = f"{i:02}-{name}"
        plt.imsave(directory / "images" / f"{slug}.png", picture + i * 0.1)
        plt.imsave(directory / "flip" / f"{slug}.png", picture)
        runs.append({
            "name": name,
            "image": f"images/{slug}.png",
            "flip": f"flip/{slug}.png",
            "varied": varied,
            "values": varied,
            "samples": [],
            "summary": {"flip": stat(flip, 0.01), "psnr": stat(30.0 + i), "gpu_ms": stat(gpu), "gpu:scene pass": stat(gpu * 0.6), "gpu:composite": stat(gpu * 0.4)},
        })
    (directory / "results.json").write_text(json.dumps({
        "experiment": "test", "description": "A test.", "investigation": "x/y", "reference": "images/reference.png",
        "varied": ["bias", "bounces"], "metrics": ["gpu_ms", "psnr", "flip"], "passes": ["scene pass", "composite"], "runs": runs,
    }))
    (directory / "manifest.json").write_text(json.dumps({"describe": "abc123-dirty", "gpu": {"name": "Test GPU", "backend": "Vulkan"}, "resolution": [32, 18], "frames": 4, "warmup": 2, "repeats": 3, "timestamps": True, "sha256": "00"}))
    (directory / "convergence.csv").write_text("run,frame,psnr,flip\nbase,1,20,0.3\nbase,2,25,0.2\nbias=1,1,21,0.25\nbias=1,2,27,0.1\n")


class Report(unittest.TestCase):
    def test_writes_every_part(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            make_results(directory)
            page = report.report(directory)
            out = directory / "report"
            for name in ("summary.md", "sweep-bias.png", "pareto.png", "passes.png", "convergence.png", "grid.png", "report.html"):
                self.assertTrue((out / name).exists(), name)
            summary = (out / "summary.md").read_text()
            self.assertIn("| run | bias | bounces | GPU time (ms) | PSNR (dB) | ꟻLIP |", summary)
            self.assertIn("abc123-dirty", summary)
            # The lowest ꟻLIP and the highest PSNR are the best.
            self.assertIn("Best ꟻLIP: **bias=1**", summary)
            self.assertIn("Best PSNR (dB): **off**", summary)
            self.assertIn("<table>", page.read_text())

    def test_numbers(self) -> None:
        self.assertEqual(report.number(True), 1.0)
        self.assertIsNone(report.number("128"))
        self.assertEqual(report.fmt(999.0), "∞")
        self.assertEqual(report.fmt(0.00012), "1.20e-04")


if __name__ == "__main__":
    unittest.main()
