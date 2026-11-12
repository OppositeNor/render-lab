"""Checks of a finished build, written to verification-report.json and .md:
the file's format and length against the timeline, chapters against their
budgets, a full decode, audio levels, captions, the render report's text and
labels, the diagrams' label size, the engine checkout's commit, every
relative link, the lab's selftest and layouts, and, for zh-CN, that the
edition shares the English narration and frames.
"""
from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any, Callable

import zh
from config import LAB, Config, Environment, log, run
from timeline import BUDGET_TOLERANCE, budget_rows, caption_width

Json = dict[str, Any]
LINK = re.compile(r'\[([^\]]*)\]\(([^)\s]+)\)')
# A zh-CN build must leave these English files as they were.
ENGLISH_FILES = ['captions.srt', 'captions.vtt', 'chapters.txt', 'script.md', 'script.txt', 'source-reading-guide.md', 'preview.html',
                 'verification-report.json', 'verification-report.md', 'contact-sheet.png', 'poster.png', 'narration.wav']


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_links(cfg: Config, index: Json, generated: list[Path]) -> list[Json]:
    """Every relative link resolves from its document."""
    build = cfg.build()
    results: list[Json] = []
    home = str(Path.home())
    generated_paths = {p.resolve() for p in generated}
    for doc in [build / 'script.md', build / 'source-reading-guide.md']:
        if not doc.exists():
            continue
        text = doc.read_text()
        if home in text:
            # A path into the user's home directory is a leak, whether or not it resolves.
            results.append({'doc': doc.name, 'target': '(home directory path)', 'ok': False})
        for _, target in LINK.findall(text):
            if re.match(r'^[a-z]+://', target) or target.startswith(('#', 'mailto:')):
                # Web links and in-page anchors are not files; the rest must exist, or be written later in this build.
                continue
            path_part = target.partition('#')[0]
            resolved = (doc.parent / path_part).resolve()
            ok = resolved.exists() or resolved in generated_paths
            detail: Json = {'doc': doc.name, 'target': target, 'ok': ok}
            results.append(detail)
    return results


def english_checksums(cfg: Config) -> dict[str, str]:
    build = cfg.build('en')
    files = {name: build / name for name in ENGLISH_FILES}
    files[cfg.video_name('en')] = cfg.video('en')
    return {name: sha256(path) for name, path in files.items() if path.exists()}


