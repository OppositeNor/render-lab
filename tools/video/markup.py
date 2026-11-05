"""A narration sentence's cues, parsed: shot cues timed to the word they
stand before, ranges over words, and formulas shown in the captions and the
formula card. A storyboard is a Lamlet presentation (presentation.py), whose
loader gives each sentence its `Sentence`; this module reads a cue's
attributes into a shot key (`cue_key`), and handles the rich text a caption
keeps for the lab to draw (src/lab/rich_text.rs): a formula is
`{@ formula="…"}`, a code name `{@ codespan="…"}`.

A cue's attributes are pairs of Strings, as the investigation's vocabulary
writes them (`render-lab story keys`; `Fields` reads its field list): any
shot parameter, `callout`,
`code`, `panel`, `demo`, `highlight=a,b`, `record=card:3`, `ease`,
`blend=0.5` (seconds), `term=n` (render_lab's Lamlet package writes
`highlight`, `record`, `animate`, `imageMark`, points, and the hide
lists), and the package's actions
(src/lab/actions.rs): `run=jumpFlood run.seconds=0.5` starts one with its
arguments, `stop=a,b` stops them; and `image=name` (`imageTitle=text-key`,
`imageCorner=top-left`), an image the investigation publishes, a PNG
resource, or, with `imageFrom=kind` and `image.<param>=value`, an instance of
an image kind its shaders define, or, with `imageGraph=node.resource`
(`imageShow=kind`), a resource of the frame's graph as it stood right after
a node (the inspector), in an inset (`imageHide=a,b` hides some, `image=""`
all; `imageClick=handler` runs the package's click handler on a click on
it); arrows and labels: `arrow=name arrowFrom=x,y,z arrowTo=point
arrowColor=#rrggbb arrowWidth=3`, `arrows=set`, `label=name labelAt=x,y,z
labelText=text-key labelColor=#rrggbb`, a point three numbers or one the
package publishes, `arrowHide=a,b`, `labelHide=a,b` (`arrow=""`, `label=""`
hide all). A range's values `a..b` jump to `a` where it opens and blend to `b`
exactly where it closes. A formula's cue (`formula`, `term=n` lighting term
n of `\\lit{n}{…}`, `card=no` for the caption only, `display=yes`,
`colors="#rrggbb,…"`) opens the formula card; `formula=off` closes it.
"""
from __future__ import annotations

import json
import math
import re
from decimal import Decimal
from dataclasses import dataclass, field
from typing import Any
from locations import DiagnosticError

Json = dict[str, Any]

FORMULA_ATTRS = ('formula', 'say', 'card', 'display', 'colors')
# Fields the reader types itself (`cue_key`); every other field of the vocabulary is kept as written.
TYPED = ('camera', 'animate', 'delay', 'blend', 'term', 'run', 'stop', 'ease', 'imageMark', 'imageHide', 'arrowHide', 'labelHide',
         'arrowFrom', 'arrowTo', 'labelAt', 'arrowWidth', 'highlight', 'record', *FORMULA_ATTRS)


class MarkupError(DiagnosticError):
    def __init__(self, message: str):
        super().__init__('story.markup', message)


@dataclass(frozen=True)
class Fields:
    """What a cue's attributes name besides values, from the investigation's
    vocabulary key list (`render-lab story keys`, which the builder writes to
    the package's build/<package>.keys.json; `"field": true` marks a key's
    fields): the fields kept as written, the camera's fields, and the easings."""
    text: frozenset[str]
    camera: frozenset[str]
    eases: frozenset[str]

    @staticmethod
    def of(keys: list[Json]) -> Fields:
        def attr(k: Json) -> str:
            return k.get('key', k['name'])
        fields = [k for k in keys if k.get('field')]
        return Fields(text=frozenset(attr(k) for k in fields if k['kind'] == 'text' and attr(k) not in TYPED and '.' not in attr(k)),
                      camera=frozenset(attr(k).removeprefix('camera.') for k in keys if not k.get('field') and attr(k).startswith('camera.')),
                      eases=frozenset(o for k in fields if attr(k) == 'ease' for o in k.get('options', [])))


