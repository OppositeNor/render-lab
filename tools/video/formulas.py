"""Render an investigation's formulas into the PNGs its readout panel shows.

    python3 tools/video/formulas.py --investigation o3de/lighting [--check]

The investigation lists them in `src/<engine>/<feature>/formulas/formulas.json`
as `{key: latex}`. In the LaTeX, `\\lit{n}{#rrggbb}{...}` marks term n of the
formula: `<key>.png` shows the formula with no term lit, and `<key>-<n>.png`
draws it dim with term n in that colour (the colour of its readout row).
KaTeX renders them (tools/video/formulas.mjs) next to formulas.json, which
the PNGs are committed beside, so the lab builds without Node.

`manifest.json` records each PNG's source hash (its expanded LaTeX, the
render settings, the KaTeX version, and the renderer) and its own hash; only
stale images are rendered again. `--check` renders nothing and fails if any
image is stale. The lab's build.rs fails on a PNG that no longer matches its
LaTeX or its recorded hash.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from config import LAB, TOOLS, Environment, log, run  # noqa: E402

Json = dict[str, Any]

# The lab's theme (src/lab/palette.rs): TEXT, and TEXT_DIM behind a lit term.
COLORS = {'text': '#eef6ff', 'dim': '#9bb5cc'}
# The readout panel's inner width at 1080p (src/lab/ui.rs: 590 - 2 x 22).
MAX_WIDTH = 546
MIN_FONT_PX = 24.0
# KaTeX scales its math by 1.21 and draws scripts at 0.7 em, so this keeps
# scripts at 24 px.
FONT_PX = 28.4
SETTINGS: Json = {'fontPx': FONT_PX, 'minFontPx': MIN_FONT_PX, 'maxWidth': MAX_WIDTH, 'colors': COLORS}
LIT = re.compile(r'\\lit\{(\d+)\}\{(#[0-9a-fA-F]{6})\}\{')


def directory(investigation: str) -> Path:
    return LAB / 'src' / Path(investigation) / 'formulas'


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def closing_brace(text: str, start: int) -> int:
    """The index of the brace that closes the one opened just before `start`."""
    depth = 1
    for i in range(start, len(text)):
        if text[i] == '{' and text[i - 1] != '\\':
            depth += 1
        elif text[i] == '}' and text[i - 1] != '\\':
            depth -= 1
            if depth == 0:
                return i
    raise ValueError(f'unbalanced braces after {text[:start]!r}')


def terms(latex: str) -> int:
    return max((int(m.group(1)) for m in LIT.finditer(latex)), default=0)


def expand(latex: str, lit: int) -> str:
    """The LaTeX with term `lit` coloured and every other `\\lit` plain."""
    out: list[str] = []
    i = 0
    while m := LIT.search(latex, i):
        end = closing_brace(latex, m.end())
        body = expand(latex[m.end():end], lit)
        out += [latex[i:m.start()], f'{{\\color{{{m.group(2)}}}{body}}}' if int(m.group(1)) == lit else f'{{{body}}}']
        i = end + 1
    return ''.join(out) + latex[i:]


def variants(formulas: dict[str, str]) -> dict[str, tuple[str, str, bool]]:
    """Every PNG: its key, the formula it comes from, its LaTeX, and whether it is dimmed."""
    out: dict[str, tuple[str, str, bool]] = {}
    for key, latex in formulas.items():
        assert re.fullmatch(r'[a-z0-9]+', key), f'formula key {key!r}: use lowercase letters and digits'
        out[key] = (key, expand(latex, 0), False)
        for n in range(1, terms(latex) + 1):
            assert f'\\lit{{{n}}}' in latex, f'{key}: terms are numbered from 1 without gaps; {n} is missing'
            out[f'{key}-{n}'] = (key, expand(latex, n), True)
    return out


def katex_version(env: Environment, recorded: str | None) -> str:
    """The installed KaTeX's version, or without Node the one the manifest recorded."""
    package = env.node_modules / 'katex' / 'package.json'
    return json.loads(package.read_text())['version'] if package.exists() else recorded or 'none'


