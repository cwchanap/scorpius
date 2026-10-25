#!/usr/bin/env python3
"""Build assets/ui/terrain.png from the Codex-generated terrain source art.

Usage: python3 tools/terrain_atlas.py SOURCE_DIR [OUTPUT_PNG] [--preview PNG]

SOURCE_DIR holds the generated images named in ``SOURCES``; their prompts are
recorded in assets/ui/terrain-art.md. Ground is projected from seamless
top-down textures onto the board's 2:1 diamond, so neighbouring cells share one
continuous surface; trees and mountains are cut from sprite sheets and planted
on top. Every geometry constant below is mirrored in src/presentation/theme.rs.

Needs Pillow and NumPy only.
"""

from __future__ import annotations

import math
import random
import sys
from collections import deque
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

# Atlas pixels per stage pixel: tiles are painted at 2x for high-DPI screens.
SCALE = 2
# The cell inset every terrain tile fills, in stage pixels.
INSET_W, INSET_H = 106, 50
# Ground repeats every PERIOD x PERIOD cells; each terrain owns one atlas row
# of PERIOD * PERIOD slots, indexed by (y % PERIOD) * PERIOD + x % PERIOD.
PERIOD = 4
SLOT_W, SLOT_H = INSET_W * SCALE, 90 * SCALE
# Row order and art heights (stage pixels) match theme::terrain_art.
ROWS = ("plain", "road", "forest", "sea", "mountain")
HEIGHTS = {"plain": 50, "road": 50, "forest": 74, "sea": 50, "mountain": 80}
# The blocking-wall prism: a full 112x56 cell diamond raised 26px.
BLOCK_W, BLOCK_TOP_H, BLOCK_RISE = 112, 56, 26
BLOCK_ORIGIN = (0, len(ROWS) * SLOT_H)
ATLAS_W = SLOT_W * PERIOD * PERIOD
ATLAS_H = BLOCK_ORIGIN[1] + (BLOCK_TOP_H + BLOCK_RISE) * SCALE

SOURCES = {
    "plain": "grass-1.png",
    "road": "road.png",
    "forest": "forest-floor.png",
    "sea": "sea-2.png",
    "mountain": "rock.png",
    "concrete": "concrete.png",
}
TREE_SHEETS = ("trees-1.png", "trees-2.png")
MOUNTAIN_SHEETS = ("mountains-1.png", "mountains-2.png")

# Per-texture grading: (contrast around the mean, brightness, saturation).
GRADES = {
    "plain": (0.75, 0.92, 0.78),
    "road": (0.7, 1.0, 1.0),
    "forest": (0.8, 1.0, 1.0),
    "sea": (0.8, 1.0, 1.0),
    "mountain": (0.8, 1.0, 1.0),
    "concrete": (0.8, 1.0, 1.0),
}

SUPERSAMPLE = 4
# theme::terrain_rim darkens each terrain's mean colour by this much.
RIM_SHADE = 0.55


def load_rgba(path: Path) -> np.ndarray:
    image = Image.open(path).convert("RGBA")
    return np.asarray(image, dtype=np.float32) / 255.0


def grade(rgb: np.ndarray, contrast: float, brightness: float, saturation: float) -> np.ndarray:
    mean = rgb.reshape(-1, 3).mean(axis=0)
    out = mean + (rgb - mean) * contrast
    gray = out.mean(axis=2, keepdims=True)
    out = gray + (out - gray) * saturation
    return np.clip(out * brightness, 0.0, 1.0)


def make_seamless(rgb: np.ndarray, overlap_ratio: float = 0.05) -> np.ndarray:
    """Cross-fade a narrow band so the texture wraps without a seam."""
    for axis in (1, 0):
        n = rgb.shape[axis]
        m = int(n * overlap_ratio)
        body = np.take(rgb, range(m, n - m), axis=axis)
        head = np.take(rgb, range(0, m), axis=axis)
        tail = np.take(rgb, range(n - m, n), axis=axis)
        t = np.linspace(0.0, 1.0, m, endpoint=False, dtype=np.float32)
        t = t.reshape((1, m, 1) if axis == 1 else (m, 1, 1))
        rgb = np.concatenate([tail * (1.0 - t) + head * t, body], axis=axis)
    return rgb


