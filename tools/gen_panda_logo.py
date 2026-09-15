"""Generate braille panda-head logos for the welcome screen."""

from __future__ import annotations

import math
from pathlib import Path

# Braille dots:
# 1 4
# 2 5
# 3 6
# 7 8
DOT_MAP = [
    (0, 0, 0),  # bit 0, dot 1
    (0, 1, 1),  # bit 1, dot 2
    (0, 2, 2),  # bit 2, dot 3
    (1, 0, 3),  # bit 3, dot 4
    (1, 1, 4),  # bit 4, dot 5
    (1, 2, 5),  # bit 5, dot 6
    (0, 3, 6),  # bit 6, dot 7
    (1, 3, 7),  # bit 7, dot 8
]


def dist(x, y, cx, cy) -> float:
    return math.hypot(x - cx, y - cy)


def rotated_ellipse(x, y, cx, cy, rx, ry, deg) -> float:
    """Normalized radius: <1 inside, =1 on edge."""
    a = math.radians(deg)
    dx, dy = x - cx, y - cy
    xr = dx * math.cos(a) + dy * math.sin(a)
    yr = -dx * math.sin(a) + dy * math.cos(a)
    return math.hypot(xr / rx, yr / ry)


def stroke_circle(x, y, cx, cy, r, w) -> bool:
    d = dist(x, y, cx, cy)
    return abs(d - r) <= w / 2


def fill_circle(x, y, cx, cy, r) -> bool:
    return dist(x, y, cx, cy) <= r


def fill_ellipse(x, y, cx, cy, rx, ry, deg=0) -> bool:
    return rotated_ellipse(x, y, cx, cy, rx, ry, deg) <= 1.0


def stroke_ellipse(x, y, cx, cy, rx, ry, w, deg=0) -> bool:
    n = rotated_ellipse(x, y, cx, cy, rx, ry, deg)
    # convert to approximate pixel thickness
    return abs(n - 1.0) * ((rx + ry) / 2) <= w / 2


def stroke_quad(x, y, p0, p1, p2, w) -> bool:
    """Distance to quadratic bezier <= w/2."""
    best = 1e9
    steps = 64
    for i in range(steps + 1):
        t = i / steps
        u = 1 - t
        bx = u * u * p0[0] + 2 * u * t * p1[0] + t * t * p2[0]
        by = u * u * p0[1] + 2 * u * t * p1[1] + t * t * p2[1]
        best = min(best, dist(x, y, bx, by))
    return best <= w / 2


def stroke_arc(x, y, cx, cy, r, deg0, deg1, w) -> bool:
    """y-down canvas: 0° = right, 90° = down, 180° = left."""
    d = dist(x, y, cx, cy)
    if abs(d - r) > w / 2:
        return False
    ang = math.degrees(math.atan2(y - cy, x - cx)) % 360
    a0, a1 = deg0 % 360, deg1 % 360
    if a0 <= a1:
        return a0 <= ang <= a1
    return ang >= a0 or ang <= a1


def paint(w: int, h: int, fn, fill_hits: int = 2) -> list[list[int]]:
    grid = [[0] * w for _ in range(h)]
    for y in range(h):
        for x in range(w):
            hits = 0
            for sy in (0.25, 0.75):
                for sx in (0.25, 0.75):
                    if fn(x + sx, y + sy):
                        hits += 1
            grid[y][x] = 1 if hits >= fill_hits else 0
    return grid


def set_px(grid, x, y, v=1) -> None:
    if 0 <= y < len(grid) and 0 <= x < len(grid[0]):
        grid[y][x] = v


def stamp_sparkle(grid, cx, cy) -> None:
    """A 2-pixel eye glint (not a punched hole)."""
    set_px(grid, cx, cy, 0)
    set_px(grid, cx + 1, cy, 0)
    set_px(grid, cx, cy + 1, 1)


def stamp_nose(grid, cx, cy, rx=2, ry=1) -> None:
    for y in range(cy - ry, cy + ry + 1):
        for x in range(cx - rx, cx + rx + 1):
            if ((x - cx) / (rx + 0.35)) ** 2 + ((y - cy) / (ry + 0.35)) ** 2 <= 1.0:
                set_px(grid, x, y, 1)


def stamp_smile(grid, cx, cy, half_w: int, depth: int) -> None:
    """Parabolic U, 1px thick, sitting above the chin."""
    span = 2 * half_w
    for i in range(span + 1):
        t = i / span
        x = cx - half_w + i
        y = cy + int(round(4 * depth * t * (1 - t)))
        set_px(grid, x, y, 1)


def to_braille(grid: list[list[int]]) -> str:
    h = len(grid)
    w = len(grid[0])
    rows = (h + 3) // 4
    cols = (w + 1) // 2
    lines = []
    for r in range(rows):
        chars = []
        for c in range(cols):
            bits = 0
            for dx, dy, bit in DOT_MAP:
                x, y = c * 2 + dx, r * 4 + dy
                if y < h and x < w and grid[y][x]:
                    bits |= 1 << bit
            chars.append(chr(0x2800 + bits))
        lines.append("".join(chars).rstrip("\u2800"))
    # pad to common width with braille blanks so the art stays rectangular
    width = max(len(line) for line in lines)
    lines = [line + "\u2800" * (width - len(line)) for line in lines]
    return "\n".join(lines) + "\n"


