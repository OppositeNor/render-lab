"""From the storyboard and the measured narration to the timeline the lab
renders (src/lab/shot.rs `Timeline`): sentences placed one after another,
chapters stretched to their time budgets with extra pauses, shot keys at
beat starts and at the words their cues stand before (markup.py), and
captions timed by their words.
"""
from __future__ import annotations

import math
import re
import textwrap
from dataclasses import dataclass, fields
from typing import Any

import diagrams
import frame_graph
import markup
import presentation
import zh
from config import Config
from narration import Durations, Narration
from locations import DiagnosticError, describe

NOTICES = 8.0    # the closing card of the package's legal notices
KEY_FIELDS = {'demo', 'camera', 'record', 'highlight', 'panel', 'params', 'callout', 'code', 'animate', 'formula', 'term', 'run', 'stop', 'image', 'imageTitle', 'imageCorner', 'imageFrom', 'imageGraph', 'imageShow', 'imageParams', 'imageHide', 'imageClick', 'imageMark', 'imageMarkText', 'arrow', 'arrowFrom', 'arrowTo', 'arrowColor', 'arrowWidth', 'arrows', 'arrowHide', 'label', 'labelAt', 'labelText', 'labelColor', 'labelHide'}
Board = dict[str, Any]
Json = dict[str, Any]


@dataclass(frozen=True)
class Pacing:
    """How a video is paced and its captions wrapped: attributes of the storyboard's video heading
    (`{#video lead=0.4 gap=0.32 …}`; seconds, and characters per caption line), each with its default when the
    heading leaves it out (the lab's heading keys, lab/src/script/keys.rs `heading_keys`, give it). The zh-CN
    edition's caption width is its translation's heading's `captionWidth` (zh.CAPTION_UNITS display units by
    default)."""
    lead: float           # silence before a beat's first sentence
    gap: float            # silence between sentences
    tail: float           # silence after a beat's last sentence
    maxExtraPause: float  # the most a chapter's budget lengthens one sentence's pause
    taggedWeight: float   # how much more of the budget's pauses a sentence with tags takes than one without
    termFade: float       # how long a lit term fades in when its cue names no blend
    captionBridge: float  # a caption stays up through a gap this short, to the next
    captionWidth: int     # characters per caption line (English)

    @staticmethod
    def of(board: Board) -> Pacing:
        return Pacing(**{f.name: int(board[f.name]) if f.name == 'captionWidth' else float(board[f.name]) for f in fields(Pacing)})


def sentences_of(beat: Json) -> list[Json]:
    return beat.get('narration', [])


def all_sentences(board: Board) -> list[Json]:
    return [s for chapter in board['chapters'] for beat in chapter['beats'] for s in sentences_of(beat)]


# A cue's lines: the video shows its caption as a one-line subtitle over the picture (lab/src/ui/ `for_video`).
CAPTION_LINES = 1


def balanced_lines(text: str, width: int) -> list[str]:
    """One line if it fits, otherwise two lines split near the middle."""
    if len(text) <= width:
        return [text]
    words = text.split()
    best: tuple[int, list[str]] | None = None
    for i in range(1, len(words)):
        # Try every cut between two words, and keep the one whose two lines are closest in length.
        a, b = ' '.join(words[:i]), ' '.join(words[i:])
        if len(a) <= width and len(b) <= width:
            score = abs(len(a) - len(b))
            if best is None or score < best[0]:
                best = (score, [a, b])
    return best[1] if best else textwrap.wrap(text, width)


