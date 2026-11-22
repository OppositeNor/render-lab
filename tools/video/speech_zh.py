"""The zh-CN edition's narration: each sentence spoken in Chinese by Qwen3-TTS, in one narrator's voice cloned from a
reference clip (`voices/zh-CN-narrator.wav`, made once with Qwen3-TTS VoiceDesign; its `.json` says how), heard
back by Qwen3-ASR to catch a skipped or repeated word, and timed unit by unit (a Chinese character, or a run of
letters and digits) by Qwen3-ForcedAligner, so that each English cue fires on the Chinese word it stands before
(zh.py `cue_marks`).

The models run in the Chinese voice's own environment (tools/video/setup.sh --zh), in a process of their own
(zh_voice.py); this writes its job and reads what it leaves in the shared narration cache: `<digest>.wav` and
`<digest>.json`, named by the hash of what is said and the voice, so an unchanged sentence is never spoken again.
A sentence still heard with more than `REVIEW_CER` of its characters wrong is listed in the build's
`narration-review.md`, to be listened to.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import wave
from pathlib import Path
from typing import Any

import markup
import narration
import zh
from locations import DiagnosticError
from config import LAB, TOOLS, Config, Environment, log
from narration import Durations, Narration

# The models, each at the revision the voice was chosen with (2026-11-21), from the local Hugging Face cache.
MODELS = {
    'tts': {'repository': 'Qwen/Qwen3-TTS-12Hz-1.7B-Base', 'revision': 'fd4b254389122332181a7c3db7f27e918eec64e3'},
    'asr': {'repository': 'Qwen/Qwen3-ASR-1.7B', 'revision': '7278e1e70fe206f11671096ffdd38061171dd6e5'},
    'aligner': {'repository': 'Qwen/Qwen3-ForcedAligner-0.6B', 'revision': 'c7cbfc2048c462b0d63a45797104fc9db3ad62b7'},
}
# The narrator: a serious, mature woman's voice (chosen 2026-11-21 over the built-in voices, whose pitch drifted
# three times as far from sentence to sentence), cloned from this clip and its transcript for every sentence.
REFERENCE = TOOLS / 'voices' / 'zh-CN-narrator.wav'
REFERENCE_TEXT = json.loads(REFERENCE.with_suffix('.json').read_text())['text']
# The reference a clip was spoken from, its audio and its transcript, which each sidecar records (`voice`): a clip
# spoken from another is not in the cache.
REFERENCE_ID = hashlib.sha256(REFERENCE.read_bytes() + b'\0' + REFERENCE_TEXT.encode()).hexdigest()[:16]
LANGUAGE = 'Chinese'
SEED = 20261120
RETRY_CER = 0.25        # a sentence heard with more character errors than this is spoken again
ATTEMPTS = 3            # at most, keeping the attempt heard best
REVIEW_CER = 0.10       # a sentence kept with more than this is listed for listening
VOICE_ID = '/'.join([*(f'{m["repository"]}@{m["revision"][:12]}' for m in MODELS.values()), hashlib.sha256(REFERENCE.read_bytes()).hexdigest()[:12],
                     str(SEED), str(RETRY_CER), str(ATTEMPTS)])

Json = dict[str, Any]


def pronounce(cfg: Config, text: str) -> str:
    """`text` with the words of video.json's `pronunciationsZh` replaced by how the Chinese voice should say them
    (a word is cut at letters and digits only, as Chinese characters run up against English words)."""
    for word, said in sorted(cfg.spec.get('pronunciationsZh', {}).items(), key=lambda p: -len(p[0])):
        # Longest words first, so a word is never replaced inside a longer one being replaced.
        text = re.sub(r'(?<![A-Za-z0-9_])' + re.escape(word) + r'(?![A-Za-z0-9_])', said, text)
    return text


def speech(cfg: Config, sentence: Json) -> tuple[str, list[int]]:
    """What the voice says for a sentence of the zh-CN edition, and the unit each of its English tags stands
    before."""
    english, chinese = markup.sentence(sentence), sentence['shown']
    pieces = zh.spoken_pieces(chinese, lambda text: pronounce(cfg, text))
    text = zh.spoken_text(pieces)
    if not zh.units(text):
        raise DiagnosticError('story.narration.empty', 'a translation paragraph needs spoken text: say its formula, ${say {math "…"} "…"}', sentence.get('shownSource'))
    return text, zh.cue_marks(english, chinese, pieces, sentence.get('shownSource'), None)


def cached(cfg: Config, digest: str) -> bool:
    """Whether a sentence's clip is in the cache, spoken from the reference as it is now (`REFERENCE_ID`)."""
    wav, sidecar = cfg.audio_cache / f'{digest}.wav', cfg.audio_cache / f'{digest}.json'
    return wav.exists() and sidecar.exists() and json.loads(sidecar.read_text()).get('voice') == REFERENCE_ID


