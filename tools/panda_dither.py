"""Image → braille dot-matrix converter with e-ink style dithering.

Pipeline (mirrors the 采样→描边→排版 flow):

1. 采样    area-average downscale to the dot grid (gamma-aware ink tone).
2. 描边    Sobel gradients on a supersampled copy, max-pooled back to the dot
           grid so thin strokes survive; strong edges are pinned solid.
3. 排版    the remaining (non-edge) tone is reproduced with error-diffusion
           (Floyd–Steinberg serpentine / Atkinson) or ordered (Bayer 8×8)
           dithering, the same families used on e-ink displays.

Usage:
    python tools/panda_dither.py <image> --cols 32 --rows 8 \
        --dither fs --out-prefix panda

Outputs `<prefix>.txt` (braille), `<prefix>.png` (dot preview) and prints
the braille art. `--ramp` additionally prints an ASCII-ramp variant where
edge direction and tone pick the characters.
"""

from __future__ import annotations

import argparse
import math
from pathlib import Path

from PIL import Image

# Braille dots:
# 1 4
# 2 5
# 3 6
# 7 8
DOT_MAP = [
    (0, 0, 0),
    (0, 1, 1),
    (0, 2, 2),
    (1, 0, 3),
    (1, 1, 4),
    (1, 2, 5),
    (0, 3, 6),
    (1, 3, 7),
]

SOBEL_X = ((-1, 0, 1), (-2, 0, 2), (-1, 0, 1))
SOBEL_Y = ((-1, -2, -1), (0, 0, 0), (1, 2, 1))


def load_ink(path: Path) -> Image.Image:
    """Grayscale ink map (1 = stroke, 0 = paper), auto-cropped to content."""
    img = Image.open(path).convert("L")
    # Treat near-white as paper; bbox over everything noticeably darker.
    mask = img.point(lambda v: 255 if v < 245 else 0)
    bbox = mask.getbbox()
    if bbox:
        # Proportional breathing room: keeps badge rings thin and stops
        # outer strokes from crowding the canvas edge.
        l, t, r, b = bbox
        pad = int(max(r - l, b - t) * 0.06)
        l, t = max(0, l - pad), max(0, t - pad)
        r, b = min(img.width, r + pad), min(img.height, b + pad)
        img = img.crop((l, t, r, b))
    # ink = darkness, normalized 0..1
    return img.point(lambda v: 255 - v)


def resize_ink(img: Image.Image, w: int, h: int, mode: str) -> list[list[float]]:
    # LANCZOS for tone: thin strokes stay connected instead of aliasing to
    # whatever BOX sample alignment happens to catch
    res = img.resize((w, h), Image.LANCZOS if mode in ("sample", "lanczos") else Image.BOX)
    px = res.load()
    return [[px[x, y] / 255.0 for x in range(w)] for y in range(h)]


def sobel_magnitude(img: Image.Image, w: int, h: int, ss: int = 3) -> list[list[float]]:
    """Sobel on a supersampled copy, max-pooled to the dot grid."""
    big_w, big_h = w * ss, h * ss
    res = img.resize((big_w, big_h), Image.LANCZOS)
    px = res.load()
    lum = [[px[x, y] / 255.0 for x in range(big_w)] for y in range(big_h)]
    mag = [[0.0] * big_w for _ in range(big_h)]
    for y in range(1, big_h - 1):
        for x in range(1, big_w - 1):
            gx = gy = 0.0
            for dy in (-1, 0, 1):
                for dx in (-1, 0, 1):
                    v = lum[y + dy][x + dx]
                    gx += v * SOBEL_X[dy + 1][dx + 1]
                    gy += v * SOBEL_Y[dy + 1][dx + 1]
            mag[y][x] = math.hypot(gx, gy)
    # max-pool ss×ss blocks; also record mean gradient direction per block
    out = [[0.0] * w for _ in range(h)]
    for by in range(h):
        for bx in range(w):
            m = 0.0
            for dy in range(ss):
                for dx in range(ss):
                    y, x = by * ss + dy, bx * ss + dx
                    if 0 < y < big_h - 1 and 0 < x < big_w - 1:
                        m = max(m, mag[y][x])
            out[by][bx] = m
    return out


