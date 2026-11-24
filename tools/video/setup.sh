#!/usr/bin/env bash
# The video build's local environment, all under render-lab/.cache (git-ignored):
# a Conda prefix with Python and FFmpeg; Piper; the voice. `--gpu` adds the
# CUDA packages for GPU narration. `--zh` adds the Chinese voice: a Conda
# prefix of its own with PyTorch, Qwen3-TTS, Qwen3-ASR, and the forced aligner,
# and their models (about 10 GB) in .cache/huggingface. Nothing is installed
# outside the lab.
set -euo pipefail
tools="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
lab="$(cd -- "$tools/../.." && pwd)"
prefix="${RENDER_LAB_VIDEO_ENV:-$lab/.cache/video-env}"
voices="${RENDER_LAB_VOICES:-$lab/.cache/video-voices}"
gpu_packages="${RENDER_LAB_TTS_GPU:-$lab/.cache/video-tts-gpu}"
zh_prefix="${RENDER_LAB_TTS_ZH:-$lab/.cache/video-tts-zh}"
models="${RENDER_LAB_TTS_MODELS:-$lab/.cache/huggingface}"
gpu=0
zh=0
for arg in "$@"; do
  case "$arg" in
    --gpu) gpu=1 ;;
    --zh) zh=1 ;;
    *) printf 'usage: setup.sh [--gpu] [--zh]\n' >&2; exit 2 ;;
  esac
done
export CONDA_PKGS_DIRS="$lab/.cache/conda-pkgs"
export PIP_CACHE_DIR="$lab/.cache/pip"
if [[ ! -x "$prefix/bin/python" ]]; then
  conda env create --prefix "$prefix" --file "$tools/environment.yml" --yes
fi
export PATH="$prefix/bin:$PATH"
python -m pip install --disable-pip-version-check -r "$tools/requirements.txt"
python "$tools/download_voice.py" "$voices"
if [[ $gpu == 1 ]]; then
  python -m pip install --disable-pip-version-check --target "$gpu_packages" --no-deps -r "$tools/requirements-gpu.txt"
fi
if [[ $zh == 1 ]]; then
  if [[ ! -x "$zh_prefix/bin/python" ]]; then
    conda env create --prefix "$zh_prefix" --file "$tools/environment-zh.yml" --yes
  fi
  "$zh_prefix/bin/python" -m pip install --disable-pip-version-check -r "$tools/requirements-zh.txt"
  "$zh_prefix/bin/python" -m pip install --disable-pip-version-check --no-deps -r "$tools/requirements-zh-asr.txt"
  # The models at the revisions speech_zh.py names (MODELS), read from this cache only when the voice runs.
  "$zh_prefix/bin/python" - "$models" "$tools" <<'PY'
import os, sys
os.environ['HF_HOME'] = sys.argv[1]
sys.path.insert(0, sys.argv[2])
from huggingface_hub import snapshot_download
from speech_zh import MODELS
for model in MODELS.values():
    print(snapshot_download(model['repository'], revision=model['revision']))
PY
fi
printf 'Video environment ready. Build with: python3 tools/video/build.py --package PACKAGE\n'
