"""Download the Piper voice once, so builds then run without network access."""
from __future__ import annotations

import hashlib
import json
import sys
import urllib.request
from pathlib import Path

BASE = 'https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/medium/'
FILES = ['en_US-lessac-medium.onnx', 'en_US-lessac-medium.onnx.json', 'MODEL_CARD']


def main(folder: Path) -> None:
    folder.mkdir(parents=True, exist_ok=True)
    checksums: dict[str, str] = {}
    for name in FILES:
        path = folder / name
        if not path.exists():
            print(f'Downloading voice asset: {name}', flush=True)
            partial = path.with_name(name + '.download')
            with urllib.request.urlopen(BASE + name, timeout=120) as response, partial.open('wb') as output:
                while chunk := response.read(1024 * 1024):
                    output.write(chunk)
            partial.replace(path)
        checksums[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    (folder / 'checksums.json').write_text(json.dumps(checksums, indent=2) + '\n')
    print('Narration voice ready.', flush=True)


if __name__ == '__main__':
    main(Path(sys.argv[1]))
