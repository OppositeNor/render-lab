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

CAPTION_UNITS = 64          # display units per caption line (a Chinese character is 2, Latin 1), unless the translation's heading gives `captionWidth`

Board = dict[str, Any]


def _at(sentence: dict[str, Any]) -> str:
    source = sentence.get('source') or {}
    return f'{source.get("file", "?")}:{source.get("line", "?")}'


def alignment(english: list[dict[str, Any]], translated: list[dict[str, Any]]) -> str:
    """Both sides of a beat, sentence by sentence, with where each is, so a
    sentence split or merged on one side is found by reading down the rows."""
    rows = []
    for i in range(max(len(english), len(translated))):
        en = english[i] if i < len(english) else None
        zh = translated[i] if i < len(translated) else None
        en_text = markup.readable(markup.sentence(en).caption) if en else '(none)'
        zh_text = markup.readable(zh['shown'].caption) if zh else '(none)'
        rows.append(f'  {i + 1:>2}. en {_at(en) if en else "-"}: {en_text[:90]}\n      zh {_at(zh) if zh else "-"}: {zh_text[:90]}')
    return '\n'.join(rows)


def localize(board: Board, translation: dict[str, Any]) -> Board:
    """The storyboard with Chinese text in place of the English. Sentences
    are matched one to one, so every shot key stays on its sentence; each
    keeps its English sentence (`parsed`), which is what is spoken."""
    out = copy.deepcopy(board)
    out['title'], out['subtitle'] = translation['title'], translation['subtitle']
    out['locale'] = translation['locale']
    if 'captionWidth' in translation:
        out['captionWidthZh'] = translation['captionWidth']
    chapters = {c['id']: c for c in translation['chapters']}
    # Every translated chapter must exist in English, and every English chapter must be translated.
    unknown = set(chapters) - {c['id'] for c in out['chapters']}
    if unknown:
        raise RuntimeError(f'translation: unknown chapters {sorted(unknown)}')
    for chapter in out['chapters']:
        zh = chapters.get(chapter['id'])
        if zh is None:
            raise RuntimeError(f'translation: no entry for chapter {chapter["id"]}')
        if zh.get('number', chapter['number']) != chapter['number']:
            raise RuntimeError(f'translation: chapter {chapter["id"]} is number {chapter["number"]}, not {zh["number"]}')
        chapter['titleEn'], chapter['title'], chapter['summary'] = chapter['title'], zh['title'], zh['summary']
        for beat in chapter['beats']:
            # Each beat must have as many translated sentences as English ones, since keys stay on their sentences.
            t = translation['beats'].get(beat['id'])
            if t is None:
                raise RuntimeError(f'translation: no entry for beat {beat["id"]}')
            sentences = beat['narration']
            if len(t['narration']) != len(sentences):
                raise RuntimeError(f'translation: beat {beat["id"]} has {len(t["narration"])} sentences, the storyboard {len(sentences)}\n'
                                   + alignment(sentences, t['narration']))
            beat['headlineEn'], beat['headline'] = beat['headline'], t['headline']
            narration = []
            for s, translated in zip(sentences, t['narration']):
                # Each sentence keeps its English (it is spoken) and gains the Chinese it shows.
                s = dict(s)
                shown, text = translated['shown'], translated['text']
                # Formulas are shown in both languages: the same LaTeX, as many times.
                english, chinese = markup.sentence(s).formulas, shown.formulas
                if sorted(english) != sorted(chinese):
                    raise RuntimeError(f'translation: beat {beat["id"]}: the sentence {shown.caption!r} has formulas {chinese}, the English {english}')
                s['text'], s['shown'] = text, shown
                if 'source' in translated:
                    s['shownSource'] = translated['source']
                narration.append(s)
            beat['narration'] = narration
    extra = set(translation['beats']) - {b['id'] for c in out['chapters'] for b in c['beats']}
    if extra:
        raise RuntimeError(f'translation: unknown beats {sorted(extra)}')
    return out


# ---------------------------------------------------------------------------
# Captions: one line per cue (the video's one-line subtitle, lab/src/ui/ `for_video`), cut by display width,
# never inside a Latin word or number, and never starting a line with punctuation.

CAPTION_LINES = 1

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
            # A space is remembered as the separator before the next token, not kept as one.
            space = True
            continue
        if out and piece in NO_LINE_START and not space:
            # Punctuation that may not start a line joins the token before it.
            out[-1][0] += piece
            continue
        if piece in NO_LINE_END:
            # An opening bracket or quote may not end a line, so it joins the token after it.
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
        # Try every cut: lines close in width, and a cut where the text allows a break.
        a, b = join(toks[:i]), join(toks[i:])
        wa, wb = display_width(a), display_width(b)
        if wa <= width and wb <= width:
            score = abs(wa - wb) + 12 * toks[i][2]
            if best is None or score < best[0]:
                best = (score, [a, b])
    return best[1] if best else None


def caption_chunks(text: str, width: int = CAPTION_UNITS) -> list[str]:
    """The fewest cues of at most `CAPTION_LINES` balanced lines, cut near equal
    widths, preferring cuts after sentence and clause punctuation."""
    toks = tokens(text)
    total = display_width(text)
    widths = [display_width(sep + t) for t, sep, _ in toks]
    starts = [sum(widths[:i]) for i in range(len(toks))]
    # Start from the fewest chunks the text could fit in, and add one until every chunk fits a line.
    n = max(1, math.ceil(total / (CAPTION_LINES * width - 4)))
    while n <= len(toks):
        cuts = [0]
        for k in range(1, n):
            # Each cut lands near the next k/n of the width, at the token that weighs distance against its penalty.
            target = total * k / n
            candidates = range(cuts[-1] + 1, len(toks))
            if not candidates:
                break
            cuts.append(min(candidates, key=lambda i: abs(starts[i] - target) / width * 8 + toks[i][2]))
        cuts.append(len(toks))
        chunks = [toks[a:b] for a, b in zip(cuts, cuts[1:]) if b > a]
        lines = [balanced_lines(c, width) for c in chunks]
        if all(line is not None and len(line) <= CAPTION_LINES for line in lines):
            return ['\n'.join(line) for line in lines if line is not None]
        n += 1
    raise RuntimeError(f'cannot fit caption: {text}')
