"""The supporting animations a story owes and plays, read from its prepared timeline: each beat's explanation density
(its heading's `density=`, 0 to 1, which the builder writes to the timeline's beat; 1 when left out) and its measured
length. A beat of density d and length T owes T * d / 6 s animations; a chapter owes the ceiling of its beats' sum,
and a beat goes at most 12 s / d without an animation playing or starting (no limit at 0).

What it counts, from the timeline's keys, is each animation a key starts: values blended over time (a sweep, a
range over words, a camera move, an inset's parameters), a value switched at once after the beat's first frame (a
step revealed), a clip that takes time (`play`, `playOver`), a camera path or orbit, an action (`run`). A camera wave
(a sway) never counts, nor a camera path that runs from the beat's first frame through all of it (a float), nor do
the values the beat's first frame sets up. Keys on one frame are one animation. The count is an upper bound: whether
each animation supports its sentence is the review's call
(new-story, Phase 6), so a story that passes here can still fail there, but one that fails here fails there too.

    python3 tools/video/check.py --package paper_restir --animations
"""
from __future__ import annotations

import json
import math
import re
import subprocess
from pathlib import Path
from typing import Any

from config import Config

Json = dict[str, Any]

# One supporting animation every SECONDS at density 1, and no gap longer than GAP.
SECONDS = 6.0
GAP = 12.0
# A clip's name and length as `render-lab clips` lists them: `name (1.00 s)`.
CLIP = re.compile(r'(\S+) \(([0-9.]+) s\)\Z')


def clip_lengths(binary: Path, package: str) -> dict[str, float]:
    """Each clip's length in seconds, as the investigation's scripts declare it."""
    out = subprocess.run([str(binary), '--package', package, 'clips'], check=True, capture_output=True, text=True).stdout
    return {m[1]: float(m[2]) for line in out.splitlines() if (m := CLIP.match(line))}


def _frames(seconds: Any, fps: int, rest: int) -> int:
    """A length in seconds as frames; without one, the rest of the beat."""
    return round(seconds * fps) if isinstance(seconds, (int, float)) else rest


def span(key: Json, fps: int, clips: dict[str, float], beat: tuple[int, int]) -> int | None:
    """How many frames the animation a key starts runs (0 for a step), or None when it starts none."""
    rest = beat[1] - key['frame']
    lengths = []
    if key.get('blend', 0) > 0 and (key.get('params') or key.get('imageParams') or key.get('camera') or key.get('playOver')):
        lengths.append(key['blend'])
    elif (key.get('params') or key.get('imageParams')) and key['frame'] > beat[0]:
        # A value switched at once mid-beat reveals a step; the beat's first frame only sets the scene up.
        lengths.append(0)
    if clips.get(key.get('play') or '', 0.0) > 0:
        lengths.append(round(clips[key['play']] * fps))
    for motion in key.get('animate') or []:
        # A path or orbit takes the camera or a value somewhere; a wave only sways it, and a camera path from the
        # beat's first frame through its whole length only floats it.
        kind, body = next(iter(motion.items()))
        frames = _frames(body.get('seconds'), fps, rest)
        camera = all(name.startswith('camera.') for name in body.get('params') or ['camera'])
        floats = kind == 'path' and camera and key['frame'] == beat[0] and frames >= rest - 1
        if kind != 'wave' and not floats:
            lengths.append(frames)
    if key.get('run'):
        lengths.append(_frames((key['run'].get('args') or {}).get('seconds'), fps, rest))
    return max(lengths) if lengths else None


def longest_gap(start: int, end: int, spans: list[tuple[int, int]]) -> int:
    """The longest stretch of [start, end) with no animation playing or starting."""
    gap, covered = 0, start
    for a, b in sorted(spans):
        gap = max(gap, a - covered)
        covered = max(covered, b)
    return max(gap, end - covered)


def beat_entry(beat: Json, keys: list[Json], fps: int, clips: dict[str, float]) -> Json:
    start, end = beat['startFrame'], beat['startFrame'] + beat['frames']
    starts: dict[int, int] = {}
    for key in keys:
        # Keys on one frame start one animation, as long as the longest of them.
        length = span(key, fps, clips, (start, end))
        if length is not None and start <= key['frame'] < end:
            starts[key['frame']] = max(starts.get(key['frame'], 0), length)
    seconds = beat['frames'] / fps
    d = float(beat.get('density', 1.0))
    gap = longest_gap(start, end, [(f, f + n) for f, n in starts.items()]) / fps
    limit = GAP / d if d > 0 else None
    return {'beat': beat['id'], 'seconds': round(seconds, 2), 'density': d, 'owes': seconds * d / SECONDS,
            'plays': len(starts), 'gap': round(gap, 1), 'maxGap': round(limit, 1) if limit else None,
            'ok': limit is None or gap <= limit}


def report(cfg: Config, binary: Path) -> Json:
    """Per chapter, each beat's seconds, density, the animations it owes and plays, and its longest gap."""
    path = cfg.timeline()
    if not path.exists():
        return {'renderLabAnimations': 1, 'measured': False, 'ok': True,
                'note': 'not measured: prepare the edition (build.py --prepare-only) for each beat\'s length'}
    timeline = json.loads(path.read_text())
    fps = timeline['fps']
    clips = clip_lengths(binary, cfg.name)
    by_beat: dict[str, list[Json]] = {}
    for key in timeline['keys']:
        by_beat.setdefault(key.get('source') or '', []).append(key)
    chapters: dict[int, Json] = {}
    for beat in timeline['beats']:
        # The closing notices are the builder's own card, not a beat of the story.
        if beat['id'] == 'notices':
            continue
        chapter = chapters.setdefault(beat['chapter'], {'chapter': beat['chapter'], 'beats': []})
        chapter['beats'].append(beat_entry(beat, by_beat.get(beat['id'], []), fps, clips))
    for chapter in chapters.values():
        # A chapter rounds what its beats owe up, and passes when it plays that many and no beat waits too long.
        beats = chapter['beats']
        chapter['seconds'] = round(sum(b['seconds'] for b in beats), 2)
        chapter['needs'] = math.ceil(round(sum(b['owes'] for b in beats), 6))
        chapter['plays'] = sum(b['plays'] for b in beats)
        chapter['ok'] = chapter['plays'] >= chapter['needs'] and all(b['ok'] for b in beats)
        for b in beats:
            b['owes'] = round(b['owes'], 2)
    out = list(chapters.values())
    return {'renderLabAnimations': 1, 'measured': True, 'lang': cfg.lang, 'ok': all(c['ok'] for c in out), 'chapters': out}


def text(result: Json) -> str:
    if not result['measured']:
        return result['note']
    lines = []
    for c in result['chapters']:
        verdict = 'ok' if c['ok'] else 'FAILS'
        lines.append(f'chapter {c["chapter"]}: {c["seconds"]:.1f} s, plays {c["plays"]} of {c["needs"]} needed: {verdict}')
        for b in c['beats']:
            limit = f'of {b["maxGap"]} s' if b['maxGap'] is not None else 's, no limit'
            flag = '' if b['ok'] else '  <- gap too long'
            lines.append(f'  {b["beat"]}: {b["seconds"]:.1f} s at density {b["density"]:g}, owes {b["owes"]:.2f}, plays {b["plays"]}, '
                         f'longest gap {b["gap"]} {limit}{flag}')
    return '\n'.join(lines)