def edge_pin(edge: float, old: float, gain: float) -> float | None:
    """Symmetric Sobel gate: on strong edges snap dark→solid ink and
    bright→paper (protects thin strokes and highlights); None = let the
    dither decide (true midtones only)."""
    if edge * gain > 0.35:
        if old > 0.45:
            return 1.0
        if old < 0.30:
            return 0.0
    return None


def floyd_steinberg(ink: list[list[float]], edge: list[list[float]],
                    edge_gain: float, serpentine: bool = True) -> list[list[int]]:
    """Error diffusion on tone; edge pixels snap via edge_pin first."""
    h, w = len(ink), len(ink[0])
    buf = [row[:] for row in ink]
    out = [[0] * w for _ in range(h)]
    for y in range(h):
        rng = range(w) if y % 2 == 0 or not serpentine else range(w - 1, -1, -1)
        sgn = 1 if y % 2 == 0 or not serpentine else -1
        for x in rng:
            old = buf[y][x]
            pin = edge_pin(edge[y][x], old, edge_gain)
            new = pin if pin is not None else (1.0 if old > 0.5 else 0.0)
            out[y][x] = int(new)
            err = old - new
            if sgn == 1:
                spread = ((x + 1, y, 7), (x + 1, y + 1, 1), (x, y + 1, 5), (x - 1, y + 1, 3))
            else:
                spread = ((x - 1, y, 7), (x - 1, y + 1, 1), (x, y + 1, 5), (x + 1, y + 1, 3))
            for nx, ny, num in spread:
                if 0 <= nx < w and 0 <= ny < h:
                    buf[ny][nx] += err * num / 16.0
    return out


def atkinson(ink: list[list[float]], edge: list[list[float]], edge_gain: float) -> list[list[int]]:
    """Atkinson: 6/8 error spread — punchier contrast, classic e-ink look."""
    h, w = len(ink), len(ink[0])
    buf = [row[:] for row in ink]
    out = [[0] * w for _ in range(h)]
    for y in range(h):
        for x in range(w):
            old = buf[y][x]
            pin = edge_pin(edge[y][x], old, edge_gain)
            new = pin if pin is not None else (1.0 if old > 0.5 else 0.0)
            out[y][x] = int(new)
            err = (old - new) / 8.0
            for nx, ny in (
                (x + 1, y), (x + 2, y),
                (x - 1, y + 1), (x, y + 1), (x + 1, y + 1),
                (x, y + 2),
            ):
                if 0 <= nx < w and 0 <= ny < h:
                    buf[ny][nx] += err
    return out


def blur3(tone: list[list[float]]) -> list[list[float]]:
    """3×3 box-ish blur: kills resize ringing dips that otherwise surface as
    light specks inside strokes, and softens palette-quantization steps."""
    h, w = len(tone), len(tone[0])
    out = [[0.0] * w for _ in range(h)]
    for y in range(h):
        for x in range(w):
            acc = num = 0.0
            for dy in (-1, 0, 1):
                for dx in (-1, 0, 1):
                    ny, nx = y + dy, x + dx
                    if 0 <= ny < h and 0 <= nx < w:
                        wt = 4.0 if (dx == 0 and dy == 0) else (2.0 if dx == 0 or dy == 0 else 1.0)
                        acc += tone[ny][nx] * wt
                        num += wt
            out[y][x] = acc / num
    return out


