"""Tags inside a narration sentence: shot cues timed to the word they stand
before, and formulas shown in the captions and the formula card.

A tag is `{@ name=value …}`; a value with spaces or braces is quoted, `"…"`,
where only `\\"` is an escape (LaTeX keeps its backslashes). The lab parses
the same tags in on-screen text (src/lab/rich_text.rs).

- `{@ opacity=1 callout=source-key}`: a point cue. Its keys fire when the
  next word starts. Attributes: any shot parameter, `callout`, `code`,
  `panel`, `demo`, `highlight=a,b`, `record=card:3`, `ease`, `blend=0.5`
  (seconds), `term=n`, and the package's actions (src/lab/actions.rs):
  `run=jumpFlood run.seconds=0.5` starts one with its arguments, `stop=a,b`
  stops them; and `image=name` (`imageTitle=text-key`, `imageCorner=top-left`),
  an image the investigation publishes, a PNG resource, or, with
  `imageFrom=kind` and `image.<param>=value`, an instance of an image kind its
  shaders define, in an inset (`imageHide=a,b` hides some, `image=""` all;
  `imageClick=handler` runs the package's click handler on a click on it);
  arrows and labels: `arrow=name arrowFrom=x,y,z arrowTo=point arrowColor=#rrggbb
  arrowWidth=3`, `arrows=set`, `label=name labelAt=x,y,z labelText=text-key
  labelColor=#rrggbb`, a point three numbers or one the package publishes,
  `arrowHide=a,b`, `labelHide=a,b` (`arrow=""`, `label=""` hide all).
- `{@intro}`: the named cue `intro` from the sentence's or the beat's `cues`
  (a full shot key, for cameras, records, and motions).
- `{@grow> radius=0..2}` … `{<grow}`: a range. `a..b` jumps to `a` where it
  opens and blends to `b` exactly where it closes; `=b` blends from the current
  value. A range named after a cue in `cues` blends that cue's values over the
  range, and its motions run for the range.
- `{@ codespan="LightCullingTilePrepare"}`: a code name, shown in the caption as
  code and voiced as written (or as `say="…"`); a translation carries it too.
- `{@ formula="\\alpha = \\frac{1}{1+n}" say="alpha equals one over one plus n"}`:
  a formula. The caption shows it, the voice says `say`, and the formula card
  opens on it (`card=no`: the caption only; `display=yes`: display style;
  `colors="#rrggbb,…"`: its terms' colours). `{@ formula=off}` closes the card,
  and `term=n` lights term n of `\\lit{n}{…}`.
"""
from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from typing import Any

Json = dict[str, Any]

# Key fields a tag attribute names directly (the rest are shot parameters).
STRING_FIELDS = ('callout', 'code', 'panel', 'demo', 'ease', 'image', 'imageTitle', 'imageCorner', 'imageFrom', 'imageClick', 'arrow', 'arrowColor', 'arrows', 'label', 'labelText', 'labelColor')
FORMULA_ATTRS = ('formula', 'say', 'card', 'display', 'colors')
# Attribute names a tag reserves; a shot parameter of one of these names is written `param.<name>`.
RESERVED = (*STRING_FIELDS, *FORMULA_ATTRS, 'highlight', 'record', 'blend', 'term', 'delay', 'animate', 'pause', 'camera', 'param', 'run', 'stop', 'imageHide', 'arrowFrom', 'arrowTo', 'arrowWidth', 'arrowHide', 'labelAt', 'labelHide')


class MarkupError(RuntimeError):
    pass


@dataclass
class Tag:
    """A tag of a sentence: what kind it is, its name (cues, ranges), its
    attributes, and the index of the spoken piece it stands before."""
    kind: str                  # 'cue', 'open', 'close', or 'formula'
    name: str | None
    attrs: list[tuple[str, str]]
    piece: int

    def attr(self, name: str) -> str | None:
        return next((v for k, v in self.attrs if k == name), None)


@dataclass
class Sentence:
    caption: str               # tags removed but formulas, as `{@ formula="…"}`
    spoken: str                # what the voice says
    pieces: list[str]          # the spoken text cut at each tag
    tags: list[Tag] = field(default_factory=list)
    formulas: list[str] = field(default_factory=list)


