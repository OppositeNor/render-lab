"""Everything a build writes besides the timeline: the narration track,
captions, chapters, the diagram frames, the rendered and muxed video, the
review stills, and the documents (script, source-reading guide, preview).
"""
from __future__ import annotations

import html
import json
import os
import shutil
import subprocess
import time
import threading
import wave
from pathlib import Path
from typing import Any

from config import LAB, Config, Environment, log, run, source_key, source_text
import diagrams
import frame_graph
import presentation
from timeline import sentences_of
from storyboard import beat_listings, read_listing

Json = dict[str, Any]

# Words the documents use, per edition.
WORDS: dict[str, dict[str, str]] = {
    'en': {
        'chapters': 'Chapters', 'starts': 'Starts at {when} · {seconds:.0f} s (budget {budget} s).', 'diagram': 'diagram', 'graph': 'the frame graph', 'demo': '3D demonstration',
        'card': 'source card', 'sources': 'Sources for this chapter: ', 'sep': ', ', 'line': 'line {line}', 'files': 'Files', 'listing': 'Python pseudo-code',
        'seeAbove': '*(Python pseudo-code “{title}”: see above.)*', 'engine': 'Engine source: {engine} {version}, commit `{commit}`. Prepared {date}.',
        'readingOrder': 'Recommended reading order', 'table': '| File | Symbol | Line | What to inspect |', 'simplify': 'What the demonstrations simplify', "mappings": "What the lab's data stands for", "mappingTable": "| Lab | Engine structure | Relationship | The engine's layout | The lab's |",
        'guideIntro': 'This guide follows the chapters of the video ({video}, script: {script}). For each topic it names the file, the symbol, the line to start reading at, and what to look for there.',
        'checkout': '- Source paths are relative to the root of the engine\'s source, and `:<line>` names the line.',
        'paper': 'Paper: {title} ({short}, {venue}), DOI `{doi}`. Prepared {date}.',
        'pages': '- Each entry names the paper\'s section and page (`§<section>, p. <page>`) and the equation, algorithm line, or figure.',
        'index': '- Every entry comes from one machine-readable index, {index}, which also drives the video\'s source cards and the viewer\'s source panel.',
        'video': 'video', 'script': 'Narrated script', 'guide': 'Source-reading guide', 'verification': 'Verification',
    },
    'zh-CN': {
        'chapters': '章节', 'starts': '开始于 {when} · {seconds:.0f} 秒（预算 {budget} 秒）。', 'diagram': '图表', 'graph': '帧图', 'demo': '3D 演示',
        'card': '源码卡片', 'sources': '本章源码：', 'sep': '、', 'line': '第 {line} 行', 'files': '文件', 'listing': 'Python 伪代码',
        'seeAbove': '*（Python 伪代码“{title}”见上文。）*', 'engine': '引擎源码：{engine} {version}，commit `{commit}`。准备于 {date}。',
        'readingOrder': '推荐阅读顺序', 'table': '| 文件 | 符号 | 行 | 要看什么 |', 'simplify': '演示做了哪些简化', 'mappings': '实验室的数据对应什么', 'mappingTable': '| 实验室 | 引擎结构 | 关系 | 引擎的布局 | 实验室的布局 |',
        'guideIntro': '本指南按照视频（{video}，脚本：{script}）的章节组织。每个主题都给出文件、符号、开始阅读的行，以及在那里要看什么。',
        'checkout': '- 源码路径相对于引擎源码的根目录，`:<行号>` 指出所在的行。',
        'paper': '论文：{title}（{short}，{venue}），DOI `{doi}`。准备于 {date}。',
        'pages': '- 每个条目都给出论文的章节和页码（`§<章节>, p. <页码>`），以及公式、算法行或图。',
        'index': '- 每个条目都来自同一个机器可读的索引 {index}，它同时驱动视频中的源码卡片和查看器的源码面板。',
        'video': '视频', 'script': '旁白脚本', 'guide': '源码阅读指南', 'verification': '验证报告',
    },
}


def stamp(seconds: float, ms: bool = True) -> str:
    unit = 1000 if ms else 100
    # Split the time into whole seconds and a fraction, in milliseconds or (without `ms`) hundredths.
    whole, fraction = divmod(round(seconds * unit), unit)
    hours, rest = divmod(whole, 3600)
    minutes, secs = divmod(rest, 60)
    return f'{hours:02}:{minutes:02}:{secs:02},{fraction:03}' if ms else f'{hours}:{minutes:02}:{secs:02}.{fraction:02}'


