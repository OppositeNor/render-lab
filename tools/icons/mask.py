"""Write a white icon mask as a PNG: a white image whose alpha is the shape, which the homepage tints and scales.

The field icons' scripts (field_*.py beside this file) give it the shape as a function of the point: its opacity
there, from 0 to 1. Every pixel averages SAMPLES x SAMPLES points of it, so edges are antialiased.
"""

import struct
import zlib
from collections.abc import Callable
from pathlib import Path

SAMPLES = 4


def pixels(size: int, opacity: Callable[[float, float], float]) -> bytes:
    """The PNG's filtered rows: white, the alpha of each pixel the mean opacity of its samples (pixel units)."""
    rows = bytearray()
    for py in range(size):
        rows.append(0)  # PNG filter: none.
        for px in range(size):
            total = sum(
                opacity(px + (sx + 0.5) / SAMPLES, py + (sy + 0.5) / SAMPLES)
                for sx in range(SAMPLES)
                for sy in range(SAMPLES)
            )
            rows.extend((255, 255, 255, round(255 * min(1.0, max(0.0, total / SAMPLES**2)))))
    return bytes(rows)


def chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def write(out: Path, size: int, opacity: Callable[[float, float], float]) -> None:
    """Draw `opacity` on a `size` x `size` image and write it to `out` as an RGBA PNG."""
    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    data = zlib.compress(pixels(size, opacity), 9)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", data) + chunk(b"IEND", b"")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(png)
    print(f"{out} ({len(png)} bytes)")
