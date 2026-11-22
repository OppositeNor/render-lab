"""The Chinese voice: speaks sentences with Qwen3-TTS in one narrator's voice, cloned from a reference clip, checks
each by transcribing it with Qwen3-ASR, and times its units with Qwen3-ForcedAligner. It runs in the Chinese voice's own environment (tools/video/setup.sh --zh, its
Python under .cache/video-tts-zh), apart from the build's; speech_zh.py writes its job and reads what it leaves.

    .cache/video-tts-zh/bin/python -I tools/video/zh_voice.py JOB.json

The job (JSON): `out` (the directory the wavs go to), `models` (the three models: `tts`, `asr`, `aligner`, each a
Hugging Face repository and the revision to use, from the local cache only), `reference` (the narrator's clip,
`audio`, its transcript, `text`, and `id`, which each sidecar records as `voice`), `language`, `device` (`auto`, `cuda`, or `cpu`), `retryCer`, `attempts`, and `items`, each a `digest`, the `text` to say, and its `seed`.
Each item leaves `<digest>.wav` (16-bit mono) and `<digest>.json`: the text, the units the aligner timed with their
start and end in seconds, the transcript, its character error rate against the text, and the seed of the attempt
kept. The items go in chunks, each written before the next starts, so a run stopped part way keeps what it made.
"""
from __future__ import annotations

import gc
import json
import sys
import time
import unicodedata
import wave
from pathlib import Path
from typing import Any

CHUNK = 48             # sentences spoken, checked, and timed before they are written
TOKENS_PER_SECOND = 12 # the voice's codec frames per second of audio (Qwen3-TTS-12Hz)
TTS_BATCH = 6          # sentences spoken at once: 2.8 times as fast as one by one (0.6B, 4.7 GB at most); a batch that does not fit is split
ASR_BATCH = 4          # transcriptions at once (more runs out of a 6 GB GPU's memory)
DIGITS = dict(zip('零〇一二两三四五六七八九', '001223456789'))

Json = dict[str, Any]


def log(message: str) -> None:
    print(f'zh voice: {message}', flush=True)


def normalized(text: str) -> str:
    """Text as the error rate compares it: letters and digits, lower case, Chinese digits as digits, and a decimal
    point (点 between two digits) dropped, as the text's own `.` is."""
    text = ''.join(DIGITS.get(c, c) for c in text.lower())
    kept = [c for i, c in enumerate(text) if not (c == '点' and 0 < i < len(text) - 1 and text[i - 1].isdigit() and text[i + 1].isdigit())]
    return ''.join(c for c in kept if unicodedata.category(c)[0] in 'LN')


def error_rate(reference: str, heard: str) -> float:
    """The character error rate of `heard` against `reference`: their edit distance over the reference's length."""
    a, b = normalized(reference), normalized(heard)
    row = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        # One row of the edit distance's table at a time.
        diagonal, row[0] = row[0], i
        for j, y in enumerate(b, 1):
            diagonal, row[j] = row[j], min(row[j] + 1, row[j - 1] + 1, diagonal + (x != y))
    return row[-1] / max(1, len(a))


class Models:
    """Where the models are, and the device and precision they run at; each is loaded for one step and freed
    after it, as the three do not fit a 6 GB GPU together."""

    def __init__(self, job: Json):
        import torch
        from huggingface_hub import snapshot_download
        self.torch = torch
        self.paths = {name: snapshot_download(m['repository'], revision=m['revision'], local_files_only=True) for name, m in job['models'].items()}
        wanted = job.get('device', 'auto')
        if wanted == 'cuda' and not torch.cuda.is_available():
            raise SystemExit('--tts-device cuda: PyTorch sees no CUDA device')
        self.device = 'cuda:0' if wanted != 'cpu' and torch.cuda.is_available() else 'cpu'
        self.dtype = torch.bfloat16 if self.device != 'cpu' else torch.float32
        log(f'models on {self.device}')

    def free(self) -> None:
        """Gives the memory of a model just dropped back to the device."""
        gc.collect()
        if self.device != 'cpu':
            self.torch.cuda.empty_cache()


def most_tokens(group: list[Json]) -> int:
    """The longest a batch may speak: half a second per character of its longest sentence, and four more, about two
    and a half times as long as it takes to say, so a take that does not stop (it repeats, or trails on) is cut short,
    heard as wrong, and spoken again, rather than running to minutes."""
    return int(TOKENS_PER_SECOND * (max(len(i['text']) for i in group) / 2 + 4))


