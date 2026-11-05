"""Untimed story data and the host's shared semantic checks; standard library only."""
from __future__ import annotations

import json
import subprocess
from typing import Any

import lamlet
import markup
from config import Config
from locations import DiagnosticError, describe

Json = dict[str, Any]


def normalized(board: Json) -> Json:
    """Cues in source order, with original symbolic Lengths and source ranges.

    A range's starting and ending values are checked separately. There are no
    measured times or frames here; close tags remain anchors for later timing.
    """
    cues: list[Json] = []
    for chapter in board['chapters']:
        for beat in chapter['beats']:
            context = {'chapter': chapter['id'], 'beat': beat['id']}
            for i, key in enumerate(beat.get('keys', [])):
                sources = beat.get('keySources', [])
                source = sources[i] if i < len(sources) else beat.get('source', {})
                cues.append({**context, 'kind': 'opening', 'paragraph': None, 'piece': None, 'source': source, 'key': key})
            for i, sentence in enumerate(beat.get('narration', [])):
                for tag in markup.sentence(sentence).tags:
                    if tag.key is None:
                        continue
                    source = tag.source or sentence.get('source', beat.get('source', {}))
                    entry = {**context, 'kind': tag.kind, 'paragraph': i + 1, 'piece': tag.piece, 'source': source}
                    if tag.kind == 'open':
                        try:
                            jump, key = markup.range_values(tag.key, describe(source))
                        except markup.MarkupError as error:
                            raise DiagnosticError('story.range', str(error), source, beat=beat['id'], field='params') from error
                        if jump:
                            cues.append({**entry, 'kind': 'range-start', 'key': {'params': jump}})
                    else:
                        key = tag.key
                    cues.append({**entry, 'key': key})
    return {'renderLabStory': 1, 'cues': cues}


def host_check(cfg: Config, data: Json, *, timeline: bool = False) -> Json:
    """Ask the selected, already-built host; its command never initializes a GPU."""
    args = [str(lamlet.command()), '--investigation', cfg.investigation, 'story', 'check', '-']
    if timeline:
        args.append('--timeline')
    proc = subprocess.run(args, input=json.dumps(data, ensure_ascii=False, allow_nan=False), capture_output=True, text=True)
    try:
        report = json.loads(proc.stdout)
    except ValueError:
        raise RuntimeError(f'story check host failed ({proc.returncode}): {proc.stderr.strip() or proc.stdout.strip()}') from None
    if report.get('renderLabStoryCheck') != 1 or bool(report.get('ok')) != (proc.returncode == 0):
        raise RuntimeError('inconsistent or unsupported story check response from host')
    if not report['ok']:
        raise CheckError(report)
    return report


class CheckError(RuntimeError):
    def __init__(self, report: Json):
        self.report = report
        super().__init__('\n'.join(f'{describe(d)}: {d["message"]} [{d["code"]}]' for d in report['diagnostics']))


def check(cfg: Config, board: Json) -> Json:
    return host_check(cfg, normalized(board))


def check_timeline(cfg: Config, timeline: Json) -> Json:
    """Validate real frames before writing prepared outputs; retain a separate sidecar."""
    provenance = [{**key.get('_provenance', {}), 'beat': key.get('source')} for key in timeline['keys']]
    clean = {**timeline, 'keys': [{k: v for k, v in key.items() if k != '_provenance'} for key in timeline['keys']]}
    host_check(cfg, {'timeline': clean, 'provenance': provenance}, timeline=True)
    timeline['keys'] = clean['keys']
    return {'renderLabTimelineProvenance': 1, 'keys': provenance}
