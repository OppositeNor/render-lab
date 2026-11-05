"""Reference namespaces through the real Lamlet reader and GPU-free host."""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lamlet  # noqa: E402
import markup  # noqa: E402
import preflight  # noqa: E402
import presentation  # noqa: E402
from config import Config  # noqa: E402

Json = dict[str, Any]


@unittest.skipUnless(lamlet.BINARY.exists(), 'build the host and its public packages first')
class References(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.cfg = Config.load('godot/ssr', 'en')
        cls.fields = markup.Fields.of(json.loads(lamlet.lab('--investigation', cls.cfg.investigation, 'story', 'keys')))
        directory = tempfile.TemporaryDirectory()
        cls.addClassCleanup(directory.cleanup)
        cls.vocabulary = lamlet.vocabulary(cls.cfg.investigation, 'test_vocab', Path(directory.name) / 'test_vocab.lam')

    def check(self, *keys: Json) -> Json:
        return preflight.host_check(self.cfg, {'renderLabStory': 1, 'cues': [
            {'beat': 'b', 'paragraph': 1, 'piece': i, 'source': {'file': 'test.lam', 'line': i + 1}, 'key': key}
            for i, key in enumerate(keys)
        ]})

    def error(self, *keys: Json, field: str = 'highlight', contains: str) -> Json:
        with self.assertRaises(preflight.CheckError) as caught:
            self.check(*keys)
        diagnostic = caught.exception.report['diagnostics'][0]
        self.assertEqual(diagnostic['field'], field)
        self.assertEqual(diagnostic['file'], 'test.lam')
        self.assertIn(contains, diagnostic['message'])
        return diagnostic

    def test_passes_use_the_active_consumer(self) -> None:
        for panel in ('graph', 'dag', 'resources', 'performance'):
            with self.subTest(panel=panel):
                self.check({'panel': panel, 'highlight': [{'kind': 'pass', 'name': 'ssrTrace'}]})
                self.error({'panel': panel, 'highlight': [{'kind': 'pass', 'name': 'ssrTrcae'}]}, contains='did you mean `ssrTrace`?')
        self.check({'panel': 'graph', 'highlight': [{'kind': 'pass', 'name': 'ssrPrepassFS'}]})
        self.check({'panel': 'performance', 'highlight': [{'kind': 'pass', 'name': 'depth pre-pass'}]})
        self.error({'panel': 'performance', 'highlight': [{'kind': 'pass', 'name': 'ssrPrepassFS'}]}, contains='unknown pass')
        self.error({'panel': 'graph', 'highlight': [{'kind': 'pass', 'name': 'depth pre-pass'}]}, contains='unknown pass')

    def test_a_bare_name_is_refused(self) -> None:
        self.error({'panel': 'graph', 'highlight': ['ssrTrace']}, field='key', contains='expected internally tagged enum Highlight')

    def test_listing_scope_and_inherited_highlights(self) -> None:
        self.error({'panel': 'code', 'code': 'trace', 'highlight': [{'kind': 'code', 'listing': 'trace', 'tag': 'compress'}]}, contains='tag in listing `trace`')
        self.check({'panel': 'code', 'code': 'validate', 'highlight': [{'kind': 'code', 'listing': 'validate', 'tag': 'compress'}]})
        scoped = {'kind': 'code', 'listing': 'trace', 'tag': 'edge'}
        self.check({'panel': 'code', 'code': 'trace', 'highlight': [scoped]})
        # Both listings have edge, but the explicit reference stays bound to trace.
        diagnostic = self.error({'panel': 'code', 'code': 'trace', 'highlight': [scoped]},
                                {'code': 'validate'}, contains='namespace does not match')
        self.assertEqual(diagnostic['piece'], 1)
        self.check({'panel': 'code', 'code': 'trace', 'highlight': [scoped]}, {'code': 'validate', 'highlight': []})
        self.error({'panel': 'code', 'code': 'trace', 'highlight': [scoped]}, {'panel': 'graph'}, contains='namespace does not match')
        self.check({'panel': 'code', 'code': 'trace', 'highlight': [scoped]}, {'demo': 'room', 'panel': 'graph'})

    def test_record_fields_do_not_share_a_namespace(self) -> None:
        camera = {'panel': 'values', 'record': {'type': 'camera'}}
        self.check({**camera, 'highlight': [{'kind': 'field', 'record': 'CameraData', 'field': 'worldToView'}]})
        self.error({**camera, 'highlight': [{'kind': 'field', 'record': 'CameraData', 'field': 'localToWorld'}]}, contains='field in `CameraData`')
        self.error({**camera, 'highlight': [{'kind': 'field', 'record': 'ObjectData', 'field': 'localToWorld'}]},
                   contains='namespace does not match')
        self.check({'panel': 'memory', 'record': {'type': 'object', 'index': 0}, 'highlight': [{'kind': 'field', 'record': 'ObjectData', 'field': 'localToWorld'}]})

    def test_a_highlight_nothing_shows_is_refused(self) -> None:
        scoped = {'kind': 'code', 'listing': 'trace', 'tag': 'edge'}
        diagnostic = self.error({'panel': 'hidden', 'highlight': [scoped]}, contains='nothing shows this highlight')
        self.assertEqual(diagnostic['code'], 'story.highlight.unseen')
        self.error({'panel': 'code', 'code': 'trace', 'highlight': [scoped]}, {'panel': 'values', 'highlight': [scoped]},
                   contains='nothing shows this highlight')
        self.error({'panel': 'hidden', 'highlight': [{'kind': 'pass', 'name': 'ssrTrace'}]}, {'panel': 'graph'},
                   contains='nothing shows this highlight')
        # An explicit target's own mistake is reported first.
        self.error({'panel': 'hidden', 'highlight': [{'kind': 'pass', 'name': 'ssrTrcae'}]}, contains='did you mean `ssrTrace`?')
        # Clearing needs no panel, and a highlight stays set while its panel is away.
        self.check({'panel': 'hidden', 'highlight': []})
        self.check({'panel': 'graph', 'highlight': [{'kind': 'pass', 'name': 'ssrTrace'}]}, {'panel': 'hidden'}, {'panel': 'graph'})

    def test_raw_references_and_options_have_spelling_help(self) -> None:
        self.error({'panel': 'grph'}, field='panel', contains='did you mean `graph`?')
        self.error({'code': 'trcae'}, field='code', contains='did you mean `trace`?')
        self.error({'params': {'ssrMaxStpes': 2}}, field='params.ssrMaxStpes', contains='did you mean `ssrMaxSteps`?')
        self.error({'callout': 'pass-mian'}, field='callout', contains='did you mean `pass-main`?')
        self.error({'image': 'screen', 'imageGraph': 'ssrTrace.ssrColor', 'imageTitle': 'ui.paly'}, field='imageTitle', contains='did you mean `ui.play`?')
        self.check({'callout': ''})

    def test_cleanup_can_precede_activation_but_names_must_exist(self) -> None:
        cleanup = {'imageHide': ['screen'], 'arrowHide': ['ray'], 'labelHide': ['caption']}
        declarations = [
            {'image': 'screen', 'imageGraph': 'ssrTrace.ssrColor'},
            {'arrow': 'ray', 'arrowFrom': [0, 0, 0], 'arrowTo': [1, 1, 1]},
            {'label': 'caption', 'labelAt': [0, 0, 0], 'labelText': 'ui.play'},
        ]
        self.check(cleanup, *declarations, cleanup, cleanup)
        self.error({'imageHide': ['scren']}, *declarations, field='imageHide', contains='did you mean `screen`?')
        self.error({'labelHide': ['ray']}, *declarations, field='labelHide', contains='unknown label')
        self.error({'image': 'screen', 'imageParams': {'exposure': 1}}, *declarations, field='image', contains='not one')

    def test_graph_images_do_not_bypass_click_and_display_checks(self) -> None:
        image = {'image': 'screen', 'imageGraph': 'ssrTrace.ssrColor'}
        self.error({**image, 'imageClick': 'missing'}, field='imageClick', contains='click handler')
        self.error({**image, 'imageShow': 'colro'}, field='imageShow', contains='did you mean `color`?')
        self.error({**image, 'imageGraph': 'ssrTrace.missing'}, field='image', contains='no resource `missing`')

    def test_typed_helpers_compile_to_scoped_data_in_source_order(self) -> None:
        template = '''$[Import* std]$[Import* content]$[Import* present]$[Import* render_lab]$[Import* test_vocab]
# Test {#video fps=30 width=1280 height=720}
## Chapter {#c number=1 target=10}
### Beat {#b kind=3d}

Before ${cue "key" {' ATTRS}}after.
'''
        cases = [
            ('panelGraph {highlightPasses {\' "ssrTrace"}}', {'panel': 'graph', 'highlight': [{'kind': 'pass', 'name': 'ssrTrace'}]}),
            ('panelCode {highlightCode "trace" {\' "edge"}}', {'panel': 'code', 'code': 'trace', 'highlight': [{'kind': 'code', 'listing': 'trace', 'tag': 'edge'}]}),
            ('panelValues recordCamera {highlightFields "CameraData" {\' "worldToView"}}', {'panel': 'values', 'record': {'type': 'camera'}, 'highlight': [{'kind': 'field', 'record': 'CameraData', 'field': 'worldToView'}]}),
            ('{highlightPasses {\' "ssrTrace"}} unhighlight', {'highlight': []}),
        ]
        for attrs, expected in cases:
            with self.subTest(attrs=attrs), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'storyboard.lam'
                path.write_text(template.replace('ATTRS', attrs))
                board = presentation.load(path, [('test_vocab', self.vocabulary)], self.fields, self.cfg.investigation)
                key = preflight.normalized(board)['cues'][0]['key']
                self.assertEqual(key, expected)
                self.assertTrue(preflight.check(self.cfg, board)['ok'])


if __name__ == '__main__':
    unittest.main()
