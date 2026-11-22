"""Tests of the Chinese narration's text and timing (zh.py's `units`, `spoken_pieces`, `cue_marks`, `check_anchors`;
zh_voice.py's error rate): `python3 -m unittest tools/video/test_speech_zh.py`. They need neither the lab nor the
Chinese voice."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import zh  # noqa: E402
import zh_voice  # noqa: E402
from locations import DiagnosticError  # noqa: E402
from markup import Sentence, Tag  # noqa: E402


def cue(piece: int, key: dict, name: str | None = None) -> Tag:
    return Tag('cue', name, [], piece, key=key)


def english(pieces: list[str], tags: list[Tag]) -> Sentence:
    return Sentence(' '.join(pieces), ' '.join(pieces), pieces, tags)


def chinese(pieces: list[str], tags: list[Tag] | None = None) -> Sentence:
    return Sentence(''.join(pieces), ''.join(pieces), pieces, tags or [])


def marks(en: Sentence, cn: Sentence) -> list[int]:
    zh.check_anchors(en, cn, 'beat')
    return zh.cue_marks(en, cn, zh.spoken_pieces(cn, lambda text: text), None, 'beat')


class Units(unittest.TestCase):
    def test_units(self) -> None:
        # The aligner's units: a CJK character each, a run of letters and digits each, punctuation dropped (so
        # letters and digits only a punctuation mark apart are one unit, as the aligner cuts them).
        self.assertEqual(zh.units('到了 Godot 的SSR，0.074 和 strong law。'), ['到', '了', 'Godot', '的', 'SSR0074', '和', 'strong', 'law'])
        self.assertEqual(zh.units('“有符号” Hi-Z'), ['有', '符', '号', 'HiZ'])


class CueMarks(unittest.TestCase):
    def test_proportional(self) -> None:
        # An unanchored cue halfway through the English words falls halfway through the Chinese units.
        en = english(['One two', 'three four'], [cue(1, {'callout': 'a'})])
        self.assertEqual(marks(en, chinese(['一二三四五六'])), [3])
        # One at the start stays at the start; one near the end stays before the last unit.
        self.assertEqual(marks(english(['', 'one'], [cue(1, {'callout': 'a'})]), chinese(['一二'])), [0])
        self.assertEqual(marks(english(['one two three four five six seven eight nine', 'ten'], [cue(1, {'callout': 'a'})]), chinese(['一二'])), [1])

    def test_anchored(self) -> None:
        # `cueAt 1` places the English sentence's first cue before the next Chinese unit.
        en = english(['One two', 'three four'], [cue(1, {'callout': 'a'})])
        self.assertEqual(marks(en, chinese(['一', '二三四'], [cue(1, {}, '1')])), [1])

    def test_ranges(self) -> None:
        # A range runs over the Chinese words it is anchored to; unanchored, over its share, at least one unit.
        en = english(['a', 'b c', 'd'], [Tag('open', 'range-1', [], 1, key={'params': {'x': 1.0}}), Tag('close', 'range-1', [], 2)])
        self.assertEqual(marks(en, chinese(['一二', '三四五', '六'], [Tag('open', '1', [], 1), Tag('close', '1', [], 2)])), [2, 5])
        self.assertEqual(marks(en, chinese(['一二三四']))[0] < marks(en, chinese(['一二三四']))[1], True)
        whole = english(['', 'all of it', ''], [Tag('open', 'range-1', [], 1, key={'params': {'x': 1.0}}), Tag('close', 'range-1', [], 2)])
        self.assertEqual(marks(whole, chinese(['一二三'])), [0, 3])

    def test_formulas(self) -> None:
        # A formula the translation only shows is not said, and falls as far in as the English says it; one it says
        # falls where it says it.
        formula = Tag('formula', None, [('formula', 'x^2'), ('say', 'x squared')], 1, key={'formula': 'x^2'})
        en = english(['so', 'x squared', 'here'], [formula])
        shown = Tag('formula', None, [('formula', 'x^2')], 1)
        cn = chinese(['所以', '', '在这'], [shown])
        # A comma the formula leaves before the full stop is not said.
        self.assertEqual(zh.spoken_text(zh.spoken_pieces(chinese(['所以，', '', '。'], [shown]), lambda text: text)), '所以。')
        self.assertEqual(zh.spoken_text(zh.spoken_pieces(cn, lambda text: text)), '所以在这')
        self.assertEqual(marks(en, cn), [1])
        said = Tag('formula', None, [('formula', 'x^2'), ('say', 'x 的平方')], 1)
        said_cn = chinese(['所以', 'x 的平方', '在这'], [said])
        self.assertEqual(zh.spoken_text(zh.spoken_pieces(said_cn, lambda text: text)), '所以 x 的平方 在这')
        self.assertEqual(marks(en, said_cn), [2])

    def test_order(self) -> None:
        # Two cues that set the same value keep their English order; two that set different ones may swap.
        same = english(['a', 'b', 'c'], [cue(1, {'params': {'x': 1.0}}), cue(2, {'params': {'x': 2.0}})])
        swapped = chinese(['一', '二', '三'], [cue(1, {}, '2'), cue(2, {}, '1')])
        with self.assertRaisesRegex(DiagnosticError, 'both set params.x'):
            marks(same, swapped)
        other = english(['a', 'b', 'c'], [cue(1, {'params': {'x': 1.0}}), cue(2, {'params': {'y': 2.0}})])
        self.assertEqual(marks(other, swapped), [2, 1])

    def test_order_demo_and_motions(self) -> None:
        # A demonstration's switch resets values, the camera, and motions: a cue setting one keeps its English order
        # around the switch. A callout is carried across a switch, so it may move.
        swapped = chinese(['一', '二', '三'], [cue(1, {}, '2'), cue(2, {}, '1')])
        demo = cue(2, {'demo': 'room', 'camera': {'yaw': 10.0}})
        with self.assertRaisesRegex(DiagnosticError, 'both set params.x'):
            marks(english(['a', 'b', 'c'], [cue(1, {'params': {'x': 1.0}}), demo]), swapped)
        self.assertEqual(marks(english(['a', 'b', 'c'], [cue(1, {'callout': 'k'}), demo]), swapped), [2, 1])
        # A motion moves its channels: a camera cue and a motion of the camera's yaw set the same thing, and so do a
        # value's cue and a wave of that value.
        path = cue(1, {'animate': [{'path': {'params': ['camera.yaw'], 'through': [[30.0]], 'seconds': 'sentence'}}]})
        with self.assertRaisesRegex(DiagnosticError, 'both set .*camera'):
            marks(english(['a', 'b', 'c'], [path, cue(2, {'camera': {'distance': 4.0}})]), swapped)
        wave = cue(1, {'animate': [{'wave': {'param': 'x', 'amplitude': 1.0, 'period': 2.0}}]})
        with self.assertRaisesRegex(DiagnosticError, 'both set .*params.x'):
            marks(english(['a', 'b', 'c'], [wave, cue(2, {'params': {'x': 2.0}})]), swapped)

    def test_word_cut(self) -> None:
        # An anchor inside a run of letters would cut a unit the aligner times whole.
        en = english(['One', 'two'], [cue(1, {'callout': 'a'})])
        with self.assertRaisesRegex(DiagnosticError, 'cuts the word'):
            marks(en, chinese(['God', 'ot'], [cue(1, {}, '1')]))

    def test_anchor_checks(self) -> None:
        en = english(['a', 'b', 'c'], [cue(1, {'callout': 'a'}), Tag('open', 'range-1', [], 2, key={'params': {'x': 1.0}}), Tag('close', 'range-1', [], 3)])
        for tags, message in (([cue(1, {}, '3')], 'has 2 cues to anchor, not a cue 3'), ([cue(1, {}, '1'), cue(2, {}, '1')], 'anchored twice'),
                              ([cue(1, {}, '2')], 'is a range'), ([Tag('open', '1', [], 1), Tag('close', '1', [], 2)], 'is a point')):
            with self.subTest(message=message), self.assertRaisesRegex(DiagnosticError, message):
                zh.check_anchors(en, chinese(['一', '二', '三'], tags), 'beat')


class Verification(unittest.TestCase):
    def test_shots(self) -> None:
        # A key's shot is what it does in its beat: its frame, blend, and a motion's seconds are the speech's; a motion's
        # other fields, and its beat (`source`), are not.
        import verify
        key = {'frame': 10, 'blend': 3, 'source': 'c1-a', 'animate': [{'orbit': {'params': ['camera.yaw'], 'degrees': 90.0, 'seconds': 4.2}}]}
        retimed = {**key, 'frame': 40, 'blend': 9, 'animate': [{'orbit': {'params': ['camera.yaw'], 'degrees': 90.0, 'seconds': 6.1}}]}
        self.assertEqual(verify.shot(key), verify.shot(retimed))
        self.assertNotEqual(verify.shot(key), verify.shot({**key, 'animate': [{'orbit': {'params': ['camera.yaw'], 'degrees': 180.0, 'seconds': 4.2}}]}))
        self.assertNotEqual(verify.shot(key), verify.shot({**key, 'source': 'c1-b'}))

    def test_scenes(self) -> None:
        # A beat shows the same scene in both editions: its math scene is compared, its translated headline is not.
        import verify
        beat = {'id': 'c2-toy', 'chapter': 2, 'kind': '2d', 'math': 'toyMath', 'headline': 'By hand'}
        self.assertEqual(verify.scenes({'beats': [beat]}), verify.scenes({'beats': [{**beat, 'headline': '手算一遍'}]}))
        self.assertNotEqual(verify.scenes({'beats': [beat]}), verify.scenes({'beats': [{**beat, 'math': 'wrsMath'}]}))


class Cache(unittest.TestCase):
    def test_reference(self) -> None:
        # A clip counts as cached only when its sidecar says it was spoken from the reference as it is now.
        import json
        import tempfile
        import types
        import speech_zh
        with tempfile.TemporaryDirectory() as d:
            cfg = types.SimpleNamespace(audio_cache=Path(d))
            (Path(d) / 'a.wav').write_bytes(b'')
            self.assertFalse(speech_zh.cached(cfg, 'a'))
            (Path(d) / 'a.json').write_text(json.dumps({'voice': 'another reference'}))
            self.assertFalse(speech_zh.cached(cfg, 'a'))
            (Path(d) / 'a.json').write_text(json.dumps({'voice': speech_zh.REFERENCE_ID}))
            self.assertTrue(speech_zh.cached(cfg, 'a'))


class ErrorRate(unittest.TestCase):
    def test_error_rate(self) -> None:
        # Numbers heard in Chinese digits count as the digits written; punctuation and spaces do not count.
        self.assertEqual(zh_voice.error_rate('在 P 处，直接光为 0.074。', '在P处，直接光为零点零七四。'), 0.0)
        self.assertAlmostEqual(zh_voice.error_rate('一二三四', '一二三'), 0.25)


if __name__ == '__main__':
    unittest.main()
