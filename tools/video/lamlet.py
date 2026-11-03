"""Lamlet regions in a storyboard: expanded before the storyboard is read.

A `.story` file (or its translation) may hold Lamlet regions (`${...}`, `$[...]`, `$name`; `$$` is a dollar).
Lamlet checks the whole file and evaluates every region (`lamlet FILE`); each expression region becomes its value's
text (a String as it is, a number or other value as Lamlet writes it), declarations and imports leave nothing, and
the prose around them is kept as it is. The result is an ordinary storyboard, which story.py then reads. render_lab's Lamlet library
(packages/render_lab/language/render_lab.lam) is available as the package `render_lab`: cues from typed values, numbers as
spoken words.

A file without a region is read as it is, so Lamlet is needed only by a storyboard that uses it. The interpreter
is the `lamlet` command, or the one the LAMLET environment variable names.
"""
from __future__ import annotations

import os
import re
import shutil
import subprocess
from pathlib import Path

LAB = Path(__file__).resolve().parents[2]
RENDER_LAB = LAB / 'packages' / 'render_lab' / 'language' / 'render_lab.lam'

# A region opener: `${`, `$[`, or `$name`, not `$$`.
REGION = re.compile(r'(?<!\$)\$(?:[\[{]|[A-Za-z_])')


class LamletError(Exception):
    pass


def has_regions(text: str) -> bool:
    """Whether a text holds a Lamlet region."""
    return bool(REGION.search(text.replace('$$', '')))


def command() -> str:
    """The Lamlet interpreter: LAMLET, or `lamlet` on the PATH."""
    given = os.environ.get('LAMLET', '')
    if given:
        return given
    found = shutil.which('lamlet')
    if not found:
        raise LamletError('this storyboard has Lamlet regions: install the `lamlet` command, or set LAMLET to it')
    return found


def expand(path: Path) -> str:
    """The storyboard `path` with its Lamlet regions replaced by their values: `lamlet FILE` prints exactly that.
    A failure raises with Lamlet's diagnostics."""
    proc = subprocess.run([command(), str(path), '--package', f'render_lab={RENDER_LAB}'], capture_output=True, text=True)
    if proc.returncode != 0:
        raise LamletError(f'{path}: Lamlet:\n{proc.stderr.strip()}')
    return proc.stdout


def read(path: Path) -> str:
    """A storyboard's text, its Lamlet regions expanded when it has any."""
    text = path.read_text()
    return expand(path) if has_regions(text) else text
