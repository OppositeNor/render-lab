"""Tests of the comparison of prepared outputs (compare.py): `python3 -m unittest tools/video/test_compare.py`."""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import compare  # noqa: E402


class Compare(unittest.TestCase):
    def sets(self, old: dict[str, str], new: dict[str, str]) -> list[str]:
        with tempfile.TemporaryDirectory() as d:
            for name, files in (('old', old), ('new', new)):
                for rel, text in files.items():
                    path = Path(d) / name / rel
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(text)
            return compare.compare(Path(d) / 'old', Path(d) / 'new')

    def test_json_by_value(self) -> None:
        old = {'p/en/timeline.json': json.dumps({'keys': [{'through': [[18, 16]], 'frame': 3}]})}
        # Numbers as numbers, keys in any order.
        self.assertEqual(self.sets(old, {'p/en/timeline.json': json.dumps({'keys': [{'frame': 3, 'through': [[18.0, 16.0]]}]})}), [])
        diffs = self.sets(old, {'p/en/timeline.json': json.dumps({'keys': [{'frame': 4, 'through': [[18, 16]]}]})})
        self.assertEqual(diffs, ['p/en/timeline.json: .keys[0].frame: 3 != 4'])
        self.assertTrue(self.sets(old, {'p/en/timeline.json': json.dumps({'keys': []})}))
        # A flag is not a number.
        self.assertTrue(self.sets({'a.json': 'true'}, {'a.json': '1'}))

    def test_text_and_files(self) -> None:
        self.assertEqual(self.sets({'p/en/captions.srt': 'a\nb\n'}, {'p/en/captions.srt': 'a\nb\n'}), [])
        self.assertEqual(self.sets({'p/en/captions.srt': 'a\nb\n'}, {'p/en/captions.srt': 'a\nc\n'}), ["p/en/captions.srt: line 2: 'b' != 'c'"])
        self.assertEqual(len(self.sets({'p/en/script.md': ''}, {})), 1)
        self.assertEqual(len(self.sets({}, {'p/en/script.md': ''})), 1)

    def test_no_new_synthesis(self) -> None:
        self.assertEqual(self.sets({'p/audio.txt': 'a.wav\nb.wav\n'}, {'p/audio.txt': 'a.wav\nb.wav\n'}), [])
        self.assertEqual(self.sets({'p/audio.txt': 'a.wav\n'}, {'p/audio.txt': 'a.wav\nc.wav\n'}), ['p/audio.txt: synthesized anew: c.wav'])


if __name__ == '__main__':
    unittest.main()
