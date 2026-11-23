#!/usr/bin/env python3
"""Publishes each public package's narrated videos as a package of its own.

A package's demos play a timeline and a narration track that tools/video builds into its `build/` (`built = true`
resources), which the CI's release, built without the video tools, does not have. They are data, the same on every
platform, so they are built here, once, and published beside the release's archives: for package `<name>`, the
content-only package `<name>_narration`, which imports `<name>` and overrides its demos' resources (they are exported
and `overridable`). A user unpacks it into the `packages/` folder beside `render-lab`.

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
# The heading the release's notes get once, above the install instructions.
NOTES_HEADING = '## Narrated videos'


@dataclass
class DemoResource:
    """A resource a package's demo plays: its key in the package, its source file, and its license."""

    key: str
    source: str
    license: str


@dataclass
class Narration:
    """One package's narration package: the package it overrides, its manifest, and the resources it replaces."""

    package: Path
    manifest: dict
    resources: list[DemoResource]

    @property
    def name(self) -> str:
        return f'{self.package.name}_narration'


def git(*args: str) -> str:
    return subprocess.run(['git', *args], cwd=ROOT, check=True, capture_output=True, text=True).stdout


def public_packages() -> list[Path]:
    """The source packages git tracks (a local package is never published)."""
    return sorted({(ROOT / path).parent for path in git('ls-files', 'packages/*/manifest.toml').split()})


def demo_resources(package: Path) -> tuple[dict, list[DemoResource]]:
    """A package's manifest, and the resources its demos play (each once), which must be its own, exported, and
    overridable."""
    manifest = tomllib.loads((package / 'manifest.toml').read_text())
    declared: dict[str, dict] = {}
    keys: list[str] = []
    for provided in manifest.get('provides', []):
        # Its collections declare the resources; its demos name them.
        if provided['type'] == 'resource':
            collection = tomllib.loads((package / provided['path']).read_text())
            declared.update({r['id']: r for r in collection.get('resource', [])})
        elif provided['type'] == 'demo':
            keys += [k for k in (provided['timeline'], provided['audio']) if k not in keys]
    resources = []
    for key in keys:
        resource = declared.get(key)
        if resource is None:
            raise SystemExit(f'{package.name}: a demo plays `{key}`, which is not one of its own resources')
        if not (resource.get('overridable') and key in manifest.get('export', [])):
            raise SystemExit(f'{package.name}: `{key}` must be exported and `overridable = true` for a narration package to override it')
        resources.append(DemoResource(key, resource['source'], resource['license']))
    return manifest, resources


def editions(package: Path) -> list[tuple[str, str]]:
    """The investigation and language of each edition of a package's video (video/video.json), English first."""
    spec_path = package / 'video' / 'video.json'
    if not spec_path.is_file():
        return []
    spec = json.loads(spec_path.read_text())
    return [(spec['investigation'], lang) for lang in ['en', *spec.get('translations', {})]]


def build_narration(narrations: list[Narration]) -> None:
    """Builds each edition's narration and timeline (tools/video's build without `--video`, from its audio cache), one after
    another."""
    for narration in narrations:
        for investigation, lang in editions(narration.package):
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
    """What is wrong with a package's built outputs: a file missing, or two editions playing the same narration (one
    left from another language)."""
    problems = []
    seen: dict[str, str] = {}
    for resource in narration.resources:
        path = narration.package / resource.source
        if not path.is_file():
            problems.append(f'{narration.package.name}: `{resource.key}` is not built ({resource.source})')
            continue
        if resource.key.startswith('narration/'):
            # Every language's narration is its own recording.
            same = seen.setdefault(digest(path), resource.key)
            if same != resource.key:
                problems.append(f'{narration.package.name}: `{resource.key}` is the same audio as `{same}`')
    return problems


def toml_string(s: str) -> str:
    return json.dumps(s, ensure_ascii=False)


