"""From the storyboard and the measured narration to the timeline the lab
renders (src/lab/shot.rs `Timeline`): sentences placed one after another,
chapters stretched to their time budgets with extra pauses, shot keys at
sentence starts (`sequence` keys spread over the sentence) and at the words
their tags stand before (markup.py), and captions timed by their words.
"""
from __future__ import annotations

import math
import re
import textwrap
from typing import Any

import markup
import story
import zh
from config import Config
from narration import Durations, Narration

LEAD = 0.40      # silence before a beat's first sentence
GAP = 0.32       # silence between sentences
TAIL = 0.70      # silence after a beat's last sentence
NOTICES = 8.0    # the closing card of the package's legal notices
MAX_EXTRA_PAUSE = 2.2
CAPTION_WIDTH = 58
KEY_FIELDS = {'demo', 'camera', 'record', 'highlight', 'panel', 'params', 'callout', 'code', 'animate', 'formula', 'term', 'run', 'stop', 'image', 'imageTitle', 'imageCorner', 'imageFrom', 'imageParams', 'imageHide', 'imageClick', 'imageMark', 'imageMarkText', 'arrow', 'arrowFrom', 'arrowTo', 'arrowColor', 'arrowWidth', 'arrows', 'arrowHide', 'label', 'labelAt', 'labelText', 'labelColor', 'labelHide'}
# How long a lit term takes to fade in when its cue names no blend (seconds).
TERM_FADE = 0.4
# How a key's blend moves (src/lab/shot.rs `Ease`); smooth when a key names none.
EASES = ('smooth', 'linear', 'in', 'out', 'inOut')

Board = dict[str, Any]
Json = dict[str, Any]


def sentences_of(beat: Json) -> list[Json]:
    return [item if isinstance(item, dict) else {'text': item} for item in beat.get('narration', [])]


def all_sentences(board: Board) -> list[Json]:
    return [s for chapter in board['chapters'] for beat in chapter['beats'] for s in sentences_of(beat)]


def balanced_lines(text: str) -> list[str]:
    """One line if it fits, otherwise two lines split near the middle."""
    if len(text) <= CAPTION_WIDTH:
        return [text]
    words = text.split()
    best: tuple[int, list[str]] | None = None
    for i in range(1, len(words)):
        a, b = ' '.join(words[:i]), ' '.join(words[i:])
        if len(a) <= CAPTION_WIDTH and len(b) <= CAPTION_WIDTH:
            score = abs(len(a) - len(b))
            if best is None or score < best[0]:
                best = (score, [a, b])
    return best[1] if best else textwrap.wrap(text, CAPTION_WIDTH)


def caption_chunks(text: str) -> list[str]:
    """The fewest cues of at most two balanced lines, cut at word boundaries
    near equal character positions."""
    words = text.split()
    n = max(1, math.ceil(len(text) / (2 * CAPTION_WIDTH - 6)))
    while True:
        total = len(text)
        bounds: list[int] = []
        pos, k = 0, 1
        for i, w in enumerate(words[:-1]):
            pos += len(w) + 1
            if k < n and pos >= total * k / n:
                prev = pos - len(w) - 1
                if bounds and bounds[-1] == i:
                    pass
                elif i > 0 and abs(prev - total * k / n) < abs(pos - total * k / n) and (not bounds or bounds[-1] < i):
                    bounds.append(i)
                else:
                    bounds.append(i + 1)
                k += 1
        cuts = [0] + bounds + [len(words)]
        chunks = [' '.join(words[a:b]) for a, b in zip(cuts, cuts[1:]) if b > a]
        lines = [balanced_lines(c) for c in chunks]
        if all(len(group) <= 2 and all(len(x) <= CAPTION_WIDTH for x in group) for group in lines):
            return ['\n'.join(group) for group in lines]
        n += 1


def plan_chapter(chapter: Json, durations: Durations) -> tuple[float, list[float], float]:
    """A chapter's natural length, and the extra pauses that fit it to its budget."""
    natural = 0.0
    weights: list[float] = []
    for beat in chapter['beats']:
        natural += LEAD + TAIL + beat.get('hold', 0.0)
        for s in sentences_of(beat):
            natural += durations[s['text']][1] + GAP + s.get('pause', 0.0)
            weights.append(2.0 if s.get('keys') or s.get('sequence') or markup.has_tags(s.get('en', s['text'])) else 1.0)
    deficit = max(0.0, chapter['targetSeconds'] - natural)
    total_weight = sum(weights) or 1.0
    extras = [min(MAX_EXTRA_PAUSE, deficit * w / total_weight) for w in weights]
    return natural, extras, deficit - sum(extras)