def digest_of(text: str) -> str:
    return hashlib.sha256((text + '\0' + VOICE_ID).encode()).hexdigest()[:24]


def run_voice(cfg: Config, env: Environment, device: str, items: list[Json]) -> None:
    """Speaks, checks, and times `items` in the Chinese voice's process, its progress logged as it goes."""
    if not env.zh_python.exists():
        raise SystemExit(f'no Chinese voice environment at {env.tts_zh}; run tools/video/setup.sh --zh (or set RENDER_LAB_TTS_ZH)')
    logs = cfg.build() / 'logs'
    logs.mkdir(parents=True, exist_ok=True)
    job = logs / 'zh-voice-job.json'
    reference = {'audio': str(REFERENCE), 'text': REFERENCE_TEXT, 'id': REFERENCE_ID}
    job.write_text(json.dumps({'out': str(cfg.audio_cache), 'models': MODELS, 'reference': reference, 'language': LANGUAGE, 'device': device,
                               'retryCer': RETRY_CER, 'attempts': ATTEMPTS, 'items': items}, ensure_ascii=False, indent=1) + '\n')
    environment = {**os.environ, 'HF_HOME': str(env.models), 'HF_HUB_OFFLINE': '1', 'TRANSFORMERS_VERBOSITY': 'error', 'TOKENIZERS_PARALLELISM': 'false'}
    log(f'Chinese voice: {len(items)} sentences to speak ({REFERENCE.name}, {MODELS["tts"]["repository"]}).')
    with (logs / 'zh-voice.log').open('w') as out:
        proc = subprocess.Popen([str(env.zh_python), '-I', str(TOOLS / 'zh_voice.py'), str(job)], stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                text=True, env=environment)
        assert proc.stdout is not None
        for line in proc.stdout:
            # Everything goes to the log; the voice's own progress lines to the console too.
            out.write(line)
            if line.startswith('zh voice:'):
                log(line.rstrip())
    if proc.wait():
        raise RuntimeError(f'the Chinese voice failed ({proc.returncode}); see {logs / "zh-voice.log"}')


def measured(cfg: Config, sentence: Json, text: str, marks: list[int], digest: str) -> tuple[Narration, Json]:
    """A spoken sentence's narration, and what was heard in it."""
    wav, sidecar = cfg.audio_cache / f'{digest}.wav', cfg.audio_cache / f'{digest}.json'
    record = json.loads(sidecar.read_text())
    cut = [u[0] for u in record['units']]
    if cut != zh.units(text):
        raise RuntimeError(f'{sentence["at"]}: the aligner cut {cut} where zh.units cuts {zh.units(text)}; they must agree (zh.units follows qwen_asr)')
    with wave.open(str(wav), 'rb') as stream:
        seconds, rate = stream.getnframes() / stream.getframerate(), stream.getframerate()
    starts: list[float] = []
    for _, start, _ in record['units']:
        # Each unit's start, never before the one before it.
        starts.append(max(start, starts[-1]) if starts else start)
    return Narration(wav, seconds, rate, starts, marks), {'at': sentence['at'], 'text': text, 'heard': record['transcript'], 'cer': record['cer']}