def write_package(narration: Narration, out: Path) -> Path:
    """Writes `<name>_narration` into `out` as a built package (lab/src/cook.rs's form, which the loader reads): its
    manifest, a collection that overrides each demo resource, and the files, linked from the sources or copied."""
    directory = out / narration.name
    shutil.rmtree(directory, ignore_errors=True)
    directory.mkdir(parents=True)
    name = narration.package.name
    manifest = [
        f'# The narrated videos of the package `{name}`: their timelines and narration, which replace its own',
        '# (written by tools/publish_narration.py). Unpack it into the packages folder beside render-lab.',
        f'name = {toml_string(narration.name)}',
        f'author = {toml_string(narration.manifest["author"])}',
        f'license = {toml_string(narration.manifest["license"])}',
        f'import = [{toml_string(name)}]',
        'export = []',
        '',
        '[[provides]]',
        'type = "resource"',
        'path = "resources.toml"',
    ]
    collection: list[str] = []
    for resource in narration.resources:
        target = directory / resource.source
        target.parent.mkdir(parents=True, exist_ok=True)
        try:
            os.link(narration.package / resource.source, target)
        except OSError:
            shutil.copy2(narration.package / resource.source, target)
        collection += [
            '[[resource]]',
            f'id = {toml_string(resource.key)}',
            'kind = "file"',
            f'file = {toml_string(resource.source)}',
            f'license = {toml_string(resource.license)}',
            f'override = {toml_string(f"{name}.{resource.key}")}',
            '',
        ]
    (directory / 'manifest.toml').write_text('\n'.join(manifest) + '\n')
    (directory / 'resources.toml').write_text('\n'.join(collection))
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
    """Loads the built packages with the narration packages beside them, as a release would (no source checkout, so
    nothing is cooked), and checks each demo now plays the narration package's files."""
    root = out / 'release'
    shutil.rmtree(root, ignore_errors=True)
    # The public packages as built (what a release ships), then the narration packages beside them.
    for public in public_packages():
        shutil.copytree(ROOT / 'target/release/packages' / public.name, root / 'packages' / public.name, copy_function=os.link, symlinks=True)
    for package in packages:
        shutil.copytree(package, root / 'packages' / package.name, copy_function=os.link)
    # A root with no source packages, so the lab reads the built ones as they are.
    (root / 'root').mkdir()
    env = {**os.environ, 'RENDER_LAB_ROOT': str(root / 'root'), 'RENDER_LAB_PACKAGES': str(root / 'packages')}
    listed = {p['name']: p for p in lab_json(binary, env, 'packages')}
    files = {r['id']: Path(r['file']).resolve() for r in lab_json(binary, env, 'resources')}
    for package in packages:
        overridden = package.name.removesuffix('_narration')
        for demo in listed[overridden]['demos']:
            # Each demo's files are now the narration package's.
            for key in (demo['timeline'], demo['audio']):
                if not files[key].is_relative_to((root / 'packages' / package.name).resolve()):
                    raise SystemExit(f'{overridden}: `{key}` plays {files[key]}, not {package.name}\'s')
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
        'The narrated videos come in a package of their own, the same on every platform. To watch one, download its',
        '`*_narration` archive below and unpack it into the `packages` folder beside `render-lab` (or `render-lab.exe`).',
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
    parser.add_argument('--package', action='append', help='only this package (repeatable)')
    parser.add_argument('--skip-build', action='store_true', help='publish the outputs as they are built')
    parser.add_argument('--no-upload', action='store_true', help='pack and check only (the tag need not be checked out)')
    parser.add_argument('--wait', action='store_true', help='wait for the CI to publish the release before uploading')
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/render-lab', help='the built lab that checks the packages')
    return parser


def main() -> None:
    args = arg_parser().parse_args()
    if not args.no_upload:
        check_tag(args.tag)
    narrations = []
    for package in public_packages():
        manifest, resources = demo_resources(package)
        # Only a package whose demos play something has narration to publish.
        if resources and (not args.package or package.name in args.package):
            narrations.append(Narration(package, manifest, resources))
    if not narrations:
        raise SystemExit('no public package has a narrated video' + (f' among {", ".join(args.package)}' if args.package else ''))
    if not args.skip_build:
        build_narration(narrations)
    problems = [p for n in narrations for p in check_outputs(n)]
    if problems:
        raise SystemExit('\n'.join(problems))
    out = ROOT / 'target' / 'narration' / args.tag
    packages = [write_package(n, out / 'packages') for n in narrations]
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