@dataclass(frozen=True)
class Range:
    """A value a range blends over its words: it jumps to `start` where the
    range opens and blends to `end` where it closes (`a..b`)."""
    start: float
    end: float


@dataclass
class Tag:
    """A tag of a sentence: what kind it is, its name (cues, ranges), its
    attributes, the index of the spoken piece it stands before, and the key
    it sets (`cue_key`; none for a range's close)."""
    kind: str                  # 'cue', 'open', 'close', or 'formula'
    name: str | None
    attrs: list[tuple[str, str]]
    piece: int
    key: Json | None = None
    source: Json | None = None

    def attr(self, name: str) -> str | None:
        return next((v for k, v in self.attrs if k == name), None)


@dataclass
class Sentence:
    caption: str               # tags removed but formulas, as `{@ formula="…"}`
    spoken: str                # what the voice says
    pieces: list[str]          # the spoken text cut at each tag
    tags: list[Tag] = field(default_factory=list)
    formulas: list[str] = field(default_factory=list)


def quote(value: str) -> str:
    return '"' + value.replace('"', '\\"') + '"'


def codespan_tag(code: str) -> str:
    """A code span as the lab draws it."""
    return '{@ codespan=' + quote(code) + '}'


def formula_tag(tag: Tag) -> str:
    """The formula as the lab draws it: its LaTeX, and display style and term colours if given."""
    parts = [f'formula={quote(tag.attr("formula") or "")}']
    if tag.attr('display'):
        parts.append(f'display={tag.attr("display")}')
    if tag.attr('colors'):
        parts.append(f'colors={quote(tag.attr("colors") or "")}')
    return '{@ ' + ' '.join(parts) + '}'


def tidy(text: str) -> str:
    """Spaces as a sentence has them once its tags are gone."""
    text = re.sub(r'\s+', ' ', text).strip()
    return re.sub(r' ([,.;:!?)])', r'\1', text)


def number(value: str | int | float, where: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (str, int, float)):
        raise MarkupError(f'{where}: {value!r} is not a number')
    try:
        out = float(value)
    except (ValueError, OverflowError):
        raise MarkupError(f'{where}: {value!r} is not a number') from None
    if not math.isfinite(out):
        raise MarkupError(f'{where}: {value!r} must be a finite number')
    return out


def integer(value: str | int | float, where: str, minimum: int = 0, maximum: int | None = None) -> int:
    n = number(value, where)
    # Check the written decimal as well: float rounding must not turn a
    # fractional chapter number or index into an integer.
    exact = Decimal(str(value))
    if not n.is_integer() or exact != exact.to_integral_value():
        raise MarkupError(f'{where}: {value!r} must be a whole number')
    result = int(exact)
    if result < minimum or (maximum is not None and result > maximum):
        limit = f'{minimum} to {maximum}' if maximum is not None else f'at least {minimum}'
        raise MarkupError(f'{where}: {value!r} must be {limit}')
    return result


def param_value(value: str, where: str) -> float | bool | str | list[float]:
    """A parameter's value as a tag writes it: a number, `true` or `false`, a
    comma-separated list of numbers (a vector), or a name (a choice's option).
    The lab checks it against the parameter's declaration when it loads the
    timeline (lab/src/values.rs, `resolve`)."""
    if value in ('true', 'false'):
        return value == 'true'
    if ',' in value:
        return [number(x, where) for x in value.split(',')]
    try:
        float(value)
    except ValueError:
        pass
    else:
        return number(value, where)
    if value[:1].isalpha() and all(c.isalnum() or c == '_' for c in value):
        return value
    raise MarkupError(f'{where}: {value!r} is not a number, true or false, a list of numbers, or a name')


def action_attr(key: Json, name: str, value: str, where: str) -> bool:
    """Reads an action's attribute into `key`: `run=name` starts the action,
    `run.<arg>=value` gives it an argument, `stop=a,b` stops actions. False
    for any other attribute."""
    if name == 'run':
        key.setdefault('run', {'name': '', 'args': {}})['name'] = value
    elif name.startswith('run.'):
        key.setdefault('run', {'name': '', 'args': {}})['args'][name[4:]] = param_value(value, where)
    elif name == 'stop':
        key['stop'] = [v for v in value.split(',') if v]
    else:
        return False
    return True


