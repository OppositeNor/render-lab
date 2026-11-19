"""Draw the Computer Graphics field's icon (packages/render_lab/assets/icons/field-computer-graphics.png).

A faceted sphere, flat-shaded by a light at the upper left, with the light's ray drawn as an arrow coming to it:
rendering, a mesh lit by a light. Each facet's opacity is its Lambert term, so the homepage's tint shades it; the
edges between facets are left clear. Written as a white mask (tools/icons/mask.py).

    python3 tools/icons/field_computer_graphics.py [output.png]
"""

import math
import sys
from pathlib import Path

from mask import write

SIZE = 256
CENTRE = (148.0, 150.0)
RADIUS = 92.0
LONGITUDES = 10
LATITUDES = 7
# The sphere's tilt toward the viewer, and the light's direction (towards the light: up, left, and front).
TILT = math.radians(20.0)
LIGHT = (-0.55, 0.62, 0.56)
EDGE = 1.6
STROKE = 7.0
# The ray: from near the image's corner to just short of the sphere, its arrowhead's length and half-width.
RAY_FROM = (22.0, 24.0)
RAY_TO = (78.0, 80.0)
HEAD = (30.0, 17.0)


def facet(x: float, y: float) -> tuple[int, int] | None:
    """The facet (longitude band, latitude band) under the image point, or None off the sphere."""
    u, v = (x - CENTRE[0]) / RADIUS, (CENTRE[1] - y) / RADIUS
    r2 = u * u + v * v
    if r2 > 1.0:
        return None
    w = math.sqrt(1.0 - r2)
    # Undo the tilt about the x axis, to the sphere's own frame (y its pole).
    sy = v * math.cos(TILT) - w * math.sin(TILT)
    sz = v * math.sin(TILT) + w * math.cos(TILT)
    longitude = (math.atan2(u, sz) + math.pi) / (2.0 * math.pi)
    latitude = math.acos(max(-1.0, min(1.0, sy))) / math.pi
    return (min(LONGITUDES - 1, int(longitude * LONGITUDES)), min(LATITUDES - 1, int(latitude * LATITUDES)))


def brightness(band: tuple[int, int]) -> float:
    """A facet's Lambert term: its centre's normal, tilted to the view, against the light."""
    longitude = ((band[0] + 0.5) / LONGITUDES) * 2.0 * math.pi - math.pi
    latitude = ((band[1] + 0.5) / LATITUDES) * math.pi
    nx = math.sin(latitude) * math.sin(longitude)
    ny = math.cos(latitude)
    nz = math.sin(latitude) * math.cos(longitude)
    # Tilt the normal toward the viewer, as the sphere is drawn.
    vy = ny * math.cos(TILT) + nz * math.sin(TILT)
    vz = -ny * math.sin(TILT) + nz * math.cos(TILT)
    length = math.hypot(*LIGHT)
    return max(0.0, (nx * LIGHT[0] + vy * LIGHT[1] + vz * LIGHT[2]) / length)


def on_ray(x: float, y: float) -> bool:
    """Whether the image point is on the ray's shaft or its arrowhead."""
    (ax, ay), (bx, by) = RAY_FROM, RAY_TO
    dx, dy = bx - ax, by - ay
    length = math.hypot(dx, dy)
    dx, dy = dx / length, dy / length
    # Along the ray and across it, from its start.
    along = (x - ax) * dx + (y - ay) * dy
    across = abs(-(x - ax) * dy + (y - ay) * dx)
    shaft = length - HEAD[0]
    if 0.0 <= along <= shaft + 1.0 and across <= STROKE / 2.0:
        return True
    return shaft <= along <= length and across <= HEAD[1] * (length - along) / HEAD[0]


def opacity(x: float, y: float) -> float:
    """The icon at an image point: the ray, the facets' shading, clear edges between facets."""
    if on_ray(x, y):
        return 1.0
    band = facet(x, y)
    if band is None:
        return 0.0
    # A point whose neighbours lie on another facet is on an edge, left clear.
    for ox, oy in ((EDGE, 0.0), (-EDGE, 0.0), (0.0, EDGE), (0.0, -EDGE)):
        other = facet(x + ox, y + oy)
        if other is not None and other != band:
            return 0.0
    return 0.22 + 0.78 * brightness(band)


def main(argv: list[str]) -> None:
    root = Path(__file__).resolve().parents[2]
    default = root / "packages/render_lab/assets/icons/field-computer-graphics.png"
    write(Path(argv[1]) if len(argv) > 1 else default, SIZE, opacity)


if __name__ == "__main__":
    main(sys.argv)
