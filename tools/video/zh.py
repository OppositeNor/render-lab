"""The zh-CN edition: merging a translation into the storyboard, the Chinese
narration's text and where the English cues fall in it, and wrapping Chinese
captions by display width. The edition speaks Chinese (speech_zh.py); its
on-screen text, headlines, chapter titles, and subtitles are Chinese, with
technical terms in English. Its shots are the English edition's, each cue
timed to the Chinese words it stands before.
"""
from __future__ import annotations

import copy
import math
import re
import unicodedata
from typing import Any

import markup
from locations import DiagnosticError

CAPTION_UNITS = 64          # display units per caption line (a Chinese character is 2, Latin 1), unless the translation's heading gives `captionWidth`

Board = dict[str, Any]
Json = dict[str, Any]


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
    keeps its English sentence (`parsed`), whose cues it times, and gains the
    Chinese one (`shown`), which is shown and spoken."""
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
                shown, at = translated['shown'], translated['at']
                # Formulas are shown in both languages: the same LaTeX, as many times.
                english, chinese = markup.sentence(s).formulas, shown.formulas
                if sorted(english) != sorted(chinese):
                    raise RuntimeError(f'translation: beat {beat["id"]}: the sentence {shown.caption!r} has formulas {chinese}, the English {english}')
                check_anchors(markup.sentence(s), shown, beat['id'])
                s['at'], s['shown'] = at, shown
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


# ---------------------------------------------------------------------------
# The Chinese narration: its text, the units the aligner times, and where each English cue falls in it.

def anchorable(english: markup.Sentence) -> list[int]:
    """The English sentence's cues a translation anchors by number (1, 2, …), as indices into its tags: its point
    cues, ranges, and formula cards closed (`formulaOff`); a formula's own cue falls where the translation shows it."""
    return [i for i, t in enumerate(english.tags) if t.kind in ('cue', 'open') or (t.kind == 'formula' and t.attr('formula') == 'off')]


def check_anchors(english: markup.Sentence, chinese: markup.Sentence, beat: str) -> None:
    """A translation's anchors (`cueAt`, `overAt`, `overTextAt`) each name one of the English sentence's cues, once,
    a point for a point and a range for a range."""
    cues = anchorable(english)
    seen: set[str] = set()
    for tag in chinese.tags:
        if tag.kind not in ('cue', 'open'):
            continue
        name = tag.name or ''
        number = int(name)
        if not 1 <= number <= len(cues):
            raise DiagnosticError('story.translation.anchor', f'the English sentence has {len(cues)} cues to anchor, not a cue {number}', tag.source, beat=beat)
        if name in seen:
            raise DiagnosticError('story.translation.anchor', f'the cue {number} is anchored twice', tag.source, beat=beat)
        seen.add(name)
        range_cue = english.tags[cues[number - 1]].kind == 'open'
        if range_cue != (tag.kind == 'open'):
            # A point anchors a point cue, a range a range.
            want = 'a range: write overTextAt or overAt' if range_cue else 'a point: write cueAt'
            raise DiagnosticError('story.translation.anchor', f'the English cue {number} is {want}', tag.source, beat=beat)


def _cjk(c: str) -> bool:
    code = ord(c)
    return (0x4E00 <= code <= 0x9FFF or 0x3400 <= code <= 0x4DBF or 0x20000 <= code <= 0x2A6DF or 0x2A700 <= code <= 0x2B73F
            or 0x2B740 <= code <= 0x2B81F or 0x2B820 <= code <= 0x2CEAF or 0xF900 <= code <= 0xFAFF)


def units(text: str) -> list[str]:
    """The units the forced aligner times in `text`, as it cuts them (qwen_asr's `Qwen3ForceAlignProcessor`
    `tokenize_space_lang`): each run between spaces keeps only its letters, digits, and apostrophes, and each CJK
    character in it is a unit of its own."""
    out: list[str] = []
    for run in text.split():
        # Of each run between spaces, its kept characters; a CJK character stands alone, the rest join up.
        kept = ''.join(c for c in run if c == "'" or unicodedata.category(c)[0] in 'LN')
        latin = ''
        for c in kept:
            if _cjk(c):
                if latin:
                    out.append(latin)
                out.append(c)
                latin = ''
            else:
                latin += c
        if latin:
            out.append(latin)
    return out


def _formula_pairs(english: markup.Sentence, chinese: markup.Sentence) -> dict[int, int]:
    """Each English formula's tag index → the Chinese formula's that shows the same LaTeX, by order of occurrence."""
    def formulas(sentence: markup.Sentence) -> list[int]:
        return [i for i, t in enumerate(sentence.tags) if t.kind == 'formula' and t.attr('formula') != 'off']
    remaining = formulas(chinese)
    pairs: dict[int, int] = {}
    for i in formulas(english):
        # The first Chinese formula not yet taken that shows this one's LaTeX.
        j = next(j for j in remaining if chinese.tags[j].attr('formula') == english.tags[i].attr('formula'))
        remaining.remove(j)
        pairs[i] = j
    return pairs


def spoken_pieces(chinese: markup.Sentence, pronounce: Any) -> list[str]:
    """The Chinese sentence's spoken pieces, as the voice says them, each with `pronounce` (video.json's
    `pronunciationsZh`) applied: a formula's piece holds its Chinese `say`, or nothing when the translation only shows
    it (its words around it say it already)."""
    pieces = list(chinese.pieces)
    for tag in chinese.tags:
        # A formula said in the translation's words is spoken there, apart from the words beside it.
        if tag.kind == 'formula' and tag.attr('say'):
            pieces[tag.piece] = f' {tag.attr("say")} '
    return [pronounce(p) for p in pieces]


