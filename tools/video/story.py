"""The storyboard file type, `.story`: a video's chapters, beats, and
narration as text, with its shots as tags (markup.py), and its translation
in a parallel file (`storyboard.zh-CN.story`). LaTeX is written as is.

    title: The opaque frame of O3DE 26.05
    fps: 30

    # 11 | Other trace paths | 150 s
    summary: …
    sources: key-a, key-b

    ## beat-id | 3d | The headline
    sources: key-c
    {@ demo=terrain camera.target=0,1.2,0.5 camera.yaw=32 hfBounds=1}
    {@define wide camera.distance=25 blend=3}

    The first sentence, {@ opacity=1}with a cue on a word.

    {@wide}The second sentence, {@ formula="\\frac{1}{1+n}" say="one over one plus n"}. {@ pause=0.5}

- `key: value` lines: the storyboard's header (before the first chapter), a
  chapter's `summary` and `sources`, a beat's `diagram`, `focus`, `sources`,
  and `hold`. A value that reads as JSON (a number, a list) is that value.
- `# number | title | N s` starts a chapter; `## id | type | headline` a beat,
  its scene's type `3d` (the lab's 3D frame) or `2d` (a page: the image a
  `diagram:` names, else the sources card of its `sources:`).
- Before a beat's first sentence, a line of tags only sets the beat's keys
  (one key per tag); `{@define name …}` defines a named cue.
- Each paragraph (lines up to a blank line) is one sentence. A tag before its
  first word fires at the sentence's start; `{@ pause=s}` adds silence after it.
- A tag's attributes: shot parameters (`hfBounds=1`), `demo`, `camera.target=x,y,z`
  and `camera.yaw` (`pitch`, `distance`, `fov`; `camera=continuous` keeps the
  camera running through a `demo` switch), `record=object:3` (or `camera`),
  `highlight=a,b`, `panel`, `code`, `callout`, `blend`, `ease`, `delay` (seconds),
  `formula`, `term`, `animate=[…]` (motions, as JSON), `run=action` with
  `run.<arg>=value` (an action the package declares) and `stop=a,b`, `image=name`
  (`imageTitle`, `imageCorner`, `imageFrom=kind` with `image.<param>=value`;
  `imageHide=a,b`, and `image=""` hides all). A parameter whose
  name a tag reserves (`formula`, `term`, …) is written `param.formula=1`.

The translation file has the same headings (its own titles and headlines) and
one paragraph per English sentence, with formulas but no cues.
"""
from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

import markup

Json = dict[str, Any]

CAMERA = ('target', 'yaw', 'pitch', 'distance', 'fov')


class StoryError(RuntimeError):
    pass


# ---------------------------------------------------------------------------
# Values

def value_of(text: str) -> Any:
    """A header or property value: JSON when it reads as JSON, else the text."""
    text = text.strip()
    try:
        return json.loads(text)
    except ValueError:
        return text


def attrs_key(attrs: list[tuple[str, str]], where: str) -> Json:
    """Tag attributes as a storyboard key, every value typed."""
    key: Json = {}
    params: Json = {}
    for name, v in attrs:
        if name.startswith('param.'):
            params[name[6:]] = markup.param_value(v, where)
        elif name == 'camera' and v == 'continuous':
            key.setdefault('camera', {})
        elif name.startswith('camera.'):
            part = name.split('.', 1)[1]
            if part not in CAMERA:
                raise StoryError(f'{where}: unknown camera field {part!r}')
            key.setdefault('camera', {})[part] = [markup.number(x, where) for x in v.split(',')] if part == 'target' else markup.number(v, where)
        elif name == 'record':
            kind, _, index = v.partition(':')
            key['record'] = {'type': kind} if not index else {'type': kind, 'index': int(markup.number(index, where))}
        elif name == 'highlight':
            key['highlight'] = [h for h in v.split(',') if h]
        elif name == 'animate':
            try:
                key['animate'] = json.loads(v)
            except ValueError as e:
                raise StoryError(f'{where}: animate is JSON: {e}') from None
        elif name in ('blend', 'delay'):
            key[name] = markup.number(v.removesuffix('s'), where)
        elif name == 'term':
            key['term'] = int(markup.number(v, where))
        elif name in ('demo', 'panel', 'code', 'callout', 'ease', 'formula', 'image', 'imageTitle', 'imageCorner', 'imageFrom', 'imageClick', 'imageMarkText', 'arrow', 'arrowColor', 'arrows', 'label', 'labelText', 'labelColor'):
            key[name] = v
        elif markup.action_attr(key, name, v, where) or markup.image_attr(key, name, v, where):
            continue
        else:
            params[name] = markup.param_value(v, where)
    markup.check_action(key, where)
    if params:
        key['params'] = params
    return key