def sample_wrapped(texture: np.ndarray, u: np.ndarray, v: np.ndarray) -> np.ndarray:
    """Bilinear sample with wrap-around; u is columns, v is rows (texels)."""
    h, w = texture.shape[:2]
    u0 = np.floor(u).astype(np.int64)
    v0 = np.floor(v).astype(np.int64)
    fu = (u - u0)[..., None]
    fv = (v - v0)[..., None]
    x0, x1 = u0 % w, (u0 + 1) % w
    y0, y1 = v0 % h, (v0 + 1) % h
    top = texture[y0, x0] * (1 - fu) + texture[y0, x1] * fu
    bottom = texture[y1, x0] * (1 - fu) + texture[y1, x1] * fu
    return top * (1 - fv) + bottom * fv


def to_image(premultiplied: np.ndarray, size: tuple[int, int]) -> Image.Image:
    """Downsample premultiplied RGBA and return a straight-alpha image."""
    image = Image.fromarray((np.clip(premultiplied, 0, 1) * 255 + 0.5).astype(np.uint8), "RGBA")
    small = np.asarray(image.resize(size, Image.Resampling.BOX), dtype=np.float32) / 255.0
    alpha = small[..., 3:4]
    rgb = np.where(alpha > 0, small[..., :3] / np.maximum(alpha, 1e-6), 0)
    out = np.concatenate([np.clip(rgb, 0, 1), alpha], axis=2)
    return Image.fromarray((out * 255 + 0.5).astype(np.uint8), "RGBA")


def project_quad(
    texture: np.ndarray,
    size: tuple[int, int],
    origin: tuple[float, float],
    edge_a: tuple[float, float],
    edge_b: tuple[float, float],
    texels: tuple[float, float],
    offset: tuple[float, float] = (0.0, 0.0),
) -> np.ndarray:
    """Map a texture onto the parallelogram origin + a*edge_a + b*edge_b.

    Returns premultiplied RGBA at SUPERSAMPLE resolution for a canvas of
    ``size`` (output pixels). ``texels`` is how many texels one unit of a/b
    covers and ``offset`` shifts the texture in units of a/b.
    """
    ss = SUPERSAMPLE
    w, h = size[0] * ss, size[1] * ss
    ys, xs = np.mgrid[0:h, 0:w].astype(np.float32)
    px = (xs + 0.5) / ss - origin[0]
    py = (ys + 0.5) / ss - origin[1]
    det = edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0]
    a = (px * edge_b[1] - py * edge_b[0]) / det
    b = (py * edge_a[0] - px * edge_a[1]) / det
    inside = (a >= 0) & (a <= 1) & (b >= 0) & (b <= 1)
    rgb = sample_wrapped(texture, (a + offset[0]) * texels[0], (b + offset[1]) * texels[1])
    alpha = inside.astype(np.float32)[..., None]
    return np.concatenate([rgb * alpha, alpha], axis=2)


def ground_tile(texture: np.ndarray, cx: int, cy: int) -> Image.Image:
    """The inset diamond for grid cell (cx, cy) of the repeating period."""
    w, h = INSET_W * SCALE, INSET_H * SCALE
    texels = texture.shape[1] / PERIOD
    quad = project_quad(
        texture,
        (w, h),
        origin=(w / 2, 0),
        edge_a=(w / 2, h / 2),  # +x runs down-right
        edge_b=(-w / 2, h / 2),  # +y runs down-left
        texels=(texels, texels),
        offset=(cx, cy),
    )
    return to_image(quad, (w, h))


def slot_with_ground(ground: Image.Image) -> Image.Image:
    slot = Image.new("RGBA", (SLOT_W, SLOT_H))
    slot.alpha_composite(ground, (0, SLOT_H - ground.height))
    return slot


def sprite_alpha(sheet: np.ndarray) -> np.ndarray:
    alpha = sheet[..., 3]
    if alpha.min() < 0.5:
        return alpha
    # No real transparency: key out a flat magenta background.
    rgb = sheet[..., :3]
    distance = np.linalg.norm(rgb - np.array([1.0, 0.0, 1.0]), axis=2)
    return np.clip((distance - 0.25) / 0.35, 0.0, 1.0)


