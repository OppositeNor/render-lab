"""The narration lint's measures of a sentence, in both languages."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lint  # noqa: E402


class English(unittest.TestCase):
    def test_counts_words_commas_and_joins_with_a_formula_as_one_word(self) -> None:
        m = lint.measure('The weight is $\\frac{a, b}{c}$; it grows, then falls.', 'x', zh=False)
        self.assertEqual((m.words, m.commas, m.joins), (8, 1, 1))
        self.assertIn('a colon or semicolon', m.flags)

    def test_flags_a_long_sentence_and_many_commas(self) -> None:
        m = lint.measure(' '.join(['word'] * 26) + ', a, b, c.', 'x', zh=False)
        self.assertEqual(m.flags, ['29 words', '3 commas'])

    def test_a_formula_first_and_a_provenance_tail(self) -> None:
        self.assertTrue(lint.measure('$c$ is the colour.', 'x', zh=False).formula_first)
        self.assertTrue(lint.measure('The level is stored in eight bits, read from the code.', 'x', zh=False).provenance_tail)
        self.assertFalse(lint.measure('The code shows that the level is stored in eight bits.', 'x', zh=False).provenance_tail)

    def test_an_opening_participle_but_not_a_gerund_subject(self) -> None:
        self.assertTrue(lint.measure('Averaged in as black, the texel lowers the sum.', 'x', zh=False).opening_participle)
        self.assertTrue(lint.measure('Counting both ends, that gives seventeen probes.', 'x', zh=False).opening_participle)
        self.assertFalse(lint.measure('Borrowing helps, because pixels need the same lights.', 'x', zh=False).opening_participle)
        self.assertFalse(lint.measure('During the trace, the ray climbs.', 'x', zh=False).opening_participle)


class Chinese(unittest.TestCase):
    def test_measures_each_sentence_of_a_paragraph_and_keeps_the_worst(self) -> None:
        m = lint.measure('这是实验室的书房，它的地板是一块抛光的金属。没有东西时，它就是黑的。', 'x', zh=True)
        self.assertEqual((m.de, m.commas, m.chars), (3, 1, 21))
        self.assertEqual(m.flags, ['3 的'])

    def test_a_long_sentence(self) -> None:
        self.assertEqual(lint.measure('长' * 46 + '。', 'x', zh=True).flags, ['46 characters'])


class Summary(unittest.TestCase):
    def test_the_edition_numbers(self) -> None:
        measures = [lint.measure(t, 'x', zh=False) for t in ('One two three.', 'A, b, c, d.', 'Four; five.')]
        s = lint.summary(measures, zh=False)
        self.assertEqual((s['sentences'], s['medianWords'], s['joins'], s['manyCommas'], s['flagged']), (3, 3, 33, 33, 2))


if __name__ == '__main__':
    unittest.main()
