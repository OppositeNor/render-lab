"""Measure a story's narration for the ear: sentence length, commas, colons, and openings (reports, never fails).

    python3 tools/video/lint.py --investigation godot/ssr
    python3 tools/video/lint.py --investigation godot/ssr --lang zh-CN --json

The rules it measures are new-story's spoken-sentence rules (references/storyboard.md): at most one appended phrase,
no colons or semicolons, a formula never first, provenance said up front rather than appended, no opening participle.
For zh-CN, per Chinese sentence (a paragraph may hold several): few 的, few commas, and short sentences. It prints the edition's numbers and every sentence over
a limit; the reviewer decides what to change. It cannot tell a term used before its definition: the cold-listener
review does that.
"""
from __future__ import annotations

import argparse
import json
import re
import statistics
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lamlet  # noqa: E402
from config import LANGS, Config  # noqa: E402
from outputs import script_text  # noqa: E402
from storyboard import load_board  # noqa: E402

Json = dict[str, Any]

# Limits, from the narration pilot (SSR chapter 6): a sentence past them is listed for the reviewer.
MAX_WORDS = 25
MAX_COMMAS = 2
MAX_DE = 2
MAX_CHARS = 45

FORMULA = re.compile(r'\$[^$]*\$')
PROVENANCE_TAIL = re.compile(r'[,:]\s*(?:a finding\s+)?(?:as\s+)?(?:read|taken|found)\s+(?:from|in)\s+the\s+(?:code|source)\b[^,]*$', re.I)
# Words ending in -ing or -ed that do not open a participle phrase.
NOT_PARTICIPLES = {'during', 'nothing', 'something', 'everything', 'anything', 'indeed', 'instead', 'hundred', 'thing',
                   'string', 'spring', 'ring', 'king', 'ceiling', 'morning', 'evening', 'red', 'need', 'speed', 'seed'}
# Words that start a subject after an opening phrase.
SUBJECT_OPENERS = {'the', 'a', 'an', 'this', 'that', 'these', 'those', 'it', 'its', 'they', 'their', 'we', 'you', 'he',
                   'she', 'each', 'every', 'one', 'both', 'all', 'no', 'some'}


@dataclass
class Measure:
    """One sentence's numbers and what in it is over a limit."""
    text: str
    at: str
    words: int = 0
    commas: int = 0
    joins: int = 0
    de: int = 0
    chars: int = 0
    formula_first: bool = False
    provenance_tail: bool = False
    opening_participle: bool = False
    flags: list[str] = field(default_factory=list)


def without_formulas(text: str) -> str:
    """`text` with each `$LaTeX$` formula as one word, F, so its symbols count as neither words nor punctuation."""
    return FORMULA.sub('F', text)


def opening_participle(words: list[str]) -> bool:
    """Whether a sentence's words open with a participle phrase before its subject ("Averaged in as black, the texel
    …"): a first word in -ing or -ed, a comma within the next few words, and a subject after it. A gerund subject
    ("Borrowing helps, because …") has its verb before the comma, so the word after the comma is not a subject."""
    first = re.sub(r'\W', '', words[0]).lower() if words else ''
    if not first.endswith(('ing', 'ed')) or first in NOT_PARTICIPLES:
        return False
    closing = next((i for i, w in enumerate(words[:8]) if w.endswith(',')), None)
    if closing is None or closing + 1 >= len(words):
        return False
    return words[closing + 1].lower() in SUBJECT_OPENERS


def measure(text: str, at: str, zh: bool) -> Measure:
    """The numbers of one sentence of an edition, `zh` for the Chinese one, and its flags."""
    plain = without_formulas(text).strip()
    m = Measure(text=text, at=at, formula_first=text.lstrip().startswith('$'))
    if zh:
        # A paragraph holds one English sentence's Chinese, maybe several sentences: each is measured, the worst kept.
        parts = [re.sub(r'\s', '', p) for p in re.split(r'[。！？]', plain) if p.strip()] or ['']
        m.chars = max(len(p) for p in parts)
        m.commas = max(p.count('，') + p.count('、') for p in parts)
        m.joins = sum(plain.count(c) for c in '；：;:')
        m.de = max(p.count('的') for p in parts)
    else:
        words = plain.split()
        m.words = len(words)
        m.commas = plain.count(',')
        m.joins = plain.count(';') + plain.count(':')
        m.provenance_tail = bool(PROVENANCE_TAIL.search(plain.rstrip('.')))
        m.opening_participle = opening_participle(words)
    m.flags = flags(m, zh)
    return m


