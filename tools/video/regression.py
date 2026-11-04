"""Capture and compare the public Lamlet consumer corpus, without rendering or synthesizing speech.

    python3 tools/video/regression.py save /tmp/before --binary target/release/render-lab
    python3 tools/video/regression.py save /tmp/after --binary /path/to/candidate/render-lab
    python3 tools/video/regression.py compare /tmp/before /tmp/after

Each host must have its matching package libraries beside it. Prepare the narration cache first. Both hosts
read this checkout's content; committed dependency pins stay unchanged. A snapshot includes the real document
IR, prepared timeline/captions/documents/audio, script hook checks, input hashes and narration cache hashes.
It checks compilation and preparation, not GPU output or the behavior of live hooks.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import compare  # noqa: E402
import lamlet  # noqa: E402
from config import LAB, LANGS, Config  # noqa: E402

# An explicit, public corpus: adding unrelated local packages does not change what this gate checks.
CORPUS = {'godot/ssr': 'godot_ssr', 'godot/sdfgi': 'godot_sdfgi',
          'godot/voxelgi': 'godot_voxelgi', 'o3de/lighting': 'o3de_lighting'}
VERSION = 1


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=1, sort_keys=True, allow_nan=False) + '\n', encoding='utf-8')


def configurations(investigations: list[str]) -> list[list[Config]]:
    cases: list[list[Config]] = []
    for investigation in investigations:
        package = LAB / 'packages' / CORPUS[investigation]
        english = Config.load(investigation, 'en', package)
        languages = ['en', *english.spec.get('translations', {})]
        if any(lang not in LANGS for lang in languages):
            raise RuntimeError(f'{investigation}: unsupported corpus language {languages}')
        cases.append([Config.load(investigation, lang, package) for lang in languages])
    return cases


def execute(command: list[str], log: Path) -> str:
    env = dict(os.environ, RENDER_LAB_ROOT=str(LAB))
    env.pop('RENDER_LAB_PACKAGES', None)
    result = subprocess.run(command, cwd=LAB, env=env, capture_output=True, text=True)
    log.write_text(result.stdout + result.stderr, encoding='utf-8')
    if result.returncode:
        raise RuntimeError(f'command failed ({result.returncode}); {log}\n{result.stderr[-4000:]}')
    return result.stdout


def input_hashes(investigations: list[str]) -> dict[str, str]:
    roots = ['packages/render_lab', *(f'packages/{CORPUS[i]}' for i in investigations), 'tools/video']
    result = subprocess.run(['git', 'ls-files', '-z', '--', *roots], cwd=LAB, capture_output=True, check=True)
    names = [name for name in result.stdout.decode().split('\0') if name]
    if not names:
        raise RuntimeError('the corpus must be captured from a source checkout')
    return {name: compare.digest(LAB / name) for name in names}


def capture(out: Path, binary: Path, investigations: list[str]) -> None:
    binary = binary.resolve()
    if not binary.is_file():
        raise RuntimeError(f'missing host binary: {binary}; build the host and its package libraries first')
    cases = configurations(investigations)
    before = input_hashes(investigations)
    # Never mix a new capture with stale files from a previous run. The manifest is written only on success.
    out.mkdir(parents=True, exist_ok=False)
    artifacts, logs = out / 'outputs', out / 'logs'
    artifacts.mkdir()
    logs.mkdir()
    for editions in cases:
        english = editions[0]
        package = english.package.name
        package_out = artifacts / package
        package_out.mkdir()
        hooks = execute([str(binary), '--investigation', english.investigation, 'script', 'check'], logs / f'{package}-scripts.log')
        (package_out / 'scripts.txt').write_text(hooks, encoding='utf-8')
        for cfg in editions:
            cfg.prepared_root = out / 'prepared' / package
            print(f'prepare {cfg.investigation} ({cfg.lang})', flush=True)
            execute([sys.executable, str(LAB / 'tools/video/build.py'), '--investigation', cfg.investigation,
                     '--lang', cfg.lang, '--prepare-only', '--tts-device', 'cpu', '--cached-audio-only',
                     '--prepare-dir', str(cfg.prepared_root),
                     '--binary', str(binary)], logs / f'{package}-{cfg.lang}.log')
            dest = compare.save_edition(cfg, artifacts, required=True)
            packages = [] if cfg.zh() else ['--package', f'{package}={cfg.package / "build" / (package + ".lam")}']
            if not cfg.zh() and cfg.story_library.exists():
                packages += ['--package', f'{package}_story={cfg.story_library}']
            chosen = [] if cfg.zh() else ['--investigation', cfg.investigation]
            ir = execute([str(binary), *chosen, 'lamlet', 'ir', str(cfg.translation if cfg.zh() else cfg.storyboard),
                          *packages], logs / f'{package}-{cfg.lang}-ir.log')
            write_json(dest / 'document.json', json.loads(ir))
        # Unlike the historical filename-only check, detect changed audio and word timings under the same name.
        write_json(package_out / 'audio-sha256.json',
                   {p.name: compare.digest(p) for p in sorted(english.audio_cache.iterdir()) if p.is_file()})
    after = input_hashes(investigations)
    if before != after:
        raise RuntimeError('corpus inputs changed during capture; retry with unchanged content')
    write_json(artifacts / 'inputs.json', before)
    files = {p.relative_to(artifacts).as_posix(): compare.digest(p) for p in sorted(artifacts.rglob('*')) if p.is_file()}
    shutil.rmtree(out / 'prepared')
    host = [binary, *sorted(binary.parent.glob('liblab.*'))]
    write_json(out / 'manifest.json', {
        'renderLabLamletRegression': VERSION,
        'cases': [{'investigation': cs[0].investigation, 'package': cs[0].package.name,
                   'languages': [cfg.lang for cfg in cs]} for cs in cases],
        'host': {str(p): compare.digest(p) for p in host if p.is_file()},
        'cargoLockSha256': compare.digest(LAB / 'Cargo.lock'),
        'files': files,
    })
    print(f'captured {sum(map(len, cases))} editions: {out}', flush=True)


def checked_manifest(snapshot: Path) -> dict[str, Any]:
    path = snapshot / 'manifest.json'
    if not path.is_file():
        raise RuntimeError(f'{snapshot}: incomplete capture (no manifest.json)')
    manifest = json.loads(path.read_text(encoding='utf-8'))
    if manifest.get('renderLabLamletRegression') != VERSION or not manifest.get('cases') or not manifest.get('files'):
        raise RuntimeError(f'{snapshot}: unsupported or empty corpus manifest')
    required = {'inputs.json'}
    for case in manifest['cases']:
        package = case['package']
        languages = case['languages']
        if CORPUS.get(case['investigation']) != package or not languages or languages[0] != 'en':
            raise RuntimeError(f'{snapshot}: invalid corpus case {case}')
        required.update(f'{package}/{name}' for name in ('scripts.txt', 'audio.txt', 'audio-sha256.json'))
        for lang in languages:
            required.update(f'{package}/{lang}/{name}' for name in ('timeline.json', 'document.json', *compare.PREPARED))
    if not required <= set(manifest['files']):
        raise RuntimeError(f'{snapshot}: missing required artifacts {sorted(required - set(manifest["files"]))}')
    artifacts = snapshot / 'outputs'
    actual = {p.relative_to(artifacts).as_posix(): compare.digest(p) for p in artifacts.rglob('*') if p.is_file()}
    if actual != manifest['files']:
        raise RuntimeError(f'{snapshot}: artifacts missing or changed since capture')
    return manifest


def differences(old: Path, new: Path) -> list[str]:
    before, after = checked_manifest(old), checked_manifest(new)
    if before['cases'] != after['cases']:
        return ['corpus cases or languages differ; capture the same corpus with both hosts']
    return compare.compare(old / 'outputs', new / 'outputs')


def main() -> None:
    parser = argparse.ArgumentParser(description=(__doc__ or '').split('\n\n')[0])
    commands = parser.add_subparsers(dest='command', required=True)
    save = commands.add_parser('save', help='prepare and capture the corpus; missing narration cache is an error')
    save.add_argument('out', type=Path)
    save.add_argument('--binary', type=Path, default=lamlet.BINARY)
    save.add_argument('--investigation', action='append', choices=CORPUS)
    diff = commands.add_parser('compare', help='validate and compare two complete captures')
    diff.add_argument('old', type=Path)
    diff.add_argument('new', type=Path)
    args = parser.parse_args()
    try:
        if args.command == 'save':
            capture(args.out.resolve(), args.binary, list(dict.fromkeys(args.investigation or CORPUS)))
        else:
            changed = differences(args.old, args.new)
            print('\n'.join(changed) if changed else 'the same')
            if changed:
                raise SystemExit(1)
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'{error}\n')


if __name__ == '__main__':
    main()
