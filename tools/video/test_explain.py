"""A beat explained, and the frames its preview takes."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import explain  # noqa: E402
import lamlet  # noqa: E402
import preflight  # noqa: E402
from config import Config  # noqa: E402
from storyboard import load_board  # noqa: E402

Json = dict[str, Any]


class Frames(unittest.TestCase):
    def test_writes_name_set_values_and_motions(self) -> None:
        key = {'params': {'x': 1.0}, 'camera': {'yaw': 4.0}, 'animate': [{'path': {'params': ['camera.pitch', 'y'], 'through': [[1, 2]], 'seconds': 'sentence'}}]}
        self.assertEqual([(w['channel'], w['how']) for w in explain.writes(key)], [('x', 'set'), ('camera.yaw', 'set'), ('camera.pitch', 'path'), ('y', 'path')])

    def test_preview_takes_cues_the_frame_before_motion_and_blend_ends(self) -> None:
        timeline: Json = {'fps': 10, 'beats': [{'id': 'a', 'startFrame': 0, 'frames': 100}, {'id': 'b', 'startFrame': 100, 'frames': 50}], 'keys': [
            {'frame': 20, 'blend': 0, 'animate': [{'path': {'params': ['x'], 'through': [[1]], 'seconds': 2.5}}]},
            {'frame': 60, 'blend': 15, 'params': {'x': 2}},
            {'frame': 120, 'blend': 0}]}
        provenance = [{'beat': 'a', 'source': {'file': 'f', 'line': 1}}, {'beat': 'a', 'source': {'file': 'f', 'line': 2}}, {'beat': 'b'}]
        frames = dict(explain.preview_frames(timeline, provenance, 'a'))
        self.assertEqual(sorted(frames), [0, 19, 20, 44, 49, 59, 60, 74, 99])
        self.assertEqual(frames[44], 'end of a motion from f:1')
        self.assertEqual(frames[74], 'end of the blend from f:2')
        self.assertEqual(frames[19], 'before the cue at f:1')


@unittest.skipUnless(lamlet.BINARY.exists(), 'build the host and its public packages first')
class Beat(unittest.TestCase):
    def test_a_real_beat_lists_its_cues_in_order_with_takeovers(self) -> None:
        cfg = Config.load('godot/ssr', 'en')
        board = load_board(cfg)
        report = explain.explain(cfg, board, preflight.normalized(board)['cues'], 'c1-adds')
        self.assertEqual(report['sentences'][0]['paragraph'], 1)
        self.assertTrue(report['cues'] and report['cues'][0]['kind'] == 'opening')
        self.assertEqual(report['warnings'], [])
        pitch = [w for c in report['cues'] for w in c['writes'] if w['channel'] == 'camera.pitch' and w['how'] == 'path']
        self.assertTrue(all('takesOverFrom' in w for w in pitch))
        with self.assertRaises(SystemExit):
            explain.explain(cfg, board, [], 'no-such-beat')


if __name__ == '__main__':
    unittest.main()
