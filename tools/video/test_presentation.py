"""Tests of the storyboard file type, a Lamlet presentation (presentation.py), read by the lab's Lamlet
(lamlet.py): `python3 -m unittest tools/video/test_presentation.py`. They need the lab built
(`cargo build --release --locked --offline`) and are skipped until it is."""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lamlet  # noqa: E402
import markup  # noqa: E402
import presentation  # noqa: E402
import timeline  # noqa: E402

FORMULA = r'\frac{1}{1+\lit{1}{n}}'
# FORMULA as a Fluent string literal writes it (backslashes doubled).
Q = FORMULA.replace('\\', '\\\\')

# The cues use `attr` and `present`'s functions, so the test needs no investigation's vocabulary; a storyboard
# writes them with it (`{cameraYaw 30.0}` for `{attr "camera.yaw" 30.0}`), as typed data.
STORY = r'''$[Import* std]$[Import* content]$[Import* present]$[Import* render_lab]
$[# leaves 42289]
# A test {#video fps=30 width=1920 height=1080 subtitle="A short one"}

## First chapter {#c1 number=1 target=60 summary="What it covers." sources="key-a, key-b"}

### One beat {#c1-one kind=3d sources=key-c}

${cue "key" {' {attr "demo" "room"} {attr "camera.target" {' 0.0 1.0 2.0}} {attr "camera.yaw" 30.0} {attr "opacity" 1.0} {attr "param.formula" 2.0} {attr "viewMode" "ggxD"} {attr "lightOn" false}}}
${cue "key" {' {attr "animate" "[{\"path\": {\"params\": [\"camera.yaw\"], \"through\": [[10], [20]], \"seconds\": \"beat\"}}]"}}}

Diffuse: a sentence that looks like a property, ${cue "key" {' {attr "callout" "key-a"}}}with a cue, and $$5.

A ray ${during "grow" {' {attr "radius" {. from 0.0 to 2.0}}} {cat {' {text "marches "} {cue "key" {' {attr "opacity" 1.0}}} {text "step by step"}}}}, with ${cue "formula" {' {attr "formula" "\\frac{1}{1+\\lit{1}{n}}"} {attr "colors" "#ffd166"}}}${say {math "\\frac{1}{1+\\lit{1}{n}}"} "one over one plus n"}, ${code "LightCullingTilePrepare"} and ${say {code "r.Example.Max"} "max intensity"}.

There are ${spoken leaves} leaves.

### A diagram {#c1-two kind=2d diagram=frame focus="A, B"}

The diagram's sentence.
'''

TRANSLATION = r'''$[Import* std]$[Import* content]
# 测试 {#video locale=zh-CN glossary="SDFGI, GI mode"}

## 第一章 {#c1 number=1 summary="内容。"}

### 一个 {#c1-one}

第一句。

第二句，${math "\\frac{1}{1+\\lit{1}{n}}"}，${code "LightCullingTilePrepare"}。

第三句。

第四句。

### 图 {#c1-two}

图的句子。
'''