def speak(models: Models, job: Json, items: list[Json], attempt: int) -> dict[str, tuple[Any, int]]:
    """Each item's audio (float samples and rate) in the reference's voice, from the seed of this attempt: spoken
    `TTS_BATCH` at a time, sentences of like length together (a batch is as long as its longest)."""
    from qwen_tts import Qwen3TTSModel
    tts = Qwen3TTSModel.from_pretrained(models.paths['tts'], device_map=models.device, dtype=models.dtype)
    # The narrator's voice: the reference clip's speech codes and transcript, which every sentence continues from.
    voice = tts.create_voice_clone_prompt(ref_audio=job['reference']['audio'], ref_text=job['reference']['text'], x_vector_only_mode=False)
    out: dict[str, tuple[Any, int]] = {}

    def batch(group: list[Json]) -> None:
        # A batch's sampling is seeded by its first sentence, so the attempt can be made again; one that does not fit
        # in the GPU's memory is spoken in halves.
        models.torch.manual_seed(group[0]['seed'] + attempt)
        spoken = None
        try:
            spoken = tts.generate_voice_clone(text=[i['text'] for i in group], language=[job['language']] * len(group),
                                              voice_clone_prompt=voice * len(group), max_new_tokens=most_tokens(group))
        except models.torch.OutOfMemoryError:
            if len(group) == 1:
                raise
        if spoken is None:
            # Out of the handler, the failed batch's memory is released; then each half is spoken.
            log(f'{len(group)} sentences at once did not fit in memory; speaking them in halves')
            models.free()
            batch(group[:len(group) // 2])
            batch(group[len(group) // 2:])
            return
        wavs, rate = spoken
        for item, wav in zip(group, wavs):
            out[item['digest']] = (wav, rate)
    ordered = sorted(items, key=lambda i: len(i['text']))
    for first in range(0, len(ordered), TTS_BATCH):
        batch(ordered[first:first + TTS_BATCH])
    del tts
    models.free()
    return out


def transcribe(models: Models, job: Json, audio: dict[str, tuple[Any, int]]) -> dict[str, str]:
    """What the speech recogniser hears in each item's audio."""
    from qwen_asr import Qwen3ASRModel
    asr = Qwen3ASRModel.from_pretrained(models.paths['asr'], max_inference_batch_size=ASR_BATCH, device_map=models.device, dtype=models.dtype)
    digests = list(audio)
    heard = asr.transcribe(audio=[audio[d] for d in digests], language=job['language'])
    del asr
    models.free()
    return {d: h.text for d, h in zip(digests, heard)}


def align(models: Models, job: Json, items: list[Json], audio: dict[str, tuple[Any, int]]) -> dict[str, list[list[Any]]]:
    """Each item's units, with the second each starts and ends in its audio."""
    from qwen_asr import Qwen3ForcedAligner
    aligner = Qwen3ForcedAligner.from_pretrained(models.paths['aligner'], device_map=models.device, dtype=models.dtype)
    out: dict[str, list[list[Any]]] = {}
    for item in items:
        # One sentence at a time: its units' spans.
        result = aligner.align(audio=audio[item['digest']], text=item['text'], language=job['language'])[0]
        out[item['digest']] = [[u.text, float(u.start_time), float(u.end_time)] for u in result.items]
    del aligner
    models.free()
    return out


def write(out: Path, item: Json, samples: Any, rate: int, record: Json) -> None:
    """The item's wav, then its sidecar, each through a temporary file, so a stopped run never leaves half of one."""
    import numpy as np
    pcm = (np.clip(samples, -1.0, 1.0) * 32767.0).astype(np.int16).tobytes()
    tmp = out / f'{item["digest"]}.tmp.wav'
    with wave.open(str(tmp), 'wb') as stream:
        stream.setnchannels(1)
        stream.setsampwidth(2)
        stream.setframerate(rate)
        stream.writeframes(pcm)
    tmp.rename(out / f'{item["digest"]}.wav')
    sidecar = out / f'{item["digest"]}.tmp.json'
    sidecar.write_text(json.dumps(record, ensure_ascii=False))
    sidecar.rename(out / f'{item["digest"]}.json')


def chunk(models: Models, job: Json, items: list[Json], out: Path) -> None:
    """Speak, check, and time one chunk of items, then write them. A sentence heard with more errors than
    `retryCer` is spoken again from another seed, up to `attempts` times; the attempt heard best is kept."""
    best: dict[str, Json] = {}
    pending = items
    for attempt in range(job['attempts']):
        # Each attempt speaks what is still pending and keeps, per item, the attempt heard best so far.
        audio = speak(models, job, pending, attempt)
        heard = transcribe(models, job, audio)
        for item in pending:
            cer = error_rate(item['text'], heard[item['digest']])
            if item['digest'] not in best or cer < best[item['digest']]['cer']:
                best[item['digest']] = {'cer': cer, 'transcript': heard[item['digest']], 'seed': item['seed'] + attempt, 'audio': audio[item['digest']]}
        pending = [item for item in pending if best[item['digest']]['cer'] > job['retryCer']]
        if not pending:
            break
        log(f'{len(pending)} sentences heard with more than {job["retryCer"]:.0%} character errors; speaking them again')
    kept = {d: b['audio'] for d, b in best.items()}
    spans = align(models, job, items, kept)
    for item in items:
        # Write each item: its audio, and what was heard and timed in it.
        b = best[item['digest']]
        record = {'text': item['text'], 'units': spans[item['digest']], 'transcript': b['transcript'], 'cer': round(b['cer'], 4), 'seed': b['seed'],
                  'voice': job['reference']['id']}
        write(out, item, b['audio'][0], b['audio'][1], record)


def main() -> None:
    job = json.loads(Path(sys.argv[1]).read_text())
    out = Path(job['out'])
    items = job['items']
    models = Models(job)
    start = time.monotonic()
    for first in range(0, len(items), CHUNK):
        # A chunk at a time, written before the next starts.
        chunk(models, job, items[first:first + CHUNK], out)
        done = min(len(items), first + CHUNK)
        log(f'{done}/{len(items)} sentences ({time.monotonic() - start:.0f} s)')


if __name__ == '__main__':
    main()
