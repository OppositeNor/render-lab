"""Narration: one Piper wav per sentence, cached by the hash of its text, the
speed, and the voice, on the GPU when the CUDA build of ONNX Runtime is
installed (tools/video/setup.sh --gpu), otherwise on the CPU.

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
from typing import Any

from config import Config, Environment, log

VOICE = 'en_US-lessac-medium'
VOICE_ID = 'piper-1.3.0-lessac-medium'
VOICE_SPEED = 1.02
SEED = 20261024

# cuDNN 9 loads its sub-libraries by name; load them first, by path.
CUDNN = ['libcudnn_graph.so.9', 'libcudnn_ops.so.9', 'libcudnn_heuristic.so.9', 'libcudnn_engines_runtime_compiled.so.9',
         'libcudnn_engines_precompiled.so.9', 'libcudnn_cnn.so.9', 'libcudnn_adv.so.9', 'libcudnn.so.9']
CUDA_RUNTIME = ['libcudart.so.13', 'libcublasLt.so.13', 'libcublas.so.13', 'libcurand.so.10', 'libcufft.so.12']

Durations = dict[str, tuple[Path, float, int]]


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


def create_voice(env: Environment, device: str, workdir: Path) -> Any:
    """The Piper voice. eSpeak's native path buffer is short, so its data is
    reached through a relative link in `workdir`, which must be the current
    directory while synthesising."""
    import numpy as np
    import onnxruntime as ort
    from piper import PiperVoice
    from piper.config import PiperConfig
    from piper.phonemize_espeak import ESPEAK_DATA_DIR
    ort.disable_telemetry_events()
    np.random.seed(SEED)
    model = env.voices / f'{VOICE}.onnx'
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
    config = PiperConfig.from_dict(json.loads(Path(str(model) + '.json').read_text()))
    return PiperVoice(config=config, session=session, espeak_data_dir=Path('espeak-ng-data'))


def spoken(cfg: Config, sentence: dict[str, Any]) -> str:
    """What is said: the English text (the zh-CN edition keeps it in `en`),
    with the words of video.json's `pronunciations` replaced by how they are
    said. Captions keep the written form."""
    text = sentence.get('en', sentence['text'])
    for word, said in sorted(cfg.spec.get('pronunciations', {}).items(), key=lambda p: -len(p[0])):
        text = re.sub(r'(?<![\w])' + re.escape(word) + r'(?![\w])', said, text)
    return text


def synthesize(cfg: Config, env: Environment, device: str, sentences: list[dict[str, Any]]) -> Durations:
    """The narration of every sentence, keyed by the sentence's (shown) text."""
    from piper.config import SynthesisConfig
    audio = cfg.audio_cache
    audio.mkdir(parents=True, exist_ok=True)
    voice = None
    durations: Durations = {}
    here = Path.cwd()
    os.chdir(audio)
    try:
        for i, sentence in enumerate(sentences):
            text = spoken(cfg, sentence)
            digest = hashlib.sha256((text + str(VOICE_SPEED) + VOICE_ID).encode()).hexdigest()[:24]
            target = audio / f'{digest}.wav'
            if not target.exists():
                if voice is None:
                    voice = create_voice(env, device, audio)
                tmp = target.with_suffix('.tmp.wav')
                with wave.open(str(tmp), 'wb') as output:
                    voice.synthesize_wav(text, output, syn_config=SynthesisConfig(length_scale=VOICE_SPEED))
                tmp.rename(target)
            with wave.open(str(target), 'rb') as stream:
                measured = (target, stream.getnframes() / stream.getframerate(), stream.getframerate())
            if durations.get(sentence['text'], measured) != measured:
                raise RuntimeError(f'two sentences share the text {sentence["text"]!r} but not their narration')
            durations[sentence['text']] = measured
            if (i + 1) % 25 == 0 or i + 1 == len(sentences):
                log(f'Narration {i + 1}/{len(sentences)} sentences')
    finally:
        os.chdir(here)
    return durations