def spoken_text(pieces: list[str]) -> str:
    """The sentence the voice is given: a comma left before the sentence's end by a formula only shown is dropped."""
    return re.sub(r'[，、,]\s*([。！？.!?])', r'\1', markup.tidy(''.join(pieces)))


def _english_fraction(english: markup.Sentence, piece: int) -> float:
    """How far into the English sentence's words a piece starts, from 0 to 1."""
    total = len(' '.join(english.pieces).split())
    return len(' '.join(english.pieces[:piece]).split()) / total if total else 0.0


def cue_marks(english: markup.Sentence, chinese: markup.Sentence, pieces: list[str], where: Json | None, beat: str | None) -> list[int]:
    """The Chinese unit (`units`) each of the English sentence's tags stands before, in the order of its tags: where
    the translation anchors it, or where it says its formula, or else as far into the Chinese units as it stands
    into the English words (a formula only shown too). Tags whose keys set the same thing must keep their English
    order."""
    count = len(units(spoken_text(pieces)))

    def before(piece: int) -> int:
        # The units of the pieces before this one; a unit must not run across the cut.
        head, tail = units(spoken_text(pieces[:piece])), units(spoken_text(pieces[piece:]))
        if len(head) + len(tail) != count:
            word = (head[-1] if head else '') + (tail[0] if tail else '')
            raise DiagnosticError('story.translation.anchor', f'a cue cuts the word {word!r}; put a space beside it', where, beat=beat)
        return len(head)
    anchors = {(t.kind, t.name): t.piece for t in chinese.tags if t.kind in ('cue', 'open', 'close') and t.name}
    numbers = {index: str(n) for n, index in enumerate(anchorable(english), 1)}
    formulas = _formula_pairs(english, chinese)
    opens = {t.name: i for i, t in enumerate(english.tags) if t.kind == 'open'}
    marks: list[int] = []
    for i, tag in enumerate(english.tags):
        # The anchor or formula that places this tag, else its share of the Chinese units.
        number = numbers.get(opens[tag.name] if tag.kind == 'close' else i)
        kind = 'cue' if tag.kind in ('cue', 'formula') else tag.kind
        if i in formulas and chinese.tags[formulas[i]].attr('say'):
            marks.append(before(chinese.tags[formulas[i]].piece))
        elif number is not None and (kind, number) in anchors:
            marks.append(before(anchors[(kind, number)]))
        elif tag.kind == 'close':
            # An unanchored range ends at its share of the units, at least one unit after it opens.
            opened = marks[opens[tag.name]]
            marks.append(min(count, max(opened + 1, round(_english_fraction(english, tag.piece) * count))))
        else:
            marks.append(min(max(0, count - 1), round(_english_fraction(english, tag.piece) * count)))
    _check_order(english, marks, where, beat)
    return marks


# What a demonstration's switch (a `demo` key) resets, by field (tools/video/README.md, "What a key resets and
# carries"): its values, motions, camera, panel, listing, record, highlights, actions, and clips; callouts, insets,
# arrows, labels, and the formula card are carried.
DEMO_RESETS = ('params.', 'camera', 'animate', 'panel', 'code', 'record', 'highlight', 'run', 'stop', 'play')


def _channel(name: str) -> str:
    """A motion's channel by the field a key sets it with: `camera` for any of the camera's, else the value's."""
    return 'camera' if name.split('.')[0] == 'camera' else f'params.{name}'


def _fields(tag: markup.Tag, english: markup.Sentence) -> set[str]:
    """What a tag's key sets (a range's close, its open's): each value of `params` by name, the camera as one, and
    each channel a motion moves (`animate`) as a key setting it would name it."""
    if tag.kind == 'close':
        tag = next(t for t in english.tags if t.kind == 'open' and t.name == tag.name)
    key = tag.key or {}
    names = {k for k in key if k not in ('delay', 'blend', 'ease', 'params', '_provenance')}
    names |= {f'params.{p}' for p in key.get('params', {})}
    for motion in key.get('animate', []):
        # A path or an orbit moves its `params`, a wave its `param`.
        body = next(iter(motion.values()), {})
        names |= {_channel(c) for c in [*body.get('params', []), *([body['param']] if 'param' in body else [])]}
    return names


def _shared(a: set[str], b: set[str]) -> set[str]:
    """What two tags' keys both set; a demonstration's switch sets everything it resets."""
    resets = lambda own, other: {f for f in other if f.startswith(DEMO_RESETS)} if 'demo' in own else set()
    return (a & b) | resets(a, b) | resets(b, a)


def _check_order(english: markup.Sentence, marks: list[int], where: Json | None, beat: str | None) -> None:
    """Two tags that set the same thing keep their English order in the Chinese sentence, so the shot ends as the
    English one does: the same value, the camera, a channel a motion moves, or what a demonstration's switch
    resets."""
    for i, a in enumerate(english.tags):
        for j in range(i + 1, len(english.tags)):
            # A later English tag placed before an earlier one: refused when they set the same thing.
            shared = _shared(_fields(a, english), _fields(english.tags[j], english)) if marks[j] < marks[i] else set()
            if shared:
                raise DiagnosticError('story.translation.order', f'the translation places the English sentence\'s tag {j + 1} before its tag {i + 1}, '
                                      f'and both set {", ".join(sorted(shared))}; anchor them in the English order', where, beat=beat)