def clock(seconds: float) -> str:
    return stamp(seconds).split(',')[0]


def relpath(path: Path, start: Path) -> str:
    return os.path.relpath(path, start).replace(os.sep, '/')


# ---------------------------------------------------------------------------
# Audio and captions

def write_audio(cfg: Config, placements: list[Json], total_frames: int, fps: int) -> Path:
    rate = placements[0]['rate']
    # A silent 16-bit mono track as long as the video, which each sentence's speech is copied into.
    samples = bytearray(int(round(total_frames / fps * rate)) * 2)
    for p in placements:
        with wave.open(p['path'], 'rb') as stream:
            assert stream.getframerate() == rate and stream.getnchannels() == 1 and stream.getsampwidth() == 2
            data = stream.readframes(stream.getnframes())
        # The sentence starts at its frame's time; speech past the video's end is cut off.
        start = int(round(p['frame'] / fps * rate)) * 2
        end = min(len(samples), start + len(data))
        samples[start:end] = data[:end - start]
    out = cfg.build() / 'narration.wav'
    with wave.open(str(out), 'wb') as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(bytes(samples))
    return out


def write_captions(cfg: Config, cues: list[Json]) -> None:
    srt: list[str] = []
    vtt = ['WEBVTT', '']
    for i, c in enumerate(cues, 1):
        srt.append(f'{i}\n{stamp(c["start"])} --> {stamp(c["end"])}\n{c["text"]}\n')
        vtt.append(f'{stamp(c["start"]).replace(",", ".")} --> {stamp(c["end"]).replace(",", ".")}\n{c["text"]}\n')
    (cfg.build() / 'captions.srt').write_text('\n'.join(srt))
    (cfg.build() / 'captions.vtt').write_text('\n'.join(vtt))


def write_chapters(cfg: Config, board: Json, timeline: Json) -> None:
    fps = timeline['fps']
    meta = [';FFMETADATA1', 'title=' + board['title'], 'comment=' + board['subtitle'], 'artist=Render Lab', 'date=' + board['date']]
    if cfg.zh():
        meta.append('language=chi')
    for c in timeline['chapters']:
        # Each chapter is a span in milliseconds, titled with its number.
        meta += ['[CHAPTER]', 'TIMEBASE=1/1000', f'START={round(c["startFrame"] / fps * 1000)}', f'END={round(c["endFrame"] / fps * 1000)}', f'title={c["number"]:02d} {c["title"]}']
    (cfg.build() / 'chapters.ffmeta').write_text('\n'.join(meta) + '\n')
    write_chapter_list(cfg, timeline)


def chapter_time(seconds: float, hours: bool) -> str:
    """A chapter's start as a video site's chapter list writes it: MM:SS, or H:MM:SS in a video of an hour or more."""
    h, rest = divmod(int(seconds), 3600)
    m, s = divmod(rest, 60)
    return f'{h}:{m:02}:{s:02}' if hours else f'{m:02}:{s:02}'


def write_chapter_list(cfg: Config, timeline: Json) -> None:
    """chapters.txt: one line per chapter, its start and title (`00:30 Title`), to paste into a video site's
    description, where the site turns the times into chapter links."""
    fps = timeline['fps']
    hours = timeline['totalFrames'] / fps >= 3600
    text = [f'{chapter_time(c["startFrame"] / fps, hours)} {c["title"]}' for c in timeline['chapters']]
    (cfg.build() / 'chapters.txt').write_text('\n'.join(text) + '\n')


# ---------------------------------------------------------------------------
# Rendering

def cargo_build(cfg: Config) -> Path:
    log('Building render-lab (cargo build --release --locked --offline).')
    process_env = dict(os.environ, CARGO_HOME=str(LAB / '.cache' / 'cargo'), CARGO_TARGET_DIR=str(LAB / 'target'))
    run(['cargo', 'build', '--release', '--locked', '--offline'], cfg.build() / 'logs' / 'cargo-build.log', env=process_env)
    return LAB / 'target' / 'release' / 'render-lab'


def lab(binary: Path, cfg: Config, *args: Any) -> list[Any]:
    """A render-lab command line for this investigation."""
    return [binary, '--investigation', cfg.investigation, '--listing-style', cfg.listing_style, *args]


