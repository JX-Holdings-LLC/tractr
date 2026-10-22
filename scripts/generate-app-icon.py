#!/usr/bin/env python3
"""Builds the Tractr app icon and the in-app brand images from docs/brand/logo.

The art itself is drawn with Vixl (docs/brand/vixl); this script only resizes it. Writes, into
src/Tractr.App/Assets:

  app-icon-256.png     the seal on a paper tile (About, Update Center, window headers)
  app.ico              16-256 px; 64 px and up use the paper tile, smaller sizes the bare seal
  tractr-logo.png      the red ribbon wordmark, 600x150 (main window header)
  paper-grain.png      a seamless 256 px paper grain tile (window and header backgrounds)

Needs Pillow. Run from the repo root:  python3 scripts/generate-app-icon.py
"""
import os
import random

from PIL import Image

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
LOGO = os.path.join(ROOT, "docs", "brand", "logo")
OUT = os.path.join(ROOT, "src", "Tractr.App", "Assets")
PAPER = (0xF2, 0xE5, 0xC6)


def load(name):
    return Image.open(os.path.join(LOGO, name)).convert("RGBA")


def fit(image, size):
    return image.resize((size, size), Image.LANCZOS)


def grain_tile(size=256, strength=7, seed=1950):
    """Paper with a faint speckle. Each pixel moves by a small random amount, so it tiles seamlessly."""
    rng = random.Random(seed)
    tile = Image.new("RGBA", (size, size))
    pixels = tile.load()
    for y in range(size):
        for x in range(size):
            d = rng.randint(-strength, strength)
            pixels[x, y] = (PAPER[0] + d, PAPER[1] + d, PAPER[2] + d, 255)
    return tile


def main():
    os.makedirs(OUT, exist_ok=True)
    tile = load("tractr-app-icon.png")
    seal = load("tractr-icon-seal.png")

    fit(tile, 256).save(os.path.join(OUT, "app-icon-256.png"), optimize=True)

    sizes = [256, 128, 64, 48, 32, 24, 16]
    frames = [fit(tile if size >= 64 else seal, size) for size in sizes]
    frames[0].save(os.path.join(OUT, "app.ico"), format="ICO", sizes=[(s, s) for s in sizes], append_images=frames[1:])

    load("tractr-logo-primary.png").resize((600, 150), Image.LANCZOS).save(os.path.join(OUT, "tractr-logo.png"), optimize=True)

    grain_tile().save(os.path.join(OUT, "paper-grain.png"), optimize=True)
    print("wrote", os.path.abspath(OUT))


if __name__ == "__main__":
    main()
