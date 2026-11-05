"""A 2D scene that presents the frame graph: a beat's `graph: <name>`.

The package's `graphs/<name>.toml` (its resource `graph/<name>`, which the
viewer's DAG panel groups its passes by too) groups the frame's passes (and resources)
into sections; the timeline carries them, their titles in the edition's
language, and the lab draws the scene from its live render graph
(lab/src/graph/scene.rs `FrameGraph::scene_graph`, in lab/src/diagram.rs): a box
per section holding its passes, an edge wherever a pass of one section writes
what a pass of another reads, the edges a longer path implies left out. A
beat's `focus:` names sections (the box lit) or passes and resources (their
row lit). A section of nothing the lab's frame runs (built offline, once, or
on the CPU, or not ported) is a dashed box, joined by the edges it declares.

    [[section]]
    id = "CULL"                  # what a beat's `focus:` names
    title-text = "frame-cull"    # a key of the package's text
    passes = ["o3deLightCullingTilePrepareCS", "o3deLightCullingCS"]

    [[section]]
    id = "TARGETS"
    title-text = "frame-targets"
    resources = ["diffuse", "specular"]

    [[section]]
    id = "LUT"
    title-text = "frame-lut"
    note-text = "frame-lut_note" # a second line, for a box without members
    outside = true               # dashed: not a pass of the lab's frame
    to = ["FWD"]                 # its edges (also `from`)

A lit box holds a picture of what it makes, captured from the frame the lab
renders unseen behind the scene: its last pass's result, its last resource as
the frame leaves it, or `picture = "<pass>.<resource>"` (drawn as the
investigation's `SceneShading::shows` says). When the pictures would make the
labels smaller than the lab's minimum, the view stays at it, on the lit boxes;
when even those do not fit, the boxes show no pictures.

Every pass of the frame belongs to a section, but the lab's own overlay and
composite (and those in the file's top-level `omit`); the lab checks that,
and the passes and resources, when it loads the timeline.
"""
from __future__ import annotations

import json
import tomllib
from pathlib import Path
from typing import Any

from config import Config

Json = dict[str, Any]


def path(cfg: Config, name: str) -> Path:
    """The package's sections: its resource `graph/<name>`."""
    return cfg.package / 'graphs' / f'{name}.toml'


def load(cfg: Config, name: str) -> Json:
    """The scene as the timeline carries it (lab/src/graph/scene.rs `GraphScene`):
    its sections, their titles and notes in the edition's language."""
    spec = tomllib.loads(path(cfg, name).read_text(encoding='utf-8'))
    text = json.loads((cfg.package / 'text' / f'{cfg.lang}.json').read_text(encoding='utf-8'))

    def say(key: str, where: str) -> str:
        if key not in text:
            raise RuntimeError(f'{path(cfg, name)}: {where}: no text `{key}`')
        return text[key]
    sections: list[Json] = []
    for s in spec.get('section', []):
        out: Json = {'id': s['id'], 'title': say(s['title-text'], s['id'])}
        if 'note-text' in s:
            out['note'] = say(s['note-text'], s['id'])
        for field in ('passes', 'resources', 'to', 'from'):
            if s.get(field):
                out[field] = list(s[field])
        if 'picture' in s:
            out['picture'] = str(s['picture'])
        if s.get('outside'):
            out['outside'] = True
        sections.append(out)
    ids = {s['id'] for s in sections}
    for s in sections:
        for other in s.get('to', []) + s.get('from', []):
            if other not in ids:
                raise RuntimeError(f'{path(cfg, name)}: {s["id"]}: no section `{other}`')
    return {'sections': sections, 'omit': list(spec.get('omit', []))}


def names(scene: Json) -> set[str]:
    """What a beat's `focus:` may name: sections, their passes and resources."""
    return {s['id'] for s in scene['sections']} | {m for s in scene['sections'] for m in s.get('passes', []) + s.get('resources', [])}


def markdown(scene: Json) -> list[str]:
    """The scene in a document: each section and what it holds."""
    lines = []
    for s in scene['sections']:
        held = ', '.join(f'`{m}`' for m in s.get('passes', []) + s.get('resources', []))
        note = f' ({s["note"]})' if 'note' in s else ''
        lines.append(f'- **{s["title"]}**{note}' + (f': {held}' if held else ''))
    return lines + ['']
