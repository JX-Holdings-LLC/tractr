"""Tractr marketing pieces: the README hero and social card, the About banner and the feature packets.

    cd docs/brand/vixl/kit && python build_marketing.py      # writes out/*.png (needs the vixl CLI and network for fonts)

Same rules as STYLE.md: flat inks on Paper, one sunburst, one ribbon and one seal per piece, hard offset
shadows only on display type, the tractor whole and facing right.
"""
from vx import *; from art import *

PAPER = "#f2e5c6"; LIGHT = "#f7eed6"; RED = "#b5332a"; GREEN = "#4f6b3a"; SPROUT = "#7d9440"; MUST = "#e3a83a"
SUN = "#f0c35e"; INK = "#3a2a1e"; SHADOW = "#6e1d17"; MUTED = "#6d5a40"; KRAFT = "#c9b48a"; SOIL = "#9b6b42"
FURROW = "#3d5530"; WHEAT = "#c58f2c"; STALK = "#a8772a"
F = [("Alfa Slab One", 400), ("Lobster", 400), ("Oswald", 700), ("Courier Prime", 400)]
C = {"body": RED, "dark": INK, "tire": "#2a1f18", "rim": MUST, "hub": "#8f2a22", "light": PAPER, "lamp": "#fff4d6"}


def frame(w, h, m=16):
    """Heavy Barn Red outer rule and a thin inner rule."""
    return [rect("frame-out", m, m, w - 2 * m, h - 2 * m, "transparent", stroke=RED, stroke_width=8),
            rect("frame-in", m + 16, m + 16, w - 2 * m - 32, h - 2 * m - 32, "transparent", stroke=RED, stroke_width=2)]


def scene(cx, w, h, ground, scale, rays=26):
    """Sunburst, sun, hills, crop rows, the tractor and wheat, centred on cx; ground is the hill-top y."""
    ops = [{"type": "shape", "shape": "sunburst", "name": "rays", "x": cx - 760, "y": ground - 1000, "width": 1520, "height": 1520,
            "fill": MUST, "count": rays, "opacity": 0.55},
           {"type": "look", "target": "rays", "look": "halftone"},
           ell("sun", cx - 175, ground - 330, 350, 350, SUN),
           {"type": "shape", "shape": "ellipse", "name": "hill2", "x": cx - 760, "y": ground + 20, "width": 900, "height": 420, "fill": SPROUT},
           {"type": "shape", "shape": "ellipse", "name": "hill", "x": cx - 330, "y": ground + 50, "width": 1100, "height": 420, "fill": GREEN}]
    for i in range(6):
        ops.append({"type": "shape", "shape": "path", "name": f"row{i}", "x": 0, "y": 0,
                    "path": f"M {cx - 260 + i * 110} {h + 20} Q {cx - 120 + i * 80} {ground + 150} {cx + 20 + i * 50} {ground + 62}",
                    "stroke": FURROW, "stroke_width": 8, "line_cap": "round"})
    tw, th = 600 * scale, 420 * scale
    ops += tractor("tr", cx - tw / 2, ground + 62 - th + 18, scale, C)
    return ops, tw, th


def seal_badge(x, y, d, tilt=12, size=22):
    return [{"type": "shape", "shape": "seal", "name": "seal", "x": x, "y": y, "width": d, "height": d, "fill": GREEN,
             "count": 24, "inner_radius": 0.88, "rotation": tilt},
            ell("seal-in", x + d * 0.1, y + d * 0.1, d * 0.8, d * 0.8, "transparent", stroke=PAPER, stroke_width=3, rotation=tilt),
            {"type": "text", "name": "seal-t", "text": "NO\nCODE\nNEEDED", "size": size, "color": PAPER, "font": "oswald-700",
             "align": "center", "line_height": 1.0, "within": "seal", "rotation": tilt, "tracking": 2}]


def hero(w=1280, h=640):
    ops = [{"type": "solid", "name": "paper", "color": PAPER}]
    sc, tw, th = scene(950, w, h, 395, 0.84)
    ops += sc
    # the packet panel: a flat Paper band that holds the words; scenery runs behind its edge
    ops += [rect("panel", 0, 0, 620, h, PAPER), rect("seam", 620, 0, 3, h, KRAFT), {"type": "look", "target": "panel", "look": "paper"}]
    ops += [wheat(f"wh{i}", x, 560, 250, WHEAT, lean=l, grains=8, stem=STALK, sw=4) for i, (x, l) in enumerate([(652, 0.05), (678, -0.08)])]
    ops = [o for g in ops for o in (g if isinstance(g, list) else [g])]
    for i, (x, l) in enumerate([(1236, -0.05), (1214, 0.08)]):
        ops += wheat(f"whr{i}", x, 560, 250, WHEAT, lean=l, grains=8, stem=STALK, sw=4)
    ops += frame(w, h)
    ops += [text("kicker", "EST. 2026  ·  SEED No. 01", 72, 78, 20, RED, "oswald-700", tracking=7),
            {"type": "shape", "shape": "ribbon", "name": "ribbon", "x": 56, "y": 128, "width": 540, "height": 124, "fill": RED},
            {"type": "text", "name": "wordmark", "text": "TRACTR", "size": 84, "color": PAPER, "font": "alfa-slab-one-400",
             "tracking": 6, "within": "ribbon"},
            {"type": "layer-style", "target": "wordmark", "name": "drop-shadow",
             "settings": {"dx": 3, "dy": 3, "blur": 0, "opacity": 1, "color": SHADOW}},
            text("script", "Grow your own worlds!", 72, 270, 60, INK, "lobster-400"),
            text("sub", "THE FARM GAME ENGINE", 72, 372, 30, INK, "oswald-700", tracking=8),
            text("desc", "Plant crops · Raise critters\nShip to every platform", 72, 432, 19, INK, "courier-prime-400", line_height=1.5),
            text("small", "Made for cozy farm games. Windows, Linux and the web.", 72, 520, 16, MUTED, "courier-prime-400")]
    ops += seal_badge(1076, 58, 140)
    ops += [{"type": "look", "target": "paper", "look": "paper"}]
    ops += [{"type": "layer-intent", "targets": ["rays", "hill", "hill2"] + [f"row{i}" for i in range(6)], "allow_crop": True},
            {"type": "layer-intent", "targets": ["paper", "rays", "sun", "hill", "hill2", "panel", "seam", "frame-out", "frame-in"]
             + [f"row{i}" for i in range(6)], "role": "background"}]
    return make("tractr-hero", w, h, PAPER, F, ops)