def caption_chunks(text: str, width: int) -> list[str]:
    """The fewest cues of at most `CAPTION_LINES` balanced lines, cut at word
    boundaries near equal character positions."""
    words = text.split()
    # Start from the fewest chunks the text could fit in, and add one until every chunk fits.
    n = max(1, math.ceil(len(text) / (CAPTION_LINES * width - 6)))
    while True:
        total = len(text)
        bounds: list[int] = []
        pos, k = 0, 1
        for i, w in enumerate(words[:-1]):
            # Walk the words; when the text so far passes the next k/n of the whole, cut near it.
            pos += len(w) + 1
            if k < n and pos >= total * k / n:
                prev = pos - len(w) - 1
                if bounds and bounds[-1] == i:
                    pass
                elif i > 0 and abs(prev - total * k / n) < abs(pos - total * k / n) and (not bounds or bounds[-1] < i):
                    # Cut before this word or after it, whichever lands closer to the ideal position.
                    bounds.append(i)
                else:
                    bounds.append(i + 1)
                k += 1
        cuts = [0] + bounds + [len(words)]
        chunks = [' '.join(words[a:b]) for a, b in zip(cuts, cuts[1:]) if b > a]
        lines = [balanced_lines(c, width) for c in chunks]
        if all(len(group) <= CAPTION_LINES and all(len(x) <= width for x in group) for group in lines):
            # Every chunk fits its lines, so these cuts are the captions.
            return ['\n'.join(group) for group in lines]
        n += 1


def plan_chapter(chapter: Json, durations: Durations, pacing: Pacing) -> tuple[float, list[float], float]:
    """A chapter's natural length, and the extra pauses that fit it to its budget."""
    natural = 0.0
    weights: list[float] = []
    for beat in chapter['beats']:
        # The natural length: each beat's lead, tail, and hold, and each sentence's speech and gap.
        natural += pacing.lead + pacing.tail + beat.get('hold', 0.0)
        for s in sentences_of(beat):
            natural += durations[s['text']][1] + pacing.gap
            # A sentence with cues gets a larger share of the extra pauses, for its cues to play.
            weights.append(pacing.taggedWeight if markup.tagged(s) else 1.0)
    # The budget's shortfall is shared out as extra pauses, each capped; what the caps leave over is returned.
    deficit = max(0.0, chapter['targetSeconds'] - natural)
    total_weight = sum(weights) or 1.0
    extras = [min(pacing.maxExtraPause, deficit * w / total_weight) for w in weights]
    return natural, extras, deficit - sum(extras)


BUDGET_TOLERANCE = 0.10   # how far a chapter's length may be from its budget, as a fraction of it


def budget_rows(timeline: Json, board: Board, plans: dict[int, Json]) -> list[Json]:
    """Each chapter's length against its budget, once its speech is measured: the
    budget, the natural length (speech and pacing alone), the extra pauses the
    budget added, how far the narration alone runs over the budget, and whether
    the chapter is within `BUDGET_TOLERANCE` of it (as verify.py requires of the
    video). The closing notices card is not the last chapter's."""
    rows: list[Json] = []
    fps = timeline['fps']
    for c, b in zip(timeline['chapters'], board['chapters']):
        # The notices card sits in the last chapter, but is not counted against its budget.
        notices = timeline.get('noticeFrames', 0) if c is timeline['chapters'][-1] else 0
        seconds = (c['endFrame'] - c['startFrame'] - notices) / fps
        target = b['targetSeconds']
        plan = plans.get(c['number'], {})
        natural = plan.get('natural', 0.0)
        deviation = (seconds - target) / target
        rows.append({'chapter': c['number'], 'title': c['title'], 'seconds': seconds, 'target': target, 'natural': natural,
                     'extraPauses': plan.get('extraPauses', 0.0), 'over': max(0.0, natural - target),
                     'deviation': deviation, 'within': abs(deviation) <= BUDGET_TOLERANCE})
    return rows


def motion_seconds(motion: Json, frame: int, ends: dict[str, int], fps: int, source: str) -> Json:
    """A motion (src/lab/animation.rs) with its `seconds` in seconds: `"sentence"`
    or `"beat"` runs it from its key to the end of the sentence or the beat."""
    if len(motion) != 1 or next(iter(motion)) not in ('path', 'orbit', 'wave'):
        raise RuntimeError(f'{source}: a motion is one of path, orbit, or wave, not {sorted(motion)}')
    kind, body = next(iter(motion.items()))
    seconds = body.get('seconds')
    if isinstance(seconds, str):
        # A motion that runs to the end of its sentence or beat gets the seconds left until that end.
        if seconds not in ends:
            raise RuntimeError(f'{source}: a {kind} here can run for seconds, or until {" or ".join(repr(e) for e in ends)}, not {seconds!r}')
        body = {**body, 'seconds': round((ends[seconds] - frame) / fps, 3)}
        if body['seconds'] <= 0:
            raise RuntimeError(f'{source}: the {kind} starts after the end of its {seconds}')
    return {kind: body}