def extract_sprites(path: Path, min_area_ratio: float = 0.004) -> list[Image.Image]:
    """Cut each separate object out of a sprite sheet, largest first."""
    sheet = load_rgba(path)
    alpha = sprite_alpha(sheet)
    sheet[..., 3] = alpha
    step = 4
    mask = alpha[::step, ::step] > 0.1
    h, w = mask.shape
    labels = np.zeros(mask.shape, dtype=np.int32)
    boxes = []
    for y in range(h):
        for x in range(w):
            if not mask[y, x] or labels[y, x]:
                continue
            label = len(boxes) + 1
            queue = deque([(y, x)])
            labels[y, x] = label
            box = [x, y, x, y, 0]
            while queue:
                qy, qx = queue.popleft()
                box[0], box[1] = min(box[0], qx), min(box[1], qy)
                box[2], box[3] = max(box[2], qx), max(box[3], qy)
                box[4] += 1
                for ny in range(max(qy - 1, 0), min(qy + 2, h)):
                    for nx in range(max(qx - 1, 0), min(qx + 2, w)):
                        if mask[ny, nx] and not labels[ny, nx]:
                            labels[ny, nx] = label
                            queue.append((ny, nx))
            boxes.append(box)
    min_area = mask.size * min_area_ratio
    sprites = []
    for x0, y0, x1, y1, area in sorted(boxes, key=lambda box: -box[4]):
        if area < min_area:
            continue
        crop = sheet[y0 * step : (y1 + 1) * step, x0 * step : (x1 + 1) * step].copy()
        image = Image.fromarray((crop * 255 + 0.5).astype(np.uint8), "RGBA")
        sprites.append(image.crop(image.getbbox()))
    return sprites


def soft_shadow(size: tuple[int, int], center: tuple[float, float], radii: tuple[float, float], strength: float) -> Image.Image:
    shadow = Image.new("L", size, 0)
    draw = ImageDraw.Draw(shadow)
    cx, cy = center
    rx, ry = radii
    draw.ellipse((cx - rx, cy - ry, cx + rx, cy + ry), fill=int(255 * strength))
    shadow = shadow.filter(ImageFilter.GaussianBlur(radius=max(rx, ry) * 0.35))
    layer = Image.new("RGBA", size, (8, 20, 10, 0))
    layer.putalpha(shadow)
    return layer


def diamond_point(a: float, b: float) -> tuple[float, float]:
    """Slot-space point for grid-local (a, b) on the inset diamond."""
    w, h = INSET_W * SCALE, INSET_H * SCALE
    top = SLOT_H - h
    return (w / 2 + (a - b) * w / 2, top + (a + b) * h / 2)


def scaled(sprite: Image.Image, height: float) -> Image.Image:
    factor = height / sprite.height
    size = (max(1, round(sprite.width * factor)), max(1, round(height)))
    return sprite.resize(size, Image.Resampling.LANCZOS)


def forest_tile(ground: Image.Image, trees: list[Image.Image], rng: random.Random) -> Image.Image:
    slot = slot_with_ground(ground)
    art_top = SLOT_H - HEIGHTS["forest"] * SCALE
    spots: list[tuple[float, float]] = []
    for _ in range(400):
        if len(spots) == 5:
            break
        a, b = rng.uniform(0.16, 0.84), rng.uniform(0.16, 0.84)
        if all(math.hypot(a - sa, b - sb) > 0.34 for sa, sb in spots):
            spots.append((a, b))
    plants = []
    for a, b in spots:
        x, y = diamond_point(a, b)
        tree = scaled(rng.choice(trees), rng.uniform(28, 36) * SCALE)
        # Keep crowns inside the slot horizontally and under the art top.
        x = min(max(x, tree.width / 2 + 2), SLOT_W - tree.width / 2 - 2)
        top = max(y - tree.height, art_top)
        plants.append((top + tree.height, x, tree))
    plants.sort(key=lambda plant: plant[0])
    for base, x, tree in plants:
        slot.alpha_composite(
            soft_shadow(slot.size, (x + tree.width * 0.12, base - 2), (tree.width * 0.42, tree.width * 0.18), 0.45)
        )
    for base, x, tree in plants:
        slot.alpha_composite(tree, (round(x - tree.width / 2), round(base - tree.height)))
    return slot


def is_snowy(peak: Image.Image) -> bool:
    pixels = np.asarray(peak, dtype=np.float32) / 255.0
    white = (pixels[..., :3].min(axis=2) > 0.82) & (pixels[..., 3] > 0.5)
    return white.mean() > 0.02