def motion_seconds(motion: Json, frame: int, ends: dict[str, int], fps: int, source: str) -> Json:
    """A motion (src/lab/animation.rs) with its `seconds` in seconds: `"sentence"`
    or `"beat"` runs it from its key to the end of the sentence or the beat."""
    if len(motion) != 1 or next(iter(motion)) not in ('path', 'orbit', 'wave'):
        raise RuntimeError(f'{source}: a motion is one of path, orbit, or wave, not {sorted(motion)}')
    kind, body = next(iter(motion.items()))
    seconds = body.get('seconds')
    if isinstance(seconds, str):
        if seconds not in ends:
            raise RuntimeError(f'{source}: a {kind} here can run for seconds, or until {" or ".join(repr(e) for e in ends)}, not {seconds!r}')
        body = {**body, 'seconds': round((ends[seconds] - frame) / fps, 3)}
        if body['seconds'] <= 0:
            raise RuntimeError(f'{source}: the {kind} starts after the end of its {seconds}')
    return {kind: body}


def make_key(k: Json, base_frame: int, fps: int, source: str, ends: dict[str, int]) -> Json:
    """A timeline key; `ends` are the frames where the key's sentence and beat end."""
    unknown = set(k) - KEY_FIELDS - {'delay', 'blend', 'ease'}
    if unknown:
        raise RuntimeError(f'{source}: unknown key fields {sorted(unknown)}')
    out: Json = {'frame': base_frame + int(round(k.get('delay', 0.0) * fps)), 'blend': int(round(k.get('blend', 0.0) * fps)), 'source': source}
    if 'ease' in k:
        if k['ease'] not in EASES:
            raise RuntimeError(f'{source}: unknown ease {k["ease"]!r} (one of {", ".join(EASES)})')
        out['ease'] = k['ease']
    # In the storyboard's order, so the timeline is the same from run to run.
    for name in (n for n in k if n in KEY_FIELDS):
        out[name] = k[name]
    if 'animate' in k:
        out['animate'] = [motion_seconds(m, out['frame'], ends, fps, source) for m in k['animate']]
    return out


def numbers(params: Json, where: str) -> Json:
    """A point cue's parameters as values: numbers, flags, lists of numbers,
    or names (markup.param_value)."""
    out: Json = {}
    for name, value in params.items():
        if isinstance(value, str) and ('..' in value or value.startswith('=')):
            raise markup.MarkupError(f'{where}: {name}={value} blends over a range; write it in {{@name> …}} … {{<name}}')
        out[name] = markup.param_value(value, where) if isinstance(value, str) else value
    return out


