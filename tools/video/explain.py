"""One beat explained: its sentences, its cues in source order, what each writes and names, and which writes take a
channel over. Untimed unless a measured timeline exists; then each cue's frame and seconds too. Standard library only.

    python3 tools/video/check.py --investigation godot/ssr --explain c1-room
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import markup
from config import LAB, Config
from locations import describe

Json = dict[str, Any]

# Key fields that name something the lab resolves (a resource, an entry, an action), not a channel.
NAMES = ('demo', 'callout', 'code', 'record', 'panel', 'image', 'imageFrom', 'imageGraph', 'imageShow', 'imageTitle', 'imageMarkText',
         'labelText', 'run', 'stop', 'highlightPasses', 'highlightCode', 'highlightFields', 'formula')


def _at(source: Json) -> str:
    """A location, its file relative to the repository."""
    file = source.get('file')
    if file and Path(file).is_absolute() and Path(file).is_relative_to(LAB):
        source = {**source, 'file': str(Path(file).relative_to(LAB))}
    return describe(source)


def writes(key: Json) -> list[Json]:
    """The channels a key writes: values it sets (blended over the key's `blend`) and the motions it starts."""
    out: list[Json] = []
    for name, value in (key.get('params') or {}).items():
        out.append({'channel': name, 'how': 'set', 'value': value})
    for field, value in (key.get('camera') or {}).items():
        out.append({'channel': f'camera.{field}', 'how': 'set', 'value': value})
    for field, value in (key.get('freeCamera') or {}).items():
        out.append({'channel': f'freeCamera.{field}', 'how': 'set', 'value': value})
    for motion in key.get('animate') or []:
        kind, body = next(iter(motion.items()))
        channels = body.get('params') or [body.get('param')]
        for c in channels:
            out.append({'channel': c, 'how': kind, 'length': body.get('seconds')})
    return out


def names(key: Json) -> Json:
    return {k: key[k] for k in NAMES if k in key and key[k] not in (None, [], {})}


def measured(cfg: Config) -> list[tuple[Json, Json]] | None:
    """The built timeline's keys with their provenance, if this edition has been prepared."""
    path = cfg.timeline()
    sidecar = path.with_suffix('.provenance.json')
    if not path.exists() or not sidecar.exists():
        return None
    timeline = json.loads(path.read_text())
    provenance = json.loads(sidecar.read_text())['keys']
    return [(timeline, {**k, **p}) for k, p in zip(timeline['keys'], provenance)]


def _same_place(a: Json, b: Json) -> bool:
    return bool(a) and bool(b) and all(a.get(f) == b.get(f) for f in ('file', 'line', 'column'))


def explain(cfg: Config, board: Json, cues: list[Json], beat_id: str) -> Json:
    beat = next((b for c in board['chapters'] for b in c['beats'] if b['id'] == beat_id), None)
    if beat is None:
        known = [b['id'] for c in board['chapters'] for b in c['beats']]
        raise SystemExit(f'no beat {beat_id!r}; the beats are {", ".join(known)}')
    chapter = next(c for c in board['chapters'] if any(b is beat for b in c['beats']))
    timed = measured(cfg)
    fps = timed[0][0]['fps'] if timed else None
    tl_beat = next((b for b in timed[0][0]['beats'] if b['id'] == beat_id), None) if timed else None
    sentences = []
    for i, s in enumerate(beat.get('narration', [])):
        parsed = markup.sentence(s)
        sentences.append({'paragraph': i + 1, 'at': _at(s.get('source') or {}), 'shown': markup.readable(parsed.caption), 'spoken': parsed.spoken})
    out_cues = []
    history: dict[str, list[int]] = {}
    warnings: list[str] = []
    for n, cue in enumerate(c for c in cues if c['beat'] == beat_id):
        entry: Json = {'index': n, 'kind': cue['kind'], 'paragraph': cue['paragraph'], 'piece': cue['piece'], 'at': _at(cue['source']), 'key': cue['key'],
                       'writes': writes(cue['key']), 'names': names(cue['key'])}
        if timed:
            frames = sorted({k['frame'] for _, k in timed if k.get('beat') == beat_id and _same_place(k.get('source') or {}, cue['source'])})
            entry['frames'] = frames
            entry['seconds'] = [round(f / fps, 3) for f in frames]
        for w in entry['writes']:
            before = history.setdefault(w['channel'], [])
            if before:
                w['takesOverFrom'] = before[-1]
                prior = out_cues[before[-1]]
                # Two values set for one channel on the same word: the first never shows. A set and then a motion
                # there is a start and a move; a range's opening value after its own jump is one range.
                same_word = prior['paragraph'] == entry['paragraph'] and prior['piece'] == entry['piece'] and entry['paragraph'] is not None
                both_set = w['how'] == 'set' and any(x['channel'] == w['channel'] and x['how'] == 'set' for x in prior['writes'])
                own_range = prior['kind'] == 'range-start' and cue['kind'] == 'open'
                if same_word and both_set and not own_range and cue['kind'] != 'range-start':
                    warnings.append(f'cue {n} ({entry["at"]}) sets {w["channel"]} on the same word as cue {before[-1]} does; the first value never shows')
            before.append(n)
        out_cues.append(entry)
    return {
        'renderLabExplain': 1, 'investigation': cfg.investigation, 'lang': cfg.lang, 'chapter': chapter['id'], 'beat': beat_id,
        'kind': beat.get('kind'), 'headline': beat.get('headline'), 'at': _at(beat.get('source') or {}),
        'timing': ({'measured': True, 'startFrame': tl_beat['startFrame'], 'frames': tl_beat['frames'], 'fps': fps} if tl_beat
                   else {'measured': False, 'note': 'not measured: prepare the edition (build.py --prepare-only) for frames and seconds'}),
        'sentences': sentences, 'cues': out_cues,
        'channels': {c: ix for c, ix in history.items()}, 'warnings': warnings,
    }


def text(report: Json) -> str:
    lines = [f'{report["beat"]} ({report["kind"]}, chapter {report["chapter"]}) at {report["at"]}: {report["headline"]}']
    t = report['timing']
    lines.append(f'  frames {t["startFrame"]}..{t["startFrame"] + t["frames"]} at {t["fps"]} fps' if t['measured'] else f'  {t["note"]}')
    for s in report['sentences']:
        lines.append(f'  [{s["paragraph"]}] {s["at"]}: {s["shown"]}')
        if s['spoken'] != s['shown']:
            lines.append(f'      spoken: {s["spoken"]}')
    for c in report['cues']:
        when = f' frames {c["frames"]}' if c.get('frames') else ''
        lines.append(f'  cue {c["index"]} {c["kind"]} (paragraph {c["paragraph"]}) {c["at"]}{when}')
        for w in c['writes']:
            detail = f'= {w["value"]}' if w['how'] == 'set' else f'{w["how"]} for {w.get("length")}'
            over = f', takes over from cue {w["takesOverFrom"]}' if 'takesOverFrom' in w else ''
            lines.append(f'      writes {w["channel"]} {detail}{over}')
        for k, v in c['names'].items():
            lines.append(f'      names {k}: {v}')
    for w in report['warnings']:
        lines.append(f'  warning: {w}')
    return '\n'.join(lines)


def preview_frames(timeline: Json, provenance: list[Json], beat_id: str) -> list[tuple[int, str]]:
    """The frames worth looking at in one beat: its start and end, each key's frame and the one before it, each
    motion's last frame, and the midpoint; each with why."""
    beat = next(b for b in timeline['beats'] if b['id'] == beat_id)
    start, end = beat['startFrame'], beat['startFrame'] + beat['frames'] - 1
    fps = timeline['fps']
    out: dict[int, str] = {start: 'beat start', end: 'beat end', (start + end) // 2: 'midpoint'}
    for key, p in zip(timeline['keys'], provenance):
        if p.get('beat') != beat_id or not start <= key['frame'] <= end:
            continue
        at = _at(p.get('source') or {})
        out.setdefault(key['frame'], f'cue at {at}')
        if key['frame'] - 1 >= start:
            out.setdefault(key['frame'] - 1, f'before the cue at {at}')
        if key.get('blend', 0) > 0:   # a timeline's blend is in frames
            out.setdefault(min(end, key['frame'] + int(key['blend']) - 1), f'end of the blend from {at}')
        for motion in key.get('animate') or []:
            seconds = next(iter(motion.values())).get('seconds')
            if isinstance(seconds, (int, float)):
                last = min(end, key['frame'] + int(round(seconds * fps)) - 1)
                out.setdefault(last, f'end of a motion from {at}')
    return sorted(out.items())


def write_preview_index(directory: Path, frames: list[tuple[int, str]]) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    (directory / 'index.json').write_text(json.dumps([{'frame': f, 'file': f'frame-{f:06d}.png', 'why': why} for f, why in frames], indent=1, ensure_ascii=False) + '\n')