def verify(cfg: Config, env: Environment, board: Json, index: Json, timeline: Json, plans: dict[int, Json], resolution: tuple[int, int] = (1920, 1080),
           decode: bool = True) -> bool:
    build = cfg.build()
    checks: list[Json] = []

    def check(name: str, ok: bool, detail: Any) -> None:
        log(f'{"PASS" if ok else "FAIL"} {name}')
        checks.append({'name': name, 'passed': bool(ok), 'detail': detail})

    fps = timeline['fps']
    video = cfg.video()
    # The file's streams, format, and chapters, as FFmpeg reads them.
    info = json.loads(run([env.ffprobe, '-v', 'error', '-show_streams', '-show_format', '-show_chapters', '-of', 'json', video]))
    v = next(s for s in info['streams'] if s['codec_type'] == 'video')
    a = next((s for s in info['streams'] if s['codec_type'] == 'audio'), None)
    duration = float(info['format']['duration'])
    colour = {k: v.get(k) for k in ('color_space', 'color_primaries', 'color_transfer', 'color_range')}
    check(f'{resolution[0]}x{resolution[1]}, 30 fps, H.264 (yuv420p, BT.709 tagged) with AAC narration',
          (v['width'], v['height']) == resolution and v['r_frame_rate'] == '30/1' and v['codec_name'] == 'h264' and v.get('pix_fmt') == 'yuv420p'
          and colour == {'color_space': 'bt709', 'color_primaries': 'bt709', 'color_transfer': 'bt709', 'color_range': 'tv'} and a is not None and a['codec_name'] == 'aac',
          {'width': v['width'], 'height': v['height'], 'fps': v['r_frame_rate'], 'video': v['codec_name'], 'audio': a and a['codec_name'], 'pixFmt': v.get('pix_fmt'), **colour})
    low, high = cfg.spec['durationMinutes']
    check(f'duration within {low}–{high} minutes and equal to the timeline', low * 60 <= duration <= high * 60 and abs(duration - timeline['totalFrames'] / fps) < 0.25,
          {'seconds': duration, 'minutes': round(duration / 60, 2), 'timelineSeconds': timeline['totalFrames'] / fps})
    frames = int(v.get('nb_frames') or 0)
    check('frame count equals the timeline', frames == timeline['totalFrames'], {'videoFrames': frames, 'timelineFrames': timeline['totalFrames']})
    # The closing notices card is not the last chapter's: it is left out of its budget (timeline.budget_rows).
    budget = budget_rows(timeline, board, plans)
    within = all(r['within'] for r in budget)
    rows = [{'chapter': r['chapter'], 'title': r['title'], 'seconds': round(r['seconds'], 1), 'target': r['target'], 'deviation': f'{r["deviation"]:+.1%}', **plans.get(r['chapter'], {})} for r in budget]
    check(f'{len(board["chapters"])} chapters in the file, each within {BUDGET_TOLERANCE:.0%} of its time budget', len(info.get('chapters', [])) == len(board['chapters']) and within, {'chapters': rows})
    if decode:
        log('Decoding the complete video and audio.')
        run([env.ffmpeg, '-v', 'error', '-nostdin', '-i', video, '-map', '0:v:0', '-map', '0:a:0', '-f', 'null', '-'], build / 'logs' / 'decode.log')
        errors = (build / 'logs' / 'decode.log').read_text().strip()
        check('complete audio and video decode without errors', not errors, {'decoderOutput': errors[:2000]})
    stats = run([env.ffmpeg, '-hide_banner', '-nostdin', '-i', video, '-map', '0:a:0', '-af', 'volumedetect', '-f', 'null', '-'], build / 'logs' / 'audio-levels.log')
    # volumedetect's mean and peak levels: the narration must be heard and never clip.
    mean_match = re.search(r'mean_volume:\s*([-\d.]+)', stats)
    peak_match = re.search(r'max_volume:\s*([-\d.]+)', stats)
    mean_db = float(mean_match.group(1)) if mean_match else -99.0
    peak_db = float(peak_match.group(1)) if peak_match else 99.0
    check('narration audible and not clipped', mean_db > -45 and peak_db <= 0, {'meanDb': mean_db, 'peakDb': peak_db})
    # The captions: how many cues, when the last ends, and the widest line, as drawn.
    srt = (build / 'captions.srt').read_text()
    cue_count = srt.count(' --> ')
    last = max(float(m.group(1)) * 3600 + float(m.group(2)) * 60 + float(m.group(3).replace(',', '.')) for m in re.finditer(r'--> (\d+):(\d+):([\d,]+)', srt))
    width: Callable[[str], int] = zh.display_width if cfg.zh() else len
    limit = caption_width(board, cfg)
    caption_lines = [line for line in srt.splitlines() if line and not re.fullmatch(r'\d+|[\d:,]+ --> [\d:,]+', line)]
    # A formula ($LaTeX$ in the files) is measured as drawn, as the captions wrap it (markup.protect).
    drawn = lambda line: re.sub(r'\$([^$]*)\$', lambda m: 'Q' * max(4, len(re.sub(r'\\[a-zA-Z]+|[{}^_&]|\\\\', '', m.group(1)))), line)
    longest = max(width(drawn(line)) for line in caption_lines)
    check('captions cover the narration (SRT and WebVTT) within the line limit', cue_count > 50 and last <= duration and (build / 'captions.vtt').exists() and longest <= limit,
          {'cues': cue_count, 'lastCueEnd': last, 'longestLine': longest, 'lineLimit': limit})
    render = json.loads((build / 'render-report.json').read_text()) if (build / 'render-report.json').exists() else {}
    check('every rendered frame: text >= 24 px, labels inside the viewport, no overlaps',
          render.get('min_text_pt', 0) >= 24 and render.get('max_label_overlaps', 1) == 0 and render.get('max_labels_outside', 1) == 0,
          {k: render.get(k) for k in ('frames', 'min_text_pt', 'max_label_overlaps', 'max_labels_outside', 'frames_with_label_issues', 'seconds', 'gpu')})
    links = check_links(cfg, index, [build / 'verification-report.md', build / 'verification-report.json', cfg.video()])
    check('every relative link resolves; no home-directory paths', all(link['ok'] for link in links),
          {'links': len(links), 'problems': [link for link in links if not link['ok']]})
    # The paper must be the PDF the index was read from; an engine's (or a paper's code's) checkout, the recorded commit.
    if 'paper' in index:
        digest = sha256(cfg.paper_pdf(index))
        check('the paper is the recorded PDF', digest == index['paper']['sha256'], {'pdf': digest, 'recorded': index['paper']['sha256']})
    if index.get('engine', {}).get('commit'):
        commit = run(['git', '-C', cfg.checkout(index), 'rev-parse', 'HEAD']).strip()
        check('engine checkout is the recorded commit', commit == index['engine']['commit'], {'checkout': commit, 'recorded': index['engine']['commit']})
    if cfg.zh():
        verify_zh(cfg, check)
    # The selftest must have run with this timeline and passed.
    selftest = build / 'selftest-report.json'
    if selftest.exists():
        data = json.loads(selftest.read_text())
        check('render-lab selftest (with this timeline)', data.get('passed') is True, {'checks': [{'name': c['name'], 'passed': c['passed']} for c in data.get('checks', [])]})
    else:
        check('render-lab selftest (with this timeline)', False, {'missing': str(selftest.relative_to(LAB))})
    layouts = json.loads(run([LAB / 'target' / 'release' / 'render-lab', '--investigation', cfg.investigation, 'layout', '--json']))
    check('Rust struct layouts equal the WGSL layouts (naga)', all(c['ok'] for c in layouts['checks']), {'records': [{'record': c['record'], 'ok': c['ok']} for c in layouts['checks']]})
    passed = all(c['passed'] for c in checks)
    toolchain = {'rustc': run(['rustc', '--version']).split()[1], 'ffmpeg': run([env.ffmpeg, '-version']).split()[2], 'python': sys.version.split()[0]}
    result: Json = {'status': 'passed' if passed else 'failed', 'investigation': cfg.investigation, 'video': cfg.video_name(), 'language': cfg.lang,
                    'bytes': video.stat().st_size, 'sha256': sha256(video), 'durationSeconds': duration, 'engine': index.get('engine', {}), 'paper': index.get('paper'), 'toolchain': toolchain, 'checks': checks}
    (build / 'verification-report.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    write_markdown(cfg, result)
    return passed


def verify_zh(cfg: Config, check: Callable[[str, bool, Any], None]) -> None:
    english = json.loads(cfg.timeline('en').read_text())
    timeline = json.loads(cfg.timeline().read_text())
    # The zh-CN edition shares the English edition's sound and timing, and must leave its files untouched.
    same = sha256(cfg.build() / 'narration.wav') == sha256(cfg.build('en') / 'narration.wav')
    check('the narration is the English edition\'s track', same, {})

    def starts(t: Json) -> list[Any]:
        return [(b['id'], b['startFrame'], b['frames']) for b in t['beats']] + [(c['number'], c['startFrame'], c['endFrame']) for c in t['chapters']]
    check('every beat and chapter starts on the same frame as in the English edition', starts(timeline) == starts(english) and timeline['totalFrames'] == english['totalFrames'],
          {'frames': timeline['totalFrames'], 'englishFrames': english['totalFrames']})
    before = json.loads((cfg.build() / 'english-checksums.json').read_text())
    after = english_checksums(cfg)
    check('the English edition unchanged by the zh-CN build', before == after, {'files': {name: before.get(name) == digest for name, digest in after.items()}})


def write_markdown(cfg: Config, result: Json) -> None:
    engine = result['engine']
    lines = ['# Verification report' + (' (zh-CN edition)' if cfg.zh() else ''), '',
             f'Status: **{result["status"]}** · `{result["video"]}` · {result["durationSeconds"] / 60:.2f} min · SHA-256 `{result["sha256"][:16]}…`', '',
             (f'Paper: {result["paper"]["title"]}, DOI `{result["paper"]["doi"]}`, PDF SHA-256 `{result["paper"]["sha256"][:16]}…`.' if result.get('paper')
              else f'Engine: {engine["name"]} {engine["version"]}, commit `{engine["commit"]}`.'), '',
             'Toolchain: ' + ', '.join(f'{k} {v}' for k, v in result['toolchain'].items()), '', '| Check | Result |', '|---|---|']
    lines += [f'| {c["name"]} | {"pass" if c["passed"] else "**FAIL**"} |' for c in result['checks']]
    # The chapter budgets' check carries a table of each chapter's timing.
    chapters = next((c for c in result['checks'] if ' chapters in the file' in c['name']), None)
    if chapters:
        lines += ['', '## Chapter timing', '', '| Chapter | Seconds | Budget | Deviation |', '|---|---:|---:|---:|']
        lines += [f'| {r["chapter"]:02d} {r["title"]} | {r["seconds"]} | {r["target"]} | {r["deviation"]} |' for r in chapters['detail']['chapters']]
    lines += ['', 'Details for every check are in [verification-report.json](verification-report.json).', '']
    (cfg.build() / 'verification-report.md').write_text('\n'.join(lines))