# ---------------------------------------------------------------------------
# Reading

def paragraphs(lines: list[str]) -> list[list[str]]:
    out: list[list[str]] = []
    current: list[str] = []
    for line in lines:
        if line.strip():
            current.append(line.strip())
        elif current:
            out.append(current)
            current = []
    if current:
        out.append(current)
    return out


def tags_only(text: str) -> list[tuple[str, list[tuple[str, str]]]] | None:
    """The tags of a line made of tags alone (head word and attributes), else None."""
    out = []
    i = 0
    while i < len(text):
        if text[i].isspace():
            i += 1
            continue
        if not text.startswith('{@', i):
            return None
        head = ''
        j = i + 2
        if j < len(text) and not text[j].isspace():
            m = re.match(r'[\w-]+', text[j:])
            head = m.group(0) if m else ''
            j += len(head)
        attrs, i = markup.parse_tag(text, j)
        out.append((head, attrs))
    return out


def split_sections(text: str, path: Path) -> tuple[list[str], list[tuple[str, list[str], list[tuple[str, list[str]]]]]]:
    """The header lines, and each chapter: its heading, its lines, and its beats (heading, lines)."""
    header: list[str] = []
    chapters: list[tuple[str, list[str], list[tuple[str, list[str]]]]] = []
    for number, line in enumerate(text.split('\n'), 1):
        if line.startswith('## '):
            if not chapters:
                raise StoryError(f'{path}:{number}: a beat before any chapter')
            chapters[-1][2].append((line[3:], []))
        elif line.startswith('# '):
            chapters.append((line[2:], [], []))
        elif not chapters:
            header.append(line)
        elif chapters[-1][2]:
            chapters[-1][2][-1][1].append(line)
        else:
            chapters[-1][1].append(line)
    return header, chapters


def properties(lines: list[str], names: tuple[str, ...] | None = None) -> tuple[Json, list[str]]:
    """The `key: value` lines at the top of a section (only `names`, when
    given, so a sentence is never read as one), and the lines after them."""
    props: Json = {}
    rest = list(lines)
    while rest and (not rest[0].strip() or ((m := re.match(r'^([A-Za-z]\w*): ', rest[0])) and (names is None or m.group(1) in names))):
        line = rest.pop(0)
        if line.strip():
            name, value = line.split(': ', 1)
            props[name] = value_of(value)
    return props, rest


# A heading's fields.
SEPARATOR = re.compile(r' \| ')

def check_scene_type(kind: str, where: str) -> None:
    """A beat's scene type is 3d or 2d."""
    if kind not in ('3d', '2d'):
        raise StoryError(f'{where}: the scene type is 3d or 2d, not {kind!r}')


def heading(text: str, parts: int, path: Path, what: str) -> list[str]:
    fields = [f.strip() for f in SEPARATOR.split(text, parts - 1)]
    if len(fields) != parts:
        raise StoryError(f'{path}: a {what} heading is {" | ".join(["…"] * parts)}: {text!r}')
    return fields


def is_3d(beat: Json) -> bool:
    """A beat whose scene is the lab's 3D frame."""
    return beat['kind'] == '3d'


def is_diagram(beat: Json) -> bool:
    """A 2D beat that shows an image: its diagram."""
    return beat['kind'] == '2d' and 'diagram' in beat


def is_card(beat: Json) -> bool:
    """A 2D beat that shows its sources card."""
    return beat['kind'] == '2d' and 'diagram' not in beat


