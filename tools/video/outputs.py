"""Everything a build writes besides the timeline: the narration track,
captions, chapters, the diagram frames, the rendered and muxed video, the
review stills, and the documents (script, source-reading guide, preview).
"""
from __future__ import annotations

import html
import json
import os
import re
import shutil
import subprocess
import time
import wave
from pathlib import Path
from typing import Any

from config import LAB, TOOLS, Config, Environment, log, run
from timeline import sentences_of

Json = dict[str, Any]

# Words the documents use, per edition.
WORDS: dict[str, dict[str, str]] = {
    'en': {
        'chapters': 'Chapters', 'starts': 'Starts at {when} · {seconds:.0f} s (budget {budget} s).', 'diagram': 'diagram', 'demo': '3D demonstration',
        'card': 'source card', 'sources': 'Sources for this chapter: ', 'sep': ', ', 'line': 'line {line}', 'files': 'Files', 'listing': 'Python pseudo-code',
        'seeAbove': '*(Python pseudo-code “{title}”: see above.)*', 'engine': 'Engine source: {engine} {version}, commit `{commit}`. Prepared {date}.',
        'readingOrder': 'Recommended reading order', 'table': '| File | Symbol | Line | What to inspect |', 'simplify': 'What the demonstrations simplify', "mappings": "What the lab's data stands for", "mappingTable": "| Lab | Engine structure | Relationship | The engine's layout | The lab's |",
        'guideIntro': 'This guide follows the chapters of the video ({video}, script: {script}). For each topic it names the file, the symbol, the line to start reading at, and what to look for there.',
        'checkout': '- Checkout: `{checkout}` relative to `render-lab/`; source paths are relative to its root. Links are relative and end in `#L<line>`.',
        'index': '- Every entry comes from one machine-readable index, {index}, which also drives the video\'s source cards and the viewer\'s source panel. The build checks each line against the checkout.',
        'video': 'video', 'script': 'Narrated script', 'guide': 'Source-reading guide', 'verification': 'Verification',
    },
    'zh-CN': {
        'chapters': '章节', 'starts': '开始于 {when} · {seconds:.0f} 秒（预算 {budget} 秒）。', 'diagram': '图表', 'demo': '3D 演示',
        'card': '源码卡片', 'sources': '本章源码：', 'sep': '、', 'line': '第 {line} 行', 'files': '文件', 'listing': 'Python 伪代码',
        'seeAbove': '*（Python 伪代码“{title}”见上文。）*', 'engine': '引擎源码：{engine} {version}，commit `{commit}`。准备于 {date}。',
        'readingOrder': '推荐阅读顺序', 'table': '| 文件 | 符号 | 行 | 要看什么 |', 'simplify': '演示做了哪些简化', 'mappings': '实验室的数据对应什么', 'mappingTable': '| 实验室 | 引擎结构 | 关系 | 引擎的布局 | 实验室的布局 |',
        'guideIntro': '本指南按照视频（{video}，脚本：{script}）的章节组织。每个主题都给出文件、符号、开始阅读的行，以及在那里要看什么。',
        'checkout': '- 源码目录：相对于 `render-lab/` 为 `{checkout}`；源码路径相对于该目录的根。链接都是相对路径，并以 `#L<行号>` 结尾。',
        'index': '- 每个条目都来自同一个机器可读的索引 {index}，它同时驱动视频中的源码卡片和查看器的源码面板。构建过程会对照源码检查每一行。',
        'video': '视频', 'script': '旁白脚本', 'guide': '源码阅读指南', 'verification': '验证报告',
    },
}


def stamp(seconds: float, ms: bool = True) -> str:
    unit = 1000 if ms else 100
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
    samples = bytearray(int(round(total_frames / fps * rate)) * 2)
    for p in placements:
        with wave.open(p['path'], 'rb') as stream:
            assert stream.getframerate() == rate and stream.getnchannels() == 1 and stream.getsampwidth() == 2
            data = stream.readframes(stream.getnframes())
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
    text = []
    for c in timeline['chapters']:
        meta += ['[CHAPTER]', 'TIMEBASE=1/1000', f'START={round(c["startFrame"] / fps * 1000)}', f'END={round(c["endFrame"] / fps * 1000)}', f'title={c["number"]:02d} {c["title"]}']
        text.append(f'{clock(c["startFrame"] / fps)} {c["number"]:02d} {c["title"]}')
    (cfg.build() / 'chapters.ffmeta').write_text('\n'.join(meta) + '\n')
    (cfg.build() / 'chapters.txt').write_text('\n'.join(text) + '\n')


