#!/usr/bin/env python3
"""Publishes the narration packages: each investigation's narrated videos, as a package of their own.

A narration package, `packages/<name>_narration/`, holds no code: its demos (`for = "<name>"`) play the timelines and
narration that tools/video builds into its `build/` from `packages/<name>/video/`. The CI's release, built without the
video tools, leaves them out of its platform archives. They are data, the same on every platform, so they are built
here, once, and published beside those archives as `<name>_narration-<tag>.zip`, which a user unpacks inside the
packages folder in the executable's directory.

    python3 tools/publish_narration.py v0.0.8              # build, pack, check, and upload to the release v0.0.8
    python3 tools/publish_narration.py v0.0.8 --no-upload  # build, pack, and check only
    python3 tools/publish_narration.py v0.0.8 --skip-build --wait   # the outputs as built; wait for the CI's release

The archives are written to `target/narration/<tag>/`. Only the packages git tracks are published.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
import tomllib
import zipfile
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUILT = ROOT / 'target' / 'release' / 'packages'
# The heading the release's notes get once, above the install instructions.
NOTES_HEADING = '## Narrated videos'


@dataclass
class Narration:
    """A narration package: its source, the package it narrates, and the sources of the resources its demos play."""

    package: Path
    narrates: str
    sources: dict[str, str]

    @property
    def name(self) -> str:
        return self.package.name


def git(*args: str) -> str:
    return subprocess.run(['git', *args], cwd=ROOT, check=True, capture_output=True, text=True).stdout


def public_packages() -> list[Path]:
    """The source packages git tracks (a local package is never published)."""
    return sorted({(ROOT / path).parent for path in git('ls-files', 'packages/*/manifest.toml').split()})


def read_narration(package: Path) -> Narration | None:
    """A package's narration: the package its demos narrate (`for`) and the sources of what they play; None for a
    package without demos."""
    manifest = tomllib.loads((package / 'manifest.toml').read_text())
    demos = [p for p in manifest.get('provides', []) if p['type'] == 'demo']
    if not demos:
        return None
    narrates = {d.get('for') for d in demos}
    if len(narrates) != 1 or None in narrates:
        raise SystemExit(f'{package.name}: every demo of a narration package names the one package it narrates (`for`)')
    declared: dict[str, str] = {}
    for provided in manifest['provides']:
        # Its collections declare the files its demos play.
        if provided['type'] == 'resource':
            collection = tomllib.loads((package / provided['path']).read_text())
            declared.update({r['id']: r['source'] for r in collection.get('resource', [])})
    keys = {k for d in demos for k in (d['timeline'], d['audio'])}
    if missing := sorted(k for k in keys if k not in declared):
        raise SystemExit(f'{package.name}: its demos play {", ".join(missing)}, which are not its own resources')
    return Narration(package, narrates.pop(), {k: declared[k] for k in sorted(keys)})


def editions(narration: Narration) -> list[tuple[str, str]]:
    """The investigation and language of each edition of the narrated package's video (video/video.json), English
    first."""
    spec = json.loads((ROOT / 'packages' / narration.narrates / 'video' / 'video.json').read_text())
    return [(spec['investigation'], lang) for lang in ['en', *spec.get('translations', {})]]


def build_narration(narrations: list[Narration]) -> None:
    """Builds each edition's narration and timeline (tools/video without `--video`, from its audio cache), one
    after another."""
    for narration in narrations:
        for investigation, lang in editions(narration):
            print(f'== {investigation} ({lang})', flush=True)
            command = [sys.executable, str(ROOT / 'tools/video/build.py'), '--investigation', investigation, '--lang', lang]
            if subprocess.run(command, cwd=ROOT).returncode != 0:
                raise SystemExit(f'the narration of {investigation} ({lang}) did not build')


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open('rb') as f:
        while chunk := f.read(1 << 20):
            sha.update(chunk)
    return sha.hexdigest()


def check_outputs(narration: Narration) -> list[str]:
    """What is wrong with a narration package's built files: one missing, or two editions playing the same narration
    (one left from another language)."""
    problems = []
    seen: dict[str, str] = {}
    for key, source in narration.sources.items():
        path = narration.package / source
        if not path.is_file():
            problems.append(f'{narration.name}: `{key}` is not built ({source})')
            continue
        if key.startswith('narration/'):
            # Every language's narration is its own recording.
            same = seen.setdefault(digest(path), key)
            if same != key:
                problems.append(f'{narration.name}: `{key}` is the same audio as `{same}`')
    return problems


def take_built(narration: Narration, out: Path) -> Path:
    """The narration package as the cook built it, copied into `out` without `cooked-from` (this checkout's path), so
    that it is a package of its own wherever it is unpacked."""
    directory = out / narration.name
    shutil.rmtree(directory, ignore_errors=True)
    shutil.copytree(BUILT / narration.name, directory, copy_function=os.link)
    manifest = directory / 'manifest.toml'
    lines = manifest.read_text().splitlines()
    kept = [line for line in lines if not line.startswith(('cooked-from', '# Built from'))]
    manifest.unlink()
    manifest.write_text('\n'.join(kept) + '\n')
    return directory


def local_paths(directory: Path) -> list[Path]:
    """The text files of a package that name this checkout or the user's home: nothing published may."""
    needles = [str(ROOT).encode(), str(Path.home()).encode()]
    texts = [p for p in directory.rglob('*') if p.is_file() and p.suffix in ('.json', '.toml')]
    return [p for p in texts if any(n in p.read_bytes() for n in needles)]


def lab_json(binary: Path, env: dict[str, str], command: str) -> list[dict]:
    """A `render-lab <command> --json` listing, run on the packages `env` names."""
    result = subprocess.run([str(binary), command, '--json'], env=env, capture_output=True, text=True)
    if result.returncode != 0:
        raise SystemExit(f'the lab did not load the packages with the narration packages beside them:\n{result.stderr}')
    return json.loads(result.stdout)


def check_with_lab(binary: Path, packages: list[Path], out: Path) -> None:
    """Loads the public built packages a release ships (no narration package among them) with the narration
    packages beside them, as a release would (no source checkout, so nothing is cooked), and checks every demo's
    files are there."""
    root = out / 'release'
    shutil.rmtree(root, ignore_errors=True)
    shipped = [p.name for p in public_packages() if read_narration(p) is None]
    for name in shipped:
        shutil.copytree(BUILT / name, root / 'packages' / name, copy_function=os.link, symlinks=True)
    for package in packages:
        shutil.copytree(package, root / 'packages' / package.name, copy_function=os.link)
    # A root with no source packages, so the lab reads the built ones as they are.
    (root / 'root').mkdir()
    env = {**os.environ, 'RENDER_LAB_ROOT': str(root / 'root'), 'RENDER_LAB_PACKAGES': str(root / 'packages')}
    listed = {p['name']: p for p in lab_json(binary, env, 'packages')}
    sizes = {r['id']: r['bytes'] for r in lab_json(binary, env, 'resources')}
    for package in packages:
        for demo in listed[package.name]['demos']:
            # Each demo's timeline and narration are files of the package.
            for key in (demo['timeline'], demo['audio']):
                if not sizes.get(key):
                    raise SystemExit(f'{package.name}: demo `{demo["key"]}` has no file for `{key}`')
    shutil.rmtree(root)


def zip_package(directory: Path, archive: Path) -> None:
    """Packs a package's folder, under its name, into a zip archive (which every platform unpacks)."""
    archive.unlink(missing_ok=True)
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as z:
        for path in sorted(directory.rglob('*')):
            if path.is_file():
                z.write(path, path.relative_to(directory.parent).as_posix())


def notes(archives: list[Path]) -> str:
    """The release notes' section on the narration packages."""
    lines = [
        NOTES_HEADING,
        '',
        'The narrated videos come in packages of their own, the same on every platform. To watch one, download its',
        '`*_narration` archive below and unpack it inside the `packages` folder in the executable\'s directory.',
        'The narration plays through FFplay (FFmpeg), which must be on your PATH.',
        '',
    ]
    return '\n'.join(lines + [f'- `{a.name}`' for a in archives]) + '\n'


def upload(tag: str, archives: list[Path], wait: bool) -> None:
    """Uploads the archives to the release `tag` (which the CI makes when the tag is pushed), and adds the install
    instructions to its notes once."""
    while subprocess.run(['gh', 'release', 'view', tag], cwd=ROOT, capture_output=True).returncode != 0:
        # The CI publishes the release when its platform archives are built.
        if not wait:
            raise SystemExit(f'no release {tag} yet: push the tag, wait for the CI, or pass --wait')
        print(f'waiting for the release {tag}...', flush=True)
        time.sleep(60)
    subprocess.run(['gh', 'release', 'upload', tag, '--clobber', *map(str, archives)], cwd=ROOT, check=True)
    body = subprocess.run(['gh', 'release', 'view', tag, '--json', 'body', '-q', '.body'], cwd=ROOT, check=True, capture_output=True, text=True).stdout
    if NOTES_HEADING not in body:
        subprocess.run(['gh', 'release', 'edit', tag, '--notes', body.rstrip() + '\n\n' + notes(archives)], cwd=ROOT, check=True)


def check_tag(tag: str) -> None:
    """The tag must name this version (Cargo.toml) and the commit checked out, with no tracked file changed."""
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    if tag != f'v{version}':
        raise SystemExit(f'{tag} is not this checkout\'s version, v{version}')
    if git('rev-parse', 'HEAD').strip() != git('rev-parse', f'{tag}^{{commit}}').strip():
        raise SystemExit(f'HEAD is not {tag}: check out the tagged commit')
    if git('status', '--porcelain', '--untracked-files=no').strip():
        raise SystemExit('tracked files are changed: the narration must be built from the tagged commit')


def arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('tag', help='the release, v<version> (Cargo.toml)')
    parser.add_argument('--package', action='append', help='only this narration package (repeatable)')
    parser.add_argument('--skip-build', action='store_true', help='publish the outputs as they are built')
    parser.add_argument('--no-upload', action='store_true', help='pack and check only (the tag need not be checked out)')
    parser.add_argument('--wait', action='store_true', help='wait for the CI to publish the release before uploading')
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/render-lab', help='the built lab that cooks and checks the packages')
    return parser


def main() -> None:
    args = arg_parser().parse_args()
    if not args.no_upload:
        check_tag(args.tag)
    narrations = [n for p in public_packages() if (n := read_narration(p)) and (not args.package or n.name in args.package)]
    if not narrations:
        raise SystemExit('no public narration package' + (f' among {", ".join(args.package)}' if args.package else ''))
    if not args.skip_build:
        build_narration(narrations)
    problems = [p for n in narrations for p in check_outputs(n)]
    if problems:
        raise SystemExit('\n'.join(problems))
    # The lab builds the packages as it would for a launch, the narration packages' new files with them.
    subprocess.run([str(args.binary), 'cook'], cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    out = ROOT / 'target' / 'narration' / args.tag
    packages = [take_built(n, out / 'packages') for n in narrations]
    if stray := [p for d in packages for p in local_paths(d)]:
        raise SystemExit('these files name a local path: ' + ', '.join(map(str, stray)))
    check_with_lab(args.binary, packages, out)
    archives = []
    for package in packages:
        archive = out / f'{package.name}-{args.tag}.zip'
        zip_package(package, archive)
        archives.append(archive)
        print(f'packed {archive.relative_to(ROOT)} ({archive.stat().st_size / 1e6:.0f} MB)', flush=True)
    if not args.no_upload:
        upload(args.tag, archives, args.wait)
        print(f'uploaded {len(archives)} narration package(s) to the release {args.tag}')


if __name__ == '__main__':
    main()
