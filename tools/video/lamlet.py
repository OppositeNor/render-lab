"""Lamlet, from the lab's build: the version lab/Cargo.toml pins, run as `render-lab lamlet …` (and
`render-lab story keys`), never a `lamlet` installed on the system. build.py builds the lab before it reads a
storyboard. A storyboard is a Lamlet presentation (presentation.py): the lab evaluates it and gives its document
IR, with render_lab's Lamlet library (packages/render_lab/language/render_lab.lam) available as the package
`render_lab`, and the investigation's cue vocabulary as a package of the investigation's name.
"""
from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

LAB = Path(__file__).resolve().parents[2]
BINARY = LAB / 'target' / 'release' / 'render-lab'


class LamletError(Exception):
    pass


def command() -> Path:
    """The lab's release build, which holds the Lamlet interpreter."""
    if not BINARY.exists():
        raise LamletError(f'a storyboard is read by the lab: build it first ({BINARY} is missing; cargo build --release --locked --offline)')
    return BINARY


def lab(*args: str) -> str:
    """A `render-lab` command's output; a failure raises with its diagnostics (Lamlet's, with the document's line)."""
    proc = subprocess.run([str(command()), *args], capture_output=True, text=True)
    if proc.returncode != 0:
        raise LamletError(proc.stderr.strip())
    return proc.stdout


def ir(path: Path, packages: list[tuple[str, Path]], investigation: str | None = None) -> Any:
    """A presentation's document IR (`render-lab lamlet ir`, lamlet-doc's JSON): its prose read as CommonMark, its
    regions evaluated, with these packages to import besides std, content, present, and render_lab. The lab's
    natives (`rl`: callouts, text keys, points, formulas) check the names it gives against `investigation`."""
    given = [a for name, p in packages for a in ('--package', f'{name}={p}')]
    chosen = ['--investigation', investigation] if investigation else []
    return json.loads(lab(*chosen, 'lamlet', 'ir', str(path), *given))


def vocabulary(investigation: str, name: str, out: Path) -> Path:
    """The investigation's cue vocabulary, the package `name`: its story keys (`render-lab story keys`) as
    lamlet-present's typed functions (`render-lab lamlet vocab`), written to `out`."""
    out.parent.mkdir(parents=True, exist_ok=True)
    keys = out.with_suffix('.keys.json')
    keys.write_text(lab('--investigation', investigation, 'story', 'keys'))
    lab('lamlet', 'vocab', name, str(keys), '-o', str(out))
    return out
