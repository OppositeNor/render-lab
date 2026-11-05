"""Preflight runs the real host's rules, before speech and before publishing a timeline."""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import build  # noqa: E402
import lamlet  # noqa: E402
import markup  # noqa: E402
import preflight  # noqa: E402
import presentation  # noqa: E402
from config import LAB, Config  # noqa: E402
from locations import location  # noqa: E402

Json = dict[str, Any]
STORY = '''$[Import* std]$[Import* content]$[Import* present]$[Import* render_lab]
# Test {#video fps=30 width=1280 height=720}
## Chapter {#c1 number=1 target=10}
### Beat {#b kind=3d}

Before ${cue "key" {' ATTRS}}after.
'''


@unittest.skipUnless(lamlet.BINARY.exists(), 'build the host and its public packages first')
class Preflight(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.cfg = Config.load('godot/ssr', 'en')
        cls.fields = markup.Fields.of(json.loads(lamlet.lab('--investigation', cls.cfg.investigation, 'story', 'keys')))

    def board(self, attrs: str) -> Json:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'storyboard.lam'
            path.write_text(STORY.replace('ATTRS', attrs))
            return presentation.load(path, [], self.fields)

    def test_rules_and_source_ranges_use_the_real_host(self) -> None:
        cases = [
            ('{attr "param.typo" "1"}', 'story.value'),
            ('{attr "run" "noSuchAction"}', 'story.action'),
            ('{attr "image" "sample"} {attr "imageGraph" "missing.resource"}', 'story.image'),
            ('{attr "panel" "typo"}', 'story.option'),
            ('{attr "camera.target" "1,2"}', 'story.key'),
            ('{attr "animate" "[{\\"orbit\\":{\\"params\\":[\\"camera.x\\",\\"camera.y\\"],\\"center\\":[0],\\"seconds\\":\\"sentence\\",\\"degrees\\":90}}]"}', 'story.motion'),
        ]
        for attrs, code in cases:
            with self.subTest(code=code, attrs=attrs):
                board = self.board(attrs)
                tag = markup.sentence(board['chapters'][0]['beats'][0]['narration'][0]).tags[0]
                with self.assertRaises(preflight.CheckError) as caught:
                    preflight.check(self.cfg, board)
                diagnostic = caught.exception.report['diagnostics'][0]
                self.assertEqual(diagnostic['code'], code)
                self.assertEqual(diagnostic['beat'], 'b')
                self.assertEqual(diagnostic['paragraph'], 1)
                self.assertEqual(diagnostic['piece'], tag.piece)
                self.assertIsNotNone(tag.source)
                for key in ('file', 'line', 'column', 'endLine', 'endColumn'):
                    self.assertEqual(diagnostic[key], tag.source[key])
                self.assertEqual(diagnostic['line'], 6)
                self.assertGreater(diagnostic['column'], 1)

    def test_symbolic_motion_is_retained(self) -> None:
        motion = [{'path': {'params': ['camera.yaw'], 'through': [[20.0]], 'seconds': 'sentence'}}]
        board = self.board('{attr "animate" ' + json.dumps(json.dumps(motion)) + '}')
        data = preflight.normalized(board)
        self.assertEqual(data['cues'][0]['key']['animate'][0]['path']['seconds'], 'sentence')
        self.assertNotIn('frame', data['cues'][0]['key'])
        self.assertTrue(preflight.host_check(self.cfg, data)['ok'])

    def test_action_arguments_are_checked_without_running_the_action(self) -> None:
        cfg = Config.load('godot/voxelgi', 'en')
        board = self.board('{attr "run" "marchCone"} {attr "run.seconds" "typo"}')
        with self.assertRaises(preflight.CheckError) as caught:
            preflight.check(cfg, board)
        self.assertEqual(caught.exception.report['diagnostics'][0]['code'], 'story.action')
        self.assertIn('seconds', caught.exception.report['diagnostics'][0]['message'])

    def test_runtime_duration_rule_matches_preflight(self) -> None:
        motion = {'path': {'params': ['camera.yaw'], 'through': [[20]], 'seconds': -1}}
        key = {'frame': 0, 'animate': [motion]}
        timeline = {'fps': 30, 'width': 1280, 'height': 720, 'totalFrames': 0,
                    'title': 'test', 'engineLabel': 'test', 'chapters': [], 'beats': [], 'keys': [key]}
        with self.assertRaises(preflight.CheckError) as caught:
            preflight.check_timeline(self.cfg, timeline)
        self.assertEqual(caught.exception.report['diagnostics'][0]['code'], 'story.motion.length')
        motion['path']['seconds'] = 0
        self.assertEqual(preflight.check_timeline(self.cfg, timeline)['renderLabTimelineProvenance'], 1)

    def test_independent_check_does_not_import_audio_or_rendering(self) -> None:
        code = '''
import importlib.abc
import runpy
import sys
class NoAudio(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname.split('.')[0] in {'narration', 'timeline', 'outputs', 'piper', 'onnxruntime', 'numpy', 'phonemizer'}:
            raise AssertionError('forbidden dependency: ' + fullname)
sys.meta_path.insert(0, NoAudio())
sys.argv = [sys.argv[1], '--investigation', 'godot/ssr', '--binary', sys.argv[2], '--json']
runpy.run_path(sys.argv[0], run_name='__main__')
'''
        env = dict(os.environ, RENDER_LAB_VIDEO_ENV='/missing-video-env', RENDER_LAB_VOICES='/missing-voices',
                   VK_DRIVER_FILES='/missing-vulkan-driver', DISPLAY='')
        result = subprocess.run([sys.executable, '-I', '-c', code, str(LAB / 'tools/video/check.py'), str(lamlet.BINARY)],
                                capture_output=True, text=True, env=env)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(json.loads(result.stdout)['ok'])

    def test_semantic_failure_precedes_device_selection_and_synthesis(self) -> None:
        import narration
        for attrs in ('{attr "param.typo" "1"}', '{attr "panel" "graph"} {highlight {\' "ssrTrcae"}}',
                      '{attr "panel" "code"} {attr "code" "trace"} {highlight {\' "compress"}}', '{attr "panel" "grph"}'):
            board = self.board(attrs)
            with self.subTest(attrs=attrs), tempfile.TemporaryDirectory() as directory, \
                    mock.patch.object(sys, 'argv', ['build.py', '--investigation', self.cfg.investigation, '--binary', str(lamlet.BINARY), '--prepare-only', '--prepare-dir', directory]), \
                    mock.patch.object(build, 'reexec_in_environment'), \
                    mock.patch.object(build, 'load_board', return_value=board), \
                    mock.patch.object(build, 'validate'), \
                    mock.patch.object(narration, 'select_device') as device, \
                    mock.patch.object(narration, 'synthesize') as synthesize:
                with self.assertRaises(preflight.CheckError):
                    build.main()
                device.assert_not_called()
                synthesize.assert_not_called()

    def test_invalid_final_timeline_is_not_written(self) -> None:
        import narration
        import outputs
        import timeline
        board = self.board('{attr "camera.yaw" "20"}')
        invalid = {'fps': 30, 'width': 1280, 'height': 720, 'totalFrames': 10,
                   'title': 'test', 'engineLabel': 'test', 'chapters': [], 'beats': [], 'keys': []}
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(sys, 'argv', ['build.py', '--investigation', self.cfg.investigation, '--binary', str(lamlet.BINARY), '--prepare-only', '--prepare-dir', directory]), \
                mock.patch.object(build, 'reexec_in_environment'), \
                mock.patch.object(build, 'load_board', return_value=board), \
                mock.patch.object(build, 'validate'), \
                mock.patch.object(narration, 'select_device', return_value=('cpu', 'test')), \
                mock.patch.object(narration, 'synthesize', return_value={}), \
                mock.patch.object(timeline, 'build_timeline', return_value=(invalid, [], [], {})), \
                mock.patch.object(outputs, 'write_audio') as audio:
            with self.assertRaises(preflight.CheckError) as caught:
                build.main()
            self.assertEqual(caught.exception.report['diagnostics'][0]['code'], 'timeline.invalid')
            self.assertFalse((Path(directory) / 'timelines/en/timeline.json').exists())
            audio.assert_not_called()

    def test_final_check_keeps_locations_and_removes_only_provenance(self) -> None:
        source = {'file': 'story.lam', 'line': 9, 'column': 17, 'endLine': 9, 'endColumn': 30}
        key = {'frame': 0, 'source': 'b', 'params': {'typo': 1}, '_provenance': {'source': source, 'paragraph': 2}}
        timeline = {'fps': 30, 'width': 1280, 'height': 720, 'totalFrames': 0,
                    'title': 'test', 'engineLabel': 'test', 'chapters': [], 'beats': [], 'keys': [key]}
        with self.assertRaises(preflight.CheckError) as caught:
            preflight.check_timeline(self.cfg, timeline)
        self.assertEqual(caught.exception.report['diagnostics'][0]['column'], 17)
        del key['params']
        sidecar = preflight.check_timeline(self.cfg, timeline)
        self.assertEqual(sidecar['renderLabTimelineProvenance'], 1)
        self.assertEqual(sidecar['keys'][0]['source'], source)
        self.assertEqual(timeline['keys'], [{'frame': 0, 'source': 'b'}])


class Locations(unittest.TestCase):
    def test_missing_container_location_is_explicit(self) -> None:
        at = {'file': 'story.lam', 'line': 4, 'column': 18, 'endLine': 4, 'endColumn': 30}
        found = location({'inlines': [{'type': 'text'}, {'at': at}]}, {'file': 'story.lam'})
        self.assertEqual(found['column'], 18)
        self.assertIn('container', found['fallback'])
        self.assertEqual(location({}, {'file': 'story.lam'}).get('column'), None)


if __name__ == '__main__':
    unittest.main()
