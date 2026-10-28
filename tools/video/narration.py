"""Narration: one Piper wav per sentence, cached by the hash of its text, the
speed, and the voice, on the GPU when the CUDA build of ONNX Runtime is
installed (tools/video/setup.sh --gpu), otherwise on the CPU.

Each wav has a sidecar, `<digest>.json`, with the time every word starts:
the voice model computes each phoneme's duration (its `/Ceil` tensor, in
frames of 256 samples) on the way to the audio, and a copy of the model that
also outputs it (`durations_model`) gives the durations of the very audio it
makes. Sentences are synthesised as Piper's `synthesize_wav` does: one chunk
per eSpeak sentence, concatenated.

`select_device` must run before onnxruntime is imported: it checks for an
NVIDIA GPU, loads the CUDA 13 runtime from the system and cuDNN from the GPU
package directory, and puts that directory first on sys.path.
"""
from __future__ import annotations

import ctypes
import hashlib
import json
import os
import re
import sys
import wave
from pathlib import Path
from typing import Any, NamedTuple

import markup
from config import Config, Environment, log

VOICE = 'en_US-lessac-medium'
VOICE_ID = 'piper-1.3.0-lessac-medium'
VOICE_SPEED = 1.02
SEED = 20261024

# cuDNN 9 loads its sub-libraries by name; load them first, by path.
CUDNN = ['libcudnn_graph.so.9', 'libcudnn_ops.so.9', 'libcudnn_heuristic.so.9', 'libcudnn_engines_runtime_compiled.so.9',
         'libcudnn_engines_precompiled.so.9', 'libcudnn_cnn.so.9', 'libcudnn_adv.so.9', 'libcudnn.so.9']
CUDA_RUNTIME = ['libcudart.so.13', 'libcublasLt.so.13', 'libcublas.so.13', 'libcurand.so.10', 'libcufft.so.12']

# The voice model's tensor of per-phoneme durations (frames of `HOP` samples).
DURATIONS_TENSOR = '/Ceil_output_0'
HOP = 256


class Narration(NamedTuple):
    """A sentence's wav, its length, its sample rate, when each spoken word
    starts (seconds), and the word each of its tags stands before
    (`len(words)` for a tag after the last word)."""
    path: Path
    seconds: float
    rate: int
    words: list[float]
    marks: list[int]


Durations = dict[str, Narration]


def _gpu_problem(env: Environment) -> str | None:
    if not (env.tts_gpu / 'onnxruntime').is_dir():
        return 'the CUDA packages are not installed (tools/video/setup.sh --gpu)'
    try:
        driver = ctypes.CDLL('libcuda.so.1')
    except OSError:
        return 'no NVIDIA driver (libcuda.so.1)'
    count = ctypes.c_int(0)
    if driver.cuInit(0) != 0 or driver.cuDeviceGetCount(ctypes.byref(count)) != 0 or count.value == 0:
        return 'no CUDA device'
    try:
        for name in CUDA_RUNTIME:
            ctypes.CDLL(name, mode=ctypes.RTLD_GLOBAL)
        for name in CUDNN:
            ctypes.CDLL(str(env.tts_gpu / 'nvidia' / 'cudnn' / 'lib' / name), mode=ctypes.RTLD_GLOBAL)
    except OSError as e:
        return f'missing CUDA library: {e}'
    return None


def select_device(env: Environment, requested: str = 'auto') -> tuple[str, str]:
    """The narration device ('cuda' or 'cpu') and why it was chosen."""
    if requested == 'cpu':
        return 'cpu', 'requested with --tts-device cpu'
    problem = _gpu_problem(env)
    if problem:
        if requested == 'cuda':
            raise RuntimeError(f'--tts-device cuda: {problem}')
        return 'cpu', problem
    sys.path.insert(0, str(env.tts_gpu))
    return 'cuda', 'CUDA'


def durations_model(env: Environment) -> Path:
    """The voice model with its per-phoneme durations as a second output,
    made once beside the voice and remade when the voice changes."""
    source = env.voices / f'{VOICE}.onnx'
    target = env.voices / f'{VOICE}-durations.onnx'
    stamp = target.with_suffix('.source-sha256')
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    if target.exists() and stamp.exists() and stamp.read_text() == digest:
        return target
    import onnx
    model = onnx.load(str(source))
    if not any(node.output and node.output[0] == DURATIONS_TENSOR for node in model.graph.node):
        raise RuntimeError(f'{source.name} has no {DURATIONS_TENSOR} tensor: word timings need the per-phoneme durations')
    model.graph.output.append(onnx.helper.make_tensor_value_info(DURATIONS_TENSOR, onnx.TensorProto.FLOAT, None))
    tmp = target.with_suffix('.tmp')
    onnx.save(model, str(tmp))
    tmp.rename(target)
    stamp.write_text(digest)
    return target