def dilate(tone: list[list[float]], times: int) -> list[list[float]]:
    """Max-filter the tone map so thin strokes grow to 2+ dots wide."""
    h, w = len(tone), len(tone[0])
    cur = [row[:] for row in tone]
    for _ in range(times):
        nxt = [row[:] for row in cur]
        for y in range(h):
            for x in range(w):
                m = cur[y][x]
                for dy in (-1, 0, 1):
                    for dx in (-1, 0, 1):
                        ny, nx = y + dy, x + dx
                        if 0 <= ny < h and 0 <= nx < w:
                            m = max(m, cur[ny][nx])
                nxt[y][x] = m
        cur = nxt
    return cur


BAYER8 = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
]


def bayer(ink: list[list[float]], edge: list[list[float]], edge_gain: float) -> list[list[int]]:
    h, w = len(ink), len(ink[0])
    out = [[0] * w for _ in range(h)]
    for y in range(h):
        for x in range(w):
            t = (BAYER8[y % 8][x % 8] + 0.5) / 64.0
            v = ink[y][x]
            pin = edge_pin(edge[y][x], v, edge_gain)
            out[y][x] = int(pin if pin is not None else v > t)
    return out


DITHERS = {"fs": floyd_steinberg, "atkinson": atkinson, "bayer": bayer}


def threshold_only(ink: list[list[float]]) -> list[list[int]]:
    return [[int(v > 0.5) for v in row] for row in ink]


def to_braille(grid: list[list[int]]) -> str:
    h, w = len(grid), len(grid[0])
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
    width = max(len(line) for line in lines)
    return "\n".join(line + "\u2800" * (width - len(line)) for line in lines) + "\n"


RAMP10 = " .:-=+*#%@"
SHADE5 = " ░▒▓█"


def cell_surrounds_dark(cell: list[list[float]], x: int, y: int) -> float:
    """Min ink of the 4-neighbours, edges excluded — high only when the cell
    sits inside a dark region (a bright pixel there is a real highlight);
    stroke-border cells always touch background, so they fail the test."""
    h, w = len(cell), len(cell[0])
    vals = []
    for dx, dy in ((-1, 0), (1, 0), (0, -1), (0, 1)):
        nx, ny = x + dx, y + dy
        if 0 <= nx < w and 0 <= ny < h:
            vals.append(cell[ny][nx])
    return min(vals) if vals else 0.0


