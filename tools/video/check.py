"""Check a storyboard without an audio environment, speech cache, or GPU.

    python3 tools/video/check.py --investigation godot/ssr --binary target/release/render-lab
    python3 tools/video/check.py --investigation godot/ssr --lang zh-CN --json
    python3 tools/video/check.py --investigation godot/ssr --explain c1-adds [--json]
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lamlet  # noqa: E402
import preflight  # noqa: E402
from config import LANGS, Config  # noqa: E402
from locations import DiagnosticError, describe  # noqa: E402
from storyboard import load_board, validate  # noqa: E402


def main() -> None:
    parser = argparse.ArgumentParser(description=(__doc__ or '').split('\n\n')[0])
    parser.add_argument('--investigation', required=True)
    parser.add_argument('--investigation-dir', type=Path)
    parser.add_argument('--lang', choices=LANGS, default='en')
    parser.add_argument('--binary', type=Path, default=lamlet.BINARY)
    parser.add_argument('--json', action='store_true', help='structured diagnostics on stdout, including on failure')
    parser.add_argument('--explain', metavar='BEAT', help='after the check, explain one beat: its sentences, its cues in source order, what each writes and names, takeovers (explain.py); frames and seconds if the edition is prepared')
    args = parser.parse_args()
    cfg = Config.load(args.investigation, args.lang, args.investigation_dir)
    lamlet.BINARY = args.binary.resolve()
    report: dict[str, Any]
    try:
        board = load_board(cfg)
        report = preflight.check(cfg, board)
        validate(cfg, board, json.loads(cfg.source_index.read_text()))
    except preflight.CheckError as error:
        report = error.report
    except (OSError, ValueError, RuntimeError, lamlet.LamletError) as error:
        # A failure without a diagnostic of its own is reported against the storyboard file.
        diagnostic = error.diagnostic if isinstance(error, DiagnosticError) else {
            'code': 'story.load', 'severity': 'error', 'file': str(cfg.translation if cfg.zh() else cfg.storyboard),
            'line': None, 'column': None, 'beat': None, 'field': None, 'message': str(error),
            'fallback': 'no structured location was supplied by this reader or host failure',
        }
        report = {'renderLabStoryCheck': 1, 'ok': False, 'diagnostics': [diagnostic]}
    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=1, allow_nan=False))
    elif report['ok']:
        print(f'{cfg.investigation} ({cfg.lang}): story check passed (symbolic timing; speech not measured)')
        for diagnostic in report['diagnostics']:
            print(f'{describe(diagnostic)}: {diagnostic["severity"]}: {diagnostic["message"]} [{diagnostic["code"]}]', file=sys.stderr)
    else:
        for diagnostic in report['diagnostics']:
            print(f'{describe(diagnostic)}: {diagnostic["message"]} [{diagnostic["code"]}]', file=sys.stderr)
    if not report['ok']:
        raise SystemExit(1)
    if args.explain:
        import explain
        detail = explain.explain(cfg, board, preflight.normalized(board)['cues'], args.explain)
        print(json.dumps(detail, ensure_ascii=False, indent=1) if args.json else explain.text(detail))


if __name__ == '__main__':
    main()
