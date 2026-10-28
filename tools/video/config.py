"""What one video build works with: the investigation's video.json, the
edition's paths, and the tools of the local environment (tools/video/setup.sh).

An investigation lists its video in `src/<engine>/<feature>/video/video.json`;
its paths are relative to that file. Intermediates and deliverables go to
`build/video/<engine>/<feature>/<lang>/` (git-ignored); the timeline and its
diagram frames go to the investigation's timeline directory, which the lab's
viewer plays.
"""
from __future__ import annotations

import json
import os
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

TOOLS = Path(__file__).resolve().parent            # render-lab/tools/video/
LAB = TOOLS.parents[1]                             # render-lab/
LANGS = ('en', 'zh-CN')


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
        """`directory`: the investigation's source directory, when it is not
        src/<engine>/<feature> (a module built in from elsewhere)."""
        if lang not in LANGS:
            raise SystemExit(f'unknown language {lang}; use one of {", ".join(LANGS)}')
        path = (directory.resolve() if directory else LAB / 'src' / Path(investigation)) / 'video' / 'video.json'
        if not path.is_file():
            raise SystemExit(f'{investigation}: no {path.relative_to(LAB)}')
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

    def timeline_dir(self, lang: str | None = None) -> Path:
        return self.path(self.spec['timelines'][lang or self.lang])

    def timeline(self, lang: str | None = None) -> Path:
        return self.timeline_dir(lang) / 'timeline.json'

    def build(self, lang: str | None = None) -> Path:
        """The edition's build directory: build/video/<investigation>/<lang>, or video.json's `build`."""
        if 'build' in self.spec:
            return self.path(self.spec['build'][lang or self.lang])
        return LAB / 'build' / 'video' / Path(self.investigation) / (lang or self.lang)

    @property
    def audio_cache(self) -> Path:
        """Shared by the editions: every edition speaks the English sentences (video.json's `audioCache` if given)."""
        if 'audioCache' in self.spec:
            return self.path(self.spec['audioCache'])
        return LAB / 'build' / 'video' / Path(self.investigation) / 'audio'

    def video_name(self, lang: str | None = None) -> str:
        return self.spec['video'][lang or self.lang]

    def video(self, lang: str | None = None) -> Path:
        return self.build(lang) / self.video_name(lang)

    def text(self, key: str) -> Any:
        """A document text of this edition (video.json `documents`)."""
        return self.spec['documents'][self.lang][key]

    def checkout(self, index: dict[str, Any]) -> Path:
        override = os.environ.get(self.spec.get('checkoutEnv', ''), '')
        return Path(override).resolve() if override else (LAB / index['engine']['checkoutFromLabRoot']).resolve()


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