def source_hash(latex: str, dim: bool, katex: str, renderer: str) -> str:
    return sha256(json.dumps({'latex': latex, 'dim': dim, 'settings': SETTINGS, 'katex': katex, 'renderer': renderer}, sort_keys=True).encode())


def stale(investigation: str, env: Environment) -> tuple[Json, list[str]]:
    """The manifest as it should be (from the sources), and the keys whose PNG is out of date."""
    folder = directory(investigation)
    formulas: dict[str, str] = json.loads((folder / 'formulas.json').read_text())
    manifest_path = folder / 'manifest.json'
    old: Json = json.loads(manifest_path.read_text()) if manifest_path.exists() else {'formulas': {}}
    old_variants = {k: v for f in old['formulas'].values() for k, v in f['variants'].items()}
    katex, renderer = katex_version(env, old.get('katex')), sha256((TOOLS / 'formulas.mjs').read_bytes())
    manifest: Json = {'katex': katex, 'renderer': renderer, 'settings': SETTINGS, 'formulas': {}}
    out_of_date: list[str] = []
    for key, (base, latex, dim) in variants(formulas).items():
        entry = manifest['formulas'].setdefault(base, {'latexSha256': sha256(formulas[base].encode()), 'variants': {}})
        source = source_hash(latex, dim, katex, renderer)
        png = folder / f'{key}.png'
        previous = old_variants.get(key, {})
        if previous.get('sourceSha256') != source or not png.exists() or sha256(png.read_bytes()) != previous.get('pngSha256'):
            out_of_date.append(key)
            previous = {}
        entry['variants'][key] = {'sourceSha256': source, **{k: previous[k] for k in ('pngSha256', 'width', 'height', 'minFontPx') if k in previous}}
    return manifest, out_of_date


def build(investigation: str, env: Environment, check: bool = False) -> Json:
    """Render the stale PNGs, remove those no formula makes, and write the manifest."""
    folder = directory(investigation)
    manifest, out_of_date = stale(investigation, env)
    expected = {k for f in manifest['formulas'].values() for k in f['variants']}
    orphans = [p for p in folder.glob('*.png') if p.stem not in expected]
    if check:
        if out_of_date or orphans:
            raise SystemExit(f'{investigation}: stale formula images {out_of_date + [p.name for p in orphans]}; run python3 tools/video/formulas.py --investigation {investigation}')
        return manifest
    if out_of_date:
        work = LAB / 'build' / 'formulas' / Path(investigation)
        work.mkdir(parents=True, exist_ok=True)
        all_variants = variants(json.loads((folder / 'formulas.json').read_text()))
        jobs = [{'key': k, 'latex': all_variants[k][1], 'dim': all_variants[k][2], 'output': str(folder / f'{k}.png')} for k in out_of_date]
        spec: Json = {'nodeModules': str(env.node_modules), **SETTINGS, 'metrics': str(work / 'metrics.json'), 'jobs': jobs}
        (work / 'jobs.json').write_text(json.dumps(spec, indent=1))
        process_env = dict(os.environ, PUPPETEER_CACHE_DIR=str(env.puppeteer_cache))
        run([env.node, TOOLS / 'formulas.mjs', work / 'jobs.json'], work / 'formulas.log', env=process_env)
        metrics = {m['key']: m for m in json.loads((work / 'metrics.json').read_text())}
        for f in manifest['formulas'].values():
            for key, entry in f['variants'].items():
                if key in metrics:
                    m = metrics[key]
                    entry.update(pngSha256=sha256((folder / f'{key}.png').read_bytes()), width=m['width'], height=m['height'], minFontPx=round(m['minFontPx'], 2))
    for p in orphans:
        p.unlink()
    (folder / 'manifest.json').write_text(json.dumps(manifest, indent=1) + '\n')
    log(f'Formulas: {len(expected)} images, {len(out_of_date)} rendered.')
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser(description=(__doc__ or '').split('\n\n')[0])
    parser.add_argument('--investigation', required=True, help='engine/feature, e.g. o3de/lighting')
    parser.add_argument('--check', action='store_true', help='render nothing; fail if an image is stale')
    args = parser.parse_args()
    build(args.investigation, Environment(), args.check)


if __name__ == '__main__':
    main()
