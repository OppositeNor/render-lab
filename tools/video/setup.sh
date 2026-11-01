#!/usr/bin/env bash
# The video build's local environment, all under render-lab/.cache (git-ignored):
# a Conda prefix with Python and FFmpeg; Piper; the voice. `--gpu` adds the
# CUDA packages for GPU narration. Nothing is installed outside the lab.
set -euo pipefail
tools="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
lab="$(cd -- "$tools/../.." && pwd)"
prefix="${RENDER_LAB_VIDEO_ENV:-$lab/.cache/video-env}"
voices="${RENDER_LAB_VOICES:-$lab/.cache/video-voices}"
gpu_packages="${RENDER_LAB_TTS_GPU:-$lab/.cache/video-tts-gpu}"
export CONDA_PKGS_DIRS="$lab/.cache/conda-pkgs"
export PIP_CACHE_DIR="$lab/.cache/pip"
if [[ ! -x "$prefix/bin/python" ]]; then
  conda env create --prefix "$prefix" --file "$tools/environment.yml" --yes
fi
export PATH="$prefix/bin:$PATH"
python -m pip install --disable-pip-version-check -r "$tools/requirements.txt"
python "$tools/download_voice.py" "$voices"
if [[ "${1:-}" == "--gpu" ]]; then
  python -m pip install --disable-pip-version-check --target "$gpu_packages" --no-deps -r "$tools/requirements-gpu.txt"
fi
printf 'Video environment ready. Build with: python3 tools/video/build.py --investigation ENGINE/FEATURE\n'
