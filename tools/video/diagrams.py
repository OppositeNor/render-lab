"""A story's diagrams: a beat's `diagram: <name>`, the package's
`video/diagrams/<name>.toml`, drawn by the lab (lab/src/graph_view.rs; the
timeline carries it as a `graph_view::Spec`, its text in the edition's
language). Boxes, and the edges between them:

    [[node]]
    id = "FWD1"                         # what a beat's `focus:` names
    title-text = "diagram-fwd1-forward" # a key of the package's text (a newline breaks a line)
    dashed = true                       # optional: a dashed box
    image = "o3de_lighting.image/room"  # optional: a PNG resource under its title (one node of
                                        # an image is how a 2D beat shows a picture)

    [[node.row]]                        # optional: rows under the title
    id = "LIGHTS"
    label-text = "diagram.fwd1.lights"

    [[edge]]
    from = "DEPTH"
    to = "FWD1"
    label-text = "diagram.fwd1.why"     # optional
    dotted = true                       # optional

An edge that closes a cycle is drawn as an arc back. (The frame graph's own
scene, `graph: frame`, is drawn from the render graph: tools/video/frame_graph.py.)
"""
from __future__ import annotations

import tomllib
from pathlib import Path
from typing import Any

import config
from config import Config

Json = dict[str, Any]


def path(cfg: Config, name: str) -> Path:
    return cfg.diagrams / f'{name}.toml'


def load(cfg: Config, name: str) -> Json:
    """The diagram as the timeline carries it, its text in the edition's language."""
    where = path(cfg, name)
    spec = tomllib.loads(where.read_text(encoding='utf-8'))
    text = config.text_table(cfg.package.name, cfg.lang)

    def say(key: str, at: str) -> str:
        if key not in text:
            raise RuntimeError(f'{where}: {at}: no text `{key}`')
        return text[key]
    nodes: list[Json] = []
    for n in spec.get('node', []):
        # Each box: its title's lines, its rows, and its optional dash and picture.
        node: Json = {'id': n['id'], 'title': say(n['title-text'], n['id']).split('\n')}
        if n.get('row'):
            node['rows'] = [{'id': r['id'], 'label': say(r['label-text'], r['id'])} for r in n['row']]
        if n.get('dashed'):
            node['dashed'] = True
        if n.get('image'):
            node['image'] = str(n['image'])
        nodes.append(node)
    ids = {n['id'] for n in nodes}
    edges: list[Json] = []
    for e in spec.get('edge', []):
        # Each edge must join two boxes; its label is translated like the boxes' text.
        for end in (e['from'], e['to']):
            if end not in ids:
                raise RuntimeError(f'{where}: an edge names `{end}`, which is no node')
        edge: Json = {'from': e['from'], 'to': e['to']}
        if e.get('dotted'):
            edge['dotted'] = True
        if 'label-text' in e:
            edge['label'] = say(e['label-text'], f'{e["from"]} -> {e["to"]}')
        edges.append(edge)
    return {'nodes': nodes, 'edges': edges}


def names(spec: Json) -> set[str]:
    """What a beat's `focus:` may name: its nodes and their rows."""
    return {n['id'] for n in spec['nodes']} | {r['id'] for n in spec['nodes'] for r in n.get('rows', [])}


def markdown(spec: Json) -> list[str]:
    """The diagram in a document: its boxes, then its edges."""
    title = {n['id']: ' '.join(n['title']) for n in spec['nodes']}
    lines = []
    for n in spec['nodes']:
        rows = ', '.join(r['label'] for r in n.get('rows', []))
        lines.append(f'- **{title[n["id"]]}**' + (f': {rows}' if rows else '') + (' (dashed)' if n.get('dashed') else ''))
    for e in spec['edges']:
        lines.append(f'- {title[e["from"]]} → {title[e["to"]]}' + (f' ({e["label"]})' if e.get('label') else ''))
    return lines + ['']
