"""Lamlet regions in a storyboard: expanded before the storyboard is read.

A `.story` file (or its translation) may hold Lamlet regions (`${...}`, `$[...]`, `$name`; `$$` is a dollar).
Lamlet checks the whole file and evaluates every region; each expression region becomes its value's text (a String
as it is, a number as Lamlet writes it), declarations and imports leave nothing, and the prose around them is kept
as it is. The result is an ordinary storyboard, which story.py then reads. rlstd's Lamlet library
(packages/rlstd/language/story.lam) is available as the package `rlstd`: cues from typed values, numbers as
spoken words.

A file without a region is read as it is, so Lamlet is needed only by a storyboard that uses it. The interpreter
is the `lamlet` command, or the one the LAMLET environment variable names.
"""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from pathlib import Path
from typing import Any

LAB = Path(__file__).resolve().parents[2]
RLSTD = LAB / 'packages' / 'rlstd' / 'language' / 'story.lam'

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


def run(verb: str, path: Path) -> dict[str, Any]:
    """`lamlet <verb> path` with rlstd's library, its document's JSON; a failure raises with its diagnostics."""
    proc = subprocess.run([command(), verb, str(path), '--package', f'rlstd={RLSTD}'], capture_output=True, text=True)
    try:
        out = json.loads(proc.stdout)
    except ValueError:
        raise LamletError(f'{path}: lamlet {verb} failed:\n{proc.stderr.strip()}') from None
    doc = out['documents'][0]
    if not out.get('ok') or not doc.get('ok'):
        raise LamletError(f'{path}: Lamlet:\n{proc.stderr.strip()}')
    return doc


def text_of(value: dict[str, Any], type_: str, where: str) -> str:
    """A region's value as story text: a String as it is, a number as Lamlet writes it."""
    if type_ == 'String':
        return value['data']
    if type_ in ('Int', 'Float', 'Bool'):
        return value['display']
    raise LamletError(f'{where}: a region in a storyboard gives a String or a number, not {type_}')


def expand(path: Path) -> str:
    """The storyboard `path` with its Lamlet regions replaced by their values."""
    spans = run('prepare', path)['spans']
    results = {r['region']: r for r in run('eval', path)['results']}
    out: list[str] = []
    for k, span in enumerate(spans):
        kind = span['kind']
        if kind == 'text':
            text = span['text']
            # A declaration alone on its line leaves no blank line behind it.
            if k > 0 and spans[k - 1]['kind'] == 'declaration' and text.startswith('\n') and (not out or out[-1].endswith('\n')):
                text = text[1:]
            out.append(text)
        elif kind == 'declaration':
            continue
        else:
            r = results.get(span['region'])
            where = f'{path}: line {r["line"] if r else "?"}'
            if r is None:
                raise LamletError(f'{where}: region {span["region"]} has no value')
            out.append(text_of(r['value'], r['type'], where))
    return ''.join(out)


def read(path: Path) -> str:
    """A storyboard's text, its Lamlet regions expanded when it has any."""
    text = path.read_text()
    return expand(path) if has_regions(text) else text
