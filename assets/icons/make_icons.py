"""Builds the macro icons embedded in the GUI from the high-resolution sources.

Usage: py -3 assets/icons/make_icons.py [source folder]

The source folder (default: assets/icons/source) holds 0.png ... 40.png and frame.png (any square size, RGBA).
Each image is scaled to 128x128 (Lanczos, Pillow premultiplies alpha) and saved as lossy WebP q90 with lossless
alpha to assets/icons/macro/<name>.webp. frame.png is the empty frame behind the number tiles 41-100.
Needs Pillow (py -3 -m pip install --user pillow).
"""

import argparse
import pathlib

from PIL import Image

SIZE = 128
NAMES = [str(n) for n in range(41)] + ["frame"]
HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE / "macro"


def convert(src: pathlib.Path, dst: pathlib.Path) -> int:
    im = Image.open(src).convert("RGBA").resize((SIZE, SIZE), Image.LANCZOS)
    im.save(dst, "WEBP", quality=90, method=6)
    return dst.stat().st_size


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("source", type=pathlib.Path, nargs="?", default=HERE / "source",
                        help="folder with 0.png ... 40.png and frame.png")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)
    total = sum(convert(args.source / f"{name}.png", OUT / f"{name}.webp") for name in NAMES)
    print(f"{OUT}: {total / 1024:.1f} KB")


if __name__ == "__main__":
    main()
