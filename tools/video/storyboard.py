"""Load and check a storyboard without importing audio or rendering tools."""
from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

import config
from config import Config
from locations import DiagnosticError
from headings import DEFAULT_CAPTION_WIDTH

Json = dict[str, Any]


def beat_listings(beat: Json) -> list[str]:
    """Pseudo-code listings a beat shows, in order of first use."""
    import markup
    names: list[str] = []
    for k in beat.get('keys', []) + [k for s in beat.get('narration', []) for k in markup.text_keys(beat, s)]:
        if k.get('code') and k['code'] not in names:
            names.append(k['code'])
    return names


def listing_path(cfg: Config, name: str) -> Path:
    return cfg.pseudocode / (name.replace('-', '_') + '.py')


def read_listing(cfg: Config, name: str) -> tuple[str, str | None, str]:
    """(title in the edition's language, source key, code without tags or marks) of a listing (lab/src/pseudocode.rs):
    its `# title-text:` is a key of the package's text tables."""
    header: dict[str, str] = {}
    code: list[str] = []
    for line in listing_path(cfg, name).read_text().splitlines():
        m = re.match(r'# (title-text|source|demo|dispatch-text): (.*)$', line)
        if m and not code:
            header[m.group(1)] = m.group(2).strip()
            continue
        code.append(re.sub(r'\s*# @[^\n]*$', '', line).rstrip())
    while code and not code[-1]:
        code.pop()
    title = config.text_table(cfg.package.name, cfg.lang)[header['title-text']] if 'title-text' in header else name
    return title, header.get('source'), '\n'.join(code)


def load_board(cfg: Config) -> Json:
    """The storyboard, a Lamlet presentation (`storyboard.lam`, presentation.py), with its translation merged
    for a translated edition. It is read with the investigation's cue vocabulary, generated first (into the
    package's build/<package>.lam), so the vocabulary always matches the investigation's values, and with the
    package's story library (`video/story.lam`, the package `<package>_story`) when it has one."""
    import lamlet
    import markup
    import presentation
    import zh
    name = cfg.package.name
    vocabulary = lamlet.vocabulary(cfg.investigation, name, cfg.package / 'build' / f'{name}.lam')
    keys = vocabulary.with_suffix('.keys.json')
    fields = markup.Fields.of(json.loads(keys.read_text()))
    packages = [(name, vocabulary)]
    if cfg.story_library.exists():
        packages.append((f'{name}_story', cfg.story_library))
    board = presentation.load(cfg.storyboard, packages, fields, cfg.investigation, keys)
    if cfg.zh():
        board = zh.localize(board, presentation.load_translation(cfg.translation))
    return board


def validate(cfg: Config, board: Json, index: Json) -> None:
    import diagrams
    import frame_graph
    import presentation
    import markup
    import zh
    keys = index['entries']
    ids: set[str] = set()
    for chapter in board['chapters']:
        for key in chapter.get('sources', []):
            if key not in keys:
                raise DiagnosticError('story.source', f'chapter {chapter["number"]}: unknown source key {key}', chapter.get('source'), field='sources')
        for beat in chapter['beats']:
            def fail(code: str, message: str, field: str) -> None:
                raise DiagnosticError(code, message, beat.get('source'), beat=beat['id'], field=field)

            if beat['id'] in ids:
                fail('story.beat.duplicate', f'duplicate beat {beat["id"]}', 'id')
            ids.add(beat['id'])
            if beat['kind'] not in ('3d', '2d'):
                fail('story.beat.kind', f'unknown scene type {beat["kind"]}', 'kind')
            if 'graph' in beat:
                known = frame_graph.names(frame_graph.load(cfg, beat['graph']))
                for node in beat.get('focus', []):
                    if node not in known:
                        fail('story.focus', f'unknown focus {node} in the frame graph {beat["graph"]}', 'focus')
            elif presentation.is_diagram(beat):
                known = diagrams.names(diagrams.load(cfg, beat['diagram']))
                for node in beat.get('focus', []):
                    if node not in known:
                        fail('story.focus', f'unknown focus node {node} in {beat["diagram"]}', 'focus')
            for key in beat.get('sources', []):
                if key not in keys:
                    fail('story.source', f'unknown source key {key}', 'sources')
            if not beat.get('narration', []):
                fail('story.narration', f'{beat["id"]} has no narration', 'narration')
            width = board.get('captionWidthZh', zh.CAPTION_UNITS) if cfg.zh() else board.get('captionWidth', DEFAULT_CAPTION_WIDTH)
            for sentence in beat.get('narration', []):
                shown = markup.shown(sentence, cfg.zh()).caption
                held, _ = markup.protect(shown)
                # No partition into two-line cues can fit a token larger
                # than this. Diagnose it before the English splitter loops
                # forever, or the Chinese splitter exhausts its candidates.
                needed = max((zh.display_width(token) for token, _, _ in zh.tokens(held)), default=0) if cfg.zh() else max(((len(word) + 1) // 2 for word in held.split()), default=0)
                if needed > width:
                    source = sentence.get('shownSource') if cfg.zh() else sentence.get('source')
                    raise DiagnosticError('story.caption.width', f'captionWidth {width} cannot fit this caption; its longest token needs at least {needed}',
                                          source or beat.get('source'), beat=beat['id'], field='captionWidth')
            for name in beat_listings(beat):
                path = listing_path(cfg, name)
                if not path.is_file():
                    fail('story.listing', f'unknown pseudo-code listing {name}', 'code')
                _, source, code = read_listing(cfg, name)
                if source not in keys:
                    fail('story.source', f'{name}: unknown source key {source}', 'sources')
                try:
                    compile(code, str(path), 'exec')
                except SyntaxError as error:
                    # Tags and header lines are removed from `code`; its line is
                    # approximate in the original listing, so do not invent one.
                    raise DiagnosticError('story.listing.syntax', str(error),
                                          {'file': str(path), 'fallback': 'syntax error in listing after removing its header and tags'},
                                          beat=beat['id'], field='code') from error


