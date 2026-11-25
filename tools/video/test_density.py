"""The animation check by explanation density (density.py): what a key counts as, the gaps, a chapter's verdict."""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import density  # noqa: E402

Json = dict[str, Any]
FPS = 30
BEAT = (0, 300)


class DensityTest(unittest.TestCase):
    def test_what_a_key_starts(self) -> None:
        clips = {'step': 1.0, 'jump': 0.0}
        span = lambda key: density.span({'frame': 30, 'blend': 0, **key}, FPS, clips, BEAT)  # noqa: E731
        self.assertEqual(span({'blend': 45, 'params': {'a': 1}}), 45)          # a sweep
        self.assertEqual(span({'params': {'a': 1}}), 0)                         # a step revealed mid-beat
        self.assertEqual(span({'play': 'step'}), 30)                            # a clip that takes time
        self.assertIsNone(span({'play': 'jump'}))                               # a clip that only jumps
        self.assertEqual(span({'animate': [{'path': {'seconds': 2.0}}]}), 60)   # a camera path
        self.assertIsNone(span({'animate': [{'wave': {'seconds': 2.0}}]}))      # a sway never counts
        # A path through the whole beat from its first frame only floats the camera.
        self.assertIsNone(density.span({'frame': 0, 'animate': [{'path': {'params': ['camera.yaw'], 'seconds': 10.0}}]}, FPS, clips, BEAT))
        # A value's path through the whole beat (a light moving) still counts, as does an inset's parameters blended.
        self.assertEqual(density.span({'frame': 0, 'animate': [{'path': {'params': ['sunYaw'], 'seconds': 10.0}}]}, FPS, clips, BEAT), 300)
        self.assertEqual(span({'blend': 45, 'imageParams': {'far': 12.0}}), 45)
        self.assertEqual(density.span({'frame': 0, 'animate': [{'path': {'seconds': 2.0}}]}, FPS, clips, BEAT), 60)
        self.assertEqual(span({'run': {'name': 'march'}}), 270)                 # an action, to the beat's end
        self.assertIsNone(span({'callout': 'x'}))
        # The beat's first frame only sets the scene up.
        self.assertIsNone(density.span({'frame': 0, 'blend': 0, 'params': {'a': 1}}, FPS, clips, BEAT))

    def test_longest_gap(self) -> None:
        self.assertEqual(density.longest_gap(0, 300, []), 300)
        self.assertEqual(density.longest_gap(0, 300, [(30, 90), (60, 120), (200, 200)]), 100)
        self.assertEqual(density.longest_gap(0, 300, [(0, 300)]), 0)

    def report(self, beats: list[Json], keys: list[Json]) -> Json:
        timeline = {'fps': FPS, 'beats': beats, 'keys': keys}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'timeline.json'
            path.write_text(json.dumps(timeline))
            cfg = mock.Mock(lang='en', timeline=lambda: path)
            cfg.name = 'test'
            with mock.patch.object(density, 'clip_lengths', return_value={}):
                return density.report(cfg, Path('render-lab'))

    def test_density_scales_what_a_chapter_needs(self) -> None:
        # 36 s at density 1 owes 6; at 0.25, 1.5 (rounded up to 2); at 0, none and no gap limit.
        sweeps = [{'frame': f, 'blend': 30, 'source': 'b', 'params': {'a': 1}} for f in (10, 300, 600)]
        full = self.report([{'id': 'b', 'chapter': 1, 'startFrame': 0, 'frames': 36 * FPS}], sweeps)
        self.assertEqual((full['chapters'][0]['needs'], full['chapters'][0]['plays'], full['ok']), (6, 3, False))
        low = self.report([{'id': 'b', 'chapter': 1, 'startFrame': 0, 'frames': 36 * FPS, 'density': 0.25}], sweeps)
        self.assertEqual(low['chapters'][0]['needs'], 2)
        self.assertEqual(low['chapters'][0]['beats'][0]['maxGap'], 48.0)
        self.assertTrue(low['ok'])
        none = self.report([{'id': 'b', 'chapter': 1, 'startFrame': 0, 'frames': 36 * FPS, 'density': 0.0}], [])
        self.assertEqual((none['chapters'][0]['needs'], none['chapters'][0]['beats'][0]['maxGap'], none['ok']), (0, None, True))


if __name__ == '__main__':
    unittest.main()
