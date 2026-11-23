"""Tests of tools/publish_narration.py: a package's demo resources, the checks of its outputs, and the narration
package it writes."""

from __future__ import annotations

import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import publish_narration as publish  # noqa: E402

MANIFEST = '''name = "demo_feature"
author = "Someone"
license = "MIT"
import = []
export = ["timeline/en", "narration/en", "narration/zh-cn"]

[[provides]]
type = "resource"
path = "resources.toml"

[[provides]]
type = "demo"
key = "video"
title = "A video"
timeline = "timeline/en"
audio = "narration/en"

[[provides]]
type = "demo"
key = "video-zh-CN"
title = "A video"
timeline = "timeline/en"
audio = "narration/zh-cn"
'''


def resource(key: str, source: str, overridable: bool = True) -> str:
    flag = 'overridable = true\n' if overridable else ''
    return f'[[resource]]\nid = "{key}"\nkind = "file"\nsource = "{source}"\nlicense = "MIT"\nbuilt = true\n{flag}\n'


class PublishNarrationTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.package = Path(self.temp.name) / 'demo_feature'
        self.package.mkdir()
        (self.package / 'manifest.toml').write_text(MANIFEST)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def write_resources(self, zh_overridable: bool = True) -> None:
        (self.package / 'resources.toml').write_text(
            resource('timeline/en', 'build/timelines/en/timeline.json')
            + resource('narration/en', 'build/video/en/narration.wav')
            + resource('narration/zh-cn', 'build/video/zh-CN/narration.wav', zh_overridable))

    def write_outputs(self, zh_audio: bytes) -> None:
        for path, data in [('build/timelines/en/timeline.json', b'{}'), ('build/video/en/narration.wav', b'english'),
                           ('build/video/zh-CN/narration.wav', zh_audio)]:
            (self.package / path).parent.mkdir(parents=True, exist_ok=True)
            (self.package / path).write_bytes(data)

    def narration(self) -> publish.Narration:
        manifest, resources = publish.demo_resources(self.package)
        return publish.Narration(self.package, manifest, resources)

    def test_demo_resources_each_once(self) -> None:
        self.write_resources()
        self.assertEqual([r.key for r in self.narration().resources], ['timeline/en', 'narration/en', 'narration/zh-cn'])

    def test_a_resource_not_overridable_is_refused(self) -> None:
        self.write_resources(zh_overridable=False)
        with self.assertRaises(SystemExit) as raised:
            publish.demo_resources(self.package)
        self.assertIn('narration/zh-cn', str(raised.exception))

    def test_missing_and_shared_audio_are_reported(self) -> None:
        self.write_resources()
        self.assertEqual(len(publish.check_outputs(self.narration())), 3)
        self.write_outputs(zh_audio=b'english')
        self.assertEqual(publish.check_outputs(self.narration()), ['demo_feature: `narration/zh-cn` is the same audio as `narration/en`'])
        self.write_outputs(zh_audio=b'chinese')
        self.assertEqual(publish.check_outputs(self.narration()), [])

    def test_the_package_overrides_each_resource(self) -> None:
        self.write_resources()
        self.write_outputs(zh_audio=b'chinese')
        directory = publish.write_package(self.narration(), Path(self.temp.name) / 'out')
        manifest = tomllib.loads((directory / 'manifest.toml').read_text())
        self.assertEqual((manifest['name'], manifest['import']), ('demo_feature_narration', ['demo_feature']))
        collection = tomllib.loads((directory / 'resources.toml').read_text())['resource']
        self.assertEqual([r['override'] for r in collection], ['demo_feature.timeline/en', 'demo_feature.narration/en', 'demo_feature.narration/zh-cn'])
        self.assertEqual((directory / 'build/video/zh-CN/narration.wav').read_bytes(), b'chinese')
        self.assertEqual(publish.local_paths(directory), [])


if __name__ == '__main__':
    unittest.main()