def parse_tag(text: str, start: int) -> tuple[list[tuple[str, str | None]], int]:
    """The attributes of the tag whose body starts at `start` (after `{@`),
    and the index past its closing brace (as `parse_tag` in rich_text.rs). A
    bare word (a cue's name) has the value None; `name=""` has ''."""
    i = start
    attrs: list[tuple[str, str | None]] = []
    while True:
        while i < len(text) and text[i].isspace():
            i += 1
        if i >= len(text):
            raise MarkupError(f'unclosed tag: {text[max(0, start - 2):]!r}')
        if text[i] == '}':
            return attrs, i + 1
        name_start = i
        while i < len(text) and not text[i].isspace() and text[i] not in '=}':
            i += 1
        name = text[name_start:i]
        if i >= len(text) or text[i] != '=':
            attrs.append((name, None))
            continue
        i += 1
        if i < len(text) and text[i] == '"':
            i += 1
            value = []
            while True:
                if i >= len(text):
                    raise MarkupError(f'unclosed quote in tag: {text[max(0, start - 2):]!r}')
                if text[i] == '"':
                    i += 1
                    break
                if text[i] == '\\' and i + 1 < len(text) and text[i + 1] == '"':
                    value.append('"')
                    i += 2
                    continue
                value.append(text[i])
                i += 1
            attrs.append((name, ''.join(value)))
        elif i < len(text) and text[i] in '[{':
            # A JSON value (motions): up to its matching bracket, past strings.
            value_start, depth, quoted = i, 0, False
            while i < len(text):
                c = text[i]
                if quoted:
                    if c == '\\':
                        i += 1
                    elif c == '"':
                        quoted = False
                elif c == '"':
                    quoted = True
                elif c in '[{':
                    depth += 1
                elif c in ']}':
                    depth -= 1
                    if depth == 0:
                        i += 1
                        break
                i += 1
            if depth:
                raise MarkupError(f'unclosed JSON value in tag: {text[max(0, start - 2):]!r}')
            attrs.append((name, text[value_start:i]))
        else:
            value_start = i
            while i < len(text) and not text[i].isspace() and text[i] != '}':
                i += 1
            attrs.append((name, text[value_start:i]))


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


def parse(text: str, spoken_only: bool = False) -> Sentence:
    """A sentence's caption, spoken text, pieces, tags, and formulas.
    `spoken_only` (a translation) allows formulas and nothing else."""
    caption: list[str] = []
    pieces: list[str] = ['']
    tags: list[Tag] = []
    formulas: list[str] = []
    i = 0
    while i < len(text):
        if text.startswith('\\{', i):
            caption.append('{')
            pieces[-1] += '{'
            i += 2
            continue
        if text.startswith('{<', i):
            end = text.find('}', i)
            if end < 0:
                raise MarkupError(f'unclosed range end in {text!r}')
            if spoken_only:
                raise MarkupError(f'a translation carries no cues: {text!r}')
            tags.append(Tag('close', text[i + 2:end].strip(), [], len(pieces)))
            pieces.append('')
            i = end + 1
            continue
        if text.startswith('{@', i):
            attrs, end = parse_tag(text, i + 2)
            if attrs and attrs[0][0] == 'codespan':
                tag = Tag('code', None, attrs, len(pieces))
                code = tag.attr('codespan') or ''
                if not code or any(k not in ('codespan', 'say') for k, _ in attrs):
                    raise MarkupError(f'a code span is {{@ codespan="name"}}, with an optional say="…": {text[i:end]!r}')
                caption.append(codespan_tag(code))
                # It is said as written (or as its say="…"), in the words around it.
                pieces[-1] += tag.attr('say') or code
                i = end
                continue
            if any(k == 'formula' for k, _ in attrs):  # (a parameter named formula is `param.formula`)
                tag = Tag('formula', None, attrs, len(pieces))
                latex = tag.attr('formula') or ''
                say = tag.attr('say')
                if latex == 'off':
                    if spoken_only:
                        raise MarkupError(f'a translation carries no cues: {text!r}')
                    tags.append(tag)
                    pieces.append('')
                elif spoken_only:
                    # A translation's formula is shown, never spoken.
                    if say:
                        raise MarkupError(f'a translation\'s formula carries no say="…": {latex!r}')
                    formulas.append(latex)
                    caption.append(formula_tag(tag))
                else:
                    if not say:
                        raise MarkupError(f'the formula {latex!r} has no say="…": the voice cannot read LaTeX')
                    formulas.append(latex)
                    caption.append(formula_tag(tag))
                    tags.append(tag)
                    # The tag stands before the first word of what is said for it.
                    pieces.append(f' {say} ')
                    pieces.append('')
                i = end
                continue
            if spoken_only:
                raise MarkupError(f'a translation carries no cues: {text!r}')
            name = None
            if attrs and attrs[0][1] is None and attrs[0][0].endswith('>'):
                name = attrs[0][0][:-1]
                tags.append(Tag('open', name, attrs[1:], len(pieces)))
            elif len(attrs) == 1 and attrs[0][1] is None:
                tags.append(Tag('cue', attrs[0][0], [], len(pieces)))
            else:
                if any(v is None for _, v in attrs):
                    raise MarkupError(f'a cue\'s attributes are name=value: {text[i:end]!r}')
                tags.append(Tag('cue', None, attrs, len(pieces)))
            pieces.append('')
            i = end
            continue
        caption.append(text[i])
        pieces[-1] += text[i]
        i += 1
    opened = [t.name for t in tags if t.kind == 'open']
    closed = [t.name for t in tags if t.kind == 'close']
    for name in opened:
        if closed.count(name) != 1:
            raise MarkupError(f'the range {name!r} must close once in its sentence: {text!r}')
    for name in closed:
        if name not in opened:
            raise MarkupError(f'{{<{name}}} closes no range: {text!r}')
    spoken = tidy(' '.join(pieces))
    return Sentence(tidy(''.join(caption)), spoken, [tidy(p) for p in pieces], tags, formulas)