def preview(grid: list[list[int]]) -> str:
    blocks = {0: " ", 1: "█"}
    return "\n".join("".join(blocks[v] for v in row) for row in grid)


# ---------------------------------------------------------------------------
# Full logo: 48x28 pixels → 24 cols × 7 rows
# Compact:  36x20 pixels → 18 cols × 5 rows
# ---------------------------------------------------------------------------


def panda_full(x: float, y: float) -> bool:
    # Canvas 48×28. Solid eye-patches (logo mark), thin jaw, separate ears.
    if fill_circle(x, y, 8.2, 5.0, 5.2):
        return True
    if fill_circle(x, y, 39.8, 5.0, 5.2):
        return True

    # 1px crown between the inner ear edges.
    if stroke_quad(x, y, (13.4, 8.0), (24.0, 6.8), (34.6, 8.0), 1.05):
        return True

    # Cheeks + chin. Clip the top so it meets the ears instead of redrawing them.
    if stroke_ellipse(x, y, 24.0, 15.4, 16.6, 11.2, 1.2, 0) and y > 11.0:
        return True

    # Teardrop patches aimed at the ears; muzzle stays open.
    if fill_ellipse(x, y, 15.4, 14.4, 5.45, 6.45, -36):
        return True
    if fill_ellipse(x, y, 32.6, 14.4, 5.45, 6.45, 36):
        return True

    return False


def panda_compact(x: float, y: float) -> bool:
    # Canvas 36×20. Ears stay separate; skip the crown bar (it fills at this size).
    if fill_circle(x, y, 6.0, 3.6, 3.9):
        return True
    if fill_circle(x, y, 30.0, 3.6, 3.9):
        return True

    if stroke_ellipse(x, y, 18.0, 11.2, 12.6, 8.0, 1.15, 0) and y > 8.2:
        return True

    if fill_ellipse(x, y, 11.6, 10.4, 4.15, 4.55, -22):
        return True
    if fill_ellipse(x, y, 24.4, 10.4, 4.15, 4.55, 22):
        return True

    return False


def main() -> None:
    full = paint(48, 28, panda_full)
    compact = paint(36, 20, panda_compact)
    # Inner-ear pits.
    set_px(full, 8, 4, 0)
    set_px(full, 39, 4, 0)
    # 2×2 eye glints, high on each patch.
    for dx, dy in ((0, 0), (1, 0), (0, 1), (1, 1)):
        set_px(full, 16 + dx, 12 + dy, 0)
        set_px(full, 30 + dx, 12 + dy, 0)
    # Button nose (4×2).
    for x in range(22, 26):
        set_px(full, x, 18, 1)
        set_px(full, x, 19, 1)
    # Balanced U smile, 1px gap under the nose. Extra corner pixels close
    # the knight-move gaps so it reads as one stroke.
    for x, y in (
        (19, 21),
        (19, 22),
        (20, 22),
        (21, 23),
        (22, 23),
        (22, 24),
        (23, 24),
        (24, 24),
        (25, 24),
        (26, 24),
        (26, 23),
        (27, 23),
        (28, 22),
        (29, 22),
        (29, 21),
    ):
        set_px(full, x, y, 1)
    set_px(compact, 12, 8, 0)
    set_px(compact, 13, 8, 0)
    set_px(compact, 22, 8, 0)
    set_px(compact, 23, 8, 0)
    for x in range(17, 20):
        set_px(compact, x, 13, 1)
        set_px(compact, x, 14, 1)
    for x, y in (
        (14, 15),
        (15, 16),
        (16, 16),
        (17, 16),
        (18, 16),
        (19, 16),
        (20, 16),
        (21, 16),
        (22, 15),
    ):
        set_px(compact, x, y, 1)
    print("=== FULL 48x28 ===")
    print(preview(full))
    print()
    print(to_braille(full))
    print("=== COMPACT 36x20 ===")
    print(preview(compact))
    print()
    print(to_braille(compact))

    root = Path(__file__).resolve().parents[1]
    logo_dir = root / "crates" / "codegen" / "xai-grok-pager" / "assets" / "logo"
    (logo_dir / "logo07.txt").write_text(to_braille(full), encoding="utf-8")
    (logo_dir / "logo05.txt").write_text(to_braille(compact), encoding="utf-8")
    print(f"wrote {logo_dir / 'logo07.txt'}")
    print(f"wrote {logo_dir / 'logo05.txt'}")

    try:
        from PIL import Image

        def dump_png(grid, path: Path, scale: int = 12) -> None:
            h, w = len(grid), len(grid[0])
            img = Image.new("RGB", (w * scale, h * scale), (18, 20, 26))
            px = img.load()
            on = (232, 236, 242)
            for y, row in enumerate(grid):
                for x, v in enumerate(row):
                    if v:
                        for dy in range(scale - 1):
                            for dx in range(scale - 1):
                                px[x * scale + dx, y * scale + dy] = on
            img.save(path)

        dump_png(full, root / "panda-logo-preview-full.png")
        dump_png(compact, root / "panda-logo-preview-compact.png")
        print("wrote png previews")
    except ImportError:
        pass


if __name__ == "__main__":
    main()