def make_key(k: Json, base_frame: int, fps: int, source: str, ends: dict[str, int]) -> Json:
    """A timeline key; `ends` are the frames where the key's sentence and beat end."""
    unknown = set(k) - KEY_FIELDS - {'delay', 'blend', 'ease', '_provenance'}
    if unknown:
        raise RuntimeError(f'{source}: unknown key fields {sorted(unknown)}')
    # A key's delay and blend, in seconds in the storyboard, become frames.
    out: Json = {'frame': base_frame + int(round(k.get('delay', 0.0) * fps)), 'blend': int(round(k.get('blend', 0.0) * fps)), 'source': source}
    if '_provenance' in k:
        out['_provenance'] = k['_provenance']
    if 'ease' in k:
        # One of the lab's easings: the reader checked it against the vocabulary's (markup.cue_key).
        out['ease'] = k['ease']
    # In the storyboard's order, so the timeline is the same from run to run.
    for name in (n for n in k if n in KEY_FIELDS):
        out[name] = k[name]
    if 'animate' in k:
        try:
            out['animate'] = [motion_seconds(m, out['frame'], ends, fps, source) for m in k['animate']]
        except RuntimeError as error:
            raise DiagnosticError('story.timing.motion', str(error), k.get('_provenance', {}).get('source'), beat=source, field='animate') from error
    return out


def tag_keys(s: Json, beat: Json, narration: Narration, fps: int, start: int, where: str, pacing: Pacing) -> list[Json]:
    """The keys of a sentence's tags, as storyboard keys with `delay` (from
    the sentence's start) on the frame its word starts, and a range's blend
    ending on the frame its closing word starts."""
    parsed = markup.sentence(s)
    words = narration.words

    def frame_of(mark: int, tag: markup.Tag) -> int:
        # Before the first word: the sentence's start, as its `keys`.
        if mark == 0:
            return 0
        if mark >= len(words):
            # After the last word only a range's close may stand: it ends with the speech.
            if tag.kind != 'close':
                raise DiagnosticError('story.timing.anchor', 'a tag after the last word cues nothing; move it before a word', tag.source, beat=beat['id'])
            return int(math.ceil(narration.seconds * fps))
        return int(round(words[mark] * fps))

    # Each tag's frame from its word; a range's close is looked up by its name when the range opens.
    frames = [frame_of(m, t) for m, t in zip(narration.marks, parsed.tags)]
    closes = {t.name: f for t, f in zip(parsed.tags, frames) if t.kind == 'close'}
    keys: list[Json] = []
    for tag, frame in zip(parsed.tags, frames):
        if tag.kind == 'close':
            continue
        if tag.key is None:
            raise DiagnosticError('story.timing.key', 'a cue needs a parsed key', tag.source, beat=beat['id'])
        provenance = {'source': tag.source or s.get('source', {}), 'paragraph': s.get('paragraph'), 'piece': tag.piece}
        key = {**tag.key, '_provenance': provenance}
        delay = frame / fps
        if tag.kind == 'open':
            # A range jumps to its start values where it opens, then blends to its end values until it closes.
            length = closes[tag.name] - frame
            if length <= 0:
                raise DiagnosticError('story.timing.range', f'the range {tag.name!r} closes where it opens', tag.source, beat=beat['id'])
            jump, key = markup.range_values(key, describe(tag.source) if tag.source else where)
            if jump:
                keys.append({'params': jump, 'delay': delay, '_provenance': provenance})
            keys.append({**key, 'blend': length / fps, 'delay': delay})
            continue
        if 'term' in key and 'blend' not in key:
            # A lit term fades in over the default time when its cue names no blend.
            key['blend'] = pacing.termFade
        if tag.key:
            keys.append({**key, 'delay': delay + key.get('delay', 0.0)})
    return keys