def tag_keys(s: Json, beat: Json, narration: Narration, fps: int, start: int, where: str) -> list[Json]:
    """The keys of a sentence's tags, as storyboard keys with `delay` (from
    the sentence's start) on the frame its word starts, and a range's blend
    ending on the frame its closing word starts."""
    parsed = markup.parse(s.get('en', s['text']))
    cues = {**beat.get('cues', {}), **s.get('cues', {})}
    words = narration.words

    def frame_of(mark: int, tag: markup.Tag) -> int:
        # Before the first word: the sentence's start, as its `keys`.
        if mark == 0:
            return 0
        if mark >= len(words):
            if tag.kind != 'close':
                raise markup.MarkupError(f'{where}: a tag after the last word cues nothing; move it before a word')
            return int(math.ceil(narration.seconds * fps))
        return int(round(words[mark] * fps))

    frames = [frame_of(m, t) for m, t in zip(narration.marks, parsed.tags)]
    closes = {t.name: f for t, f in zip(parsed.tags, frames) if t.kind == 'close'}
    keys: list[Json] = []
    for tag, frame in zip(parsed.tags, frames):
        if tag.kind == 'close':
            continue
        key = markup.cue_key(tag, cues, where)
        delay = frame / fps
        if tag.kind == 'open':
            length = closes[tag.name] - frame
            if length <= 0:
                raise markup.MarkupError(f'{where}: the range {tag.name!r} closes where it opens')
            if not tag.attrs:
                # A named cue over the range: it blends there, its motions run for it.
                key = {**key, 'blend': length / fps}
                key['animate'] = [{kind: {**body, 'seconds': length / fps} if body.get('seconds') == 'range' else body} for m in key.get('animate', []) for kind, body in m.items()]
                if not key['animate']:
                    del key['animate']
                keys.append({**key, 'delay': delay})
                continue
            jump: Json = {}
            to: Json = {}
            for name, value in key.pop('params', {}).items():
                if '..' in value:
                    a, b = value.split('..', 1)
                    jump[name] = markup.number(a, where)
                    to[name] = markup.number(b, where)
                else:
                    to[name] = markup.number(value.removeprefix('='), where)
            if jump:
                keys.append({'params': jump, 'delay': delay})
            keys.append({**key, 'params': to, 'blend': length / fps, 'delay': delay})
            continue
        if 'params' in key:
            key['params'] = numbers(key['params'], where)
        if 'term' in key and 'blend' not in key:
            key['blend'] = TERM_FADE
        if key:
            keys.append({**key, 'delay': delay + key.get('delay', 0.0)})
    return keys


def caption_cues(text: str, s: Json, narration: Narration, start: int, end: int, fps: int, cfg: Config) -> list[Json]:
    """A sentence's captions: chunks of at most two lines, formulas kept
    whole, each shown from the frame its first word starts (English; the
    Chinese text follows the English audio by its display width)."""
    caption = markup.parse(text, spoken_only=cfg.zh()).caption
    held, tags = markup.protect(caption)
    chunks = zh.caption_chunks(held) if cfg.zh() else caption_chunks(held)
    starts: list[int] = []
    if cfg.zh() or not narration.words:
        weights = [zh.display_width(c) if cfg.zh() else len(c) for c in chunks]
        t = start
        for w in weights:
            starts.append(t)
            t += max(1, int(round((end - start) * w / sum(weights))))
    else:
        # Each written word stands for one spoken word, a formula for the
        # words said for it, scaled to the words the voice says; a chunk
        # starts when its first word does.
        parsed = markup.parse(s.get('en', s['text']))
        says = iter([len((t.attr('say') or '').split()) for t in parsed.tags if t.kind == 'formula' and t.attr('formula') != 'off'])
        # A formula weighs the words said for it; a code span the words it is said as.
        held = [next(says) if tag.startswith('{@ formula') else len(markup.readable(tag).split()) for tag in tags]

        def weight(word: str) -> int:
            m = re.match(r'QQF(\d+)', word)
            return held[int(m.group(1))] if m else 1
        words = narration.words
        scale = len(words) / max(1, sum(weight(w) for c in chunks for w in c.split()))
        seen = 0
        for c in chunks:
            index = min(len(words) - 1, int(round(seen * scale)))
            starts.append(start + int(round(words[index] * fps)) if starts else start)
            seen += sum(weight(w) for w in c.split())
    out: list[Json] = []
    for i, chunk in enumerate(chunks):
        stop = starts[i + 1] if i + 1 < len(chunks) else end
        out.append({'start': starts[i], 'end': max(starts[i] + 1, min(end, stop)), 'text': markup.restore(chunk, tags)})
    out[-1]['end'] = end
    return out


