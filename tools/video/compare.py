"""Compare two sets of prepared video outputs (`build.py --prepare-only`), to check that a change to the
storyboards or the pipeline changes nothing the video says or shows.

    python3 tools/video/compare.py save OUT [--investigation o3de/lighting ...]   # copy every edition's prepared outputs
    python3 tools/video/compare.py OLD NEW                                        # compare two saved sets

`save` copies, for each video the packages define (or those named) and each of its editions, what
`--prepare-only` writes (`PREPARED`): the timeline, the chapter plans, the captions, the chapters, the documents,
and the narration track, under `OUT/<package>/<lang>/`, with the names of the cached narration clips
(`OUT/<package>/audio.txt`). Comparing two sets:

- JSON files (the timeline, the chapter plans) are equal by value: numbers as numbers (Lamlet writes `18` as
  `18.0`), objects without regard to key order;
- every other file is equal byte for byte (the narration track by its hash);
- the cached clips of NEW are those of OLD: nothing was synthesized anew.

It prints each difference (the first few per file) and exits 1 when there is any.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from build import videos  # noqa: E402
from config import Config  # noqa: E402

# What `build.py --prepare-only` writes in an edition's build directory (beside its timeline).
PREPARED = ('chapter-plans.json', 'captions.srt', 'captions.vtt', 'chapters.ffmeta', 'chapters.txt', 'script.md', 'script.txt',
            'source-reading-guide.md', 'preview.html', 'NOTICE.txt', 'narration.wav')
# Differences shown per file.
SHOWN = 8


def save_edition(cfg: Config, out: Path, *, required: bool = False) -> Path:
    """Copy one edition. A regression capture requires every prepared output, never a partial build."""
    sources = [(cfg.timeline(), 'timeline.json'), *((cfg.build() / name, name) for name in PREPARED)]
    if required:
        missing = [str(path) for path, _ in sources if not path.is_file()]
        if missing:
            raise RuntimeError('missing prepared outputs:\n' + '\n'.join(missing))
    dest = out / cfg.package.name / cfg.lang
    dest.mkdir(parents=True, exist_ok=True)
    for path, name in sources:
        if path.is_file():
            shutil.copy2(path, dest / name)
    clips = sorted(p.name for p in cfg.audio_cache.glob('*') if p.is_file())
    (out / cfg.package.name / 'audio.txt').write_text('\n'.join(clips) + '\n')
    return dest


def save(out: Path, investigations: list[str]) -> None:
    """Copies every edition's prepared outputs under `out`."""
    for investigation, langs in videos():
        if investigations and investigation not in investigations:
            continue
        for lang in langs:
            cfg = Config.load(investigation, lang)
            if not cfg.timeline().exists():
                continue
            save_edition(cfg, out)
        print(f'saved {investigation}')


def same(a: Any, b: Any, path: str, out: list[str]) -> None:
    """The differences between two JSON values, numbers compared as numbers."""
    if isinstance(a, bool) or isinstance(b, bool):
        if a is not b:
            out.append(f'{path}: {a!r} != {b!r}')
    elif isinstance(a, (int, float)) and isinstance(b, (int, float)):
        if a != b:
            out.append(f'{path}: {a!r} != {b!r}')
    elif isinstance(a, dict) and isinstance(b, dict):
        for k in sorted(set(a) | set(b)):
            if k not in a:
                out.append(f'{path}.{k}: only in new ({json.dumps(b[k])[:80]})')
            elif k not in b:
                out.append(f'{path}.{k}: only in old ({json.dumps(a[k])[:80]})')
            else:
                same(a[k], b[k], f'{path}.{k}', out)
    elif isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            out.append(f'{path}: {len(a)} items != {len(b)}')
        for i, (x, y) in enumerate(zip(a, b)):
            same(x, y, f'{path}[{i}]', out)
    elif a != b:
        out.append(f'{path}: {json.dumps(a)[:80]} != {json.dumps(b)[:80]}')


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def file_differences(old: Path, new: Path) -> list[str]:
    if old.suffix == '.json':
        out: list[str] = []
        same(json.loads(old.read_text()), json.loads(new.read_text()), '', out)
        return out
    if old.name == 'audio.txt':
        added = sorted(set(new.read_text().split()) - set(old.read_text().split()))
        return [f'synthesized anew: {", ".join(added[:SHOWN])}{" …" if len(added) > SHOWN else ""}'] if added else []
    if old.read_bytes() == new.read_bytes():
        return []
    if old.suffix == '.wav':
        return [f'sha256 {digest(old)[:16]} != {digest(new)[:16]}']
    a, b = old.read_text().splitlines(), new.read_text().splitlines()
    lines = [f'line {i + 1}: {x!r} != {y!r}' for i, (x, y) in enumerate(zip(a, b)) if x != y]
    if len(a) != len(b):
        lines.append(f'{len(a)} lines != {len(b)}')
    return lines or ['bytes differ (line endings or final newline)']


def compare(old: Path, new: Path) -> list[str]:
    """Every difference between two saved sets, as lines."""
    out: list[str] = []
    files_old = {p.relative_to(old) for p in old.rglob('*') if p.is_file()}
    files_new = {p.relative_to(new) for p in new.rglob('*') if p.is_file()}
    for rel in sorted(files_old - files_new):
        out.append(f'{rel}: only in {old}')
    for rel in sorted(files_new - files_old):
        out.append(f'{rel}: only in {new}')
    for rel in sorted(files_old & files_new):
        diffs = file_differences(old / rel, new / rel)
        out.extend(f'{rel}: {d}' for d in diffs[:SHOWN])
        if len(diffs) > SHOWN:
            out.append(f'{rel}: … {len(diffs) - SHOWN} more')
    return out


def main() -> None:
    if len(sys.argv) > 1 and sys.argv[1] == 'save':
        parser = argparse.ArgumentParser(description='copy every edition\'s prepared outputs')
        parser.add_argument('command')
        parser.add_argument('out', type=Path)
        parser.add_argument('--investigation', action='append', default=[])
        args = parser.parse_args()
        save(args.out, args.investigation)
        return
    parser = argparse.ArgumentParser(description=(__doc__ or '').split('\n\n')[0])
    parser.add_argument('old', type=Path)
    parser.add_argument('new', type=Path)
    args = parser.parse_args()
    differences = compare(args.old, args.new)
    for line in differences:
        print(line)
    print(f'{len(differences)} difference(s)' if differences else 'the same')
    sys.exit(1 if differences else 0)


if __name__ == '__main__':
    main()
