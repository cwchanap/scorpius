#!/usr/bin/env python3
"""Small executable self-check for the native capture comparator."""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image


def main() -> int:
    comparator = Path(__file__).with_name("compare_ui.py")
    with tempfile.TemporaryDirectory(prefix="scorpius-compare-ui-") as temporary:
        root = Path(temporary)
        reference = root / "reference.png"
        actual = root / "actual.png"
        output = root / "output"

        Image.new("RGB", (2, 2), (10, 20, 30)).save(reference)
        changed = Image.new("RGB", (2, 2), (10, 20, 30))
        changed.putpixel((1, 1), (10, 20, 35))
        changed.save(actual)

        result = subprocess.run(
            [
                sys.executable,
                str(comparator),
                "--reference",
                str(reference),
                "--actual",
                str(actual),
                "--output",
                str(output),
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        assert result.returncode == 0, result.stderr
        assert "size: 2x2" in result.stdout
        assert "differing_pixels: 1/4" in result.stdout
        assert "max_channel_delta: 5" in result.stdout
        assert (output / "side-by-side.png").is_file()
        assert (output / "overlay.png").is_file()
        assert (output / "difference.png").is_file()

        unequal = root / "unequal.png"
        Image.new("RGB", (3, 2), (10, 20, 30)).save(unequal)
        mismatch = subprocess.run(
            [
                sys.executable,
                str(comparator),
                "--reference",
                str(reference),
                "--actual",
                str(unequal),
                "--output",
                str(root / "mismatch-output"),
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        assert mismatch.returncode != 0
        assert (
            "image sizes must match exactly: reference=2x2, actual=3x2"
            in mismatch.stderr
        )

    print("compare_ui self-check: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
