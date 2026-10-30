"""Tests of the storyboard file type (story.py): `python3 -m unittest tools/video/test_story.py`."""
from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import story  # noqa: E402

STORY = r'''title: A test
fps: 30

# 1 | First chapter | 60 s
summary: What it covers.
sources: key-a, key-b

## c1-one | 3d | One beat
sources: key-c
{@ demo=room camera.target=0,1,2 camera.yaw=30 opacity=1 param.formula=2 viewMode=ggxD lightOn=false lightPosition=0.4,1.9,0.8}
{@define wide camera.distance=8 blend=3}
{@define flood run=jumpFlood run.seconds=0.5 run.cell=1,2,3}
{@define halt stop=jumpFlood,sweep}
{@define seeds image=rlstd.icon/globe imageCorner=bottom-left blend=0.5}
{@define maps image=floor imageFrom=probeMap image.probe=7,7,8 imageHide=seeds}
{@define point arrow=a arrowFrom=0,1,2 arrowTo=probe.top arrowWidth=4 label=l labelAt=1,2,3 labelText=demos.hit labelHide=old}

Diffuse: a sentence that looks like a property, {@ callout=key-a}with a cue.

{@wide}A formula {@ formula="\frac{1}{1+\lit{1}{n}}" say="one over one plus n"} with single backslashes. {@ pause=0.5}

## c1-two | 2d | A diagram
diagram: frame
focus: A, B

The diagram's sentence.
'''


class Story(unittest.TestCase):
    def load(self, text: str, name: str = 'board.story') -> dict:
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / name
            path.write_text(text)
            return story.load(path) if not name.endswith('.zh-CN.story') else story.load_translation(path)

    def test_reads_every_part(self) -> None:
        board = self.load(STORY)
        self.assertEqual((board['title'], board['fps']), ('A test', 30))
        chapter = board['chapters'][0]
        self.assertEqual((chapter['number'], chapter['title'], chapter['targetSeconds'], chapter['sources']), (1, 'First chapter', 60.0, ['key-a', 'key-b']))
        one, two = chapter['beats']
        # A parameter is a number, a flag, an option's name, or a vector, as the lab's declarations read them.
        self.assertEqual(one['keys'], [{'demo': 'room', 'camera': {'target': [0.0, 1.0, 2.0], 'yaw': 30.0}, 'params': {'opacity': 1.0, 'formula': 2.0, 'viewMode': 'ggxD', 'lightOn': False, 'lightPosition': [0.4, 1.9, 0.8]}}])
        self.assertEqual(one['cues']['wide'], {'camera': {'distance': 8.0}, 'blend': 3.0})
        # An action with its arguments, typed as a parameter's are, and the actions a cue stops.
        self.assertEqual(one['cues']['flood'], {'run': {'name': 'jumpFlood', 'args': {'seconds': 0.5, 'cell': [1.0, 2.0, 3.0]}}})
        self.assertEqual(one['cues']['halt'], {'stop': ['jumpFlood', 'sweep']})
        self.assertEqual(one['cues']['seeds'], {'image': 'rlstd.icon/globe', 'imageCorner': 'bottom-left', 'blend': 0.5})
        self.assertEqual(one['cues']['point'], {'arrow': 'a', 'arrowFrom': [0.0, 1.0, 2.0], 'arrowTo': 'probe.top', 'arrowWidth': 4.0, 'label': 'l', 'labelAt': [1.0, 2.0, 3.0], 'labelText': 'demos.hit', 'labelHide': ['old']})
        self.assertEqual(one['cues']['maps'], {'image': 'floor', 'imageFrom': 'probeMap', 'imageParams': {'probe': [7.0, 7.0, 8.0]}, 'imageHide': ['seeds']})
        self.assertTrue(one['narration'][0]['text'].startswith('Diffuse: a sentence'))
        self.assertIn(r'formula="\frac{1}{1+\lit{1}{n}}"', one['narration'][1]['text'])
        self.assertEqual(one['narration'][1]['pause'], 0.5)
        self.assertEqual((two['diagram'], two['focus']), ('frame', ['A', 'B']))

    def test_round_trip(self) -> None:
        board = self.load(STORY)
        for beat in board['chapters'][0]['beats']:
            beat.setdefault('cues', {})
        again = self.load(story.dump(board))
        for beat in again['chapters'][0]['beats']:
            beat.setdefault('cues', {})
        self.assertEqual(again, board)

    def test_scene_types(self) -> None:
        board = self.load(STORY)
        one, two = board['chapters'][0]['beats']
        self.assertEqual((one['kind'], two['kind']), ('3d', '2d'))
        self.assertTrue(story.is_3d(one) and story.is_diagram(two) and not story.is_card(two))
        # A 2d beat shows a diagram or a sources card; the separator is ` | ` only.
        with self.assertRaises(story.StoryError):
            self.load(STORY.replace('diagram: frame\nfocus: A, B\n', ''))
        with self.assertRaises(story.StoryError):
            self.load(STORY.replace('# 1 | First chapter | 60 s', '# 1 · First chapter · 60 s'))

    def test_translation(self) -> None:
        translation = self.load('title: 测试\nlocale: zh-CN\n\n# 1 | 第一章\nsummary: 内容。\n\n## c1-one | 一个\n\n第一句。\n\n第二句，{@ formula="\\frac{1}{1+n}"}。\n', 'board.zh-CN.story')
        self.assertEqual(translation['chapters'], [{'number': 1, 'title': '第一章', 'summary': '内容。'}])
        self.assertEqual(translation['beats']['c1-one']['narration'], ['第一句。', '第二句，{@ formula="\\frac{1}{1+n}"}。'])


if __name__ == '__main__':
    unittest.main()