def to_ascii(tone: list[list[float]], edge: list[list[float]],
             dirx: list[list[float]], diry: list[list[float]],
             style: str, use_edges: bool, smooth: bool = True) -> str:
    """Character-cell typesetting: shading depth picks the ramp char, edge
    direction picks a stroke char. Tone grid is at 2× vertical resolution;
    each row pair merges into one character row. With `smooth`, the tone is
    Floyd–Steinberg-diffused over the *palette index* domain, so a 5-glyph
    palette renders near-continuous gradients."""
    h, w = len(tone), len(tone[0])
    rows = h // 2
    palette = SHADE5 if style == "shade" else RAMP10
    n = len(palette)
    # cell tone: max-pair keeps thin strokes, blended 50/50 with the average
    # so soft gradients don't get eaten
    cell = [[(max(tone[2 * y][x], tone[2 * y + 1][x]) * 0.5
              + (tone[2 * y][x] + tone[2 * y + 1][x]) * 0.25) for x in range(w)]
            for y in range(rows)]
    idx_grid = [[0] * w for _ in range(rows)]
    if smooth == "fs":
        buf = [r[:] for r in cell]
        for y in range(rows):
            rng = range(w) if y % 2 == 0 else range(w - 1, -1, -1)
            sgn = 1 if y % 2 == 0 else -1
            for x in rng:
                old = buf[y][x]
                i = max(0, min(n - 1, int(old * n)))
                idx_grid[y][x] = i
                err = old - (i / (n - 1) if n > 1 else i)
                if sgn == 1:
                    spread = ((x + 1, y, 7), (x + 1, y + 1, 1), (x, y + 1, 5), (x - 1, y + 1, 3))
                else:
                    spread = ((x - 1, y, 7), (x - 1, y + 1, 1), (x, y + 1, 5), (x + 1, y + 1, 3))
                for nx, ny, num in spread:
                    if 0 <= nx < w and 0 <= ny < rows:
                        buf[ny][nx] += err * num / 16.0
    elif smooth == "bayer":
        for y in range(rows):
            for x in range(w):
                b = (BAYER8[y % 8][x % 8] + 0.5) / 64.0 - 0.5
                i = max(0, min(n - 1, int((cell[y][x] + b / n) * n)))
                idx_grid[y][x] = i
    else:
        for y in range(rows):
            for x in range(w):
                idx_grid[y][x] = max(0, min(n - 1, int(cell[y][x] * n)))

    lines = []
    for y in range(rows):
        row = []
        for x in range(w):
            t = cell[y][x]
            e = max(edge[2 * y][x], edge[2 * y + 1][x])
            if e <= 0.3 and t < 0.22:
                # paper pin: far from any stroke and clearly light — keep it
                # clean white so bayer crossfade can't turn seams/background
                # into gray mush
                row.append(palette[0])
            elif use_edges and e > 0.5 and 0.25 < t < 0.85:
                gx = dirx[2 * y][x] + dirx[2 * y + 1][x]
                gy = diry[2 * y][x] + diry[2 * y + 1][x]
                ang = math.degrees(math.atan2(gy, gx)) % 180.0
                # gradient ⊥ stroke direction
                if ang < 22.5 or ang >= 157.5:
                    row.append("|")
                elif ang < 67.5:
                    row.append("/")
                elif ang < 112.5:
                    row.append("-")
                else:
                    row.append("\\")
            elif e > 0.5:
                # symmetric edge pin: only true ink cores snap solid — a
                # lower threshold welds thin white gaps (ear/outline seams)
                # into black bands. Bright pin: highlights inside dark
                # regions (eye glints) punch white.
                if t > 0.55:
                    row.append(palette[n - 1])
                elif t < 0.38 and cell_surrounds_dark(cell, x, y) > 0.55:
                    row.append(palette[0])
                else:
                    row.append(palette[idx_grid[y][x]])
            else:
                row.append(palette[idx_grid[y][x]])
        lines.append("".join(row).rstrip())
    return "\n".join(lines) + "\n"