def build_timeline(board: Board, durations: Durations, cfg: Config) -> tuple[Json, list[Json], list[Json], dict[int, Json]]:
    """The timeline, the narration placements, the caption cues, and the chapter plans."""
    fps = board['fps']
    frame = 0
    beats_out: list[Json] = []
    keys_out: list[Json] = []
    chapters_out: list[Json] = []
    placements: list[Json] = []
    cues: list[Json] = []
    plans: dict[int, Json] = {}

    def to_frames(seconds: float) -> int:
        return int(round(seconds * fps))

    for chapter in board['chapters']:
        natural, extras, leftover = plan_chapter(chapter, durations)
        plans[chapter['number']] = {'natural': natural, 'extraPauses': sum(extras), 'leftover': leftover}
        chapter_start = frame
        sentence_index = 0
        scene_beats = [b for b in chapter['beats'] if story.is_3d(b)] or chapter['beats']
        leftover_per_beat = leftover / len(scene_beats)
        for beat in chapter['beats']:
            beat_start = frame
            cursor = beat_start + to_frames(LEAD)
            captions: list[Json] = []
            pending: list[tuple[list[tuple[Json, int]], int]] = []
            for s in sentences_of(beat):
                narration = durations[s['text']]
                path, seconds, rate = narration.path, narration.seconds, narration.rate
                start = cursor
                end = start + int(math.ceil(seconds * fps))
                placements.append({'path': str(path), 'frame': start, 'rate': rate})
                captions.extend(caption_cues(s['text'], s, narration, start, end, fps, cfg))
                extra = extras[sentence_index] if sentence_index < len(extras) else 0.0
                sentence_index += 1
                # A sentence runs until the next one starts (or the beat ends).
                sentence_keys = [(k, start) for k in s.get('keys', [])]
                sequence = s.get('sequence', [])
                sentence_keys += [({'delay': i * seconds / len(sequence), **k}, start) for i, k in enumerate(sequence)]
                sentence_keys += [(k, start) for k in tag_keys(s, beat, narration, fps, start, beat['id'])]
                cursor = end + to_frames(GAP + s.get('pause', 0.0) + extra)
                pending.append((sentence_keys, cursor))
            hold = beat.get('hold', 0.0) + (leftover_per_beat if beat in scene_beats else 0.0)
            beat_end = cursor - to_frames(GAP) + to_frames(TAIL + hold)
            for i, (sentence_keys, sentence_end) in enumerate(pending):
                ends = {'sentence': beat_end if i == len(pending) - 1 else sentence_end, 'beat': beat_end}
                keys_out.extend(make_key(k, base, fps, beat['id'], ends) for k, base in sentence_keys)
            keys_out.extend(make_key(k, beat_start, fps, beat['id'], {'beat': beat_end}) for k in beat.get('keys', []))
            # A formula card closes with its beat.
            if any('formula' in k for k in keys_out if k.get('source') == beat['id']):
                keys_out.append(make_key({'formula': '', 'term': 0}, beat_end, fps, beat['id'], {'beat': beat_end}))
            # Captions stay up through short gaps.
            for a, b in zip(captions, captions[1:]):
                if b['start'] - a['end'] <= to_frames(0.6):
                    a['end'] = b['start']
            entry: Json = {
                'id': beat['id'], 'chapter': chapter['number'], 'kind': beat['kind'], 'startFrame': beat_start,
                'frames': beat_end - beat_start, 'headline': beat['headline'], 'captions': captions, 'sources': beat.get('sources', []),
            }
            if story.is_diagram(beat):
                entry['image'] = f'mermaid/{beat["id"]}.png'
            beats_out.append(entry)
            cues.extend({'start': c['start'] / fps, 'end': c['end'] / fps, 'text': markup.readable(c['text']), 'beat': beat['id']} for c in captions)
            frame = beat_end
        chapters_out.append({'number': chapter['number'], 'title': chapter['title'], 'startFrame': chapter_start, 'endFrame': frame})
    # The package's legal notices close the video: a silent card at the end of the last chapter.
    notice_frames = 0
    if cfg.notices() and chapters_out:
        notice_frames = to_frames(NOTICES)
        beats_out.append({'id': 'notices', 'chapter': chapters_out[-1]['number'], 'kind': '2d', 'startFrame': frame, 'frames': notice_frames,
                          'headline': cfg.lab_text('ui.notices_title'), 'captions': [], 'sources': [], 'notices': True})
        frame += notice_frames
        chapters_out[-1]['endFrame'] = frame
    keys_out.sort(key=lambda k: k['frame'])
    timeline: Json = {
        'fps': fps, 'width': board['width'], 'height': board['height'], 'totalFrames': frame, 'noticeFrames': notice_frames,
        'title': board['title'], 'engineLabel': cfg.spec['engineLabel'],
        'chapters': chapters_out, 'beats': beats_out, 'keys': keys_out,
    }
    if cfg.zh():
        timeline['locale'] = cfg.lang
    return timeline, placements, cues, plans