def point_value(value: str, where: str) -> list[float] | str:
    """An arrow's end or a label's point as a tag writes it: three numbers,
    or the key of a point the package publishes (`probe.top`)."""
    if ',' in value:
        point = [number(x, where) for x in value.split(',')]
        if len(point) != 3:
            raise MarkupError(f'{where}: {value!r} is not a point (three numbers)')
        return point
    if value and all(c.isalnum() or c in '._-' for c in value):
        return value
    raise MarkupError(f'{where}: {value!r} is not a point (three numbers, or a point\'s key)')


def image_attr(key: Json, name: str, value: str, where: str) -> bool:
    """Reads an image's attribute into `key`: `image.<param>=value` sets a
    parameter of an image kind's instance, `imageMark=u0,v0,u1,v1` points out a
    region of it, `imageHide=a,b` hides images. False for any other attribute."""
    if name.startswith('image.'):
        key.setdefault('imageParams', {})[name[6:]] = param_value(value, where)
    elif name == 'imageMark':
        # u0,v0,u1,v1 (fractions of the picture), or empty to fade the mark out.
        mark = [number(v, where) for v in value.split(',') if v]
        if len(mark) not in (0, 4):
            raise MarkupError(f'{where}: imageMark is u0,v0,u1,v1 or "", not {value!r}')
        key[name] = mark
    elif name in ('imageHide', 'arrowHide', 'labelHide'):
        key[name] = [v for v in value.split(',') if v]
    elif name in ('arrowFrom', 'arrowTo', 'labelAt'):
        key[name] = point_value(value, where)
    elif name == 'arrowWidth':
        key[name] = number(value, where)
    else:
        return False
    return True


def check_action(key: Json, where: str) -> None:
    run = key.get('run')
    if run is not None and not run['name']:
        raise MarkupError(f'{where}: run.{next(iter(run["args"]), "…")} gives an argument, but no run=… names the action')
    if run is not None and not run['args']:
        del run['args']


def value(text: str, where: str) -> float | bool | str | list[float] | Range:
    """A value as a cue writes it (`param_value`), or a range's `a..b`."""
    if '..' in text:
        a, b = text.split('..', 1)
        return Range(number(a, where), number(b, where))
    return param_value(text, where)


def cue_key(tag: Tag, where: str, fields: Fields) -> Json:
    """The shot key a cue (a point cue, a range's start, a formula, or one of
    a beat's keys) sets, before timing: fields as the storyboard writes them
    (`blend` and `delay` in seconds), values typed (`value`)."""
    key: Json = {}
    params: Json = {}
    for name, v in tag.attrs:
        if name.startswith('param.'):
            params[name[6:]] = value(v, where)
        elif name == 'camera' and v == 'continuous':
            key.setdefault('camera', {})
        elif name.startswith('camera.'):
            part = name.split('.', 1)[1]
            if part not in fields.camera:
                raise MarkupError(f'{where}: unknown camera field {part!r}')
            key.setdefault('camera', {})[part] = [number(x, where) for x in v.split(',')] if part == 'target' else number(v, where)
        elif name == 'animate':
            try:
                key['animate'] = json.loads(v)
            except ValueError as e:
                raise MarkupError(f'{where}: animate is JSON: {e}') from None
        elif name in ('blend', 'delay'):
            key[name] = number(v.removesuffix('s'), where)
        elif name == 'ease':
            if v not in fields.eases:
                raise MarkupError(f'{where}: unknown ease {v!r} (one of {", ".join(sorted(fields.eases))})')
            key['ease'] = v
        elif action_attr(key, name, v, where) or image_attr(key, name, v, where):
            continue
        elif name in fields.text:
            key[name] = v
        elif name == 'highlight':
            key['highlight'] = [h for h in v.split(',') if h]
        elif name == 'record':
            kind, _, index = v.partition(':')
            key['record'] = {'type': kind} if not index else {'type': kind, 'index': integer(index, where)}
        elif name == 'term':
            key['term'] = integer(v, where)
        elif name == 'formula':
            key['formula'] = '' if v == 'off' else formula_tag(tag)
        elif name in FORMULA_ATTRS:
            continue
        else:
            params[name] = value(v, where)
    if tag.kind == 'formula' and tag.attr('card') == 'no':
        key.pop('formula', None)
    check_action(key, where)
    if params:
        key['params'] = params
    return key