def about_banner(w=880, h=330):
    """The scene alone, for the About window and the welcome screen."""
    ops = [{"type": "solid", "name": "paper", "color": PAPER}]
    sc, tw, th = scene(w / 2, w, h, 190, 0.62, rays=22)
    ops += sc
    for i, (x, l) in enumerate([(150, 0.05), (178, -0.08), (702, -0.05), (730, 0.08)]):
        ops += wheat(f"wh{i}", x, h - 6, 190, WHEAT, lean=l, grains=7, stem=STALK, sw=3)
    ops += [{"type": "look", "target": "paper", "look": "paper"},
            {"type": "layer-intent", "targets": ["rays", "hill", "hill2"] + [f"row{i}" for i in range(6)], "allow_crop": True},
            {"type": "layer-intent", "targets": ["paper", "rays", "sun", "hill", "hill2"] + [f"row{i}" for i in range(6)], "role": "background"}]
    return make("tractr-about-banner", w, h, PAPER, F, ops)


def packets(w=1280, h=440):
    """Four seed packets: the four things Tractr does."""
    ops = [{"type": "solid", "name": "paper", "color": PAPER}]
    items = [("01", "EDIT", "Paint maps, fill in forms", "tiles"),
             ("02", "PLAY", "Test it in one click", "wheat"),
             ("03", "MOD", "Plugins and content packs", "cog"),
             ("04", "SHIP", "Windows, Linux and web", "tractor")]
    pw, ph, gap = 276, 340, 28
    x0 = (w - (4 * pw + 3 * gap)) / 2
    for i, (n, title, line, motif) in enumerate(items):
        x = x0 + i * (pw + gap); y = (h - ph) / 2
        k = f"p{i}"
        ops += [rect(f"{k}-card", x, y, pw, ph, LIGHT, r=12, stroke=KRAFT, stroke_width=2),
                rect(f"{k}-band", x + 14, y + 14, pw - 28, 44, RED, r=6),
                rect(f"{k}-no-s", x + 14, y + 14, pw - 28, 44, "transparent"),
                {"type": "text", "name": f"{k}-no", "text": f"SEED No. {n}", "size": 18, "color": PAPER, "font": "oswald-700", "tracking": 5, "within": f"{k}-no-s"},
                rect(f"{k}-title-s", x + 14, y + 236, pw - 28, 60, "transparent"),
                {"type": "text", "name": f"{k}-title", "text": title, "size": 44, "color": RED, "font": "alfa-slab-one-400", "tracking": 3, "within": f"{k}-title-s"},
                rect(f"{k}-line-s", x + 14, y + 296, pw - 28, 28, "transparent"),
                {"type": "text", "name": f"{k}-line", "text": line, "size": 15, "color": INK, "font": "courier-prime-400", "within": f"{k}-line-s"}]
        cx, cy = x + pw / 2, y + 150
        if motif != "tiles":
            ops.append(ell(f"{k}-sun", cx - 62, cy - 70, 124, 124, SUN))
        if motif == "tiles":
            cols = [GREEN, SPROUT, SOIL]
            for r_ in range(4):
                for c_ in range(5):
                    col = SOIL if (1 <= c_ <= 3 and 1 <= r_ <= 2) else (GREEN if (r_ + c_) % 2 else SPROUT)
                    ops.append(rect(f"{k}-t{r_}{c_}", cx - 75 + c_ * 30, cy - 50 + r_ * 30, 28, 28, col, r=3))
        elif motif == "wheat":
            ops.append({"type": "shape", "shape": "half-circle", "name": f"{k}-hill", "x": cx - 100, "y": cy + 30, "width": 200, "height": 62, "fill": GREEN})
            for j, (dx, l) in enumerate([(-30, -0.08), (0, 0.04), (30, 0.1)]):
                ops += wheat(f"{k}-w{j}", cx + dx, cy + 80, 130, WHEAT, lean=l, grains=7, stem=STALK, sw=3)
        elif motif == "cog":
            ops += [{"type": "shape", "shape": "cog", "name": f"{k}-cog", "x": cx - 62, "y": cy - 60, "width": 124, "height": 124, "fill": INK, "teeth": 14, "depth": 0.08, "hole": 0},
                    ell(f"{k}-rim", cx - 40, cy - 38, 80, 80, MUST), ell(f"{k}-hub", cx - 17, cy - 15, 34, 34, RED), ell(f"{k}-bolt", cx - 6, cy - 4, 12, 12, INK)]
        else:
            ops += tractor(f"{k}-tr", cx - 90, cy - 44, 0.3, C)
    ops += [{"type": "look", "target": "paper", "look": "paper"}]
    ops += frame(w, h, 10)
    return make("tractr-packets", w, h, PAPER, F, ops)


if __name__ == "__main__":
    import sys
    os.makedirs(OUT, exist_ok=True)
    only = sys.argv[1:] or ["hero", "about", "packets"]
    if "hero" in only: hero()
    if "about" in only: about_banner()
    if "packets" in only: packets()