def caption_cues(s: Json, narration: Narration, start: int, end: int, fps: int, cfg: Config, width: int) -> list[Json]:
    """A sentence's captions: chunks of at most two lines, formulas kept
    whole, each shown from the frame its first word starts (English; the
    Chinese text follows the English audio by its display width)."""
    caption = markup.shown(s, cfg.zh()).caption
    held, tags = markup.protect(caption)
    chunks = zh.caption_chunks(held, width) if cfg.zh() else caption_chunks(held, width)
    starts: list[int] = []
    if cfg.zh() or not narration.words:
        # Without word timings (or for the translation), chunks share the sentence's time by their width.
        weights = [zh.display_width(c) if cfg.zh() else len(c) for c in chunks]
        t = start
        for w in weights:
            starts.append(t)
            t += max(1, int(round((end - start) * w / sum(weights))))
    else:
        # Each written word stands for one spoken word, a formula for the
        # words said for it, scaled to the words the voice says; a chunk
        # starts when its first word does.
        parsed = markup.sentence(s)
        says = iter([len((t.attr('say') or '').split()) for t in parsed.tags if t.kind == 'formula' and t.attr('formula') != 'off'])
        # A formula weighs the words said for it; a code span the words it is said as.
        held = [next(says) if tag.startswith('{ FORMULA(') else len(markup.readable(tag).split()) for tag in tags]

        def weight(word: str) -> int:
            m = re.match(r'QQF(\d+)', word)
            return held[int(m.group(1))] if m else 1
        words = narration.words
        scale = len(words) / max(1, sum(weight(w) for c in chunks for w in c.split()))
        seen = 0
        for c in chunks:
            # Each chunk starts at the spoken word that the written words before it stand for.
            index = min(len(words) - 1, int(round(seen * scale)))
            starts.append(start + int(round(words[index] * fps)) if starts else start)
            seen += sum(weight(w) for w in c.split())
    out: list[Json] = []
    for i, chunk in enumerate(chunks):
        # A chunk stays up until the next one starts; the last stays until the sentence ends.
        stop = starts[i + 1] if i + 1 < len(chunks) else end
        out.append({'start': starts[i], 'end': max(starts[i] + 1, min(end, stop)), 'text': markup.restore(chunk, tags)})
    out[-1]['end'] = end
    return out


def caption_width(board: Board, cfg: Config) -> int:
    """Characters per caption line, or the zh-CN edition's display units (its translation's `captionWidth`)."""
    if cfg.zh():
        return int(board.get('captionWidthZh', zh.CAPTION_UNITS))
    return Pacing.of(board).captionWidth