def mountain_peaks(peaks: list[Image.Image], rng: random.Random) -> list[Image.Image]:
    """One peak per period slot; only a few slots get a snow cap so a range
    doesn't read as a field of white dots."""
    snowy = [peak for peak in peaks if is_snowy(peak)]
    bare = [peak for peak in peaks if not is_snowy(peak)] or snowy
    slots = PERIOD * PERIOD
    snow_slots = set(rng.sample(range(slots), 3)) if snowy else set()
    order = bare * (slots // len(bare) + 1)
    rng.shuffle(order)
    return [rng.choice(snowy) if slot in snow_slots else order[slot] for slot in range(slots)]


def mountain_tile(ground: Image.Image, peak: Image.Image, rng: random.Random) -> Image.Image:
    slot = slot_with_ground(ground)
    art_top = SLOT_H - HEIGHTS["mountain"] * SCALE
    base = SLOT_H - 6 - rng.uniform(0, 4)
    # The sheets' massifs are wide and low; stretch them a little taller so a
    # range reads as mountains rather than hills at map zoom. Varying size and
    # position keeps neighbouring peaks from lining up into stripes.
    width = INSET_W * SCALE * rng.uniform(0.84, 1.0)
    peak = peak.resize((round(width), round(1.2 * peak.height * width / peak.width)), Image.Resampling.LANCZOS)
    if base - peak.height < art_top:
        peak = scaled(peak, base - art_top)
    slack = (SLOT_W - peak.width) / 2
    x = SLOT_W / 2 + rng.uniform(-slack, slack)
    slot.alpha_composite(
        soft_shadow(slot.size, (x + peak.width * 0.08, base - 10), (peak.width * 0.46, peak.width * 0.16), 0.5)
    )
    slot.alpha_composite(peak, (round(x - peak.width / 2), round(base - peak.height)))
    return slot


def hazard_band(face: np.ndarray, u: np.ndarray, v: np.ndarray, band: tuple[float, float]) -> np.ndarray:
    stripes = (np.floor((u * 9.0 + v * 2.5)) % 2).astype(np.float32)[..., None]
    yellow = np.array([0.93, 0.72, 0.18], dtype=np.float32)
    black = np.array([0.13, 0.13, 0.15], dtype=np.float32)
    colour = yellow * stripes + black * (1 - stripes)
    inside = ((v >= band[0]) & (v <= band[1]))[..., None]
    return np.where(inside, colour, face)


def blocker(concrete: np.ndarray) -> Image.Image:
    """A concrete bunker block with hazard trim on the cell's full diamond."""
    w, top_h, rise = BLOCK_W * SCALE, BLOCK_TOP_H * SCALE, BLOCK_RISE * SCALE
    size = (w, top_h + rise)
    texels = concrete.shape[1] / 2
    canvas = np.zeros((size[1] * SUPERSAMPLE, size[0] * SUPERSAMPLE, 4), dtype=np.float32)
    ss = SUPERSAMPLE
    ys, xs = np.mgrid[0 : size[1] * ss, 0 : size[0] * ss].astype(np.float32)
    px, py = (xs + 0.5) / ss, (ys + 0.5) / ss

    def face(origin, edge_a, edge_b, shade, band):
        det = edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0]
        dx, dy = px - origin[0], py - origin[1]
        a = (dx * edge_b[1] - dy * edge_b[0]) / det
        b = (dy * edge_a[0] - dx * edge_a[1]) / det
        inside = (a >= 0) & (a <= 1) & (b >= 0) & (b <= 1)
        rgb = sample_wrapped(concrete, a * texels, b * texels * (rise / top_h if band else 1))
        if band:
            rgb = hazard_band(rgb, a, b, band)
            # Darken toward the ground for a little ambient occlusion.
            rgb = rgb * (1.0 - 0.25 * b[..., None])
        rgb = rgb * shade
        mask = inside[..., None]
        canvas[...] = np.where(mask, np.concatenate([rgb, np.ones_like(rgb[..., :1])], axis=2), canvas)

    top = (w / 2, 0)
    left, right, bottom = (0, top_h / 2), (w, top_h / 2), (w / 2, top_h)
    face(left, (w / 2, top_h / 2), (0, rise), 0.78, (0.1, 0.32))
    face(bottom, (w / 2, -top_h / 2), (0, rise), 0.6, (0.1, 0.32))
    face(top, (w / 2, top_h / 2), (-w / 2, top_h / 2), 1.08, None)
    rgb = canvas[..., :3] * canvas[..., 3:4]
    image = to_image(np.concatenate([rgb, canvas[..., 3:4]], axis=2), size)
    # Light rims on the top face's front edges separate it from the walls.
    draw = ImageDraw.Draw(image)
    draw.line([left, bottom, right], fill=(222, 228, 232, 200), width=2)
    return image


