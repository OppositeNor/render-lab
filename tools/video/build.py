"""Build an investigation's narrated video.

Narration is synthesised per sentence with a Piper voice and measured; the
storyboard's shot keys are placed at sentence starts, turned into frame
indices, and written as a timeline that `render-lab render` records frame by
frame. The muxed video, captions, chapters, script, source-reading guide, and
verification report come from the same data.

    python3 tools/video/build.py --investigation o3de/lighting [--lang zh-CN] [--resolution 1080p] [--prepare-only | --snapshots-only | --verify-only]

The video is 1920 x 1080 unless `--resolution` names another size (720p,
1080p, 1440p, 2160p or 4k, or WIDTHxHEIGHT); the stills and the verification
use the same size, so give `--verify-only` the size the video was built at.

The zh-CN edition speaks the English sentences, so it shares the English
edition's narration and timing; its on-screen text, headlines, chapter titles,
subtitles, and documents are Chinese (technical terms stay English). Build the
English edition first. The tools come from tools/video/setup.sh (or the paths
in RENDER_LAB_VIDEO_ENV and its siblings, see config.py).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from config import LANGS, Config, Environment, log, parse_resolution, run  # noqa: E402

Json = dict[str, Any]


def reexec_in_environment(env: Environment) -> None:
    """Run under the environment's Python, which has Piper."""
    if Path(sys.executable).resolve() == env.python.resolve() or os.environ.get('RENDER_LAB_VIDEO_REEXEC'):
        return
    if not env.python.exists():
        raise SystemExit(f'no video environment at {env.prefix}; run tools/video/setup.sh (or set RENDER_LAB_VIDEO_ENV)')
    os.environ['RENDER_LAB_VIDEO_REEXEC'] = '1'
    os.environ['PATH'] = f'{env.prefix / "bin"}{os.pathsep}{os.environ.get("PATH", "")}'
    os.execv(str(env.python), [str(env.python), '-I', __file__, *sys.argv[1:]])


def load_board(cfg: Config) -> Json:
    import zh
    import story
    board = story.load_any(cfg.storyboard)
    if cfg.zh():
        board = zh.localize(board, story.load_any(cfg.translation, translation=True))
    return board


def validate(cfg: Config, board: Json, index: Json) -> None:
    import markup
    import story
    from outputs import beat_listings, listing_path, read_listing
    from timeline import sentences_of
    keys = index['entries']
    ids: set[str] = set()
    for chapter in board['chapters']:
        for key in chapter.get('sources', []):
            assert key in keys, f'chapter {chapter["number"]}: unknown source key {key}'
        for beat in chapter['beats']:
            assert beat['id'] not in ids, f'duplicate beat {beat["id"]}'
            ids.add(beat['id'])
            assert beat['kind'] in ('3d', '2d'), beat['id']
            if story.is_diagram(beat):
                graph = (cfg.diagrams / f'{beat["diagram"]}.mmd').read_text()
                for node in beat.get('focus', []):
                    if not re.search(r'\b' + re.escape(node) + r'\s*[\[({]', graph):
                        raise RuntimeError(f'{beat["id"]}: unknown focus node {node} in {beat["diagram"]}')
            for key in beat.get('sources', []):
                assert key in keys, f'{beat["id"]}: unknown source key {key}'
            for item in [beat] + sentences_of(beat):
                tagged = markup.text_keys(beat, item) if item is not beat else []
                for k in item.get('keys', []) + item.get('sequence', []) + tagged + list(item.get('cues', {}).values()):
                    if k.get('callout'):
                        assert k['callout'] in keys, f'{beat["id"]}: unknown callout {k["callout"]}'
            assert sentences_of(beat), f'{beat["id"]} has no narration'
            for name in beat_listings(beat):
                path = listing_path(cfg, name)
                assert path.is_file(), f'{beat["id"]}: unknown pseudo-code listing {name}'
                _, _, source, code = read_listing(cfg, name)
                assert source in keys, f'{name}: unknown source key {source}'
                compile(code, str(path), 'exec')


def resolution_arg(value: str) -> tuple[int, int]:
    try:
        return parse_resolution(value)
    except ValueError as e:
        raise argparse.ArgumentTypeError(str(e)) from None