# ---------------------------------------------------------------------------
# Rendering

def render_mermaid(cfg: Config, env: Environment, board: Json) -> list[Json]:
    build = cfg.build()
    jobs = [{'id': beat['id'], 'diagram': str(cfg.diagrams / f'{beat["diagram"]}.mmd'), 'focus': beat.get('focus', []),
             'output': str(cfg.timeline_dir() / 'mermaid' / f'{beat["id"]}.png')}
            for chapter in board['chapters'] for beat in chapter['beats'] if beat['kind'] == 'mermaid']
    if not jobs:
        return []
    spec: Json = {'width': 1840, 'height': 728, 'jobs': jobs, 'metrics': str(build / 'mermaid-metrics.json'), 'nodeModules': str(env.node_modules)}
    if cfg.zh():
        # Chinese glyphs fall back to Source Han Sans, the font the lab bundles.
        spec['fontFamily'] = "'DejaVu Sans', 'Source Han Sans CN', 'Noto Sans CJK SC'"
    (build / 'mermaid-jobs.json').write_text(json.dumps(spec, indent=1))
    process_env = dict(os.environ, PUPPETEER_CACHE_DIR=str(env.puppeteer_cache))
    run([env.node, TOOLS / 'mermaid.mjs', build / 'mermaid-jobs.json'], build / 'logs' / 'mermaid.log', env=process_env)
    return json.loads((build / 'mermaid-metrics.json').read_text())


def cargo_build(cfg: Config) -> Path:
    log('Building render-lab (cargo build --release --locked --offline).')
    process_env = dict(os.environ, CARGO_HOME=str(LAB / '.cache' / 'cargo'), CARGO_TARGET_DIR=str(LAB / 'target'))
    run(['cargo', 'build', '--release', '--locked', '--offline'], cfg.build() / 'logs' / 'cargo-build.log', env=process_env)
    return LAB / 'target' / 'release' / 'render-lab'


def lab(binary: Path, cfg: Config, *args: Any) -> list[Any]:
    """A render-lab command line for this investigation."""
    return [binary, '--investigation', cfg.investigation, *args]