def range_values(key: Json, where: str) -> tuple[Json, Json]:
    """A range's initial jump and final values, shared by preflight and timing."""
    jump: Json = {}
    to: Json = {}
    rest = dict(key)
    for name, value in rest.pop('params', {}).items():
        if isinstance(value, Range):
            jump[name] = value.start
            to[name] = value.end
        elif isinstance(value, float):
            to[name] = value
        else:
            raise MarkupError(f'{where}: {name} in a range blends to a number, not {value!r}')
    return jump, {**rest, 'params': to}


FORMULA_TAG = re.compile(r'\{@ formula="((?:[^"\\]|\\.)*)"[^}]*\}')
CODESPAN_TAG = re.compile(r'\{@ codespan="((?:[^"\\]|\\.)*)"\}')
# A caption's drawn tags: formulas and code spans.
DRAWN_TAG = re.compile(r'\{@ (formula|codespan)="((?:[^"\\]|\\.)*)"[^}]*\}')


def unlit(latex: str) -> str:
    """LaTeX without its term markers: `\\lit{n}{body}` becomes `body`."""
    out = ''
    while (i := latex.find('\\lit{')) >= 0:
        out += latex[:i]
        j = latex.index('}', i) + 1
        depth, k = 0, j
        while True:
            depth += {'{': 1, '}': -1}.get(latex[k], 0)
            if depth == 0:
                break
            k += 1
        out += unlit(latex[j + 1:k])
        latex = latex[k + 1:]
    return out + latex


def readable(caption: str) -> str:
    """A caption with its formulas as `$LaTeX$` (term markers dropped) and
    its code spans as their text, for documents and subtitle files."""
    caption = CODESPAN_TAG.sub(lambda m: m.group(1).replace('\\"', '"'), caption)
    return FORMULA_TAG.sub(lambda m: '$' + unlit(m.group(1).replace('\\"', '"')) + '$', caption)


def protect(caption: str) -> tuple[str, list[str]]:
    """A caption with each formula tag and code span replaced by one
    placeholder word about as wide as it is drawn, so captions wrap and chunk
    around them, never inside one; `restore` puts them back."""
    tags: list[str] = []

    def hold(m: re.Match[str]) -> str:
        tags.append(m.group(0))
        body = m.group(2)
        visible = body if m.group(1) == 'codespan' else re.sub(r'\\[a-zA-Z]+|[{}^_&]|\\\\', '', body)
        return f'QQF{len(tags) - 1}' + 'Q' * max(0, len(visible) - 4)
    return DRAWN_TAG.sub(hold, caption), tags


def restore(text: str, tags: list[str]) -> str:
    return re.sub(r'QQF(\d+)Q*', lambda m: tags[int(m.group(1))], text)


def sentence(s: Json) -> Sentence:
    """A narration sentence parsed, in English (what is said and cued)."""
    return s['parsed']


def shown(s: Json, translation: bool) -> Sentence:
    """A narration sentence as its edition shows it: the translation's
    (`shown`, in the zh-CN edition), else the English."""
    if translation:
        return s['shown']
    return s['parsed']


def tagged(s: Json) -> bool:
    """Whether a sentence has tags: cues, ranges, formulas, or code spans."""
    parsed = sentence(s)
    return bool(parsed.tags) or '{@' in parsed.caption


def text_keys(beat: Json, s: Json) -> list[Json]:
    """The keys a sentence's tags set, untimed: for checks that look at every
    key (callouts, listings)."""
    return [t.key for t in sentence(s).tags if t.key is not None]
