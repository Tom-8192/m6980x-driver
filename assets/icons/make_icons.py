"""Builds the macro icons embedded in the GUI from the high-resolution sources.

Usage: py -3 assets/icons/make_icons.py <source folder> [--frame <empty frame png>]

The source folder holds 0.png ... 40.png (any square size, RGBA). Each icon is scaled to
128x128 (Lanczos, Pillow premultiplies alpha) and saved as lossy WebP q90 with lossless alpha
to assets/icons/macro/<n>.webp. The optional empty frame becomes frame.webp, the background of
the number tiles 41-100. Needs Pillow (py -3 -m pip install --user pillow).
"""

import argparse
import pathlib

from PIL import Image

SIZE = 128
COUNT = 41
OUT = pathlib.Path(__file__).resolve().parent / "macro"


def convert(src: pathlib.Path, dst: pathlib.Path) -> int:
    im = Image.open(src).convert("RGBA").resize((SIZE, SIZE), Image.LANCZOS)
    im.save(dst, "WEBP", quality=90, method=6)
    return dst.stat().st_size


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("source", type=pathlib.Path, help="folder with 0.png ... 40.png")
    parser.add_argument("--frame", type=pathlib.Path, help="empty frame png for the number tiles")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)
    total = 0
    for n in range(COUNT):
        total += convert(args.source / f"{n}.png", OUT / f"{n}.webp")
    if args.frame:
        total += convert(args.frame, OUT / "frame.webp")
    print(f"{OUT}: {total / 1024:.1f} KB")


if __name__ == "__main__":
    main()