def has_tags(text: str) -> bool:
    return '{@' in text or '{<' in text


def number(value: str, where: str) -> float:
    try:
        return float(value)
    except ValueError:
        raise MarkupError(f'{where}: {value!r} is not a number') from None


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
        return float(value)
    except ValueError:
        pass
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
    parameter of an image kind's instance, `imageHide=a,b` hides images. False
    for any other attribute."""
    if name.startswith('image.'):
        key.setdefault('imageParams', {})[name[6:]] = param_value(value, where)
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


def cue_key(tag: Tag, cues: Json, where: str) -> Json:
    """The shot key a point cue or a range's start sets, before timing:
    fields as the storyboard writes them (`blend` in seconds)."""
    if tag.kind in ('cue', 'open') and tag.name and not tag.attrs:
        if tag.name not in cues:
            raise MarkupError(f'{where}: no cue named {tag.name!r} in the sentence\'s or the beat\'s cues')
        return dict(cues[tag.name])
    key: Json = {}
    params: Json = {}
    for name, value in tag.attrs:
        if name.startswith('param.'):
            params[name[6:]] = value
        elif name == 'camera' and value == 'continuous':
            key.setdefault('camera', {})
        elif name.startswith('camera.'):
            part = name.split('.', 1)[1]
            if part not in ('target', 'yaw', 'pitch', 'distance', 'fov'):
                raise MarkupError(f'{where}: unknown camera field {part!r}')
            key.setdefault('camera', {})[part] = [number(x, where) for x in value.split(',')] if part == 'target' else number(value, where)
        elif name == 'animate':
            try:
                key['animate'] = json.loads(value)
            except ValueError as e:
                raise MarkupError(f'{where}: animate is JSON: {e}') from None
        elif name == 'delay':
            key['delay'] = number(value.removesuffix('s'), where)
        elif action_attr(key, name, value, where) or image_attr(key, name, value, where):
            continue
        elif name in STRING_FIELDS:
            key[name] = value
        elif name == 'highlight':
            key['highlight'] = [v for v in value.split(',') if v]
        elif name == 'record':
            kind, _, index = value.partition(':')
            key['record'] = {'type': kind} if not index else {'type': kind, 'index': int(number(index, where))}
        elif name == 'blend':
            key['blend'] = number(value.removesuffix('s'), where)
        elif name == 'term':
            key['term'] = int(number(value, where))
        elif name == 'formula':
            key['formula'] = '' if value == 'off' else formula_tag(tag)
        elif name in ('say', 'card', 'display', 'colors'):
            continue
        else:
            params[name] = value
    if tag.kind == 'formula' and tag.attr('card') == 'no':
        key.pop('formula', None)
    check_action(key, where)
    if params:
        key['params'] = params
    return key


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


def text_keys(beat: Json, sentence: Json) -> list[Json]:
    """The keys a sentence's tags set, untimed: for checks that look at every
    key (callouts, listings)."""
    parsed = parse(sentence.get('en', sentence['text']))
    cues = {**beat.get('cues', {}), **sentence.get('cues', {})}
    return [cue_key(t, cues, beat['id']) for t in parsed.tags if t.kind != 'close']