def flags(m: Measure, zh: bool) -> list[str]:
    """What in a measured sentence is over a limit, in words a reviewer reads."""
    out: list[str] = []
    if zh:
        if m.chars > MAX_CHARS:
            out.append(f'{m.chars} characters')
        if m.de > MAX_DE:
            out.append(f'{m.de} 的')
    else:
        if m.words > MAX_WORDS:
            out.append(f'{m.words} words')
        if m.provenance_tail:
            out.append('provenance at the end')
        if m.opening_participle:
            out.append('opens with a participle')
    if m.commas > MAX_COMMAS:
        out.append(f'{m.commas} commas')
    if m.joins:
        out.append('a colon or semicolon')
    if m.formula_first:
        out.append('a formula first')
    return out


def summary(measures: list[Measure], zh: bool) -> Json:
    """An edition's numbers: the narration-clarity baseline's columns (shares in per cent)."""
    n = len(measures) or 1

    def share(count: int) -> int:
        return round(100 * count / n)
    out: Json = {'sentences': len(measures),
                 'joins': share(sum(1 for m in measures if m.joins)),
                 'manyCommas': share(sum(1 for m in measures if m.commas > MAX_COMMAS)),
                 'formulaFirst': sum(1 for m in measures if m.formula_first),
                 'flagged': sum(1 for m in measures if m.flags)}
    if zh:
        out |= {'medianChars': statistics.median(m.chars for m in measures) if measures else 0,
                'long': sum(1 for m in measures if m.chars > MAX_CHARS),
                'manyDe': sum(1 for m in measures if m.de > MAX_DE)}
    else:
        out |= {'medianWords': statistics.median(m.words for m in measures) if measures else 0,
                'long': share(sum(1 for m in measures if m.words > MAX_WORDS)),
                'provenanceTail': sum(1 for m in measures if m.provenance_tail),
                'openingParticiple': sum(1 for m in measures if m.opening_participle)}
    return out


def summary_line(cfg: Config, s: Json) -> str:
    """An edition's numbers as one line of text."""
    if cfg.zh():
        body = (f'median {s["medianChars"]:g} characters · over {MAX_CHARS} characters {s["long"]} · '
                f'over {MAX_DE} 的 {s["manyDe"]}')
    else:
        body = (f'median {s["medianWords"]:g} words · over {MAX_WORDS} words {s["long"]}% · '
                f'provenance at the end {s["provenanceTail"]} · opening participle {s["openingParticiple"]}')
    return (f'{cfg.investigation} ({cfg.lang}): {s["sentences"]} sentences · {body} · with ; or : {s["joins"]}% · '
            f'{MAX_COMMAS + 1}+ commas {s["manyCommas"]}% · formula first {s["formulaFirst"]} · flagged {s["flagged"]}')


def measures_of(cfg: Config, board: Json, root: Path) -> list[Measure]:
    """Every narrated sentence of a loaded storyboard, in order, measured; `at` relative to `root` where it can be."""
    out: list[Measure] = []
    for chapter in board['chapters']:
        for beat in chapter['beats']:
            for sentence in beat.get('narration', []):
                at = str(sentence.get('at', beat['id']))
                if at.startswith(str(root) + '/'):
                    at = at[len(str(root)) + 1:]
                out.append(measure(script_text(cfg, sentence), at, cfg.zh()))
    return out


def main() -> None:
    parser = argparse.ArgumentParser(description=(__doc__ or '').split('\n\n')[0])
    parser.add_argument('--investigation', required=True)
    parser.add_argument('--investigation-dir', type=Path)
    parser.add_argument('--lang', choices=LANGS, default='en')
    parser.add_argument('--binary', type=Path, default=lamlet.BINARY)
    parser.add_argument('--json', action='store_true', help='the numbers and every sentence, measured, on stdout')
    args = parser.parse_args()
    cfg = Config.load(args.investigation, args.lang, args.investigation_dir)
    lamlet.BINARY = args.binary.resolve()
    root = Path(__file__).resolve().parents[2]
    measures = measures_of(cfg, load_board(cfg), root)
    totals = summary(measures, cfg.zh())
    if args.json:
        print(json.dumps({'renderLabNarrationLint': 1, 'investigation': cfg.investigation, 'lang': cfg.lang,
                          'summary': totals, 'sentences': [asdict(m) for m in measures]}, ensure_ascii=False, indent=1))
        return
    print(summary_line(cfg, totals))
    for m in measures:
        if m.flags:  # only the sentences over a limit are listed, each with what it is over
            print(f'{m.at}: {", ".join(m.flags)}: {m.text}')


if __name__ == '__main__':
    main()
