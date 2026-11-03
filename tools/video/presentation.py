"""The storyboard file type, a Lamlet presentation (`storyboard.lam`): a video's chapters, beats, and narration
as a document, with its shots as typed cues, and its translation in a parallel presentation
(`storyboard.zh-CN.lam`). The lab reads it with the Lamlet it pins (`render-lab lamlet ir`, lamlet.py), and this
loader turns the document into the storyboard the builder reads.

    $[Import* std]$[Import* content]$[Import* present]$[Import* render_lab]$[Import* godot_voxelgi]
    # How VoxelGI Lights a Room {#video subtitle="..." fps=30 width=1920 height=1080}

    ## Where it runs {#c1 number=1 target=185 summary="..." sources="frame-order, settings"}

    ### The headline {#c1-room kind=3d sources="frame-order"}

    ${cue "key" {' {demoRoom} {cameraYaw 24.0} {cameraDistance 6.2}}}

    The first sentence, ${cue "key" {' {callout "settings"}}}with a cue on a word, and
    ${over {' {voxelgiMixRange 0.0 1.0} {blend 2.0}} {text "a range over words"}}.

- The level-1 heading is the video: its text is the title, its attributes the rest of the header (`subtitle`,
  `fps`, `width`, ...; a value that reads as JSON is that value). Level 2 is a chapter (`number`, `target` in
  seconds, `summary`, `sources`), level 3 a beat: its id, and `kind` (`3d` or `2d`), `diagram`, `graph`, `focus`,
  `sources`, and `hold`.
- Before a beat's first sentence, a paragraph of cues only sets the beat's keys, one key per cue.
- Each other paragraph is one sentence. Its prose is CommonMark; `$` is written `$$`.
- Cues are `present`'s: `${cue "key" ATTRS}` at a point (it fires on the next word), `${during NAME ATTRS
  CONTENT}` (or `duringText`, or render_lab's `over` and `overText` without a name, which this loader gives one)
  over words, a range whose `xRange a b` values blend from `a` to `b`. Attributes are the investigation's vocabulary
  (`render-lab story keys`; the builder writes it to the package's `build/<package>.lam`): `{cameraYaw 24.0}`,
  `{panelCode}`, `{listing "voxelize"}` (`code=`), `{inset "t0"}` (`image=`), `{easing "inOut"}` (`ease=`), and
  `{attr "key" "value"}` for any other; and render_lab's typed ones: `{motion {' {cameraPath …}}}` (`animate=`),
  `{mark …}`, `{at …}`, `{highlight {' …}}`, and the checked `callout`, `imageTitle`, `imageMarkText`, `labelText`.
- A formula is a cue and what is said for it: `${formulaSaid "R + TT" "R plus T T" {'}}` (render_lab's, the cue
  `${cue "formula" {' {formula "R + TT"}}}` and `${say {math "R + TT"} "R plus T T"}`); `${cue "formula" {'
  {formulaOff}}}` closes the card. A code name is `${code "Name"}` (said as
  written) or `${say {code "Name"} "how it is said"}`.

The translation has the same headings, by id (its own title, chapter titles and summaries, and headlines; the
video's `subtitle`, `locale`, `glossary`, comma-separated, and `captionWidth`; a chapter's number is the
English's), and one paragraph per English sentence, with no cues: its formulas are `${math "..."}`, its code
names `${code "..."}`.
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import lamlet
import markup

Json = dict[str, Any]

# A point cue's name (the rest are named ranges), and the cue that carries a formula's attributes.
KEY, FORMULA = 'key', 'formula'


class PresentationError(RuntimeError):
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


def as_list(value: Any) -> list[str]:
    return value if isinstance(value, list) else [v.strip() for v in str(value).split(',') if v.strip()]


def point_key(tag: markup.Tag, where: str, fields: markup.Fields) -> Json:
    """The key a point cue (or one of a beat's keys) sets: its values set at once, none blended over a range."""
    key = markup.cue_key(tag, where, fields)
    for name, v in key.get('params', {}).items():
        if isinstance(v, markup.Range):
            raise PresentationError(f'{where}: {name}={v.start}..{v.end} blends over a range; write it in a range, ${{during "name" …}}')
    return key


# ---------------------------------------------------------------------------
# Beats

def check_scene_type(kind: str, where: str) -> None:
    """A beat's scene type is 3d or 2d."""
    if kind not in ('3d', '2d'):
        raise PresentationError(f'{where}: the scene type is 3d or 2d, not {kind!r}')


def is_3d(beat: Json) -> bool:
    """A beat whose scene is the lab's 3D frame."""
    return beat['kind'] == '3d'


def is_diagram(beat: Json) -> bool:
    """A 2D beat that shows an image: its diagram, or the frame graph (`graph:`)."""
    return beat['kind'] == '2d' and ('diagram' in beat or 'graph' in beat)


def is_card(beat: Json) -> bool:
    """A 2D beat that shows its sources card."""
    return beat['kind'] == '2d' and not is_diagram(beat)


# ---------------------------------------------------------------------------
# The document

def where(node: Json, path: Path) -> str:
    at = node.get('at')
    if at is None:
        for child in node.get('inlines', []) + node.get('shown', []):
            if (w := where(child, path)) != str(path):
                return w
        return str(path)
    return f'{at["file"]}:{at["line"]}'


def pairs(attrs: list[list[str]]) -> list[tuple[str, str]]:
    return [(k, v) for k, v in attrs]


def plain(inlines: list[Json], at: str) -> str:
    """A heading's text."""
    out = []
    for x in inlines:
        if x['type'] in ('text', 'code'):
            out.append(x['text'])
        else:
            raise PresentationError(f'{at}: a heading holds text, not {x["type"]}')
    return ''.join(out)


def cues_only(inlines: list[Json]) -> bool:
    """A paragraph of point cues alone (and the space between them)."""
    return all(x['type'] == 'anchor' or (x['type'] == 'text' and not x['text'].strip()) or x['type'] == 'break' for x in inlines) \
        and any(x['type'] == 'anchor' for x in inlines)


class Reader:
    """A paragraph's inlines as a parsed sentence: its caption, its spoken pieces cut at each cue,
    its tags, and its formulas."""

    def __init__(self, at: str, translation: bool, fields: markup.Fields | None):
        self.at = at
        self.translation = translation
        self.fields = fields
        self.caption: list[str] = []
        self.pieces: list[str] = ['']
        self.tags: list[markup.Tag] = []
        self.formulas: list[str] = []
        self.formula: list[tuple[str, str]] | None = None   # a formula cue's attributes, waiting for its `say`

    def no_cues(self, what: str) -> None:
        if self.translation:
            raise PresentationError(f'{self.at}: a translation carries no cues ({what})')

    def inlines(self, xs: list[Json]) -> None:
        for x in xs:
            kind = x['type']
            if self.formula is not None and kind != 'say':
                raise PresentationError(f'{self.at}: a formula cue stands right before its `say {{math ...}} "..."`')
            if kind == 'text':
                self.caption.append(x['text'])
                self.pieces[-1] += x['text']
            elif kind == 'break':
                self.caption.append(' ')
                self.pieces[-1] += ' '
            elif kind == 'code':
                # A code name, said as written.
                self.caption.append(markup.codespan_tag(x['text']))
                self.pieces[-1] += x['text']
            elif kind == 'math':
                if not self.translation:
                    raise PresentationError(f'{self.at}: the formula {x["tex"]!r} has no say: the voice cannot read LaTeX')
                # A translation's formula is shown, never spoken.
                self.formulas.append(x['tex'])
                self.caption.append(markup.formula_tag(markup.Tag(FORMULA, None, [('formula', x['tex'])], 0)))
            elif kind == 'say':
                self.say(x)
            elif kind == 'anchor':
                self.no_cues(f'the cue {x["name"]!r}')
                attrs = pairs(x['attrs'])
                if x['name'] == FORMULA:
                    if dict(attrs).get('formula') == 'off':
                        self.tag(markup.Tag('formula', None, attrs, len(self.pieces)))
                        self.pieces.append('')
                    else:
                        self.formula = attrs
                elif x['name'] == KEY:
                    self.tag(markup.Tag('cue', None, attrs, len(self.pieces)))
                    self.pieces.append('')
                else:
                    raise PresentationError(f'{self.at}: a point cue is named "{KEY}" (or "{FORMULA}"), not {x["name"]!r}')
            elif kind == 'span':
                self.no_cues(f'the range {x["name"]!r}')
                # A range written without a name (render_lab's `over`) is named by its place.
                name = x['name'] or f'range-{sum(t.kind == "open" for t in self.tags) + 1}'
                if any(t.kind == 'open' and t.name == name for t in self.tags):
                    raise PresentationError(f'{self.at}: two ranges named {name!r} in one sentence')
                self.tag(markup.Tag('open', name, pairs(x['attrs']), len(self.pieces)))
                self.pieces.append('')
                self.inlines(x['inlines'])
                self.tags.append(markup.Tag('close', name, [], len(self.pieces)))
                self.pieces.append('')
            else:
                raise PresentationError(f'{self.at}: a sentence holds text, cues, ranges, formulas, and code names, not {kind}')
        if self.formula is not None:
            raise PresentationError(f'{self.at}: a formula cue stands right before its `say {{math ...}} "..."`')

    def tag(self, tag: markup.Tag) -> None:
        """A cue, a range's start, or a formula, with the key it sets."""
        assert self.fields is not None
        tag.key = markup.cue_key(tag, self.at, self.fields) if tag.kind == 'open' else point_key(tag, self.at, self.fields)
        self.tags.append(tag)

    def say(self, x: Json) -> None:
        shown, spoken = x['shown'], x['spoken']
        if len(shown) == 1 and shown[0]['type'] == 'code':
            # A code name said another way, in the words around it.
            self.caption.append(markup.codespan_tag(shown[0]['text']))
            self.pieces[-1] += spoken
        elif len(shown) == 1 and shown[0]['type'] == 'math':
            if self.translation:
                raise PresentationError(f'{self.at}: a translation\'s formula is shown, never said: write it ${{math "..."}}')
            attrs, self.formula = self.formula, None
            tex = shown[0]['tex']
            if attrs is None or dict(attrs).get('formula') != tex:
                raise PresentationError(f'{self.at}: the formula {tex!r} follows its cue, ${{cue "formula" {{\' {{formula "..."}}}}}}, with the same LaTeX')
            tag = markup.Tag('formula', None, [*attrs, ('say', spoken)], len(self.pieces))
            self.formulas.append(tex)
            self.caption.append(markup.formula_tag(tag))
            self.tag(tag)
            # The tag stands before the first word of what is said for it.
            self.pieces.append(f' {spoken} ')
            self.pieces.append('')
        else:
            raise PresentationError(f'{self.at}: `say` shows a formula (math) or a code name (code)')

    def sentence(self) -> markup.Sentence:
        return markup.Sentence(markup.tidy(''.join(self.caption)), markup.tidy(' '.join(self.pieces)),
                               [markup.tidy(p) for p in self.pieces], self.tags, self.formulas)


def parsed(inlines: list[Json], at: str, fields: markup.Fields | None, translation: bool = False) -> markup.Sentence:
    reader = Reader(at, translation, fields)
    reader.inlines(inlines)
    return reader.sentence()


def sections(doc: Json, path: Path) -> tuple[Json, list[tuple[Json, list[tuple[Json, list[Json]]]]]]:
    """The video's heading, and each chapter: its heading and its beats (heading, paragraphs)."""
    video: Json | None = None
    chapters: list[tuple[Json, list[tuple[Json, list[Json]]]]] = []
    for block in doc['blocks']:
        at = where(block, path)
        if block['type'] == 'heading':
            level = block['level']
            if level == 1:
                if video is not None or chapters:
                    raise PresentationError(f'{at}: one level-1 heading (the video), before the chapters')
                video = block
            elif level == 2:
                chapters.append((block, []))
            elif level == 3:
                if not chapters:
                    raise PresentationError(f'{at}: a beat before any chapter')
                chapters[-1][1].append((block, []))
            else:
                raise PresentationError(f'{at}: headings are the video (#), chapters (##), and beats (###)')
        elif block['type'] == 'paragraph':
            if not chapters or not chapters[-1][1]:
                raise PresentationError(f'{at}: a paragraph before the first beat')
            chapters[-1][1][-1][1].append(block)
        else:
            raise PresentationError(f'{at}: a storyboard holds headings and paragraphs, not {block["type"]}')
    if video is None:
        raise PresentationError(f'{path}: no level-1 heading (the video)')
    return video, chapters


def header(video: Json, path: Path, lists: tuple[str, ...] = ()) -> Json:
    """The video's properties: its title, then its heading's attributes in order."""
    attrs = pairs(video['attrs'])
    out: Json = {}
    if 'title' not in dict(attrs):
        out['title'] = plain(video['inlines'], where(video, path))
    for name, value in attrs:
        out[name] = as_list(value) if name in lists else value_of(value)
    return out


def number(value: str | None, what: str, at: str) -> float:
    if value is None:
        raise PresentationError(f'{at}: a {what} needs it')
    return markup.number(value, at)


def load(path: Path, packages: list[tuple[str, Path]], fields: markup.Fields, investigation: str | None = None) -> Json:
    """A presentation as the storyboard the builder reads, with these packages to import (its vocabulary), the
    fields the vocabulary's key list names (`markup.Fields`), and the investigation the lab checks its names
    against (callouts, text keys, points, formulas)."""
    video, chapters = sections(lamlet.ir(path, packages, investigation), path)
    board = header(video, path)
    board['chapters'] = []
    for chapter_heading, beats in chapters:
        at = where(chapter_heading, path)
        attrs = dict(pairs(chapter_heading['attrs']))
        chapter_id = chapter_heading.get('id') or ''
        if not chapter_id:
            raise PresentationError(f'{at}: a chapter heading names its id, {{#c1 number=1 …}}')
        chapter: Json = {'id': chapter_id, 'number': int(number(attrs.get('number'), 'chapter `number`', at)), 'title': plain(chapter_heading['inlines'], at),
                         'targetSeconds': number(attrs.get('target'), 'chapter `target` (seconds)', at),
                         'summary': value_of(attrs['summary']) if 'summary' in attrs else '', 'sources': as_list(value_of(attrs.get('sources', '[]'))), 'beats': []}
        for beat_heading, paragraphs in beats:
            at = where(beat_heading, path)
            beat_id = beat_heading.get('id') or ''
            if not beat_id:
                raise PresentationError(f'{at}: a beat heading names its id, {{#c1-room kind=3d}}')
            props = dict(pairs(beat_heading['attrs']))
            kind = props.get('kind', '')
            check_scene_type(kind, f'{path}: {beat_id}')
            beat: Json = {'id': beat_id, 'kind': kind, 'headline': plain(beat_heading['inlines'], at)}
            if kind == '2d' and not {'diagram', 'graph', 'sources'} & set(props):
                raise PresentationError(f'{path}: {beat_id}: a 2d beat shows a diagram (diagram=), the frame graph (graph=), or a sources card (sources=)')
            if 'diagram' in props and 'graph' in props:
                raise PresentationError(f'{path}: {beat_id}: a beat shows a diagram or the frame graph, not both')
            unknown = set(props) - {'kind', 'diagram', 'graph', 'focus', 'sources', 'hold'}
            if unknown:
                raise PresentationError(f'{path}: {beat_id}: unknown beat attributes {sorted(unknown)}')
            for name in ('diagram', 'graph', 'hold'):
                if name in props:
                    beat[name] = value_of(props[name])
            for name in ('focus', 'sources'):
                if name in props:
                    beat[name] = as_list(value_of(props[name]))
            keys: list[Json] = []
            narration: list[Json] = []
            for paragraph in paragraphs:
                at = where(paragraph, path)
                inlines = paragraph['inlines']
                if cues_only(inlines) and not narration:
                    for x in inlines:
                        if x['type'] != 'anchor':
                            continue
                        if x['name'] != KEY:
                            raise PresentationError(f'{at}: a beat\'s keys are point cues, ${{cue "{KEY}" ...}}')
                        keys.append(point_key(markup.Tag('cue', None, pairs(x['attrs']), 0), f'{path}: {beat_id}', fields))
                    continue
                narration.append({'text': at, 'parsed': parsed(inlines, at, fields)})
            if keys:
                beat['keys'] = keys
            beat['narration'] = narration
            chapter['beats'].append(beat)
        board['chapters'].append(chapter)
    return board


def load_translation(path: Path) -> Json:
    """A translation presentation as the translation the builder reads (zh.localize)."""
    video, chapters = sections(lamlet.ir(path, []), path)
    translation = header(video, path, lists=('glossary',))
    translation['chapters'] = []
    translation['beats'] = {}
    for chapter_heading, beats in chapters:
        at = where(chapter_heading, path)
        attrs = dict(pairs(chapter_heading['attrs']))
        # A chapter is the English one of the same id; its number is the English's (a `number` given must agree).
        chapter: Json = {'id': chapter_heading.get('id') or '', 'title': plain(chapter_heading['inlines'], at),
                         'summary': value_of(attrs['summary']) if 'summary' in attrs else ''}
        if 'number' in attrs:
            chapter['number'] = int(number(attrs['number'], 'chapter `number`', at))
        translation['chapters'].append(chapter)
        for beat_heading, paragraphs in beats:
            at = where(beat_heading, path)
            narration = [{'text': where(p, path), 'shown': parsed(p['inlines'], where(p, path), None, translation=True)} for p in paragraphs]
            translation['beats'][beat_heading.get('id') or ''] = {'headline': plain(beat_heading['inlines'], at), 'narration': narration}
    return translation
