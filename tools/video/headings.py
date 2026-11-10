"""Story heading contracts, checked on ordered IR before any dictionary conversion.

Which attributes a heading of each level takes, their types, and the defaults of those it leaves out are the
lab's heading keys (`render-lab story headings`, lab/src/script/keys.rs `heading_keys`), which `lamlet ir --keys`
applies before this reads the IR: an unknown attribute, a wrong type, a `kind` or `locale` not among its options
are refused there. What a key's kind cannot say is checked here: an attribute given twice, a key a heading
needs, a number's limits, the even resolution, and a time's frame count."""
from __future__ import annotations

import math
import re
import sys
from pathlib import Path
from typing import Any, NoReturn

import markup
from config import parse_resolution
from locations import DiagnosticError, describe, location

Json = dict[str, Any]
Sections = list[tuple[Json, list[tuple[Json, list[Json]]]]]
# The video heading's times, whose frame count must fit the timeline (all but `taggedWeight`, a ratio).
PACING = frozenset(('lead', 'gap', 'tail', 'maxExtraPause', 'taggedWeight', 'termFade', 'captionBridge'))
# Archival metadata (heading keys of kind `any`, kept as written), copied onto its chapter or beat.
METADATA = frozenset(('engineVersion', 'commit', 'date'))
META_NAME = re.compile(r'meta\.[A-Za-z_][A-Za-z0-9_.-]*\Z')
U32_MAX = 2**32 - 1
U64_MAX = 2**64 - 1


def source(node: Json, path: Path) -> Json:
    return location(node, {'file': str(path)})


def fail(code: str, message: str, node: Json, path: Path, field: str, first: Json | None = None) -> NoReturn:
    related = None
    if first is not None:
        origin = source(first, path)
        related = [{**origin, 'message': 'first declaration'}]
        message += f'; first declared at {describe(origin)}'
    raise DiagnosticError(code, message, source(node, path), beat=node.get('id') if node.get('level') == 3 else None,
                          field=field, related=related)


def attributes(node: Json, path: Path) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for name, value in node['attrs']:
        # An attribute given twice is an error, pointing back at its first declaration.
        if name in out:
            fail('story.heading.duplicate', f'duplicate heading attribute {name!r}', node, path, name, node)
        out[name] = value
    return out


def numeric(attrs: dict[str, Any], name: str, node: Json, path: Path, *, whole: bool = False, minimum: int = 0,
            maximum: int | None = None) -> float | int:
    if name not in attrs:
        fail('story.heading.missing', f'a heading needs {name!r}', node, path, name)
    try:
        result = markup.integer(attrs[name], name, minimum, maximum) if whole else markup.number(attrs[name], name)
    except markup.MarkupError as error:
        fail('story.heading.number', str(error), node, path, name)
    if result < minimum:
        fail('story.heading.number', f'{name} must be at least {minimum}, not {attrs[name]!r}', node, path, name)
    return result


def check(video: Json, chapters: Sections, path: Path, translation: bool) -> None:
    """Reject structural errors while both copies of a declaration still exist."""
    if not chapters:
        fail('story.empty', 'a story needs at least one chapter', video, path, 'chapters')
    attrs = attributes(video, path)
    fps = 1
    if not translation:
        # Only the English heading sets the frame rate, size, and pacing; each time must fit the timeline's frames.
        fps = int(numeric(attrs, 'fps', video, path, whole=True, minimum=1, maximum=U32_MAX))
        width = numeric(attrs, 'width', video, path, whole=True, minimum=2, maximum=8192)
        height = numeric(attrs, 'height', video, path, whole=True, minimum=2, maximum=8192)
        try:
            parse_resolution(f'{width}x{height}')
        except ValueError as error:
            fail('story.heading.resolution', str(error), video, path, 'width/height')
        for name in attrs:
            if name in PACING:
                value = numeric(attrs, name, video, path)
                if name != 'taggedWeight':
                    check_frames(value, fps, name, video, path)
    if 'captionWidth' in attrs:
        # Caption splitting divides by 2*w-6 (English), 2*w-4 (Chinese).
        numeric(attrs, 'captionWidth', video, path, whole=True, minimum=3 if translation else 4, maximum=sys.maxsize)
    video_attrs = attrs
    chapter_ids: dict[str, Json] = {}
    beat_ids: dict[str, Json] = {}
    numbers: dict[int, Json] = {}
    for chapter, beats in chapters:
        # For each chapter: a unique id and number, a budget that fits, and at least one beat with a unique id.
        unique_id(chapter, chapter_ids, path, 'chapter')
        attrs = attributes(chapter, path)
        if not translation or 'number' in attrs:
            number = int(numeric(attrs, 'number', chapter, path, whole=True, minimum=1, maximum=U32_MAX))
            if number in numbers:
                fail('story.chapter.number.duplicate', f'duplicate chapter number {number}', chapter, path, 'number', numbers[number])
            numbers[number] = chapter
        if not translation:
            target = numeric(attrs, 'target', chapter, path)
            check_frames(target, fps, 'target', chapter, path)
            # The current planner multiplies the budget by a sentence's
            # weight before dividing by the total. Guard that arithmetic.
            weight = numeric(video_attrs, 'taggedWeight', video, path)
            count = sum(len(paragraphs) for _, paragraphs in beats)
            if not math.isfinite(weight * count) or not math.isfinite(weight * target):
                fail('story.heading.number', 'taggedWeight overflows this chapter\'s pause allocation', video, path, 'taggedWeight')
        if not beats:
            fail('story.chapter.empty', 'a chapter needs at least one beat', chapter, path, 'beats')
        for beat, _ in beats:
            unique_id(beat, beat_ids, path, 'beat')
            props = attributes(beat, path)
            if 'hold' in props:
                hold = numeric(props, 'hold', beat, path)
                check_frames(hold, fps, 'hold', beat, path)


def check_frames(seconds: float | int, fps: int, name: str, node: Json, path: Path) -> None:
    frames = seconds * fps
    if not math.isfinite(frames) or frames > U64_MAX:
        fail('story.heading.number', f'{name} cannot fit the timeline\'s 64-bit frame range at {fps} fps', node, path, name)


def unique_id(node: Json, previous: dict[str, Json], path: Path, kind: str) -> None:
    name = node.get('id') or ''
    if not name:
        fail(f'story.{kind}.id', f'a {kind} heading names its id', node, path, 'id')
    if name in previous:
        fail(f'story.{kind}.duplicate', f'duplicate {kind} ID {name!r}', node, path, 'id', previous[name])
    previous[name] = node