def create_voice(env: Environment, device: str, workdir: Path) -> Any:
    """The Piper voice, running the model that also outputs its durations.
    eSpeak's native path buffer is short, so its data is reached through a
    relative link in `workdir`, which must be the current directory while
    synthesising or phonemizing."""
    import numpy as np
    import onnxruntime as ort
    from piper import PiperVoice
    from piper.config import PiperConfig
    from piper.phonemize_espeak import ESPEAK_DATA_DIR
    ort.disable_telemetry_events()
    np.random.seed(SEED)
    model = durations_model(env)
    link = workdir / 'espeak-ng-data'
    if not link.exists():
        link.symlink_to(os.path.relpath(ESPEAK_DATA_DIR, workdir), target_is_directory=True)
    options = ort.SessionOptions()
    options.intra_op_num_threads = 2
    options.inter_op_num_threads = 1
    if device == 'cuda':
        # HEURISTIC picks cuDNN convolutions without benchmarking; the default
        # search leaves the decoder's last convolution in a slow fallback.
        options.log_severity_level = 3
        providers: list[Any] = [('CUDAExecutionProvider', {'cudnn_conv_algo_search': 'HEURISTIC'}), 'CPUExecutionProvider']
    else:
        providers = ['CPUExecutionProvider']
    session = ort.InferenceSession(str(model), sess_options=options, providers=providers)
    if device == 'cuda' and session.get_providers()[0] != 'CUDAExecutionProvider':
        raise RuntimeError(f'ONNX Runtime did not start the CUDA provider: {session.get_providers()}')
    config = PiperConfig.from_dict(json.loads((env.voices / f'{VOICE}.onnx.json').read_text()))
    return PiperVoice(config=config, session=session, espeak_data_dir=Path('espeak-ng-data'))


def pronounce(cfg: Config, text: str) -> str:
    """`text` with the words of video.json's `pronunciations` replaced by how they are said."""
    for word, said in sorted(cfg.spec.get('pronunciations', {}).items(), key=lambda p: -len(p[0])):
        text = re.sub(r'(?<![\w])' + re.escape(word) + r'(?![\w])', said, text)
    return text


def spoken(cfg: Config, sentence: dict[str, Any]) -> str:
    """What is said: the English text (the zh-CN edition keeps it in `en`),
    its tags removed and its formulas said as their `say`, with the words of
    video.json's `pronunciations` replaced by how they are said. Captions keep
    the written form."""
    return pronounce(cfg, markup.parse(sentence.get('en', sentence['text'])).spoken)


def create_phonemizer(env: Environment, workdir: Path) -> Any:
    """The voice for phonemizing only: its config and eSpeak, no model."""
    from piper import PiperVoice
    from piper.config import PiperConfig
    from piper.phonemize_espeak import ESPEAK_DATA_DIR
    link = workdir / 'espeak-ng-data'
    if not link.exists():
        link.symlink_to(os.path.relpath(ESPEAK_DATA_DIR, workdir), target_is_directory=True)
    config = PiperConfig.from_dict(json.loads((env.voices / f'{VOICE}.onnx.json').read_text()))
    return PiperVoice(config=config, session=None, espeak_data_dir=Path('espeak-ng-data'))


def marks(cfg: Config, phonemizer: Any, parsed: markup.Sentence, spoken_words: int, where: str) -> list[int]:
    """The spoken word each tag stands before: the sentence's words less the
    words of the text after the tag. Where the voice reads two words across a
    tag as one ("from the"), the tag lands on that word's start."""
    def words_after(piece: int) -> int:
        text = ' '.join(p for p in parsed.pieces[piece:] if p.strip())
        return len(phoneme_words(phonemizer, pronounce(cfg, text))) if text.strip() else 0
    out: list[int] = []
    for tag in parsed.tags:
        mark = max(0, spoken_words - words_after(tag.piece))
        if out and mark < out[-1]:
            raise markup.MarkupError(f'{where}: its tags fall out of order on the spoken words')
        out.append(mark)
    return out


def phoneme_words(voice: Any, text: str) -> list[list[str]]:
    """The spoken words of `text` as Piper phonemizes it: the phonemes between
    its spaces, across all its eSpeak sentences."""
    words: list[list[str]] = []
    for phonemes in voice.phonemize(text):
        current: list[str] = []
        for p in phonemes:
            if p == ' ':
                if current:
                    words.append(current)
                current = []
            else:
                current.append(p)
        if current:
            words.append(current)
    return words


