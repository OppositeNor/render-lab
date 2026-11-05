"""Heading, numeric and translation errors must fail before dictionary conversion or speech."""
from __future__ import annotations

import json
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
import presentation  # noqa: E402
import storyboard  # noqa: E402
import zh  # noqa: E402
from config import LAB, Config  # noqa: E402
from locations import DiagnosticError  # noqa: E402

Json = dict[str, Any]
STORY = '''$[Import* std]$[Import* content]$[Import* present]$[Import* render_lab]
# Story {#video fps=30 width=1920 height=1080 subtitle=Test}
## Chapter {#c1 number=1 target=10}
### Beat {#b kind=3d}

A sentence.
'''
TRANSLATION = '''$[Import* std]$[Import* content]
# 故事 {#video locale=zh-CN subtitle=测试}
## 章节 {#c1 number=1}
### 镜头 {#b}

正文。
'''
SECOND = '\n## Another {#c2 number=2 target=0}\n### Another beat {#b2 kind=3d}\n\nAnother sentence.\n'


@unittest.skipUnless(lamlet.BINARY.exists(), 'build the host and its public package libraries first')
class Structure(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.fields = markup.Fields.of(json.loads(lamlet.lab('--investigation', 'godot/ssr', 'story', 'keys')))

    def load(self, text: str, translation: bool = False) -> Json:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'story.lam'
            path.write_text(text)
            return presentation.load_translation(path) if translation else presentation.load(path, [], self.fields)

    def failure(self, text: str, code: str, translation: bool = False, field: str | None = None) -> Json:
        with self.assertRaises(DiagnosticError) as caught:
            self.load(text, translation)
        diagnostic = caught.exception.diagnostic
        self.assertEqual(diagnostic['code'], code)
        self.assertIsNotNone(diagnostic['line'])
        self.assertIn('fallback', diagnostic)  # the IR locates heading text, not individual attributes
        if field is not None:
            self.assertEqual(diagnostic['field'], field)
        return diagnostic

    def test_duplicate_ids_and_numbers_name_both_declarations(self) -> None:
        cases = [
            (STORY + SECOND.replace('#c2 ', '#c1 '), 'story.chapter.duplicate', False),
            (STORY + SECOND.replace('number=2', 'number=1'), 'story.chapter.number.duplicate', False),
            (STORY + SECOND.replace('#b2 ', '#b '), 'story.beat.duplicate', False),
            (TRANSLATION + '\n## Again {#c1 number=2}\n### Again {#b2}\n\n内容。\n', 'story.chapter.duplicate', True),
            (TRANSLATION + '\n### Again {#b}\n\n内容。\n', 'story.beat.duplicate', True),
            (TRANSLATION + '\n## Again {#c2 number=1}\n### Again {#b2}\n\n内容。\n', 'story.chapter.number.duplicate', True),
        ]
        for text, code, translation in cases:
            with self.subTest(code=code, translation=translation):
                diagnostic = self.failure(text, code, translation)
                first = diagnostic['related'][0]
                self.assertEqual(first['file'], diagnostic['file'])
                self.assertLess(first['line'], diagnostic['line'])
                self.assertIn('first declared at', diagnostic['message'])

    def test_ids_are_required_in_both_editions(self) -> None:
        for text, translation in [(STORY, False), (TRANSLATION, True)]:
            for token, kind in [('#c1 ', 'chapter'), ('#b', 'beat')]:
                with self.subTest(kind=kind, translation=translation):
                    self.failure(text.replace(token, ''), f'story.{kind}.id', translation, 'id')

    def test_duplicate_heading_attributes_do_not_replace_earlier_values(self) -> None:
        cases = [
            (STORY.replace('fps=30', 'fps=30 fps=60'), False, 'fps'),
            (STORY.replace('number=1', 'number=1 number=2'), False, 'number'),
            (STORY.replace('kind=3d', 'kind=3d hold=0 hold=1'), False, 'hold'),
            (TRANSLATION.replace('locale=zh-CN', 'locale=zh-CN locale=en'), True, 'locale'),
            (TRANSLATION.replace('number=1', 'number=1 number=2'), True, 'number'),
            (TRANSLATION.replace('#b}', '#b meta.note=one meta.note=two}'), True, 'meta.note'),
        ]
        for text, translation, field in cases:
            with self.subTest(field=field, translation=translation):
                diagnostic = self.failure(text, 'story.heading.duplicate', translation, field)
                self.assertEqual(diagnostic['related'][0]['line'], diagnostic['line'])

    def test_unknown_heading_attributes_offer_a_candidate(self) -> None:
        cases = [
            (STORY.replace('fps=30', 'fps=30 gaap=0.5'), False, 'gaap', 'gap'),
            (STORY.replace('target=10', 'target=10 targte=2'), False, 'targte', 'target'),
            (STORY.replace('kind=3d', 'kind=3d holdd=1'), False, 'holdd', 'hold'),
            (TRANSLATION.replace('locale=zh-CN', 'locale=zh-CN captionWidh=64'), True, 'captionWidh', 'captionWidth'),
            (TRANSLATION.replace('number=1', 'number=1 summry=内容'), True, 'summry', 'summary'),
        ]
        for text, translation, field, candidate in cases:
            with self.subTest(field=field):
                diagnostic = self.failure(text, 'story.heading.unknown', translation, field)
                self.assertIn(f'did you mean {candidate!r}', diagnostic['message'])
        self.failure(TRANSLATION.replace('#b}', '#b kind=3d}'), 'story.heading.unknown', True, 'kind')

    def test_archival_and_namespaced_metadata_are_allowed(self) -> None:
        text = STORY.replace('fps=30', 'engineVersion="4.7.2-stable" commit=abcdef date=2026-11-04 meta.review=approved fps=30')
        text = text.replace('target=10', 'target=10 meta.owner=author').replace('kind=3d', 'kind=3d meta.note=example')
        board = self.load(text)
        self.assertEqual((board['engineVersion'], board['commit'], board['date'], board['meta.review']),
                         ('4.7.2-stable', 'abcdef', '2026-11-04', 'approved'))
        self.assertEqual(board['chapters'][0]['meta.owner'], 'author')
        self.assertEqual(board['chapters'][0]['beats'][0]['meta.note'], 'example')

    def test_integer_fields_do_not_truncate_or_accept_booleans(self) -> None:
        for value in ('1.7', '1.00000000000000001', '0', '-1', 'true', 'null', 'NaN', 'Infinity', '4294967296'):
            for text, translation in [(STORY, False), (TRANSLATION, True)]:
                with self.subTest(number=value, translation=translation):
                    self.failure(text.replace('number=1', f'number={value}'), 'story.heading.number', translation, 'number')
        board = self.load(STORY.replace('number=1', 'number=3.0') + SECOND.replace('number=2', 'number=7'))
        self.assertEqual([chapter['number'] for chapter in board['chapters']], [3, 7])
        board = self.load(STORY.replace('fps=30', 'fps=3e1').replace('width=1920', 'width=1920.0'))
        self.assertIs(type(board['fps']), int)
        self.assertIs(type(board['width']), int)

    def test_nonfinite_and_negative_heading_numbers_are_rejected(self) -> None:
        for field, token in [('fps', 'fps=30'), ('target', 'target=10'), ('hold', 'kind=3d'),
                             *[(name, 'fps=30') for name in ('lead', 'gap', 'tail', 'maxExtraPause', 'taggedWeight', 'termFade', 'captionBridge')]]:
            for value in ('NaN', 'Infinity', '-Infinity', '1e999', '-0.01', 'true', '[]'):
                addition = f'{field}={value}' if field in ('fps', 'target') else f'{token} {field}={value}'
                with self.subTest(field=field, value=value):
                    self.failure(STORY.replace(token, addition), 'story.heading.number', field=field)

    def test_fps_dimensions_and_caption_width_have_valid_domains(self) -> None:
        for field, value in [('fps', '0'), ('fps', '30.1'), ('fps', '30.00000000000000001'),
                             ('width', '0'), ('width', '8193'), ('width', '1920.5'), ('height', 'NaN')]:
            old = {'fps': 30, 'width': 1920, 'height': 1080}[field]
            with self.subTest(field=field, value=value):
                self.failure(STORY.replace(f'{field}={old}', f'{field}={value}'), 'story.heading.number', field=field)
        self.failure(STORY.replace('width=1920', 'width=1919'), 'story.heading.resolution', field='width/height')
        self.failure(STORY.replace('height=1080', 'height=1079'), 'story.heading.resolution', field='width/height')
        self.failure(STORY.replace('width=1920 ', ''), 'story.heading.missing', field='width')
        for text, translation, invalid in [(STORY, False, ('0', '1', '2', '3', '4.5', 'NaN')),
                                           (TRANSLATION, True, ('0', '1', '2', '3.5', 'NaN'))]:
            for value in invalid:
                with self.subTest(captionWidth=value, translation=translation):
                    self.failure(text.replace('{#video ', f'{{#video captionWidth={value} '), 'story.heading.number', translation, 'captionWidth')
        self.failure(STORY.replace('fps=30', f'fps=30 captionWidth={sys.maxsize + 1}'), 'story.heading.number', field='captionWidth')

    def test_locale_uses_a_supported_language(self) -> None:
        self.failure(STORY.replace('fps=30', 'fps=30 locale=typo'), 'story.heading.option', field='locale')
        self.failure(TRANSLATION.replace('locale=zh-CN', 'locale=typo'), 'story.heading.option', True, 'locale')

    def test_time_values_must_fit_the_actual_frame_representation(self) -> None:
        for field, token in [('lead', 'fps=30'), ('target', 'target=10'), ('hold', 'kind=3d')]:
            addition = f'{field}=1e308' if field == 'target' else f'{token} {field}=1e308'
            with self.subTest(field=field):
                self.failure(STORY.replace(token, addition), 'story.heading.number', field=field)
        self.failure(STORY.replace('fps=30', 'fps=30 taggedWeight=1e308'), 'story.heading.number', field='taggedWeight')

    def test_name_lists_reject_nonstring_elements(self) -> None:
        for field, token in [('sources', 'target=10'), ('focus', 'kind=3d')]:
            with self.subTest(field=field):
                self.failure(STORY.replace(token, f'{token} {field}=[1]'), 'story.heading.type', field=field)

    def test_caption_width_checks_the_text_before_audio(self) -> None:
        board = self.load(STORY.replace('fps=30', 'fps=30 captionWidth=4'))
        cfg = Config.load('godot/ssr', 'en')
        with self.assertRaises(DiagnosticError) as caught:
            storyboard.validate(cfg, board, {'entries': {}})
        self.assertEqual(caught.exception.diagnostic['code'], 'story.caption.width')
        self.assertEqual(caught.exception.diagnostic['line'], 6)
        translated = self.load(TRANSLATION.replace('locale=zh-CN', 'locale=zh-CN captionWidth=3').replace('正文。', 'Reflection。'), True)
        board = zh.localize(self.load(STORY), translated)
        cfg = Config.load('godot/ssr', 'zh-CN')
        with self.assertRaises(DiagnosticError) as caught:
            storyboard.validate(cfg, board, {'entries': {}})
        self.assertEqual(caught.exception.diagnostic['code'], 'story.caption.width')
        self.assertEqual(caught.exception.diagnostic['line'], translated['beats']['b']['narration'][0]['source']['line'])

    def test_zero_budget_hold_and_pacing_preserve_existing_meanings(self) -> None:
        zeros = 'lead=0 gap=0 tail=0 maxExtraPause=0 taggedWeight=0 termFade=0 captionBridge=0'
        board = self.load(STORY.replace('target=10', 'target=0').replace('kind=3d', 'kind=3d hold=0').replace('fps=30', f'fps=30 {zeros}'))
        self.assertEqual(board['chapters'][0]['targetSeconds'], 0)
        self.assertEqual(board['chapters'][0]['beats'][0]['hold'], 0)
        for field in zeros.split():
            self.assertEqual(board[field.split('=')[0]], 0)

    def test_empty_story_chapter_and_narration_fail(self) -> None:
        for text, translation in [(STORY, False), (TRANSLATION, True)]:
            with self.subTest(translation=translation):
                self.failure(text.split('## ')[0], 'story.empty', translation)
                self.failure(text.split('### ')[0], 'story.chapter.empty', translation)
                empty = text.replace('A sentence.', '').replace('正文。', '')
                self.failure(empty, 'story.narration.empty', translation)
        self.failure(STORY.replace('A sentence.', '${cue "key" {\' {attr "camera.yaw" "10"}}}'), 'story.narration.empty')
        self.failure(STORY.replace('A sentence.', '${say {code "visible"} ""}'), 'story.narration.empty')
        self.failure(STORY.replace('A sentence.', '...'), 'story.narration.empty')
        self.assertTrue(self.load(TRANSLATION.replace('正文。', '${math "x"}'), True)['beats']['b']['narration'])

    def test_cue_overrides_still_follow_source_order(self) -> None:
        text = STORY.replace('A sentence.', '${cue "key" {\' {attr "camera.yaw" "10"} {attr "camera.yaw" "20"}}}\n\nA sentence.')
        beat = self.load(text)['chapters'][0]['beats'][0]
        self.assertEqual(beat['keys'], [{'camera': {'yaw': 20.0}}])
        text = STORY.replace('A sentence.', '${cue "key" {\' {attr "camera.yaw" "10"}}}\n${cue "key" {\' {attr "camera.yaw" "20"}}}\n\nA sentence.')
        self.assertEqual(self.load(text)['chapters'][0]['beats'][0]['keys'], [{'camera': {'yaw': 10.0}}, {'camera': {'yaw': 20.0}}])

    def test_translation_alignment_checks_remain_active(self) -> None:
        english = self.load(STORY)
        for text, message in [(TRANSLATION.replace('#c1 ', '#other '), 'unknown chapters'),
                              (TRANSLATION.replace('#b}', '#other}'), 'no entry for beat'),
                              (TRANSLATION.replace('number=1', 'number=2'), 'is number 1'),
                              (TRANSLATION.replace('正文。', '正文。\n\n另一句。'), 'sentences'),
                              (TRANSLATION.replace('正文。', '正文，${math "x"}。'), 'formulas')]:
            with self.subTest(message=message):
                with self.assertRaisesRegex(RuntimeError, message):
                    zh.localize(english, self.load(text, True))

    def config(self, root: Path, text: str) -> Config:
        cfg = Config.load('godot/ssr', 'en')
        directory = root / 'fixture' / 'video'
        directory.mkdir(parents=True)
        (directory / 'story.lam').write_text(text)
        spec = {**cfg.spec, 'storyboard': 'story.lam', 'sourceIndex': str(cfg.source_index)}
        (directory / 'video.json').write_text(json.dumps(spec))
        return Config.load(cfg.investigation, 'en', directory.parent)

    def test_json_diagnostics_survive_python_optimization(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            cfg = self.config(Path(directory), STORY + SECOND.replace('#c2 ', '#c1 '))
            for optimize in ([], ['-O']):
                result = subprocess.run([sys.executable, *optimize, str(LAB / 'tools/video/check.py'),
                                         '--investigation', cfg.investigation, '--investigation-dir', str(cfg.package), '--json'],
                                        text=True, capture_output=True)
                self.assertEqual(result.returncode, 1, result.stderr)
                diagnostic = json.loads(result.stdout)['diagnostics'][0]
                self.assertEqual(diagnostic['code'], 'story.chapter.duplicate')
                self.assertLess(diagnostic['related'][0]['line'], diagnostic['line'])

    def test_structure_failure_precedes_synthesis(self) -> None:
        import narration
        for text in (STORY.replace('fps=30', 'fps=0'), STORY.replace('number=1', 'number=1.7'),
                     STORY.replace('target=10', 'target=NaN'), STORY + SECOND.replace('#c2 ', '#c1 '),
                     STORY.replace('fps=30', 'fps=30 captionWidth=4')):
            with self.subTest(text=text), tempfile.TemporaryDirectory() as directory:
                cfg = self.config(Path(directory), text)
                with mock.patch.object(sys, 'argv', ['build.py', '--investigation', cfg.investigation, '--binary', str(lamlet.BINARY), '--prepare-only', '--prepare-dir', str(Path(directory) / 'out')]), \
                        mock.patch.object(build, 'reexec_in_environment'), \
                        mock.patch.object(build.Config, 'load', return_value=cfg), \
                        mock.patch.object(narration, 'select_device') as device, \
                        mock.patch.object(narration, 'synthesize') as synthesize:
                    with self.assertRaises(DiagnosticError):
                        build.main()
                    device.assert_not_called()
                    synthesize.assert_not_called()


class Numbers(unittest.TestCase):
    def test_name_lists_keep_csv_and_json_forms(self) -> None:
        self.assertEqual(presentation.as_list('GI, SSR'), ['GI', 'SSR'])
        self.assertEqual(presentation.as_list('["GI", "SSR"]'), ['GI', 'SSR'])

    def test_numeric_cue_values_are_finite_without_slider_clamping(self) -> None:
        for value in ('NaN', 'Infinity', '-Infinity', '1e999', '0,NaN,1'):
            with self.subTest(value=value), self.assertRaises(markup.MarkupError):
                markup.param_value(value, 'cue')
        self.assertEqual(markup.param_value('-10000', 'cue'), -10000.0)
        self.assertEqual(markup.integer(str(sys.maxsize), 'caption', maximum=sys.maxsize), sys.maxsize)
        for value in ('1.7', '1.00000000000000001', '-1'):
            with self.subTest(value=value), self.assertRaises(markup.MarkupError):
                markup.integer(value, 'cue')

    def test_cue_ordinals_do_not_truncate(self) -> None:
        fields = markup.Fields(frozenset(), frozenset(), frozenset())
        for name, value in [('term', '1.7'), ('record', 'object:1.7'), ('term', '-1')]:
            with self.subTest(name=name), self.assertRaises(markup.MarkupError):
                markup.cue_key(markup.Tag('cue', None, [(name, value)], 0), 'cue', fields)


if __name__ == '__main__':
    unittest.main()