def dump_png(grid: list[list[int]], path: Path, scale: int = 12) -> None:
    from PIL import Image as I

    h, w = len(grid), len(grid[0])
    img = I.new("RGB", (w * scale, h * scale), (250, 250, 248))
    px = img.load()
    for y, row in enumerate(grid):
        for x, v in enumerate(row):
            if v:
                for dy in range(scale - 1):
                    for dx in range(scale - 1):
                        px[x * scale + dx, y * scale + dy] = (24, 26, 32)
    img.save(path)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("image", type=Path)
    ap.add_argument("--style", choices=("braille", "ascii", "shade"), default="ascii",
                    help="braille = 2×4 dot matrix; ascii = density ramp chars; "
                         "shade = ░▒▓█ blocks")
    ap.add_argument("--cols", type=int, default=None, help="character/dot columns")
    ap.add_argument("--rows", type=int, default=None, help="character/dot rows")
    ap.add_argument("--dither", choices=("fs", "atkinson", "bayer", "none"), default="fs")
    ap.add_argument("--edge-gain", type=float, default=1.0)
    ap.add_argument("--gamma", type=float, default=1.15, help="ink tone gamma")
    ap.add_argument("--density", type=float, default=1.0,
                    help="ink gain before dithering (>1 = denser/darker output)")
    ap.add_argument("--dilate", type=int, default=0,
                    help="max-filter passes on tone: 1 grows strokes to 2 dots wide")
    ap.add_argument("--vignette", type=float, default=0.0,
                    help="synthetic top-left key light: 0..0.6, fades large ink "
                         "regions toward the far edge for a 3D gradient")
    ap.add_argument("--levels", type=float, nargs=2, default=(0.0, 1.0),
                    metavar=("LO", "HI"),
                    help="contrast stretch: tones below LO -> 0, above HI -> 1, "
                         "in-between rescaled (e-ink style pre-flat for line art)")
    ap.add_argument("--out-prefix", type=Path, default=Path("panda_dither_out"))
    ap.add_argument("--ramp", action="store_true", help="also print the ascii variant")
    ap.add_argument("--smooth", choices=("none", "fs", "bayer"), default="none",
                    help="palette-index dithering for ascii/shade: fs = error "
                         "diffusion, bayer = ordered crossfade (smoother ramps)")
    ap.add_argument("--edges", action="store_true",
                    help="edge-direction stroke chars in ascii/shade output")
    args = ap.parse_args()

    # braille: --cols/--rows are braille cells (dots = 2×/4×);
    # ascii/shade: --cols/--rows are character cells, sampled at 2× vertical res.
    if args.cols is None:
        args.cols = 48 if args.style == "braille" else 64
    if args.rows is None:
        args.rows = 12 if args.style == "braille" else 24
    if args.style == "braille":
        w, h = args.cols * 2, args.rows * 4
    else:
        w, h = args.cols, args.rows * 2
    img = load_ink(args.image)

    # 采样: area-average tone + gamma + levels stretch + density;
    # 描边: supersampled Sobel, max-pooled.
    lo, hi = args.levels
    def stretch(v: float) -> float:
        if v <= lo:
            return 0.0
        if v >= hi:
            return 1.0
        return (v - lo) / (hi - lo)
    tone = resize_ink(img, w, h, "sample")
    tone = [[min(1.0, stretch(v ** args.gamma) * args.density) for v in row]
            for row in tone]
    if args.dilate:
        tone = dilate(tone, args.dilate)
    tone = blur3(tone)
    if args.vignette > 0.0:
        # key light from top-left: ink fades with distance from the light
        cx, cy = w * 0.35, h * 0.35
        maxd = math.hypot(max(cx, w - cx), max(cy, h - cy))
        for y in range(h):
            for x in range(w):
                d = math.hypot(x - cx, y - cy) / maxd
                tone[y][x] *= 1.0 - args.vignette * d * d
    mag = sobel_magnitude(img, w, h)
    edge = [[min(1.0, m * 2.0) for m in row] for row in mag]

    # gradient direction on the sampling grid (for edge-direction typesetting)
    dirx = [[0.0] * w for _ in range(h)]
    diry = [[0.0] * w for _ in range(h)]
    for y in range(1, h - 1):
        for x in range(1, w - 1):
            gx = tone[y][x + 1] - tone[y][x - 1]
            gy = tone[y + 1][x] - tone[y - 1][x]
            n = math.hypot(gx, gy) or 1.0
            dirx[y][x], diry[y][x] = gx / n, gy / n

    if args.style == "braille":
        if args.dither == "none":
            grid = threshold_only(tone)
        else:
            grid = DITHERS[args.dither](tone, edge, args.edge_gain)
        art = to_braille(grid)
        suffix = f".{args.dither}" if args.dither != "fs" else ""
        args.out_prefix = args.out_prefix.with_name(
            args.out_prefix.name + suffix) if suffix else args.out_prefix
        dump_png(grid, args.out_prefix.with_suffix(".png"))
    else:
        art = to_ascii(tone, edge, dirx, diry, args.style, args.edges, args.smooth)

    print(art)

    args.out_prefix.parent.mkdir(parents=True, exist_ok=True)
    args.out_prefix.with_suffix(".txt").write_text(art, encoding="utf-8")
    print(f"wrote {args.out_prefix}.txt  "
          f"({args.cols}x{args.rows} cells, style={args.style}, "
          f"density={args.density}, dilate={args.dilate})")


if __name__ == "__main__":
    main()