def render_video(cfg: Config, env: Environment, binary: Path, frames: str | None, resolution: tuple[int, int], jobs: int = 1) -> None:
    """Render the timeline to build/video.mp4. With `jobs` above one, the frames are split into that many ranges,
    rendered by as many render-lab processes at once (each range after `WARMUP` frames it does not write, so it starts
    with a whole render's history), and the segments joined without re-encoding."""
    build = cfg.build()
    if jobs > 1 and frames:
        raise SystemExit('--jobs renders the whole timeline; it does not go with --frames')
    total = int(json.loads(cfg.timeline().read_text(encoding='utf-8'))['totalFrames'])
    # No more ranges than the timeline has room for, each at least ten warm-ups long.
    jobs = max(1, min(jobs, total // (10 * WARMUP) or 1))
    bounds = [total * k // jobs for k in range(jobs + 1)]
    log('Rendering the timeline with render-lab (every frame streams to FFmpeg)' + (f', in {jobs} parallel ranges.' if jobs > 1 else '.'))
    started = time.time()
    parts: list[tuple[Path, Path, subprocess.Popen[str], list[str]]] = []
    for k in range(jobs):
        # One render writes the video itself; parallel ranges each write a part, joined below.
        video = build / 'video.mp4' if jobs == 1 else build / f'video.part{k}.mp4'
        report = build / 'render-report.json' if jobs == 1 else build / f'render-report.part{k}.json'
        args = lab(binary, cfg, 'render', '--timeline', cfg.timeline(), '--resolution', f'{resolution[0]}x{resolution[1]}', '--output', video,
                   '--report', report, '--ffmpeg', env.ffmpeg, '--crf', '18', '--preset', 'medium')
        if frames:
            # A requested range is rendered as it is; a parallel range below also renders a warm-up before its first frame.
            args += ['--frames', frames]
        elif jobs > 1:
            args += ['--frames', f'{bounds[k]}:{bounds[k + 1]}', '--warmup', str(WARMUP if k else 0)]
        proc = subprocess.Popen([str(a) for a in args], cwd=LAB, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        parts.append((video, report, proc, []))
    # Read every process's output as it comes, so none blocks on a full pipe.
    def drain(k: int, proc: subprocess.Popen[str], lines: list[str]) -> None:
        assert proc.stdout is not None
        for line in proc.stdout:
            lines.append(line)
            if line.startswith(('frame', 'rendered', 'GPU')):
                log('  ' + (f'[{k + 1}/{jobs}] ' if jobs > 1 else '') + line.rstrip())
    readers = [threading.Thread(target=drain, args=(k, p, lines)) for k, (_, _, p, lines) in enumerate(parts)]
    for r in readers:
        r.start()
    for r in readers:
        r.join()
    failed = []
    for k, (_, _, proc, lines) in enumerate(parts):
        # Once every range has finished, keep each one's log, and the tail of any that failed.
        proc.wait()
        (build / 'logs' / ('render.log' if jobs == 1 else f'render.part{k}.log')).write_text(''.join(lines))
        if proc.returncode:
            failed.append(f'range {k + 1}:\n' + ''.join(lines[-40:]))
    if failed:
        raise RuntimeError('render-lab render failed:\n' + '\n'.join(failed))
    if jobs > 1:
        # One report for the whole video, as a single render writes it.
        reports = [json.loads(r.read_text()) for _, r, _, _ in parts]
        merged = dict(reports[0])
        merged['frames'] = sum(r['frames'] for r in reports)
        merged['first_frame'] = reports[0]['first_frame']
        merged['min_text_pt'] = min(r['min_text_pt'] for r in reports)
        for key in ('max_label_overlaps', 'max_labels_outside'):
            merged[key] = max(r[key] for r in reports)
        for key in ('frames_with_label_issues', 'frame_sha256'):
            merged[key] = [x for r in reports for x in r[key]]
        merged['seconds'] = time.time() - started
        merged['output'] = str(build / 'video.mp4')
        merged['jobs'] = jobs
        (build / 'render-report.json').write_text(json.dumps(merged, indent=2))
        # Join the parts in order with FFmpeg's concat demuxer, copying the streams, then drop the parts.
        listing = build / 'video.parts.txt'
        listing.write_text(''.join(f"file '{v.name}'\n" for v, _, _, _ in parts))
        run([env.ffmpeg, '-hide_banner', '-loglevel', 'error', '-nostdin', '-y', '-f', 'concat', '-safe', '0', '-i', listing, '-c', 'copy', build / 'video.mp4'],
            build / 'logs' / 'concat.log')
        for v, r, _, _ in parts:
            v.unlink()
            r.unlink()
        listing.unlink()
    log(f'  render finished in {time.time() - started:.0f} s')


# Frames a parallel range renders before its first, unwritten (`render --warmup`): a second of the timeline, enough
# for the read-backs and the last frame's image a frame depends on to be its whole render's.
WARMUP = 30


def mux(cfg: Config, env: Environment) -> Path:
    log('Muxing the video, the normalised narration, and the chapters.')
    build = cfg.build()
    out = cfg.video()
    # Mux beside the target, then rename: a player that has the old file open keeps reading it.
    partial = out.with_name(out.stem + '.muxing' + out.suffix)
    run([env.ffmpeg, '-hide_banner', '-loglevel', 'error', '-nostdin', '-y', '-i', build / 'video.mp4', '-i', build / 'narration.wav', '-f', 'ffmetadata', '-i', build / 'chapters.ffmeta',
         '-map', '0:v:0', '-map', '1:a:0', '-map_metadata', '2', '-map_chapters', '2', '-c:v', 'copy', '-metadata:s:a:0', 'language=eng',
         # BT.709 primaries, transfer, and matrix in the H.264 VUI, limited range (lossless).
         '-bsf:v', 'h264_metadata=colour_primaries=1:transfer_characteristics=1:matrix_coefficients=1:video_full_range_flag=0',
         '-af', 'loudnorm=I=-16:TP=-1.5:LRA=11', '-c:a', 'aac', '-b:a', '160k', '-ar', '48000', '-movflags', '+faststart', partial], build / 'logs' / 'mux.log')
    partial.replace(out)
    return out


def snapshots(cfg: Config, env: Environment, binary: Path, timeline: Json, resolution: tuple[int, int]) -> list[Path]:
    """Beat-midpoint stills for review, a contact sheet, and the poster."""
    build = cfg.build()
    shots = build / 'snapshots'
    if shots.exists():
        shutil.rmtree(shots)
    frames = [b['startFrame'] + b['frames'] // 2 for b in timeline['beats']]
    run(lab(binary, cfg, 'render', '--timeline', cfg.timeline(), '--resolution', f'{resolution[0]}x{resolution[1]}', '--snapshot', ','.join(map(str, frames)),
            '--snapshot-dir', shots, '--ffmpeg', env.ffmpeg),
        build / 'logs' / 'snapshots.log')
    chosen: list[Path] = []
    for chapter in timeline['chapters']:
        # The contact sheet holds one still per chapter: its middle 3D beat's.
        beats = [b for b in timeline['beats'] if b['chapter'] == chapter['number'] and b['kind'] == '3d']
        if beats:
            b = beats[len(beats) // 2]
            chosen.append(shots / f'frame-{b["startFrame"] + b["frames"] // 2:06d}.png')
    # Scale each still to 480x270 and tile them four to a row.
    inputs: list[Any] = []
    for p in chosen:
        inputs += ['-i', p]
    layout = '|'.join(f'{480 * (i % 4)}_{270 * (i // 4)}' for i in range(len(chosen)))
    scaled = ''.join(f'[{i}]scale=480:270[s{i}];' for i in range(len(chosen)))
    stack = ''.join(f'[s{i}]' for i in range(len(chosen)))
    run([env.ffmpeg, '-hide_banner', '-loglevel', 'error', '-y', *inputs, '-filter_complex', f'{scaled}{stack}xstack=inputs={len(chosen)}:layout={layout}', '-frames:v', '1', build / 'contact-sheet.png'],
        build / 'logs' / 'contact.log')
    shutil.copy(chosen[0], build / 'poster.png')
    return sorted(shots.glob('frame-*.png'))


# ---------------------------------------------------------------------------
# Listings and links

def script_text(cfg: Config, sentence: Json) -> str:
    """A sentence as the script shows it: its tags gone, its formulas as `$LaTeX$`."""
    import markup
    return markup.readable(markup.shown(sentence, cfg.zh()).caption)


def source_link(cfg: Config, doc_dir: Path, index: Json, key: str, text: str | None = None) -> str:
    """An entry's symbol (or `text`) and where it is in the engine's source,
    as text: the source is wherever the reader keeps it, so no link."""
    entry = index['entries'][key]
    return f'{text or "`" + entry["symbol"] + "`"} (`{place(entry)}`)'


def place(entry: Json) -> str:
    """Where an entry is: `path:line` in a checkout, or `§section, p. page` in a paper (lab/src/sources.rs)."""
    if 'page' in entry:
        return f'§{entry["section"]}, p. {entry["page"]}' if 'section' in entry else f'p. {entry["page"]}'
    return f'{entry["path"]}:{entry["line"]}'


def edition(cfg: Config, index: Json, date: str) -> str:
    """The source the index was read from, in a sentence: the paper, or the engine's version and commit."""
    w = WORDS[cfg.lang]
    if 'paper' in index:
        paper = index['paper']
        return w['paper'].format(title=paper['title'], short=paper['short'], venue=paper['venue'], doi=paper['doi'], date=date)
    engine = index['engine']
    return w['engine'].format(engine=engine['name'], version=engine['version'], commit=engine['commit'], date=date)


def repo_link(doc_dir: Path, path: Path, text: str) -> str:
    return f'[{text}]({relpath(path, doc_dir)})'


def listing_markdown(cfg: Config, name: str, doc_dir: Path, index: Json, shown: set[str]) -> list[str]:
    """The listing as a Python code block the first time; later, a pointer back to it."""
    title, source, code = read_listing(cfg, name)
    if name in shown:
        # A listing printed earlier in the document is pointed back to, not printed again.
        return [WORDS[cfg.lang]['seeAbove'].format(title=title), '']
    shown.add(name)
    link = source_link(cfg, doc_dir, index, source) if source else ''
    return [f'**{WORDS[cfg.lang]["listing"]}: {title}** (≈ {link})', '', '```python', code, '```', '']


# ---------------------------------------------------------------------------
# Documents

def write_script(cfg: Config, board: Json, index: Json, timeline: Json) -> None:
    w = WORDS[cfg.lang]
    fps = timeline['fps']
    doc_dir = cfg.build()
    joiner = '' if cfg.zh() else ' '
    lines = [f'# {board["title"]}', '', board['subtitle'], '',
             edition(cfg, index, board['date']), '',
             '> ' + cfg.text('intro'), '', f'## {w["chapters"]}', '']
    for c in timeline['chapters']:
        # The table of contents: each chapter's start and a link to its section.
        lines.append(f'- {clock(c["startFrame"] / fps)} [{c["number"]:02d} · {c["title"]}](#chapter-{c["number"]})')
    lines.append('')
    beat_frames = {b['id']: b for b in timeline['beats']}
    shown: set[str] = set()
    demos = cfg.text('demos')
    for chapter in board['chapters']:
        # Then each chapter: its start and budget, and each beat's headline, kind, sentences, and listings.
        tc = next(c for c in timeline['chapters'] if c['number'] == chapter['number'])
        lines += [f'<a id="chapter-{chapter["number"]}"></a>', f'## {chapter["number"]:02d} · {chapter["title"]}', '',
                  w['starts'].format(when=clock(tc['startFrame'] / fps), seconds=(tc['endFrame'] - tc['startFrame']) / fps, budget=chapter['targetSeconds']) + ' ' + chapter['summary'], '']
        for beat in chapter['beats']:
            when = clock(beat_frames[beat['id']]['startFrame'] / fps)
            lines += [f'### {beat["headline"]}', '']
            if 'graph' in beat:
                # A beat says what it shows: the frame graph, a diagram, a demonstration, or a source card.
                lines += [f'*{when} · {w["graph"]} `{beat["graph"]}.toml`*', ''] + frame_graph.markdown(frame_graph.load(cfg, beat['graph']))
            elif presentation.is_diagram(beat):
                lines += [f'*{when} · {w["diagram"]} `{beat["diagram"]}.toml`*', ''] + diagrams.markdown(diagrams.load(cfg, beat['diagram']))
            elif presentation.is_3d(beat):
                demo = next((k['demo'] for k in beat.get('keys', []) if 'demo' in k), None)
                lines += [f'*{when} · {w["demo"]}{": " + demos.get(demo, demo) if demo else ""}*', '']
            else:
                lines += [f'*{when} · {w["card"]}*', '']
            lines += [joiner.join(script_text(cfg, s) for s in sentences_of(beat)), '']
            for name in beat_listings(beat):
                lines += listing_markdown(cfg, name, doc_dir, index, shown)
            if presentation.is_card(beat):
                # A source card's entries are listed with what to inspect in each.
                for key in beat.get('sources', []):
                    entry = index['entries'][key]
                    inspect = source_text(cfg, f'entry-{key}-inspect', entry['inspect'])
                    at = f'`{entry["path"].rsplit("/", 1)[-1]}` {w["line"].format(line=entry["line"])}' if 'page' not in entry else f'`{place(entry)}`'
                    lines.append(f'- {source_link(cfg, doc_dir, index, key)} — {at}: {inspect}')
                lines.append('')
        lines += [w['sources'] + w['sep'].join(source_link(cfg, doc_dir, index, k) for k in chapter['sources']) + '.', '']
    build = cfg.build()
    # Every diagram may be the frame graph (drawn from the lab's graph), so the directory may not exist.
    diagram_dir = f' · {repo_link(doc_dir, cfg.diagrams, "diagrams/")}' if cfg.diagrams.is_dir() and any(cfg.diagrams.glob('*.toml')) else ''
    lines += [f'## {w["files"]}', '',
              f'- {repo_link(doc_dir, cfg.video(), cfg.video_name())} · {repo_link(doc_dir, build / "captions.srt", "captions.srt")} · {repo_link(doc_dir, build / "captions.vtt", "captions.vtt")} · {repo_link(doc_dir, build / "chapters.txt", "chapters.txt")}',
              f'- {repo_link(doc_dir, cfg.storyboard, cfg.storyboard.name)} · {repo_link(doc_dir, cfg.source_index, "source-index.json")}{diagram_dir} · {repo_link(doc_dir, cfg.pseudocode, "pseudocode/")}', '']
    (build / 'script.md').write_text('\n'.join(lines) + '\n')
    # script.txt: the narration alone, chapter by chapter.
    plain: list[str] = []
    for chapter in board['chapters']:
        plain.append(f'{chapter["number"]:02d}. {chapter["title"]}\n')
        for beat in chapter['beats']:
            plain.append(joiner.join(script_text(cfg, s) for s in sentences_of(beat)) + '\n')
    (build / 'script.txt').write_text('\n'.join(plain) + '\n')


def write_guide(cfg: Config, board: Json, index: Json, timeline: Json) -> None:
    w = WORDS[cfg.lang]
    build = cfg.build()
    doc_dir = build
    engine = index['engine']
    # A paper's guide names the paper and its pages; an engine's, its commit and the checkout's paths.
    source = [f'- {edition(cfg, index, board["date"])}'.rstrip(), w['pages']] if 'paper' in index else [f'- {engine["name"]} {engine["version"]}, commit `{engine["commit"]}`.', w['checkout']]
    lines = [f'# {cfg.text("guideTitle")}', '',
             w['guideIntro'].format(video=repo_link(doc_dir, cfg.video(), cfg.video_name()), script=repo_link(doc_dir, build / 'script.md', 'script.md')), '',
             *source,
             w['index'].format(index=repo_link(doc_dir, cfg.source_index, relpath(cfg.source_index, LAB))), '',
             f'## {w["readingOrder"]}', '']
    for i, key in enumerate(cfg.spec['readingPath'], 1):
        # The reading order first, then a table per chapter of each entry to read.
        entry = index['entries'][key]
        lines.append(f'{i}. {source_link(cfg, doc_dir, index, key)}' + (f' · `{entry["path"].rsplit("/", 1)[-1]}`' if 'page' not in entry else ''))
    lines.append('')
    for chapter in index['chapters']:
        title = source_text(cfg, f'chapter-{chapter["number"]}-title', chapter['title'])
        lines += [f'## {chapter["number"]}. {title}', '', w['table'], '|---|---|---:|---|']
        for key in chapter['reading']:
            entry = index['entries'][key]
            inspect = source_text(cfg, f'entry-{key}-inspect', entry['inspect'])
            notes = [(d.get('page', d.get('line')), source_text(cfg, f'entry-{key}-detail-{n}', d['note'])) for n, d in enumerate(entry.get('details', []), 1)]
            if notes:
                # An entry's notes on particular lines follow what to inspect, each with its line.
                inspect += ' (' + '; '.join(w['line'].format(line=line) + ': ' + note for line, note in notes) + ')'
            file_link = f'`{entry["path"]}`' if 'page' not in entry else f'`§{entry.get("section", "")}`'
            lines.append(f'| {file_link} | `{entry["symbol"]}` | {entry.get("page", entry.get("line"))} | {inspect} |')
        lines.append('')
    if index.get('mappings'):
        # What the lab's data stands for in the engine, when the index says.
        lines += [f'## {w["mappings"]}', '', w['mappingTable'], '|---|---|---|---|---|']
        for m in index['mappings']:
            name, exact, lab = (source_text(cfg, f'mapping-{source_key(m["lab"])}-{field}', m[english]) for field, english in (('engine-name', 'engineName'), ('exact', 'exact'), ('lab-layout', 'labLayout')))
            cited = ', '.join(source_link(cfg, doc_dir, index, key) for key in m['sources'])
            lines.append(f'| `{m["lab"]}` | {name} ({cited}) | {m["relationship"]} | {exact} | {lab} |')
        lines.append('')
    lines += [f'## {w["simplify"]}', ''] + [f'- {item}' for item in cfg.text('simplifications')] + ['']
    (build / 'source-reading-guide.md').write_text('\n'.join(lines) + '\n')


def write_notice(cfg: Config) -> None:
    """NOTICE.txt beside the video: the package's legal notices, then the full text of each one's terms."""
    notices = cfg.notices()
    out = cfg.build() / 'NOTICE.txt'
    if not notices:
        # A package with no notices gets no NOTICE.txt (an old one is removed).
        out.unlink(missing_ok=True)
        return
    parts = [cfg.lab_text('ui-notices_title'), ''] + [text for text, _ in notices]
    for text, license in notices:
        if license is not None:
            # Each notice with a license file is followed by that license's full text.
            parts += ['', '-' * 78, license.name, '-' * 78, '', license.read_text(encoding='utf-8').rstrip()]
    out.write_text('\n'.join(parts) + '\n', encoding='utf-8')


def write_preview(cfg: Config, board: Json, timeline: Json) -> None:
    w = WORDS[cfg.lang]
    fps = timeline['fps']
    buttons = '\n'.join(f'<button data-time="{c["startFrame"] / fps:.3f}">{c["number"]:02d} · {html.escape(c["title"])}</button>' for c in timeline['chapters'])
    lang = 'zh' if cfg.zh() else 'en'
    label = '中文' if cfg.zh() else 'English'
    page = f'''<!doctype html><html lang="{cfg.lang}"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{html.escape(board["title"])}</title>
<style>body{{margin:0;background:#091421;color:#eef5ff;font:17px system-ui,'Source Han Sans CN',sans-serif;max-width:1200px;padding:32px;margin:auto}}video{{width:100%;background:#000;border-radius:12px}}h1{{font-size:32px}}p{{color:#a9bfd4}}nav{{display:grid;grid-template-columns:repeat(auto-fit,minmax(270px,1fr));gap:8px;margin:20px 0}}button{{background:#163044;color:#d9f4f0;border:1px solid #35576a;border-radius:8px;padding:12px;text-align:left;cursor:pointer;font:inherit}}a{{color:#70e1d2}}</style>
<h1>{html.escape(board["title"])}</h1><p>{html.escape(board["subtitle"])}</p>
<video controls preload="metadata" poster="poster.png"><source src="{cfg.video_name()}" type="video/mp4"><track kind="captions" src="captions.vtt" srclang="{lang}" label="{label}"{" default" if cfg.zh() else ""}></video>
<nav>{buttons}</nav><p><a href="script.md">{w["script"]}</a> · <a href="source-reading-guide.md">{w["guide"]}</a> · <a href="verification-report.md">{w["verification"]}</a></p>
{"".join(f"<p>{html.escape(text)}</p>" for text, _ in cfg.notices())}{'<p><a href="NOTICE.txt">NOTICE.txt</a></p>' if any(lic for _, lic in cfg.notices()) else ""}
<script>const v=document.querySelector('video');for(const b of document.querySelectorAll('button'))b.onclick=()=>{{v.currentTime=+b.dataset.time;v.play();}};</script></html>'''
    (cfg.build() / 'preview.html').write_text(page)