def as_list(value: Any) -> list[str]:
    return value if isinstance(value, list) else [v.strip() for v in str(value).split(',') if v.strip()]


def load(path: Path) -> Json:
    """A `.story` file as the storyboard the builder reads."""
    header, chapters = split_sections(path.read_text(), path)
    board, _ = properties(header)
    board['chapters'] = []
    for chapter_heading, chapter_lines, beats in chapters:
        number, rest_heading = heading(chapter_heading, 2, path, 'chapter')
        *title, seconds = [f.strip() for f in SEPARATOR.split(rest_heading)]
        title = ' | '.join(title)
        props, _ = properties(chapter_lines, ('summary', 'sources'))
        chapter: Json = {'number': int(number), 'title': title, 'targetSeconds': markup.number(seconds.removesuffix(' s'), str(path)),
                         'summary': props.get('summary', ''), 'sources': as_list(props.get('sources', [])), 'beats': []}
        for beat_heading, beat_lines in beats:
            beat_id, kind, headline = heading(beat_heading, 3, path, 'beat')
            check_scene_type(kind, f'{path}: {beat_id}')
            props, rest = properties(beat_lines, ('diagram', 'focus', 'sources', 'hold'))
            beat: Json = {'id': beat_id, 'kind': kind, 'headline': headline}
            if beat['kind'] == '2d' and 'diagram' not in props and 'sources' not in props:
                raise StoryError(f'{path}: {beat_id}: a 2d beat shows a diagram (diagram:) or a sources card (sources:)')
            for name in ('diagram', 'hold'):
                if name in props:
                    beat[name] = props[name]
            for name in ('focus', 'sources'):
                if name in props:
                    beat[name] = as_list(props[name])
            keys: list[Json] = []
            cues: Json = {}
            narration: list[Json] = []
            for paragraph in paragraphs(rest):
                text = ' '.join(paragraph)
                where = f'{path}: {beat_id}'
                only = tags_only(text)
                if only is not None and not narration:
                    for head, attrs in only:
                        if head == 'define':
                            name = attrs.pop(0)[0]
                            if any(v is None for _, v in attrs):
                                raise StoryError(f'{where}: {{@define {name} …}} takes name=value attributes')
                            cues[name] = attrs_key(attrs, where)
                        elif head:
                            raise StoryError(f'{where}: unknown beat tag {{@{head}}}')
                        else:
                            keys.append(attrs_key(attrs, where))
                    continue
                sentence: Json = {'text': text}
                m = re.search(r'\s*\{@ pause=([\d.]+)\}\s*$', text)
                if m:
                    sentence = {'text': text[:m.start()], 'pause': float(m.group(1))}
                narration.append(sentence)
            if keys:
                beat['keys'] = keys
            if cues:
                beat['cues'] = cues
            beat['narration'] = narration
            chapter['beats'].append(beat)
        board['chapters'].append(chapter)
    return board


def load_translation(path: Path) -> Json:
    """A translation `.story` file as the translation the builder reads."""
    header, chapters = split_sections(path.read_text(), path)
    translation, _ = properties(header)
    translation['chapters'] = []
    translation['beats'] = {}
    for chapter_heading, chapter_lines, beats in chapters:
        number, title = heading(chapter_heading, 2, path, 'chapter')
        props, _ = properties(chapter_lines, ('summary',))
        translation['chapters'].append({'number': int(number), 'title': title, 'summary': props.get('summary', '')})
        for beat_heading, beat_lines in beats:
            beat_id, headline = heading(beat_heading, 2, path, 'beat')
            _, rest = properties(beat_lines, ())
            translation['beats'][beat_id] = {'headline': headline, 'narration': [' '.join(p) for p in paragraphs(rest)]}
    return translation


def load_any(path: Path, translation: bool = False) -> Json:
    """A storyboard or translation from `.story` or (older) `.json`."""
    if path.suffix == '.story':
        return load_translation(path) if translation else load(path)
    board = json.loads(path.read_text())
    for chapter in [] if translation else board.get('chapters', []):
        for beat in chapter['beats']:
            check_scene_type(beat['kind'], f'{path}: {beat["id"]}')
    return board
