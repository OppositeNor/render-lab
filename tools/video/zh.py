"""The zh-CN edition: merging a translation into the storyboard, and wrapping
Chinese captions by display width. The edition keeps the English narration;
its on-screen text, headlines, chapter titles, and subtitles are Chinese, with
technical terms in English.
"""
from __future__ import annotations

import copy
import math
import re
import unicodedata
from typing import Any

import markup

CAPTION_UNITS = 64          # display units per caption line: a Chinese character is 2, Latin 1

Board = dict[str, Any]


def localize(board: Board, translation: dict[str, Any]) -> Board:
    """The storyboard with Chinese text in place of the English. Sentences
    are matched one to one, so every shot key stays on its sentence; each
    keeps its English text in `en`, which is what is spoken."""
    out = copy.deepcopy(board)
    out['title'], out['subtitle'] = translation['title'], translation['subtitle']
    out['locale'] = translation['locale']
    chapters = {c['number']: c for c in translation['chapters']}
    for chapter in out['chapters']:
        zh = chapters[chapter['number']]
        chapter['titleEn'], chapter['title'], chapter['summary'] = chapter['title'], zh['title'], zh['summary']
        for beat in chapter['beats']:
            t = translation['beats'].get(beat['id'])
            if t is None:
                raise RuntimeError(f'translation: no entry for beat {beat["id"]}')
            sentences = beat['narration']
            if len(t['narration']) != len(sentences):
                raise RuntimeError(f'translation: beat {beat["id"]} has {len(t["narration"])} sentences, the storyboard {len(sentences)}')
            beat['headlineEn'], beat['headline'] = beat['headline'], t['headline']
            narration = []
            for s, text in zip(sentences, t['narration']):
                s = dict(s) if isinstance(s, dict) else {'text': s}
                # Formulas are shown in both languages: the same LaTeX, as many times.
                english, chinese = markup.parse(s['text']).formulas, markup.parse(text, spoken_only=True).formulas
                if sorted(english) != sorted(chinese):
                    raise RuntimeError(f'translation: beat {beat["id"]}: the sentence {text!r} has formulas {chinese}, the English {english}')
                s['en'], s['text'] = s['text'], text
                narration.append(s)
            beat['narration'] = narration
    extra = set(translation['beats']) - {b['id'] for c in out['chapters'] for b in c['beats']}
    if extra:
        raise RuntimeError(f'translation: unknown beats {sorted(extra)}')
    return out


# ---------------------------------------------------------------------------
# Captions: at most two lines per cue, wrapped by display width, never
# inside a Latin word or number, and never starting a line with punctuation.

NO_LINE_START = set('，。、；：！？）》」』”’…,.;:!?)%')
NO_LINE_END = set('（《「『“‘(')
PENALTY_AFTER = {'。': 0, '！': 0, '？': 0, '；': 0, '：': 1, '，': 1, '、': 2}
TOKEN = re.compile(r'[A-Za-z0-9][A-Za-z0-9_:./+×²°#\-]*|\s+|.')

Token = list[Any]   # [text, separator before, break penalty before]


def char_width(c: str) -> int:
    return 2 if unicodedata.east_asian_width(c) in ('W', 'F') else 1


def display_width(text: str) -> int:
    return sum(char_width(c) for c in text)


def tokens(text: str) -> list[Token]:
    """Units that a line break may separate."""
    out: list[Token] = []
    space = False
    opening = ''
    for m in TOKEN.finditer(text):
        piece = m.group(0)
        if piece.isspace():
            space = True
            continue
        if out and piece in NO_LINE_START and not space:
            out[-1][0] += piece
            continue
        if piece in NO_LINE_END:
            opening += piece
            continue
        if out:
            last = out[-1][0][-1]
            # Between two English words is a worse break than between Chinese characters.
            latin_pair = space and last.isascii() and last.isalnum() and piece[0].isascii() and piece[0].isalnum()
            penalty = PENALTY_AFTER.get(last, 7 if latin_pair else 3 if space else 4)
        else:
            penalty = 0
        out.append([opening + piece, ' ' if space else '', penalty])
        space, opening = False, ''
    return out


def join(toks: list[Token]) -> str:
    return ''.join((sep if i else '') + t for i, (t, sep, _) in enumerate(toks))


def balanced_lines(toks: list[Token], width: int = CAPTION_UNITS) -> list[str] | None:
    text = join(toks)
    if display_width(text) <= width:
        return [text]
    best: tuple[float, list[str]] | None = None
    for i in range(1, len(toks)):
        a, b = join(toks[:i]), join(toks[i:])
        wa, wb = display_width(a), display_width(b)
        if wa <= width and wb <= width:
            score = abs(wa - wb) + 12 * toks[i][2]
            if best is None or score < best[0]:
                best = (score, [a, b])
    return best[1] if best else None


def caption_chunks(text: str, width: int = CAPTION_UNITS) -> list[str]:
    """The fewest cues of at most two balanced lines, cut near equal widths,
    preferring cuts after sentence and clause punctuation."""
    toks = tokens(text)
    total = display_width(text)
    widths = [display_width(sep + t) for t, sep, _ in toks]
    starts = [sum(widths[:i]) for i in range(len(toks))]
    n = max(1, math.ceil(total / (2 * width - 4)))
    while n <= len(toks):
        cuts = [0]
        for k in range(1, n):
            target = total * k / n
            candidates = range(cuts[-1] + 1, len(toks))
            if not candidates:
                break
            cuts.append(min(candidates, key=lambda i: abs(starts[i] - target) / width * 8 + toks[i][2]))
        cuts.append(len(toks))
        chunks = [toks[a:b] for a, b in zip(cuts, cuts[1:]) if b > a]
        lines = [balanced_lines(c, width) for c in chunks]
        if all(line is not None for line in lines):
            return ['\n'.join(line) for line in lines if line is not None]
        n += 1
    raise RuntimeError(f'cannot fit caption: {text}')