def build_timeline(board: Board, durations: Durations, cfg: Config) -> tuple[Json, list[Json], list[Json], dict[int, Json]]:
    """The timeline, the narration placements, the caption cues, and the chapter plans."""
    fps = board['fps']
    pacing = Pacing.of(board)
    width = caption_width(board, cfg)
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
        # For each chapter, the extra pauses that fit it to its budget; what they leave over goes to its 3D beats.
        natural, extras, leftover = plan_chapter(chapter, durations, pacing)
        plans[chapter['number']] = {'natural': natural, 'extraPauses': sum(extras), 'leftover': leftover}
        chapter_start = frame
        sentence_index = 0
        scene_beats = [b for b in chapter['beats'] if presentation.is_3d(b)] or chapter['beats']
        leftover_per_beat = leftover / len(scene_beats)
        for beat in chapter['beats']:
            beat_start = frame
            cursor = beat_start + to_frames(pacing.lead)
            captions: list[Json] = []
            pending: list[tuple[list[tuple[Json, int]], int]] = []
            for s in sentences_of(beat):
                # Each sentence is placed at the cursor: its speech, its captions, and its tags' keys.
                narration = durations[s['text']]
                path, seconds, rate = narration.path, narration.seconds, narration.rate
                start = cursor
                end = start + int(math.ceil(seconds * fps))
                placements.append({'path': str(path), 'frame': start, 'rate': rate})
                captions.extend(caption_cues(s, narration, start, end, fps, cfg, width))
                extra = extras[sentence_index] if sentence_index < len(extras) else 0.0
                sentence_index += 1
                # A sentence runs until the next one starts (or the beat ends).
                sentence_keys = [(k, start) for k in tag_keys(s, beat, narration, fps, start, beat['id'], pacing)]
                cursor = end + to_frames(pacing.gap + extra)
                pending.append((sentence_keys, cursor))
            # The beat ends after its last sentence, its tail, and its hold (with its share of the leftover).
            hold = beat.get('hold', 0.0) + (leftover_per_beat if beat in scene_beats else 0.0)
            beat_end = cursor - to_frames(pacing.gap) + to_frames(pacing.tail + hold)
            for i, (sentence_keys, sentence_end) in enumerate(pending):
                # The sentences' keys are made now that the beat's end is known; a motion may run until it.
                ends = {'sentence': beat_end if i == len(pending) - 1 else sentence_end, 'beat': beat_end}
                keys_out.extend(make_key(k, base, fps, beat['id'], ends) for k, base in sentence_keys)
            for i, k in enumerate(beat.get('keys', [])):
                # The beat's own keys take effect at its start.
                sources = beat.get('keySources', [])
                source = sources[i] if i < len(sources) else beat.get('source', {})
                keys_out.append(make_key({**k, '_provenance': {'source': source, 'chapter': chapter['id']}}, beat_start, fps, beat['id'], {'beat': beat_end}))
            # A formula card closes with its beat.
            if any('formula' in k for k in keys_out if k.get('source') == beat['id']):
                keys_out.append(make_key({'formula': '', 'term': 0, '_provenance': {'source': {**beat.get('source', {}), 'fallback': 'generated formula close at the end of this beat'}, 'chapter': chapter['id']}}, beat_end, fps, beat['id'], {'beat': beat_end}))
            # Captions stay up through short gaps.
            for a, b in zip(captions, captions[1:]):
                if b['start'] - a['end'] <= to_frames(pacing.captionBridge):
                    a['end'] = b['start']
            entry: Json = {
                'id': beat['id'], 'chapter': chapter['number'], 'kind': beat['kind'], 'startFrame': beat_start,
                'frames': beat_end - beat_start, 'headline': beat['headline'], 'captions': captions, 'sources': beat.get('sources', []),
            }
            if beat.get('references'):
                # The closing card of a paper's story: the works it cites (lab/src/ui/panel.rs).
                entry['references'] = True
            if 'graph' in beat:
                # The lab draws the frame graph as the scene's sections group it (lab/src/graph/).
                entry['graph'] = frame_graph.load(cfg, beat['graph'])
                entry['focus'] = beat.get('focus', [])
            elif presentation.is_diagram(beat):
                # The lab draws the diagram (lab/src/diagram.rs), its focus nodes lit.
                entry['diagram'] = diagrams.load(cfg, beat['diagram'])
                entry['focus'] = beat.get('focus', [])
            beats_out.append(entry)
            cues.extend({'start': c['start'] / fps, 'end': c['end'] / fps, 'text': markup.readable(c['text']), 'beat': beat['id']} for c in captions)
            frame = beat_end
        chapters_out.append({'number': chapter['number'], 'title': chapter['title'], 'startFrame': chapter_start, 'endFrame': frame})
    # The package's legal notices close the video: a silent card at the end of the last chapter.
    notice_frames = 0
    if cfg.notices() and chapters_out:
        notice_frames = to_frames(NOTICES)
        beats_out.append({'id': 'notices', 'chapter': chapters_out[-1]['number'], 'kind': '2d', 'startFrame': frame, 'frames': notice_frames,
                          'headline': cfg.lab_text('ui-notices_title'), 'captions': [], 'sources': [], 'notices': True})
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
    # A part of a series: the chapters' tag says which part (lab/src/ui/).
    if (part := cfg.part()) is not None:
        timeline['part'] = part
    return timeline, placements, cues, plans
