"""Tests of tools/publish_narration.py: a narration package's demos and files, and the checks of its outputs."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import publish_narration as publish  # noqa: E402

DEMOS = '''
[[provides]]
type = "resource"
path = "resources.toml"

[[provides]]
type = "demo"
key = "video"
title = "A video"
timeline = "timeline/en"
audio = "narration/en"
for = "demo_feature"

[[provides]]
type = "demo"
key = "video-zh-CN"
title = "A video"
timeline = "timeline/en"
audio = "narration/zh-cn"
{for_zh}
'''
HEADER = 'name = "demo_feature_narration"\nauthor = "Someone"\nlicense = "MIT"\nimport = ["demo_feature"]\nexport = []\n'
SOURCES = {'timeline/en': 'build/timelines/en/timeline.json', 'narration/en': 'build/video/en/narration.wav',
           'narration/zh-cn': 'build/video/zh-CN/narration.wav'}


class PublishNarrationTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.package = Path(self.temp.name) / 'demo_feature_narration'
        self.package.mkdir()
        (self.package / 'resources.toml').write_text(''.join(
            f'[[resource]]\nid = "{k}"\nkind = "file"\nsource = "{s}"\nlicense = "MIT"\nbuilt = true\n\n' for k, s in SOURCES.items()))

    def tearDown(self) -> None:
        self.temp.cleanup()

    def write_manifest(self, for_zh: str = 'for = "demo_feature"') -> None:
        (self.package / 'manifest.toml').write_text(HEADER + DEMOS.format(for_zh=for_zh))

    def write_outputs(self, zh_audio: bytes) -> None:
        for source, data in zip(SOURCES.values(), [b'{}', b'english', zh_audio]):
            (self.package / source).parent.mkdir(parents=True, exist_ok=True)
            (self.package / source).write_bytes(data)

    def test_a_narration_package_and_what_it_plays(self) -> None:
        self.write_manifest()
        narration = publish.read_narration(self.package)
        assert narration is not None
        self.assertEqual((narration.narrates, narration.sources), ('demo_feature', dict(sorted(SOURCES.items()))))

    def test_a_package_without_demos_is_none(self) -> None:
        (self.package / 'manifest.toml').write_text(HEADER)
        self.assertIsNone(publish.read_narration(self.package))

    def test_every_demo_names_what_it_narrates(self) -> None:
        self.write_manifest(for_zh='')
        with self.assertRaises(SystemExit) as raised:
            publish.read_narration(self.package)
        self.assertIn('`for`', str(raised.exception))

    def test_missing_and_shared_audio_are_reported(self) -> None:
        self.write_manifest()
        narration = publish.read_narration(self.package)
        assert narration is not None
        self.assertEqual(len(publish.check_outputs(narration)), 3)
        self.write_outputs(zh_audio=b'english')
        self.assertEqual(publish.check_outputs(narration), ['demo_feature_narration: `narration/zh-cn` is the same audio as `narration/en`'])
        self.write_outputs(zh_audio=b'chinese')
        self.assertEqual(publish.check_outputs(narration), [])
        self.assertEqual(publish.local_paths(self.package), [])


if __name__ == '__main__':
    unittest.main()