def main() -> None:
    parser = argparse.ArgumentParser(description=(__doc__ or '').split('\n\n')[0])
    parser.add_argument('--investigation', required=True, help='engine/feature, e.g. o3de/lighting')
    parser.add_argument('--investigation-dir', type=Path, help='the directory of its video/video.json, when not its package in packages/')
    parser.add_argument('--lang', default='en', choices=LANGS)
    parser.add_argument('--resolution', default='1080p', type=resolution_arg, help='frame size: 720p, 1080p (default), 1440p, 2160p, 4k, or WIDTHxHEIGHT')
    parser.add_argument('--prepare-only', action='store_true', help='narration, timeline, captions, and documents only')
    parser.add_argument('--snapshots-only', action='store_true', help='prepare, then render beat-midpoint stills for review')
    parser.add_argument('--skip-render', action='store_true', help='reuse the rendered video.mp4')
    parser.add_argument('--verify-only', action='store_true')
    parser.add_argument('--frames', help='render only START:END (testing; the result is not a complete video)')
    parser.add_argument('--tts-device', default='auto', choices=['auto', 'cuda', 'cpu'])
    args = parser.parse_args()
    env = Environment()
    reexec_in_environment(env)

    import narration
    import outputs
    import timeline as timing
    import verify as checks
    cfg = Config.load(args.investigation, args.lang, args.investigation_dir)
    build = cfg.build()
    for d in (build, build / 'logs'):
        d.mkdir(parents=True, exist_ok=True)
    if cfg.zh() and not args.verify_only:
        if not cfg.timeline('en').exists():
            raise SystemExit('build the English edition first: the zh-CN edition uses its narration and timing')
        (build / 'english-checksums.json').write_text(json.dumps(checks.english_checksums(cfg), indent=1) + '\n')
    board = load_board(cfg)
    index = json.loads(cfg.source_index.read_text())
    validate(cfg, board, index)
    problems = [r for r in checks.check_sources(cfg, index) if not r['ok']]
    if problems:
        raise RuntimeError(f'the source index does not match the checkout: {problems[:5]}')
    if args.verify_only:
        tl = json.loads(cfg.timeline().read_text())
        plans = {int(k): v for k, v in json.loads((build / 'chapter-plans.json').read_text()).items()}
        sys.exit(0 if checks.verify(cfg, env, board, index, tl, plans, args.resolution) else 1)

    device, reason = narration.select_device(env, args.tts_device)
    log(f'Narration device: {device} ({reason}).')
    durations = narration.synthesize(cfg, env, device, timing.all_sentences(board))
    tl, placements, cues, plans = timing.build_timeline(board, durations, cfg)
    cfg.timeline_dir().mkdir(parents=True, exist_ok=True)
    cfg.timeline().write_text(json.dumps(tl, indent=1, ensure_ascii=False) + '\n')
    (build / 'chapter-plans.json').write_text(json.dumps(plans, indent=1) + '\n')
    log(f'Timeline: {tl["totalFrames"]} frames, {tl["totalFrames"] / tl["fps"] / 60:.2f} min, {len(tl["beats"])} beats, {len(tl["keys"])} keys.')
    if '{@ formula=' in cfg.timeline().read_text():
        # The lab typesets them; check each against its limits before anything is rendered.
        binary = outputs.cargo_build(cfg)
        run([binary, 'formulas', '--check', cfg.timeline()], build / 'logs' / 'formulas.log')
        log('Formulas: each typesets within the lab\'s limits.')
    for c, b in zip(tl['chapters'], board['chapters']):
        log(f'  {c["number"]:02d} {c["title"]}: {(c["endFrame"] - c["startFrame"]) / tl["fps"]:.1f} s (budget {b["targetSeconds"]}, natural {plans[c["number"]]["natural"]:.1f})')
    outputs.write_audio(cfg, placements, tl['totalFrames'], tl['fps'])
    outputs.write_captions(cfg, cues)
    outputs.write_chapters(cfg, board, tl)
    outputs.write_script(cfg, board, index, tl)
    outputs.write_guide(cfg, board, index, tl)
    outputs.write_preview(cfg, board, tl)
    if args.prepare_only:
        return
    log('Rendering the diagrams.')
    outputs.render_mermaid(cfg, env, board)
    binary = outputs.cargo_build(cfg)
    if args.snapshots_only:
        outputs.snapshots(cfg, env, binary, tl, args.resolution)
        log(f'Snapshots in {build / "snapshots"}')
        return
    if not args.skip_render:
        outputs.render_video(cfg, env, binary, args.frames, args.resolution)
    if args.frames:
        log('Partial render finished; skipping the mux and the verification.')
        return
    outputs.mux(cfg, env)
    outputs.snapshots(cfg, env, binary, tl, args.resolution)
    log('Running render-lab selftest with the timeline.')
    if (build / 'selftest').exists():
        shutil.rmtree(build / 'selftest')
    try:
        run(outputs.lab(binary, cfg, 'selftest', '--report', build / 'selftest-report.json', '--timeline', cfg.timeline(), '--work-dir', build / 'selftest', '--ffmpeg', env.ffmpeg),
            build / 'logs' / 'selftest.log')
    except RuntimeError as e:
        log(f'selftest failed: {str(e)[:400]}')
    if not checks.verify(cfg, env, board, index, tl, plans, args.resolution):
        sys.exit(1)


if __name__ == '__main__':
    main()
