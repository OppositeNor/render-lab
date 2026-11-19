"""Draw the homepage's Other folder icon (packages/render_lab/assets/icons/field-other.png).

A folder's outline: a rounded body with a tab on its upper left, for the investigations of packages that join no
field. Written as a white mask (tools/icons/mask.py).

    python3 tools/icons/field_other.py [output.png]
"""

import math
import sys
from pathlib import Path

from mask import write

SIZE = 256
STROKE = 10.0
# The body's corners and corner radius; the tab's left and right ends and its height above the body.
BODY = (28.0, 76.0, 228.0, 212.0)
CORNER = 18.0
TAB = (28.0, 112.0, 30.0)


def rounded_distance(x: float, y: float, box: tuple[float, float, float, float], radius: float) -> float:
    """The signed distance from the point to a rounded rectangle's edge (negative inside)."""
    cx, cy = (box[0] + box[2]) / 2.0, (box[1] + box[3]) / 2.0
    hx, hy = (box[2] - box[0]) / 2.0 - radius, (box[3] - box[1]) / 2.0 - radius
    qx, qy = abs(x - cx) - hx, abs(y - cy) - hy
    outside = math.hypot(max(qx, 0.0), max(qy, 0.0))
    return outside + min(max(qx, qy), 0.0) - radius


def opacity(x: float, y: float) -> float:
    """The folder at an image point: the body's outline and the tab's, as one stroke."""
    body = rounded_distance(x, y, BODY, CORNER)
    # The tab reaches down past the body's corner, so their left edges meet in one straight line.
    tab_box = (TAB[0], BODY[1] - TAB[2], TAB[1], BODY[1] + 3.0 * CORNER)
    tab = rounded_distance(x, y, tab_box, CORNER / 2.0)
    # The outline of the body and the tab joined: the edge of their union.
    union = min(body, tab)
    return 1.0 if abs(union) <= STROKE / 2.0 else 0.0


def main(argv: list[str]) -> None:
    root = Path(__file__).resolve().parents[2]
    write(Path(argv[1]) if len(argv) > 1 else root / "packages/render_lab/assets/icons/field-other.png", SIZE, opacity)


if __name__ == "__main__":
    main(sys.argv)
