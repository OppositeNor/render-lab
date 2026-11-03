"""Tests of a cue's attributes and a caption's rich text (markup.py): `python3 -m unittest tools/video/test_markup.py`."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import markup  # noqa: E402

FORMULA = r'\alpha = \frac{1}{1 + \lit{1}{n}}'

# A vocabulary key list as `render-lab story keys` writes it: values, then the key's fields.
KEYS = [{'name': 'camera.target', 'kind': 'vector'}, {'name': 'camera.yaw', 'kind': 'float'}, {'name': 'opacity', 'kind': 'float'},
        *({'name': n, 'kind': 'text', 'field': True} for n in ('callout', 'panel', 'imageTitle', 'imageCorner', 'imageGraph', 'imageShow',
                                                                 'imageClick', 'imageMarkText', 'arrow', 'arrowColor', 'arrows', 'label', 'labelText')),
        {'name': 'inset', 'key': 'image', 'kind': 'text', 'field': True}, {'name': 'imageFrom', 'kind': 'text', 'options': ['probeMap'], 'field': True},
        {'name': 'easing', 'key': 'ease', 'kind': 'text', 'options': ['smooth', 'sine'], 'field': True}]
FIELDS = markup.Fields.of(KEYS)


def cue(*attrs: tuple[str, str]) -> markup.Tag:
    return markup.Tag('cue', None, list(attrs), 1)


def key(tag: markup.Tag) -> dict:
    return markup.cue_key(tag, 'x', FIELDS)


class CueKey(unittest.TestCase):
    def test_parameters_and_fields(self) -> None:
        self.assertEqual(key(cue(('opacity', '1'), ('callout', 'x'))), {'callout': 'x', 'params': {'opacity': 1.0}})
        self.assertEqual(key(cue(('term', '1'))), {'term': 1})
        self.assertEqual(key(cue(('param.formula', '2.0'), ('camera.target', '0.0,1.0,2.0'), ('camera.yaw', '30.0'))),
                         {'camera': {'target': [0.0, 1.0, 2.0], 'yaw': 30.0}, 'params': {'formula': 2.0}})
        formula = markup.Tag('formula', None, [('formula', FORMULA), ('colors', '#ffd166'), ('say', 'alpha')], 1)
        self.assertEqual(key(formula), {'formula': f'{{@ formula="{FORMULA}" colors="#ffd166"}}'})
        self.assertEqual(key(markup.Tag('formula', None, [('formula', 'off')], 1)), {'formula': ''})
        # A range's value; the easings and the camera's fields are the vocabulary's.
        self.assertEqual(key(cue(('opacity', '0..1'))), {'params': {'opacity': markup.Range(0.0, 1.0)}})
        self.assertEqual(key(cue(('ease', 'sine'))), {'ease': 'sine'})
        for bad in (('ease', 'bounce'), ('camera.roll', '1')):
            with self.assertRaises(markup.MarkupError):
                key(cue(bad))

    def test_actions(self) -> None:
        self.assertEqual(key(cue(('run', 'jumpFlood'), ('run.seconds', '0.5'), ('run.cell', '34,52,61'), ('opacity', '1'))),
                         {'run': {'name': 'jumpFlood', 'args': {'seconds': 0.5, 'cell': [34.0, 52.0, 61.0]}}, 'params': {'opacity': 1.0}})
        self.assertEqual(key(cue(('stop', 'jumpFlood'))), {'stop': ['jumpFlood']})
        self.assertEqual(key(cue(('run', 'sweep'))), {'run': {'name': 'sweep'}})
        # An argument without the action it belongs to.
        with self.assertRaises(markup.MarkupError):
            key(cue(('run.seconds', '1')))

    def test_images(self) -> None:
        self.assertEqual(key(cue(('image', 'floodSeeds'), ('imageTitle', 'demos.seeds'), ('imageCorner', 'top-left'))),
                         {'image': 'floodSeeds', 'imageTitle': 'demos.seeds', 'imageCorner': 'top-left'})
        self.assertEqual(key(cue(('image', ''))), {'image': ''})
        self.assertEqual(key(cue(('image', 'floor'), ('imageFrom', 'probeMap'), ('image.probe', '7,7,8'), ('image.gain', '2'))),
                         {'image': 'floor', 'imageFrom': 'probeMap', 'imageParams': {'probe': [7.0, 7.0, 8.0], 'gain': 2.0}})
        self.assertEqual(key(cue(('imageHide', 'floor,ceiling'))), {'imageHide': ['floor', 'ceiling']})
        # A resource of the frame's graph after a node, drawn as counts.
        self.assertEqual(key(cue(('panel', 'graph'), ('image', 'lights'), ('imageGraph', 'o3deLightCullingCS.tileData'), ('imageShow', 'count'), ('image.range', '0,16'))),
                         {'panel': 'graph', 'image': 'lights', 'imageGraph': 'o3deLightCullingCS.tileData', 'imageShow': 'count', 'imageParams': {'range': [0.0, 16.0]}})
        self.assertEqual(key(cue(('arrow', 'a'), ('arrowFrom', '0,1,2'), ('arrowTo', 'probe.top'), ('arrowColor', '#50d7ff'), ('arrowWidth', '4'))),
                         {'arrow': 'a', 'arrowFrom': [0.0, 1.0, 2.0], 'arrowTo': 'probe.top', 'arrowColor': '#50d7ff', 'arrowWidth': 4.0})
        self.assertEqual(key(cue(('label', 'l'), ('labelAt', 'probe.top'), ('labelText', 'demos.hit'))), {'label': 'l', 'labelAt': 'probe.top', 'labelText': 'demos.hit'})
        self.assertEqual(key(cue(('arrows', 'rays'), ('arrowHide', 'a'), ('labelHide', 'l'))), {'arrows': 'rays', 'arrowHide': ['a'], 'labelHide': ['l']})
        self.assertEqual(key(cue(('image', 'map'), ('imageClick', 'pickCell'))), {'image': 'map', 'imageClick': 'pickCell'})
        self.assertEqual(key(cue(('image', 'map'), ('imageMark', '0,0.5,0.25,1'), ('imageMarkText', 'demos.near'), ('blend', '1'))),
                         {'image': 'map', 'imageMark': [0.0, 0.5, 0.25, 1.0], 'imageMarkText': 'demos.near', 'blend': 1.0})
        self.assertEqual(key(cue(('image', 'map'), ('imageMark', ''), ('imageMarkText', ''))), {'image': 'map', 'imageMark': [], 'imageMarkText': ''})
        with self.assertRaises(markup.MarkupError):
            key(cue(('image', 'map'), ('imageMark', '0,1')))

    def test_parameter_values(self) -> None:
        where = 'test'
        self.assertEqual([markup.param_value(v, where) for v in ('1.5', 'true', 'false', '0.4,1.9,0.8', 'ggxD')], [1.5, True, False, [0.4, 1.9, 0.8], 'ggxD'])
        for bad in ('a-b', '"x"', '1,b', ''):
            with self.assertRaises(markup.MarkupError):
                markup.param_value(bad, where)


class RichText(unittest.TestCase):
    def test_readable_and_protect(self) -> None:
        caption = f'with {{@ formula="{FORMULA}"}}, and {{@ codespan="r.Example.MaxIntensity"}} up to ten.'
        self.assertEqual(markup.readable(caption), r'with $\alpha = \frac{1}{1 + n}$, and r.Example.MaxIntensity up to ten.')
        held, tags = markup.protect(caption)
        self.assertNotIn('{', held)
        self.assertEqual(markup.restore(held, tags), caption)

    def test_tidy(self) -> None:
        self.assertEqual(markup.tidy('  A ray  marches , step by step ;  then  '), 'A ray marches, step by step; then')


if __name__ == '__main__':
    unittest.main()
