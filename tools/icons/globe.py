"""Draw the homepage's globe icon (packages/render_lab/assets/icons/globe.png).

A white globe on a transparent background: its outline, the equator, two
parallels, the central meridian, and one pair of meridians as an ellipse.
Every pixel averages 4x4 samples, so the strokes are antialiased; the
homepage tints and scales it.

    python3 tools/icons/globe.py [output.png]
"""

import math
import struct
import sys
import zlib
from pathlib import Path

SIZE = 64
RADIUS = 28.0
STROKE = 3.6
SAMPLES = 4


def covered(x: float, y: float) -> bool:
    """Whether the point (relative to the centre, in pixels) is on a stroke."""
    half = STROKE / 2.0
    r = math.hypot(x, y)
    if abs(r - RADIUS) <= half:
        return True
    if r > RADIUS:
        return False
    # The equator, two parallels, and the central meridian.
    if any(abs(y - level) <= half for level in (0.0, -RADIUS * 0.5, RADIUS * 0.5)) or abs(x) <= half:
        return True
    # Two meridians, as an ellipse: its implicit function over its gradient's length.
    a = RADIUS * 0.5
    f = (x / a) ** 2 + (y / RADIUS) ** 2 - 1.0
    gradient = math.hypot(2.0 * x / a**2, 2.0 * y / RADIUS**2)
    return gradient > 0.0 and abs(f) / gradient <= half


def pixels() -> bytes:
    rows = bytearray()
    centre = SIZE / 2.0
    for py in range(SIZE):
        rows.append(0)  # PNG filter: none.
        for px in range(SIZE):
            hits = sum(
                covered(px + (sx + 0.5) / SAMPLES - centre, py + (sy + 0.5) / SAMPLES - centre)
                for sx in range(SAMPLES)
                for sy in range(SAMPLES)
            )
            rows.extend((255, 255, 255, round(255 * hits / SAMPLES**2)))
    return bytes(rows)


def chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def main(argv: list[str]) -> None:
    root = Path(__file__).resolve().parents[2]
    out = Path(argv[1]) if len(argv) > 1 else root / "packages/render_lab/assets/icons/globe.png"
    header = struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(pixels(), 9)) + chunk(b"IEND", b"")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(png)
    print(f"{out} ({len(png)} bytes)")


if __name__ == "__main__":
    main(sys.argv)
