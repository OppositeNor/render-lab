"""Tests of a cue's attributes and a caption's rich text (markup.py): `python3 -m unittest tools/video/test_markup.py`."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import markup  # noqa: E402

FORMULA = r'\alpha = \frac{1}{1 + \lit{1}{n}}'


def cue(*attrs: tuple[str, str]) -> markup.Tag:
    return markup.Tag('cue', None, list(attrs), 1)


class CueKey(unittest.TestCase):
    def test_parameters_and_fields(self) -> None:
        self.assertEqual(markup.cue_key(cue(('opacity', '1'), ('callout', 'x')), 'x'), {'callout': 'x', 'params': {'opacity': '1'}})
        self.assertEqual(markup.cue_key(cue(('term', '1')), 'x'), {'term': 1})
        self.assertEqual(markup.cue_key(cue(('param.formula', '2.0'), ('camera.target', '0.0,1.0,2.0'), ('camera.yaw', '30.0')), 'x'),
                         {'camera': {'target': [0.0, 1.0, 2.0], 'yaw': 30.0}, 'params': {'formula': '2.0'}})
        formula = markup.Tag('formula', None, [('formula', FORMULA), ('colors', '#ffd166'), ('say', 'alpha')], 1)
        self.assertEqual(markup.cue_key(formula, 'x'), {'formula': f'{{@ formula="{FORMULA}" colors="#ffd166"}}'})
        self.assertEqual(markup.cue_key(markup.Tag('formula', None, [('formula', 'off')], 1), 'x'), {'formula': ''})

    def test_actions(self) -> None:
        self.assertEqual(markup.cue_key(cue(('run', 'jumpFlood'), ('run.seconds', '0.5'), ('run.cell', '34,52,61'), ('opacity', '1')), 'x'),
                         {'run': {'name': 'jumpFlood', 'args': {'seconds': 0.5, 'cell': [34.0, 52.0, 61.0]}}, 'params': {'opacity': '1'}})
        self.assertEqual(markup.cue_key(cue(('stop', 'jumpFlood')), 'x'), {'stop': ['jumpFlood']})
        self.assertEqual(markup.cue_key(cue(('run', 'sweep')), 'x'), {'run': {'name': 'sweep'}})
        # An argument without the action it belongs to.
        with self.assertRaises(markup.MarkupError):
            markup.cue_key(cue(('run.seconds', '1')), 'x')

    def test_images(self) -> None:
        self.assertEqual(markup.cue_key(cue(('image', 'floodSeeds'), ('imageTitle', 'demos.seeds'), ('imageCorner', 'top-left')), 'x'),
                         {'image': 'floodSeeds', 'imageTitle': 'demos.seeds', 'imageCorner': 'top-left'})
        self.assertEqual(markup.cue_key(cue(('image', '')), 'x'), {'image': ''})
        self.assertEqual(markup.cue_key(cue(('image', 'floor'), ('imageFrom', 'probeMap'), ('image.probe', '7,7,8'), ('image.gain', '2')), 'x'),
                         {'image': 'floor', 'imageFrom': 'probeMap', 'imageParams': {'probe': [7.0, 7.0, 8.0], 'gain': 2.0}})
        self.assertEqual(markup.cue_key(cue(('imageHide', 'floor,ceiling')), 'x'), {'imageHide': ['floor', 'ceiling']})
        # A resource of the frame's graph after a node, drawn as counts.
        self.assertEqual(markup.cue_key(cue(('panel', 'graph'), ('image', 'lights'), ('imageGraph', 'o3deLightCullingCS.tileData'), ('imageShow', 'count'), ('image.range', '0,16')), 'x'),
                         {'panel': 'graph', 'image': 'lights', 'imageGraph': 'o3deLightCullingCS.tileData', 'imageShow': 'count', 'imageParams': {'range': [0.0, 16.0]}})
        self.assertEqual(markup.cue_key(cue(('arrow', 'a'), ('arrowFrom', '0,1,2'), ('arrowTo', 'probe.top'), ('arrowColor', '#50d7ff'), ('arrowWidth', '4')), 'x'),
                         {'arrow': 'a', 'arrowFrom': [0.0, 1.0, 2.0], 'arrowTo': 'probe.top', 'arrowColor': '#50d7ff', 'arrowWidth': 4.0})
        self.assertEqual(markup.cue_key(cue(('label', 'l'), ('labelAt', 'probe.top'), ('labelText', 'demos.hit')), 'x'), {'label': 'l', 'labelAt': 'probe.top', 'labelText': 'demos.hit'})
        self.assertEqual(markup.cue_key(cue(('arrows', 'rays'), ('arrowHide', 'a'), ('labelHide', 'l')), 'x'), {'arrows': 'rays', 'arrowHide': ['a'], 'labelHide': ['l']})
        self.assertEqual(markup.cue_key(cue(('image', 'map'), ('imageClick', 'pickCell')), 'x'), {'image': 'map', 'imageClick': 'pickCell'})
        self.assertEqual(markup.cue_key(cue(('image', 'map'), ('imageMark', '0,0.5,0.25,1'), ('imageMarkText', 'demos.near'), ('blend', '1')), 'x'),
                         {'image': 'map', 'imageMark': [0.0, 0.5, 0.25, 1.0], 'imageMarkText': 'demos.near', 'blend': 1.0})
        self.assertEqual(markup.cue_key(cue(('image', 'map'), ('imageMark', ''), ('imageMarkText', '')), 'x'), {'image': 'map', 'imageMark': [], 'imageMarkText': ''})
        with self.assertRaises(markup.MarkupError):
            markup.cue_key(cue(('image', 'map'), ('imageMark', '0,1')), 'x')

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