def main(argv: list[str]) -> int:
    args = [arg for arg in argv[1:] if not arg.startswith("--")]
    preview = argv[argv.index("--preview") + 1] if "--preview" in argv else None
    if preview in args:
        args.remove(preview)
    if not args:
        print(__doc__)
        return 2
    source = Path(args[0])
    output = Path(args[1]) if len(args) > 1 else Path("assets/ui/terrain.png")

    textures = {}
    for name, file in SOURCES.items():
        rgb = load_rgba(source / file)[..., :3]
        # The concrete's panel seams must not be cross-faded; the blocker
        # never wraps it.
        if name != "concrete":
            rgb = make_seamless(rgb)
        textures[name] = grade(rgb, *GRADES[name])
    trees = [tree for sheet in TREE_SHEETS for tree in extract_sprites(source / sheet)]
    peaks = [
        peak
        for sheet in MOUNTAIN_SHEETS
        for peak in extract_sprites(source / sheet, min_area_ratio=0.02)
    ]
    print(f"{len(trees)} trees, {len(peaks)} mountains")

    rng = random.Random(9)
    slot_peaks = mountain_peaks(peaks, rng)
    atlas = Image.new("RGBA", (ATLAS_W, ATLAS_H))
    for row, terrain in enumerate(ROWS):
        for index in range(PERIOD * PERIOD):
            cx, cy = index % PERIOD, index // PERIOD
            ground = ground_tile(textures[terrain], cx, cy)
            if terrain == "forest":
                slot = forest_tile(ground, trees, rng)
            elif terrain == "mountain":
                slot = mountain_tile(ground, slot_peaks[index], rng)
            else:
                slot = slot_with_ground(ground)
            atlas.alpha_composite(slot, (index * SLOT_W, row * SLOT_H))
    atlas.alpha_composite(blocker(textures["concrete"]), BLOCK_ORIGIN)
    atlas.save(output, optimize=True)
    print(f"wrote {output} ({ATLAS_W}x{ATLAS_H})")
    rims = {terrain: rim_colour(textures[terrain]) for terrain in ROWS}
    for terrain, rim in rims.items():
        print(f"{terrain} rim: Color::srgb_u8{rim}")

    if preview:
        write_preview(atlas, rims, Path(preview))
    return 0


def rim_colour(texture: np.ndarray) -> tuple[int, int, int]:
    """The idle cell rim: the terrain's mean colour, darkened into a groove."""
    mean = texture.reshape(-1, 3).mean(axis=0) * RIM_SHADE
    return tuple(int(round(channel * 255)) for channel in mean)


def write_preview(atlas: Image.Image, rims: dict[str, tuple[int, int, int]], path: Path) -> None:
    """Lay a small patch of every terrain on the iso grid, like the game."""
    layout = [
        "SSSSPPPPFFFFMMMM",
        "SSSPPPPPFFFFMMMM",
        "SSPPPPRRRRRFFMMM",
        "SPPPPPRPPPPFFFMM",
        "PPPPBPRPPPPPFFFF",
        "PPPPBPRPPPPPPFFF",
        "PPBBBPRRRRRRRRRR",
        "PPPPPPPPPPPPPPPP",
    ]
    rows = {"P": 0, "R": 1, "F": 2, "S": 3, "M": 4}
    tile_w, tile_h, gap = 112 * SCALE, 56 * SCALE, 3 * SCALE
    height, width = len(layout), len(layout[0])
    canvas = Image.new("RGBA", ((width + height) * tile_w // 2 + tile_w, (width + height) * tile_h // 2 + 220), (5, 8, 15, 255))
    origin_x, origin_y = height * tile_w // 2, 160
    cells = sorted(((x + y, y, x) for y in range(height) for x in range(width)))
    block = atlas.crop((BLOCK_ORIGIN[0], BLOCK_ORIGIN[1], BLOCK_ORIGIN[0] + 224, BLOCK_ORIGIN[1] + 164))
    for _, y, x in cells:
        kind = layout[y][x]
        cx = origin_x + (x - y) * tile_w // 2
        cy = origin_y + (x + y) * tile_h // 2
        terrain = "P" if kind == "B" else kind
        row = rows[terrain]
        rim = Image.new("RGBA", (tile_w, tile_h))
        ImageDraw.Draw(rim).polygon(
            [(tile_w / 2, 0), (tile_w, tile_h / 2), (tile_w / 2, tile_h), (0, tile_h / 2)],
            fill=rims[ROWS[row]] + (255,),
        )
        canvas.alpha_composite(rim, (cx - tile_w // 2, cy - tile_h // 2))
        index = (y % PERIOD) * PERIOD + x % PERIOD
        slot = atlas.crop((index * SLOT_W, row * SLOT_H, (index + 1) * SLOT_W, (row + 1) * SLOT_H))
        canvas.alpha_composite(slot, (cx - tile_w // 2 + gap, cy - tile_h // 2 + gap + 50 * SCALE - SLOT_H))
        if kind == "B":
            canvas.alpha_composite(block, (cx - tile_w // 2, cy - tile_h // 2 - 26 * SCALE))
    canvas.convert("RGB").save(path)
    print(f"wrote {path}")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
