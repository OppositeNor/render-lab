"""Lamlet regions in a storyboard (lamlet.py, packages/rlstd/language/story.lam): skipped without Lamlet."""
from __future__ import annotations

import shutil
import os
import tempfile
import unittest
from pathlib import Path

import lamlet
import story

STORY = r'''title: A test
fps: 30

# 1 | Counting | 20 s
summary: Numbers and cues from Lamlet.

$[Import* std]
$[Import* rlstd]
$[# leaves 42289]

## c1-count | 3d | The leaves
${cue {' {attr "demo" "cost"} {num "focusRow" -1} {attr "callout" ""}}}

${callout "sort-cells"}There are ${spoken leaves} leaves, ${rounded leaves 2} in round numbers.

${sweep "lv" {' {span "octreeLevel" 7 1}} "The mips climb a level at a time"}, costs $$5.
'''


@unittest.skipUnless(os.environ.get('LAMLET') or shutil.which('lamlet'), 'no lamlet interpreter (LAMLET)')
class LamletStory(unittest.TestCase):
    def test_expands_cues_and_numbers(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / 'storyboard.story'
            path.write_text(STORY)
            board = story.load(path)
        beat = board['chapters'][0]['beats'][0]
        self.assertEqual(beat['keys'], [{'demo': 'cost', 'callout': '', 'params': {'focusRow': -1}}])
        said = [s['text'] for s in beat['narration']]
        self.assertEqual(said[0], '{@ callout=sort-cells}There are forty-two thousand two hundred eighty-nine leaves, forty-two thousand in round numbers.')
        self.assertEqual(said[1], '{@lv> octreeLevel=7..1}The mips climb a level at a time{<lv}, costs $5.')

    def test_no_regions_needs_no_lamlet(self) -> None:
        self.assertFalse(lamlet.has_regions('A price of $$5, and {@ demo=room} cues.'))
        self.assertTrue(lamlet.has_regions('${spoken 3}'))

    def test_errors_carry_the_diagnostic(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / 'storyboard.story'
            path.write_text('$[Import* std]\n${unknownName 3}\n')
            with self.assertRaises(lamlet.LamletError) as e:
                lamlet.read(path)
        self.assertIn('unknownName', str(e.exception))


if __name__ == '__main__':
    unittest.main()
