"""What one video build works with: the investigation's video.json, the
edition's paths, and the tools of the local environment (tools/video/setup.sh).

An investigation lists its video in its package, `packages/<package>/video/video.json`
(the one whose `investigation` names it); its paths are relative to that file. Everything a build writes
stays in the package, under its `build/` (git-ignored): intermediates and deliverables in
`build/video/<lang>/`, and the timeline and its diagram frames in `build/timelines/<lang>/`, which the
package's demos name and the lab's viewer plays.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

TOOLS = Path(__file__).resolve().parent            # render-lab/tools/video/
LAB = TOOLS.parents[1]                             # render-lab/
LANGS = ('en', 'zh-CN')
# `--resolution` presets (16:9, by height); any other size is WIDTHxHEIGHT.
RESOLUTIONS = {'720p': (1280, 720), '1080p': (1920, 1080), '1440p': (2560, 1440), '2160p': (3840, 2160), '4k': (3840, 2160)}


def parse_resolution(value: str) -> tuple[int, int]:
    """A preset or WIDTHxHEIGHT, as `render-lab render --resolution` takes it
    (even sizes, for FFmpeg's yuv420p)."""
    size = RESOLUTIONS.get(value.lower())
    if size is None:
        try:
            w, h = value.lower().split('x')
            size = (int(w), int(h))
        except ValueError:
            raise ValueError(f'{value}: expected WIDTHxHEIGHT or one of {", ".join(RESOLUTIONS)}') from None
    if any(n < 2 or n > 8192 or n % 2 for n in size):
        raise ValueError(f'{size[0]}x{size[1]}: width and height are even, from 2 to 8192')
    return size


def env_path(name: str, default: Path) -> Path:
    value = os.environ.get(name)
    return Path(value).resolve() if value else default


@dataclass(frozen=True)
class Environment:
    """The Conda prefix (python, ffmpeg, node), the voice, the Node packages,
    the CUDA packages for GPU narration, and Puppeteer's browser cache. Each can
    point at an existing install through its environment variable."""
    prefix: Path = field(default_factory=lambda: env_path('RENDER_LAB_VIDEO_ENV', LAB / '.cache' / 'video-env'))
    voices: Path = field(default_factory=lambda: env_path('RENDER_LAB_VOICES', LAB / '.cache' / 'video-voices'))
    node_modules: Path = field(default_factory=lambda: env_path('RENDER_LAB_NODE_MODULES', TOOLS / 'node_modules'))
    tts_gpu: Path = field(default_factory=lambda: env_path('RENDER_LAB_TTS_GPU', LAB / '.cache' / 'video-tts-gpu'))
    puppeteer_cache: Path = field(default_factory=lambda: env_path('RENDER_LAB_PUPPETEER_CACHE', LAB / '.cache' / 'puppeteer'))

    @property
    def python(self) -> Path:
        return self.prefix / 'bin' / 'python'

    @property
    def ffmpeg(self) -> Path:
        return self.prefix / 'bin' / 'ffmpeg'

    @property
    def ffprobe(self) -> Path:
        return self.prefix / 'bin' / 'ffprobe'

    @property
    def node(self) -> Path:
        return self.prefix / 'bin' / 'node'


@dataclass
class Config:
    """One investigation's video in one language."""
    investigation: str
    lang: str
    spec: dict[str, Any]
    base: Path                       # the directory of video.json

    @staticmethod
    def load(investigation: str, lang: str, directory: Path | None = None) -> Config:
        """`directory`: the directory whose video/video.json to use, when it is
        not the investigation's package in packages/."""
        if lang not in LANGS:
            raise SystemExit(f'unknown language {lang}; use one of {", ".join(LANGS)}')
        if directory:
            path = directory.resolve() / 'video' / 'video.json'
            if not path.is_file():
                raise SystemExit(f'{investigation}: no {path}')
        else:
            found = [p for p in sorted((LAB / 'packages').glob('*/video/video.json')) if json.loads(p.read_text()).get('investigation') == investigation]
            if len(found) != 1:
                raise SystemExit(f'{investigation}: {len(found)} packages/*/video/video.json name it, not one')
            path = found[0]
        spec = json.loads(path.read_text())
        if spec['investigation'] != investigation:
            raise SystemExit(f'{path}: names {spec["investigation"]}, not {investigation}')
        return Config(investigation, lang, spec, path.parent)

    def path(self, value: str) -> Path:
        return (self.base / value).resolve()

    def zh(self) -> bool:
        return self.lang == 'zh-CN'

    @property
    def storyboard(self) -> Path:
        return self.path(self.spec['storyboard'])

    @property
    def translation(self) -> Path:
        return self.path(self.spec['translations'][self.lang])

    @property
    def diagrams(self) -> Path:
        """The edition's Mermaid sources (one directory, or one per language)."""
        value = self.spec['diagrams']
        return self.path(value if isinstance(value, str) else value[self.lang])

    @property
    def source_index(self) -> Path:
        return self.path(self.spec['sourceIndex'])

    @property
    def pseudocode(self) -> Path:
        return self.path(self.spec['pseudocode'])

    @property
    def package(self) -> Path:
        """The package video.json belongs to (its `video/` holds video.json)."""
        return self.base.parent

    def timeline_dir(self, lang: str | None = None) -> Path:
        """The edition's timeline and diagram frames: the package's build/timelines/<lang> (generated, so git
        ignores it; the package's demo names the same file)."""
        return self.package / 'build' / 'timelines' / (lang or self.lang)

    def timeline(self, lang: str | None = None) -> Path:
        return self.timeline_dir(lang) / 'timeline.json'

    def build(self, lang: str | None = None) -> Path:
        """The edition's build directory: the package's build/video/<lang>, or video.json's `build`."""
        if 'build' in self.spec:
            return self.path(self.spec['build'][lang or self.lang])
        return self.package / 'build' / 'video' / (lang or self.lang)

    @property
    def audio_cache(self) -> Path:
        """Shared by the editions: every edition speaks the English sentences (video.json's `audioCache` if given)."""
        if 'audioCache' in self.spec:
            return self.path(self.spec['audioCache'])
        return self.package / 'build' / 'video' / 'audio'

    def video_name(self, lang: str | None = None) -> str:
        return self.spec['video'][lang or self.lang]

    def video(self, lang: str | None = None) -> Path:
        return self.build(lang) / self.video_name(lang)

    def text(self, key: str) -> Any:
        """A document text of this edition (video.json `documents`)."""
        return self.spec['documents'][self.lang][key]

    def lab_text(self, key: str) -> str:
        """A text of the lab's own package (rlstd), in this edition's language."""
        return json.loads((LAB / 'packages' / 'rlstd' / 'text' / f'{self.lang}.json').read_text(encoding='utf-8'))[key]

    def notices(self) -> list[tuple[str, Path | None]]:
        """The package's legal notices (its manifest's `[[notice]]`, lab/src/packages.rs): each one's text in this
        edition's language, and the file of its terms, if it has one."""
        manifest = tomllib.loads((self.package / 'manifest.toml').read_text(encoding='utf-8'))
        texts = json.loads((self.package / 'text' / f'{self.lang}.json').read_text(encoding='utf-8'))
        return [(symbols(texts[n['notice-text']]), self.package / n['license'] if 'license' in n else None) for n in manifest.get('notice', [])]

    def checkout(self, index: dict[str, Any]) -> Path:
        override = os.environ.get(self.spec.get('checkoutEnv', ''), '')
        return Path(override).resolve() if override else (LAB / index['engine']['checkoutFromLabRoot']).resolve()


# The lab's symbol tags (lab/src/rich_text.rs SYMBOLS), for text a file shows as plain text.
SYMBOLS = {'copyright': '©', 'registered': '®', 'trademark': '™'}


def symbols(text: str) -> str:
    """`text` with its `{@ symbol=name}` tags written as their characters."""
    return re.sub(r'\{@ symbol=(\w+)\}', lambda m: SYMBOLS[m.group(1)], text)


def log(message: str) -> None:
    print(message, flush=True)


def run(args: list[Any], log_file: Path | None = None, cwd: Path = LAB, env: dict[str, str] | None = None, timeout: float | None = None) -> str:
    result = subprocess.run([str(a) for a in args], cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=env, timeout=timeout)
    if log_file:
        log_file.parent.mkdir(parents=True, exist_ok=True)
        log_file.write_text(result.stdout)
    if result.returncode:
        raise RuntimeError(f'command failed ({result.returncode}): {" ".join(map(str, args[:3]))}\n{result.stdout[-8000:]}')
    return result.stdout
