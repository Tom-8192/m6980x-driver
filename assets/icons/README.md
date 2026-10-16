# Macro icons

Own icons for the macro pictures, made with an image AI from the descriptions in
[`../icon_descriptions.json`](../icon_descriptions.json). They contain no Gigabyte material and fall under the license
of this repository.

- `macro/0.webp` – empty slot (shown for "no icon")
- `macro/1.webp` … `macro/40.webp` – pictures 1–40 (same meaning as the built-in pictures of the original software)
- `macro/frame.webp` – empty frame; the GUI draws the numbers 41–100 on it

All files are 128×128, WebP quality 90 with lossless alpha, and are embedded into the GUI binary. To rebuild them from
the high-resolution sources:

```
py -3 -m pip install --user pillow
py -3 assets/icons/make_icons.py <folder with 0.png … 40.png> --frame <empty frame png>
```
