#!/usr/bin/env python3
"""Compare two native UI captures without changing their pixel geometry."""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image, ImageChops


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", required=True, type=Path)
    parser.add_argument("--actual", required=True, type=Path)
    parser.add_argument(
        "--output",
        required=True,
        type=Path,
        help="directory receiving side-by-side.png, overlay.png, and difference.png",
    )
    return parser.parse_args()


def load_rgb(path: Path) -> Image.Image:
    with Image.open(path) as image:
        return image.convert("RGB")


def main() -> int:
    args = parse_args()
    reference = load_rgb(args.reference)
    actual = load_rgb(args.actual)
    if reference.size != actual.size:
        raise SystemExit(
            "image sizes must match exactly: "
            f"reference={reference.size[0]}x{reference.size[1]}, "
            f"actual={actual.size[0]}x{actual.size[1]}"
        )

    args.output.mkdir(parents=True, exist_ok=True)
    width, height = reference.size
    side_by_side = Image.new("RGB", (width * 2, height))
    side_by_side.paste(reference, (0, 0))
    side_by_side.paste(actual, (width, 0))
    overlay = Image.blend(reference, actual, 0.5)
    difference = ImageChops.difference(reference, actual)

    side_by_side_path = args.output / "side-by-side.png"
    overlay_path = args.output / "overlay.png"
    difference_path = args.output / "difference.png"
    side_by_side.save(side_by_side_path)
    overlay.save(overlay_path)
    difference.save(difference_path)

    differing_pixels = sum(pixel != (0, 0, 0) for pixel in difference.getdata())
    max_channel_delta = max(
        channel_max
        for _, channel_max in difference.getextrema()
    )
    total_pixels = width * height
    print(f"size: {width}x{height}")
    print(f"differing_pixels: {differing_pixels}/{total_pixels}")
    print(f"max_channel_delta: {max_channel_delta}")
    print(f"side_by_side: {side_by_side_path}")
    print(f"overlay_50_percent: {overlay_path}")
    print(f"absolute_difference: {difference_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
