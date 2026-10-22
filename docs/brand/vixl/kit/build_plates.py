"""Catalog plates: a screenshot framed like a page from a seed catalog, for the README and the docs.

    python build_plates.py SHOT.png "FIG. 1" "EDIT MODE" "Paint maps with the tile brush..." OUT_NAME

SHOT is a 1280x800 screenshot (the editor tests write them with FRM_SCREENSHOT_DIR). Writes out/OUT_NAME.png.
"""
import sys
from vx import *; from art import *

PAPER = "#f2e5c6"; RED = "#b5332a"; INK = "#3a2a1e"; MUTED = "#6d5a40"; KRAFT = "#c9b48a"; SHADOW = "#6e1d17"
F = [("Alfa Slab One", 400), ("Oswald", 700), ("Courier Prime", 400)]


def plate(shot, fig, title, caption, name, w=1280, h=960):
    sw, sh = 1100, 688
    sx, sy = (w - sw) // 2, 168
    ops = [{"type": "solid", "name": "paper", "color": PAPER}, {"type": "look", "target": "paper", "look": "paper"},
           rect("frame-out", 16, 16, w - 32, h - 32, "transparent", stroke=RED, stroke_width=8),
           rect("frame-in", 32, 32, w - 64, h - 64, "transparent", stroke=RED, stroke_width=2),
           text("fig", fig, sx, 70, 20, RED, "oswald-700", tracking=7),
           text("title", title, sx, 96, 44, INK, "alfa-slab-one-400", tracking=2),
           rect("mat", sx - 10, sy - 10, sw + 20, sh + 20, "#f7eed6", r=6, stroke=KRAFT, stroke_width=2),
           {"type": "add", "path": os.path.abspath(shot), "name": "shot", "x": sx, "y": sy, "width": sw, "height": sh},
           text("caption", caption, sx, sy + sh + 36, 18, MUTED, "courier-prime-400")]
    return make(name, w, h, PAPER, F, ops)


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    plate(*sys.argv[1:6])