def synthesize(cfg: Config, env: Environment, device: str, sentences: list[Json], *, cached_only: bool = False) -> Durations:
    """The Chinese narration of every sentence, keyed by where its translation is written (its `at`)."""
    cfg.audio_cache.mkdir(parents=True, exist_ok=True)
    # Every sentence's text and cue marks first, so a translation's mistake stops the build before any speech.
    spoken = [(s, *speech(cfg, s)) for s in sentences]
    missing: dict[str, Json] = {}
    for s, text, _ in spoken:
        # A sentence not in the cache (wav and sidecar, spoken from this reference) is spoken; the same text twice once.
        digest = digest_of(text)
        if not cached(cfg, digest):
            missing[digest] = {'digest': digest, 'text': text, 'seed': SEED}
    if missing and cached_only:
        first = next(iter(missing.values()))
        raise RuntimeError(f'narration cache miss ({first["digest"]}): {first["text"]!r}; prepare this corpus before capturing a regression baseline')
    if missing:
        run_voice(cfg, env, device, list(missing.values()))
    durations: Durations = {}
    review: list[Json] = []
    for s, text, marks in spoken:
        # Each sentence's audio and unit times, and how well it was heard.
        durations[s['at']], heard = measured(cfg, s, text, marks, digest_of(text))
        if heard['cer'] > REVIEW_CER:
            review.append(heard)
    write_review(cfg, review, len(sentences))
    log(f'Narration {len(sentences)}/{len(sentences)} sentences (Chinese)')
    return durations


def pace(cfg: Config, sentences: list[Json], durations: Durations) -> float:
    """How long the Chinese narration takes against the English: the seconds of all its speech over the English
    edition's, read from the English sentences' clips in the narration cache (the English edition is built first)."""
    english = 0.0
    for s in sentences:
        # Each sentence's English clip, as narration.py names it, and its length.
        clip = cfg.audio_cache / f'{narration.clip_digest(narration.spoken(cfg, s))}.wav'
        if not clip.exists():
            raise SystemExit(f'{s["at"]}: no English narration in the cache ({clip.name}); build the English edition first')
        with wave.open(str(clip), 'rb') as stream:
            english += stream.getnframes() / stream.getframerate()
    return round(sum(durations[s['at']].seconds for s in sentences) / english, 3)


def fit_budgets(cfg: Config, board: Json, speed: float) -> None:
    """The edition's chapter budgets: the English ones scaled by its pace (`pace`), which also scales the video's
    expected length (verify.py); written to the build's `budgets.json`, which `--verify-only` reads back."""
    board['pace'] = speed
    for chapter in board['chapters']:
        chapter['targetSeconds'] = round(chapter['targetSeconds'] * speed, 1)
    targets = {chapter['number']: chapter['targetSeconds'] for chapter in board['chapters']}
    (cfg.build() / 'budgets.json').write_text(json.dumps({'pace': speed, 'targets': targets}, indent=1) + '\n')


def load_budgets(cfg: Config, board: Json) -> None:
    """The chapter budgets and pace a build of the edition wrote (`fit_budgets`), for `--verify-only`."""
    written = json.loads((cfg.build() / 'budgets.json').read_text())
    board['pace'] = written['pace']
    for chapter in board['chapters']:
        chapter['targetSeconds'] = written['targets'][str(chapter['number'])]


def write_review(cfg: Config, review: list[Json], total: int) -> None:
    """The sentences heard with the most errors, for a listener: where each is, what was meant, and what was heard."""
    lines = ['# Chinese narration to listen to', '',
             f'{len(review)} of {total} sentences were heard back (Qwen3-ASR) with more than {REVIEW_CER:.0%} of their characters different from the',
             'text. Most are English terms or numbers written another way; listen to each and fix the text or `pronunciationsZh`.', '']
    lines += [f'- `{r["at"].removeprefix(str(LAB) + "/")}` ({r["cer"]:.0%})\n  - said: {r["text"]}\n  - heard: {r["heard"]}' for r in sorted(review, key=lambda r: -r['cer'])]
    cfg.build().mkdir(parents=True, exist_ok=True)
    (cfg.build() / 'narration-review.md').write_text('\n'.join(lines) + '\n')
    if review:
        log(f'Chinese narration: {len(review)} sentences to listen to, in {cfg.build() / "narration-review.md"}')