def render_video(cfg: Config, env: Environment, binary: Path, frames: str | None, resolution: tuple[int, int]) -> None:
    build = cfg.build()
    args = lab(binary, cfg, 'render', '--timeline', cfg.timeline(), '--resolution', f'{resolution[0]}x{resolution[1]}', '--output', build / 'video.mp4',
               '--report', build / 'render-report.json', '--ffmpeg', env.ffmpeg, '--crf', '18', '--preset', 'medium')
    if frames:
        args += ['--frames', frames]
    log('Rendering the timeline with render-lab (every frame streams to FFmpeg).')
    started = time.time()
    proc = subprocess.Popen([str(a) for a in args], cwd=LAB, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    lines: list[str] = []
    assert proc.stdout is not None
    for line in proc.stdout:
        lines.append(line)
        if line.startswith(('frame', 'rendered', 'GPU')):
            log('  ' + line.rstrip())
    proc.wait()
    (build / 'logs' / 'render.log').write_text(''.join(lines))
    if proc.returncode:
        raise RuntimeError('render-lab render failed:\n' + ''.join(lines[-40:]))
    log(f'  render finished in {time.time() - started:.0f} s')


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
        beats = [b for b in timeline['beats'] if b['chapter'] == chapter['number'] and b['kind'] == 'scene']
        if beats:
            b = beats[len(beats) // 2]
            chosen.append(shots / f'frame-{b["startFrame"] + b["frames"] // 2:06d}.png')
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
    return markup.readable(markup.parse(sentence['text'], spoken_only=cfg.zh()).caption)


def beat_listings(beat: Json) -> list[str]:
    """Pseudo-code listings a beat shows, in order of first use."""
    import markup
    names: list[str] = []
    for item in [beat] + sentences_of(beat):
        tagged = markup.text_keys(beat, item) if item is not beat else []
        for k in item.get('keys', []) + item.get('sequence', []) + tagged:
            if k.get('code') and k['code'] not in names:
                names.append(k['code'])
    return names


def listing_path(cfg: Config, name: str) -> Path:
    return cfg.pseudocode / (name.replace('-', '_') + '.py')


def read_listing(cfg: Config, name: str) -> tuple[str, str, str | None, str]:
    """(title, Chinese title, source key, code without tags) of a listing."""
    header: dict[str, str] = {}
    code: list[str] = []
    for line in listing_path(cfg, name).read_text().splitlines():
        m = re.match(r'# (title|source|demo): (.*)$', line)
        if m and not code:
            header[m.group(1)] = m.group(2).strip()
            continue
        code.append(re.sub(r'\s*# @[^\n]*$', '', line).rstrip())
    while code and not code[-1]:
        code.pop()
    title, _, title_zh = header.get('title', name).partition(' | ')
    return title, title_zh or title, header.get('source'), '\n'.join(code)


def source_link(cfg: Config, doc_dir: Path, index: Json, key: str, text: str | None = None) -> str:
    entry = index['entries'][key]
    target = LAB / index['engine']['checkoutFromLabRoot'] / entry['path']
    return f'[{text or "`" + entry["symbol"] + "`"}]({relpath(target, doc_dir)}#L{entry["line"]})'


def repo_link(doc_dir: Path, path: Path, text: str) -> str:
    return f'[{text}]({relpath(path, doc_dir)})'


def listing_markdown(cfg: Config, name: str, doc_dir: Path, index: Json, shown: set[str]) -> list[str]:
    """The listing as a Python code block the first time; later, a pointer back to it."""
    title, title_zh, source, code = read_listing(cfg, name)
    title = title_zh if cfg.zh() else title
    if name in shown:
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
    engine = index['engine']
    joiner = '' if cfg.zh() else ' '
    lines = [f'# {board["title"]}', '', board['subtitle'], '',
             w['engine'].format(engine=engine['name'], version=engine['version'], commit=engine['commit'], date=board['date']), '',
             '> ' + cfg.text('intro'), '', f'## {w["chapters"]}', '']
    for c in timeline['chapters']:
        lines.append(f'- {clock(c["startFrame"] / fps)} [{c["number"]:02d} · {c["title"]}](#chapter-{c["number"]})')
    lines.append('')
    beat_frames = {b['id']: b for b in timeline['beats']}
    shown: set[str] = set()
    demos = cfg.text('demos')
    for chapter in board['chapters']:
        tc = next(c for c in timeline['chapters'] if c['number'] == chapter['number'])
        lines += [f'<a id="chapter-{chapter["number"]}"></a>', f'## {chapter["number"]:02d} · {chapter["title"]}', '',
                  w['starts'].format(when=clock(tc['startFrame'] / fps), seconds=(tc['endFrame'] - tc['startFrame']) / fps, budget=chapter['targetSeconds']) + ' ' + chapter['summary'], '']
        for beat in chapter['beats']:
            when = clock(beat_frames[beat['id']]['startFrame'] / fps)
            lines += [f'### {beat["headline"]}', '']
            if beat['kind'] == 'mermaid':
                lines += [f'*{when} · {w["diagram"]} `{beat["diagram"]}.mmd`*', '', '```mermaid', (cfg.diagrams / f'{beat["diagram"]}.mmd').read_text().strip(), '```', '']
            elif beat['kind'] == 'scene':
                demo = next((k['demo'] for k in beat.get('keys', []) if 'demo' in k), None)
                lines += [f'*{when} · {w["demo"]}{": " + demos.get(demo, demo) if demo else ""}*', '']
            else:
                lines += [f'*{when} · {w["card"]}*', '']
            lines += [joiner.join(script_text(cfg, s) for s in sentences_of(beat)), '']
            for name in beat_listings(beat):
                lines += listing_markdown(cfg, name, doc_dir, index, shown)
            if beat['kind'] == 'sources':
                for key in beat.get('sources', []):
                    entry = index['entries'][key]
                    inspect = entry.get('inspectZh', entry['inspect']) if cfg.zh() else entry['inspect']
                    lines.append(f'- {source_link(cfg, doc_dir, index, key)} — `{entry["path"].rsplit("/", 1)[-1]}` {w["line"].format(line=entry["line"])}: {inspect}')
                lines.append('')
        lines += [w['sources'] + w['sep'].join(source_link(cfg, doc_dir, index, k) for k in chapter['sources']) + '.', '']
    build = cfg.build()
    lines += [f'## {w["files"]}', '',
              f'- {repo_link(doc_dir, cfg.video(), cfg.video_name())} · {repo_link(doc_dir, build / "captions.srt", "captions.srt")} · {repo_link(doc_dir, build / "captions.vtt", "captions.vtt")} · {repo_link(doc_dir, build / "chapters.txt", "chapters.txt")}',
              f'- {repo_link(doc_dir, cfg.storyboard, cfg.storyboard.name)} · {repo_link(doc_dir, cfg.source_index, "source-index.json")} · {repo_link(doc_dir, cfg.diagrams, "diagrams/")} · {repo_link(doc_dir, cfg.pseudocode, "pseudocode/")}', '']
    (build / 'script.md').write_text('\n'.join(lines) + '\n')
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
    lines = [f'# {cfg.text("guideTitle")}', '',
             w['guideIntro'].format(video=repo_link(doc_dir, cfg.video(), cfg.video_name()), script=repo_link(doc_dir, build / 'script.md', 'script.md')), '',
             f'- {engine["name"]} {engine["version"]}, commit `{engine["commit"]}`.',
             w['checkout'].format(checkout=engine['checkoutFromLabRoot']),
             w['index'].format(index=repo_link(doc_dir, cfg.source_index, relpath(cfg.source_index, LAB))), '',
             f'## {w["readingOrder"]}', '']
    for i, key in enumerate(cfg.spec['readingPath'], 1):
        entry = index['entries'][key]
        lines.append(f'{i}. {source_link(cfg, doc_dir, index, key)} · `{entry["path"].rsplit("/", 1)[-1]}`')
    lines.append('')
    for chapter in index['chapters']:
        title = chapter.get('titleZh', chapter['title']) if cfg.zh() else chapter['title']
        lines += [f'## {chapter["number"]}. {title}', '', w['table'], '|---|---|---:|---|']
        for key in chapter['reading']:
            entry = index['entries'][key]
            inspect = entry.get('inspectZh', entry['inspect']) if cfg.zh() else entry['inspect']
            notes = [(d['line'], d.get('noteZh', d['note']) if cfg.zh() else d['note']) for d in entry.get('details', [])]
            if notes:
                inspect += ' (' + '; '.join(w['line'].format(line=line) + ': ' + note for line, note in notes) + ')'
            file_link = source_link(cfg, doc_dir, index, key, f'`{entry["path"].rsplit("/", 1)[-1]}`')
            lines.append(f'| {file_link} | `{entry["symbol"]}` | {entry["line"]} | {inspect} |')
        lines.append('')
    if index.get('mappings'):
        lines += [f'## {w["mappings"]}', '', w['mappingTable'], '|---|---|---|---|---|']
        for m in index['mappings']:
            name, exact, lab = (m.get('engineNameZh', m['engineName']), m.get('exactZh', m['exact']), m.get('labLayoutZh', m['labLayout'])) if cfg.zh() else (m['engineName'], m['exact'], m['labLayout'])
            cited = ', '.join(source_link(cfg, doc_dir, index, key) for key in m['sources'])
            lines.append(f'| `{m["lab"]}` | {name} ({cited}) | {m["relationship"]} | {exact} | {lab} |')
        lines.append('')
    lines += [f'## {w["simplify"]}', ''] + [f'- {item}' for item in cfg.text('simplifications')] + ['']
    (build / 'source-reading-guide.md').write_text('\n'.join(lines) + '\n')


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
<script>const v=document.querySelector('video');for(const b of document.querySelectorAll('button'))b.onclick=()=>{{v.currentTime=+b.dataset.time;v.play();}};</script></html>'''
    (cfg.build() / 'preview.html').write_text(page)