@unittest.skipUnless(lamlet.BINARY.exists(), 'the lab is not built (cargo build --release --locked --offline)')
class Presentation(unittest.TestCase):
    fields: markup.Fields

    @classmethod
    def setUpClass(cls) -> None:
        # The key fields of an investigation's vocabulary (the cues below write them with `attr`).
        cls.fields = markup.Fields.of(json.loads(lamlet.lab('--investigation', 'godot/voxelgi', 'story', 'keys')))

    def load(self, text: str, translation: bool = False) -> dict:
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / 'storyboard.lam'
            path.write_text(text)
            return presentation.load_translation(path) if translation else presentation.load(path, [], self.fields)

    def test_reads_every_part(self) -> None:
        board = self.load(STORY)
        self.assertEqual((board['title'], board['fps'], board['subtitle']), ('A test', 30, 'A short one'))
        chapter = board['chapters'][0]
        self.assertEqual((chapter['number'], chapter['title'], chapter['targetSeconds'], chapter['summary'], chapter['sources']), (1, 'First chapter', 60.0, 'What it covers.', ['key-a', 'key-b']))
        one, two = chapter['beats']
        self.assertEqual((one['id'], one['kind'], one['headline'], one['sources']), ('c1-one', '3d', 'One beat', ['key-c']))
        # A beat's keys, one per cue: a parameter is a number, a flag, an option's name, or a vector.
        self.assertEqual(one['keys'][0], {'demo': 'room', 'camera': {'target': [0.0, 1.0, 2.0], 'yaw': 30.0}, 'params': {'opacity': 1.0, 'formula': 2.0, 'viewMode': 'ggxD', 'lightOn': False}})
        self.assertEqual(one['keys'][1], {'animate': [{'path': {'params': ['camera.yaw'], 'through': [[10], [20]], 'seconds': 'beat'}}]})
        self.assertEqual((two['kind'], two['diagram'], two['focus']), ('2d', 'frame', ['A', 'B']))
        self.assertTrue(presentation.is_3d(one) and presentation.is_diagram(two) and not presentation.is_card(two))

    def test_sentences_are_parsed(self) -> None:
        one = self.load(STORY)['chapters'][0]['beats'][0]
        first, second, third = (markup.sentence(s) for s in one['narration'])
        self.assertEqual(first.spoken, 'Diffuse: a sentence that looks like a property, with a cue, and $5.')
        self.assertEqual([(t.kind, t.attrs) for t in first.tags], [('cue', [('callout', 'key-a')])])
        # Each cue stands before the piece it opens; a range closes where its words end.
        self.assertEqual(second.spoken, 'A ray marches step by step, with one over one plus n, LightCullingTilePrepare and max intensity.')
        self.assertEqual([(t.kind, t.name) for t in second.tags], [('open', 'grow'), ('cue', None), ('close', 'grow'), ('formula', None)])
        self.assertEqual([second.pieces[t.piece] for t in second.tags], ['marches', 'step by step', ', with', 'one over one plus n'])
        self.assertEqual(second.caption, f'A ray marches step by step, with {{ FORMULA("{Q}", colors: "#ffd166") }}, {{ CODE("LightCullingTilePrepare") }} and {{ CODE("r.Example.Max") }}.')
        self.assertEqual(second.formulas, [FORMULA])
        self.assertEqual(second.tags[3].key, {'formula': f'{{ FORMULA("{Q}", colors: "#ffd166") }}'})
        self.assertEqual(second.tags[0].key, {'params': {'radius': markup.Range(0.0, 2.0)}})
        # render_lab's numbers, said in words.
        self.assertEqual(third.spoken, 'There are forty-two thousand two hundred eighty-nine leaves.')

    def test_translation(self) -> None:
        translation = self.load(TRANSLATION, translation=True)
        self.assertEqual((translation['title'], translation['glossary']), ('测试', ['SDFGI', 'GI mode']))
        self.assertEqual(translation['chapters'], [{'id': 'c1', 'number': 1, 'title': '第一章', 'summary': '内容。'}])
        narration = translation['beats']['c1-one']['narration']
        self.assertEqual(narration[1]['shown'].caption, f'第二句，{{ FORMULA("{Q}") }}，{{ CODE("LightCullingTilePrepare") }}。')
        self.assertEqual(narration[1]['shown'].formulas, [FORMULA])
        # A chapter is the English one of its id; its number may be left out.
        self.assertEqual(self.load(TRANSLATION.replace(' number=1', ''), translation=True)['chapters'], [{'id': 'c1', 'title': '第一章', 'summary': '内容。'}])
        # A translation carries no cues.
        with self.assertRaisesRegex(presentation.PresentationError, 'carries no cues'):
            self.load(TRANSLATION.replace('第一句。', '$[Import* present]${cue "key" {\' {attr "a" "1"}}}第一句。'), translation=True)

    def test_names_the_lab_checks(self) -> None:
        # A callout, a text key, a published point, and a formula are checked against the investigation at the
        # storyboard's line, before any speech.
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / 'storyboard.lam'
            for wrong in ('{callout "no-such-key"}', '{imageTitle "no.such.text"}', '{arrowTo {point "no.such.point"}}'):
                with self.subTest(wrong=wrong):
                    path.write_text(STORY.replace('{attr "callout" "key-a"}', wrong))
                    with self.assertRaises(lamlet.LamletError) as e:
                        presentation.load(path, [], self.fields, 'godot/voxelgi')
                    self.assertIn('storyboard.lam:12:', str(e.exception))
            path.write_text(STORY.replace('There are ${spoken leaves} leaves.', 'A ${formulaSaid "x^{y^{z^{w}}}" "x to the y" {\'}}.'))
            with self.assertRaisesRegex(lamlet.LamletError, 'storyboard.lam:16:'):
                presentation.load(path, [], self.fields, 'godot/voxelgi')

    def test_key_check(self) -> None:
        # With the investigation's key list, every cue attribute is checked, also one written with `attr`: its key,
        # its value's type, a closed key's option. Every problem is reported, each at its line.
        story = ('$[Import* std]$[Import* content]$[Import* present]$[Import* render_lab]\n# A test {#video fps=30 width=1920 height=1080}\n\n'
                 '## One {#c1 number=1 target=60}\n\n### Beat {#c1-one kind=3d}\n\n'
                 '${cue "key" {\' {attr "ease" "smooth"} {attr "camera.yaw" 30.0} {attr "demo" "room"} {attr "image.level" 1.0}}}A sentence.\n\n'
                 '${cue "key" {\' WRONG}}Another sentence.\n')
        with tempfile.TemporaryDirectory() as d:
            keys = Path(d) / 'keys.json'
            keys.write_text(lamlet.lab('--investigation', 'godot/voxelgi', 'story', 'keys'))
            path = Path(d) / 'storyboard.lam'
            path.write_text(story.replace('WRONG', '{attr "blend" 1.0}'))
            presentation.load(path, [], self.fields, 'godot/voxelgi', keys)
            wrong = '{attr "ease" "bounce"} {attr "camera.roll" 1.0} {attr "blend" "1"} {attr "panel" "nowhere"} {attr "stop" {\' "noSuchAction"}}'
            path.write_text(story.replace('WRONG', wrong))
            with self.assertRaises(lamlet.LamletError) as e:
                presentation.load(path, [], self.fields, 'godot/voxelgi', keys)
            message = str(e.exception)
            for part in ('`bounce` is not one of', '`camera.roll` is not a cue attribute', '`blend="1"` is not a number', '`nowhere` is not one of',
                         '`noSuchAction` is not one of'):
                self.assertIn(part, message)
            self.assertEqual(message.count('storyboard.lam:10:'), 5, message)

    def test_pacing(self) -> None:
        # The video heading's pacing attributes, each with its default when left out.
        board = self.load(STORY.replace('fps=30', 'fps=30 gap=0.5 captionWidth=60'))
        pacing = timeline.Pacing.of(board)
        self.assertEqual((pacing.gap, pacing.captionWidth, pacing.lead), (0.5, 60, timeline.Pacing().lead))

    def test_errors(self) -> None:
        for kind in ('scene', 'sources'):
            with self.subTest(kind=kind):
                with self.assertRaisesRegex(presentation.PresentationError, 'the scene type is 3d or 2d'):
                    self.load(STORY.replace('kind=3d', f'kind={kind}'))
        # A 2d beat shows a diagram, the frame graph, or a sources card.
        with self.assertRaisesRegex(presentation.PresentationError, 'a 2d beat shows'):
            self.load(STORY.replace(' diagram=frame focus="A, B"', ''))
        # A point cue sets its values at once; a range blends them.
        with self.assertRaisesRegex(presentation.PresentationError, 'blends over a range'):
            self.load(STORY.replace('{cue "key" {\' {attr "opacity" 1.0}}}', '{cue "key" {\' {attr "opacity" {. from 0.0 to 1.0}}}}'))
        # A formula needs what is said for it.
        with self.assertRaisesRegex(presentation.PresentationError, 'no spoken form|has no say'):
            self.load(STORY.replace('There are', 'There are ${math "x"}'))
        # Lamlet's diagnostic, with the document's line.
        with self.assertRaises(lamlet.LamletError) as e:
            self.load(STORY.replace('There are ${spoken leaves}', 'There are ${unknownName 3}'))
        self.assertIn('unknownName', str(e.exception))
        self.assertIn('storyboard.lam:16:', str(e.exception))


if __name__ == '__main__':
    unittest.main()
