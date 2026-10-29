"""The storyboard file type, `.story`: a video's chapters, beats, and
narration as text, with its shots as tags (markup.py), and its translation
in a parallel file (`storyboard.zh-CN.story`). LaTeX is written as is.

    title: The opaque frame of O3DE 2.8
    fps: 30

    # 11 · Other trace paths · 150 s
    summary: …
    sources: key-a, key-b

    ## beat-id · scene · The headline
    sources: key-c
    {@ demo=terrain camera.target=0,1.2,0.5 camera.yaw=32 hfBounds=1}
    {@define wide camera.distance=25 blend=3}

    The first sentence, {@ opacity=1}with a cue on a word.

    {@wide}The second sentence, {@ formula="\\frac{1}{1+n}" say="one over one plus n"}. {@ pause=0.5}

- `key: value` lines: the storyboard's header (before the first chapter), a
  chapter's `summary` and `sources`, a beat's `diagram`, `focus`, `sources`,
  and `hold`. A value that reads as JSON (a number, a list) is that value.
- `# number · title · N s` starts a chapter; `## id · kind · headline` a beat.
- Before a beat's first sentence, a line of tags only sets the beat's keys
  (one key per tag); `{@define name …}` defines a named cue.
- Each paragraph (lines up to a blank line) is one sentence. A tag before its
  first word fires at the sentence's start; `{@ pause=s}` adds silence after it.
- A tag's attributes: shot parameters (`hfBounds=1`), `demo`, `camera.target=x,y,z`
  and `camera.yaw` (`pitch`, `distance`, `fov`; `camera=continuous` keeps the
  camera running through a `demo` switch), `record=probe:67` (or `ray`),
  `highlight=a,b`, `panel`, `code`, `callout`, `blend`, `ease`, `delay` (seconds),
  `formula`, `term`, `animate=[…]` (motions, as JSON), `run=action` with
  `run.<arg>=value` (an action the package declares) and `stop=a,b`. A parameter whose
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

KEY_ORDER = ('demo', 'camera', 'record', 'panel', 'code', 'highlight', 'callout', 'params', 'formula', 'term', 'animate', 'stop', 'run', 'blend', 'ease', 'delay')
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


def number_text(x: float) -> str:
    return str(int(x)) if float(x).is_integer() else repr(float(x))


def param_text(x: float | bool | str | list[float]) -> str:
    """A parameter's value as a tag writes it (markup.param_value reads it back)."""
    if isinstance(x, bool):
        return 'true' if x else 'false'
    if isinstance(x, str):
        return x
    if isinstance(x, list):
        return ','.join(number_text(n) for n in x)
    return number_text(x)


def attr_text(name: str, value: str) -> str:
    if value[:1] in ('[', '{'):
        return f'{name}={value}'
    if value == '' or re.search(r'[\s{}"]', value):
        return f'{name}={markup.quote(value)}'
    return f'{name}={value}'


def key_attrs(key: Json) -> list[tuple[str, str]]:
    """A storyboard key as tag attributes."""
    unknown = set(key) - set(KEY_ORDER)
    if unknown:
        raise StoryError(f'cannot write key fields {sorted(unknown)}')
    out: list[tuple[str, str]] = []
    for field in KEY_ORDER:
        if field not in key:
            continue
        v = key[field]
        if field == 'camera':
            # An empty camera keeps the camera continuous across a demonstration switch.
            if not v:
                out.append(('camera', 'continuous'))
            for part in CAMERA:
                if part in v:
                    out.append((f'camera.{part}', ','.join(number_text(x) for x in v[part]) if part == 'target' else number_text(v[part])))
        elif field == 'record':
            out.append(('record', v['type'] + (f':{v["index"]}' if 'index' in v else '')))
        elif field == 'highlight':
            out.append(('highlight', ','.join(v)))
        elif field == 'params':
            out.extend((f'param.{name}' if name in markup.RESERVED else name, param_text(x)) for name, x in v.items())
        elif field == 'animate':
            out.append(('animate', json.dumps(v, ensure_ascii=False)))
        elif field == 'stop':
            out.append(('stop', ','.join(v)))
        elif field == 'run':
            out.append(('run', v['name']))
            out.extend((f'run.{name}', param_text(x)) for name, x in v.get('args', {}).items())
        elif field in ('blend', 'delay', 'term'):
            out.append((field, number_text(v)))
        else:
            out.append((field, str(v)))
    return out


def tag(attrs: list[tuple[str, str]], head: str = '') -> str:
    parts = ([head] if head else []) + [attr_text(n, v) for n, v in attrs]
    return '{@ ' + ' '.join(parts) + '}' if not head else '{@' + ' '.join(parts) + '}'


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
        elif name in ('demo', 'panel', 'code', 'callout', 'ease', 'formula'):
            key[name] = v
        elif markup.action_attr(key, name, v, where):
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


