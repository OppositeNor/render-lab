"""Tests of the narration markup (markup.py): `python3 -m unittest tools/video/test_markup.py`."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import markup  # noqa: E402

FORMULA = r'\alpha = \frac{1}{1 + \lit{1}{n}}'


class Parse(unittest.TestCase):
    def test_cues_ranges_and_formulas(self) -> None:
        s = markup.parse('A ray {@ opacity=1 callout=x} marches, {@grow> radius=0..2} step by step {<grow}, with '
                         f'{{@ formula="{FORMULA}" say="alpha equals one over one plus n" colors="#ffd166"}}, {{@ term=1}} up to ten.')
        self.assertEqual(s.spoken, 'A ray marches, step by step, with alpha equals one over one plus n, up to ten.')
        self.assertEqual(s.caption, f'A ray marches, step by step, with {{@ formula="{FORMULA}" colors="#ffd166"}}, up to ten.')
        self.assertEqual([t.kind for t in s.tags], ['cue', 'open', 'close', 'formula', 'cue'])
        self.assertEqual(s.formulas, [FORMULA])
        # Each tag stands before the piece it opens.
        self.assertEqual([s.pieces[t.piece] for t in s.tags], ['marches,', 'step by step', ', with', 'alpha equals one over one plus n', 'up to ten.'])
        self.assertEqual(markup.cue_key(s.tags[0], {}, 'x'), {'callout': 'x', 'params': {'opacity': '1'}})
        self.assertEqual(markup.cue_key(s.tags[4], {}, 'x'), {'term': 1})

    def test_named_cues(self) -> None:
        s = markup.parse('{@wide}A ray.')
        self.assertEqual(markup.cue_key(s.tags[0], {'wide': {'camera': {'yaw': 3}}}, 'x'), {'camera': {'yaw': 3}})
        with self.assertRaises(markup.MarkupError):
            markup.cue_key(s.tags[0], {}, 'x')

    def test_quotes_and_escapes(self) -> None:
        attrs, end = markup.parse_tag(r'{@ formula="a \\ \"b\"" display=yes}x', 2)
        self.assertEqual(attrs, [('formula', r'a \\ "b"'), ('display', 'yes')])
        self.assertEqual(markup.parse(r'a \{b} c').spoken, 'a {b} c')

    def test_translations_carry_formulas_only(self) -> None:
        s = markup.parse(f'以 {{@ formula="{FORMULA}"}} 混合。', spoken_only=True)
        self.assertEqual(s.formulas, [FORMULA])
        for bad in [f'以 {{@ formula="{FORMULA}" say="x"}}。', '{@ a=1}以', '{<m}以']:
            with self.assertRaises(markup.MarkupError):
                markup.parse(bad, spoken_only=True)

    def test_errors(self) -> None:
        for bad in ['x {@ formula="a"} y', 'x {@m> a=1} y', 'x {<m} y', 'x {@ a=1', 'x {@ formula="a', 'x {@ a=1 b} y']:
            with self.assertRaises(markup.MarkupError, msg=bad):
                markup.parse(bad)

    def test_readable_and_protect(self) -> None:
        caption = f'with {{@ formula="{FORMULA}"}}, up to ten.'
        self.assertEqual(markup.readable(caption), r'with $\alpha = \frac{1}{1 + n}$, up to ten.')
        held, tags = markup.protect(caption)
        self.assertNotIn('{', held)
        self.assertEqual(markup.restore(held, tags), caption)


if __name__ == '__main__':
    unittest.main()