def speak(voice: Any, text: str, syn_config: Any) -> tuple[bytes, int, list[float]]:
    """Piper's `synthesize_wav`, keeping each word's start: the int16 audio,
    its sample rate, and the second each phonemized word starts at."""
    import numpy as np
    from piper.const import BOS, EOS, PAD
    config = voice.config
    id_map = config.phoneme_id_map
    noise_scale = config.noise_scale if syn_config.noise_scale is None else syn_config.noise_scale
    length_scale = config.length_scale if syn_config.length_scale is None else syn_config.length_scale
    noise_w = config.noise_w_scale if syn_config.noise_w_scale is None else syn_config.noise_w_scale
    chunks: list[bytes] = []
    starts: list[float] = []
    offset = 0
    for phonemes in voice.phonemize(text):
        # phonemes_to_ids, remembering where each phoneme's id lands.
        ids: list[int] = [*id_map[BOS], *id_map[PAD]]
        at: list[int | None] = []
        for p in phonemes:
            if p not in id_map:
                at.append(None)
                continue
            at.append(len(ids))
            ids.extend(id_map[p])
            ids.extend(id_map[PAD])
        ids.extend(id_map[EOS])
        args: dict[str, Any] = {
            'input': np.expand_dims(np.array(ids, dtype=np.int64), 0),
            'input_lengths': np.array([len(ids)], dtype=np.int64),
            'scales': np.array([noise_scale, length_scale, noise_w], dtype=np.float32),
        }
        if config.num_speakers > 1:
            args['sid'] = np.array([syn_config.speaker_id or 0], dtype=np.int64)
        audio, frames = voice.session.run(None, args)
        audio = audio.squeeze()
        frames = frames.reshape(-1)
        if int(frames.sum()) * HOP != audio.shape[0]:
            raise RuntimeError(f'phoneme durations ({int(frames.sum())} x {HOP}) do not add up to the audio ({audio.shape[0]} samples)')
        before = np.concatenate([[0], np.cumsum(frames)[:-1]]) * HOP
        word_start = True
        for p, index in zip(phonemes, at):
            if p == ' ':
                word_start = True
            elif word_start and index is not None:
                starts.append((offset + float(before[index])) / config.sample_rate)
                word_start = False
        # The rest of Piper's synthesize: normalise, scale, clip, int16.
        if syn_config.normalize_audio:
            peak = np.max(np.abs(audio))
            audio = np.zeros_like(audio) if peak < 1e-8 else audio / peak
        if syn_config.volume != 1.0:
            audio = audio * syn_config.volume
        audio = np.clip(audio, -1.0, 1.0).astype(np.float32)
        chunks.append(np.clip(audio * 32767.0, -32767.0, 32767.0).astype(np.int16).tobytes())
        offset += audio.shape[0]
    return b''.join(chunks), config.sample_rate, starts


def synthesize(cfg: Config, env: Environment, device: str, sentences: list[dict[str, Any]]) -> Durations:
    """The narration of every sentence, keyed by the sentence's (shown) text."""
    from piper.config import SynthesisConfig
    audio = cfg.audio_cache
    audio.mkdir(parents=True, exist_ok=True)
    voice = None
    phonemizer = None
    durations: Durations = {}
    here = Path.cwd()
    os.chdir(audio)
    try:
        for i, sentence in enumerate(sentences):
            text = spoken(cfg, sentence)
            digest = hashlib.sha256((text + str(VOICE_SPEED) + VOICE_ID).encode()).hexdigest()[:24]
            target = audio / f'{digest}.wav'
            sidecar = audio / f'{digest}.json'
            # A wav made before word timings has no sidecar: make both again.
            if not (target.exists() and sidecar.exists()):
                if voice is None:
                    voice = create_voice(env, device, audio)
                pcm, rate, starts = speak(voice, text, SynthesisConfig(length_scale=VOICE_SPEED))
                tmp = target.with_suffix('.tmp.wav')
                with wave.open(str(tmp), 'wb') as output:
                    output.setframerate(rate)
                    output.setsampwidth(2)
                    output.setnchannels(1)
                    output.writeframes(pcm)
                sidecar.write_text(json.dumps({'text': text, 'words': starts}))
                tmp.rename(target)
            with wave.open(str(target), 'rb') as stream:
                seconds, rate = stream.getnframes() / stream.getframerate(), stream.getframerate()
            words = json.loads(sidecar.read_text())['words']
            parsed = markup.parse(sentence.get('en', sentence['text']))
            tag_words: list[int] = []
            if parsed.tags:
                phonemizer = phonemizer or create_phonemizer(env, audio)
                tag_words = marks(cfg, phonemizer, parsed, len(words), repr(sentence['text'][:60]))
            measured = Narration(target, seconds, rate, words, tag_words)
            if sentence['text'] in durations and durations[sentence['text']][:3] != measured[:3]:
                raise RuntimeError(f'two sentences share the text {sentence["text"]!r} but not their narration')
            durations[sentence['text']] = measured
            if (i + 1) % 25 == 0 or i + 1 == len(sentences):
                log(f'Narration {i + 1}/{len(sentences)} sentences')
    finally:
        os.chdir(here)
    return durations