def heading(text: str, parts: int, path: Path, what: str) -> list[str]:
    fields = [f.strip() for f in text.split(' · ', parts - 1)]
    if len(fields) != parts:
        raise StoryError(f'{path}: a {what} heading is {" · ".join(["…"] * parts)}: {text!r}')
    return fields


def as_list(value: Any) -> list[str]:
    return value if isinstance(value, list) else [v.strip() for v in str(value).split(',') if v.strip()]


def load(path: Path) -> Json:
    """A `.story` file as the storyboard the builder reads."""
    header, chapters = split_sections(path.read_text(), path)
    board, _ = properties(header)
    board['chapters'] = []
    for chapter_heading, chapter_lines, beats in chapters:
        number, rest_heading = heading(chapter_heading, 2, path, 'chapter')
        title, seconds = [f.strip() for f in rest_heading.rsplit(' · ', 1)]
        props, _ = properties(chapter_lines, ('summary', 'sources'))
        chapter: Json = {'number': int(number), 'title': title, 'targetSeconds': markup.number(seconds.removesuffix(' s'), str(path)),
                         'summary': props.get('summary', ''), 'sources': as_list(props.get('sources', [])), 'beats': []}
        for beat_heading, beat_lines in beats:
            beat_id, kind, headline = heading(beat_heading, 3, path, 'beat')
            props, rest = properties(beat_lines, ('diagram', 'focus', 'sources', 'hold'))
            beat: Json = {'id': beat_id, 'kind': kind, 'headline': headline}
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
    return json.loads(path.read_text())


# ---------------------------------------------------------------------------
# Writing (converting a JSON storyboard)

def header_line(name: str, value: Any) -> str:
    return f'{name}: {value if isinstance(value, str) else json.dumps(value, ensure_ascii=False)}'


def sentence_text(s: Json, beat: Json) -> str:
    """A storyboard sentence as text: its keys as tags at the start (with
    their delays), its `sequence` spread over its words, its pause at the end."""
    if isinstance(s, str):
        return s
    text = s['text']
    lead = ' '.join(tag(key_attrs(k)) for k in s.get('keys', []))
    sequence = s.get('sequence', [])
    if sequence:
        words = text.split(' ')
        at = [round(i * len(words) / len(sequence)) for i in range(len(sequence))]
        for i in reversed(range(len(sequence))):
            if at[i] == 0:
                lead = (lead + ' ' + tag(key_attrs(sequence[i]))).strip()
            else:
                words.insert(at[i], tag(key_attrs(sequence[i])))
        text = ' '.join(words)
    out = (lead + ' ' + text).strip() if lead else text
    if 'pause' in s:
        out += f' {{@ pause={number_text(s["pause"])}}}'
    return out


def dump(board: Json) -> str:
    lines = [header_line(k, v) for k, v in board.items() if k != 'chapters'] + ['']
    for chapter in board['chapters']:
        lines += [f'# {chapter["number"]} · {chapter["title"]} · {number_text(chapter["targetSeconds"])} s',
                  header_line('summary', chapter['summary']), 'sources: ' + ', '.join(chapter['sources']), '']
        for beat in chapter['beats']:
            lines.append(f'## {beat["id"]} · {beat["kind"]} · {beat["headline"]}')
            for name in ('diagram', 'hold'):
                if name in beat:
                    lines.append(header_line(name, beat[name]))
            for name in ('focus', 'sources'):
                if name in beat:
                    lines.append(f'{name}: ' + ', '.join(beat[name]))
            cues = dict(beat.get('cues', {}))
            for s in beat['narration']:
                if isinstance(s, dict):
                    cues.update(s.get('cues', {}))
            for k in beat.get('keys', []):
                lines.append(tag(key_attrs(k)))
            for name, k in cues.items():
                lines.append('{@define ' + name + (' ' + ' '.join(attr_text(n, v) for n, v in key_attrs(k)) if k else '') + '}')
            lines.append('')
            for s in beat['narration']:
                lines += [sentence_text(s, beat), '']
    return '\n'.join(lines).rstrip() + '\n'


def dump_translation(translation: Json, board: Json) -> str:
    lines = [header_line(k, v) for k, v in translation.items() if k not in ('chapters', 'beats')] + ['']
    chapters = {c['number']: c for c in translation['chapters']}
    for chapter in board['chapters']:
        c = chapters[chapter['number']]
        lines += [f'# {chapter["number"]} · {c["title"]}', header_line('summary', c['summary']), '']
        for beat in chapter['beats']:
            t = translation['beats'][beat['id']]
            lines += [f'## {beat["id"]} · {t["headline"]}', '']
            for s in t['narration']:
                lines += [s, '']
    return '\n'.join(lines).rstrip() + '\n'
