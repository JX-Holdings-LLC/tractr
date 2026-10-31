"""Brambleford's crops, items and map props, drawn as palette grids for Vixl `pixel-art` layers."""

from pixels import Grid

CROP_PALETTE = {
    ".": "transparent",
    "d": "#6b4a2e", "D": "#4f3520",            # soil mound
    "l": "#5fae4a", "L": "#3f7f36", "v": "#2f5f2a",  # leaves
    "o": "#e8642c", "O": "#b8401e", "y": "#ffd36b",  # emberroot
    "m": "#bfe6f2", "M": "#8cc4dc", "n": "#5f9fbf", "w": "#ffffff",  # moonmelon
    "f": "#fff6d8", "F": "#ffd84a",            # flowers
}


def _mound(g, cx=16, y=44):
    g.ellipse(cx - 0.5, y, 5, 1.6, "d")
    g.hline(cx - 4, cx + 3, y + 1, "D")


def _leaf(g, x, y, length, direction, c="l", c2="L"):
    """A small diagonal leaf from (x, y) going up-left (-1) or up-right (+1)."""
    for i in range(length):
        g.set(x + direction * i, y - i // 2, c)
        g.set(x + direction * i, y - i // 2 + 1, c2)


def emberroot(stage):
    g = Grid(32, 48)
    _mound(g)
    if stage == 0:
        g.pixels([(15, 42), (16, 41), (16, 42)], "l")
        return g
    if stage == 1:
        g.rect(15, 38, 16, 42, "L")
        _leaf(g, 14, 39, 4, -1)
        _leaf(g, 17, 39, 4, 1)
        return g
    height = {2: 9, 3: 13, 4: 15}[stage]
    top = 43 - height
    for i, dx in enumerate((-5, -2, 1, 4)):
        x = 15 + dx // 2
        g.rect(x, top + (i % 2) * 2, x + 1, 42, "L")
    g.ellipse(15.5, top + 3, 6 if stage == 2 else 8, 4 if stage == 2 else 5, "l")
    g.ellipse(18, top + 5, 4, 3, "L", only="l")
    if stage >= 3:
        g.pixels([(12, top + 2), (14, top + 4), (17, top + 1), (19, top + 3)], "O")  # red veins
    if stage == 4:
        g.ellipse(15.5, 42, 4.5, 3, "o")
        g.ellipse(17, 43, 2.5, 1.6, "O", only="o")
        g.pixels([(13, 41), (14, 40)], "y")
    return g


def moonmelon(stage):
    g = Grid(32, 48)
    _mound(g)
    if stage == 0:
        g.pixels([(15, 42), (16, 41), (15, 41)], "l")
        return g
    g.rect(15, 39, 16, 42, "L")
    _leaf(g, 14, 40, 4, -1)
    _leaf(g, 17, 40, 4, 1)
    if stage == 1:
        return g
    # vines curling out along the ground
    for x in range(8, 24):
        y = 42 - (1 if (x // 3) % 2 else 0)
        g.set(x, y, "v")
    g.ellipse(9, 39, 3, 2, "l")
    g.ellipse(22, 39, 3, 2, "l")
    g.ellipse(15.5, 37, 4, 3, "l")
    g.ellipse(17, 38, 2, 1.5, "L", only="l")
    if stage == 2:
        g.pixels([(11, 37), (21, 36)], "f")
        g.pixels([(10, 37), (12, 37), (11, 36), (11, 38)], "F")
        return g
    if stage == 3:
        g.ellipse(18.5, 40, 3.5, 2.8, "m")
        g.ellipse(19.5, 41, 2, 1.6, "M", only="m")
        return g
    g.ellipse(17.5, 38.5, 6.5, 5, "m")
    g.ellipse(19, 40, 4.5, 3.2, "M", only="m")
    for x in (14, 17, 20):
        g.rect(x, 35, x, 42, "n", only="mM")
    g.pixels([(13, 36), (14, 35)], "w")
    g.pixels([(25, 32), (24, 33), (26, 33), (25, 34), (9, 30)], "w")  # sparkle
    return g


ITEM_PALETTE = dict(CROP_PALETTE, **{
    "p": "#e9d9b4", "P": "#c9b48a", "t": "#7a5a3a", "r": "#c94a3a", "b": "#3f6fa0",
})


def item_emberroot():
    g = Grid(32, 32)
    g.ellipse(15, 20, 6, 7, "o")
    g.ellipse(17, 22, 3.5, 4.5, "O", only="o")
    g.rect(14, 26, 16, 29, "o")
    g.set(15, 30, "O")
    g.pixels([(12, 16), (12, 17), (13, 15)], "y")
    for i, dx in enumerate((-3, 0, 3)):
        g.rect(15 + dx, 8 + abs(dx), 16 + dx, 13, "l" if i != 1 else "L")
    return g


def item_moonmelon():
    g = Grid(32, 32)
    g.ellipse(15.5, 18, 10, 8.5, "m")
    g.ellipse(18, 20, 7, 6, "M", only="m")
    for x in (10, 15, 20):
        g.rect(x, 11, x, 26, "n", only="mM")
    g.pixels([(9, 13), (10, 12), (11, 12)], "w")
    g.rect(15, 7, 16, 9, "L")
    g.pixels([(17, 7), (18, 6)], "l")
    return g


def seed_packet(color, picture):
    g = Grid(32, 32)
    g.rect(8, 5, 23, 27, "p")
    g.rect(20, 6, 23, 27, "P")
    g.hline(8, 23, 5, "P")
    g.rect(8, 22, 23, 24, color)
    pic = picture()
    for y in range(32):
        for x in range(32):
            c = pic.get(x, y)
            if c != "." and 8 <= x // 2 + 8 < 24:
                g.set(x // 2 + 8, y // 2 + 4, c)
    return g


LIGHT_PALETTE = {
    ".": "transparent",
    "w": "#6b4a2e", "W": "#4a321f",      # wood
    "i": "#3a3640", "I": "#5a5560",      # iron
    "y": "#ffd36b", "Y": "#fff1b8", "o": "#f29a3a",  # flame glow
    "g": "#4f8f3a", "G": "#3a6f2c",
    "r": "#e8556a", "u": "#8a6ad8", "s": "#ffe066", "S": "#ffffff",
    "b": "#9a7a52", "B": "#7a5c3a", "n": "#c9a878",
    "c": "#8a8a95", "C": "#6b6b78", "q": "#bfe6f2",
}


def lantern(lit):
    g = Grid(32, 64)
    g.rect(15, 26, 16, 61, "i")
    g.rect(16, 26, 16, 61, "I")
    g.rect(12, 60, 19, 62, "i")
    g.rect(10, 22, 21, 23, "i")  # arm
    g.rect(11, 12, 20, 21, "i")
    g.rect(12, 13, 19, 20, "y" if lit else "C")
    if lit:
        g.rect(14, 15, 17, 18, "Y")
        g.pixels([(15, 16), (16, 16)], "o")
    g.rect(10, 9, 21, 11, "i")
    g.rect(14, 7, 17, 8, "i")
    return g


def fence():
    g = Grid(32, 32)
    for x in (4, 26):
        g.rect(x, 10, x + 2, 28, "w")
        g.rect(x + 2, 11, x + 2, 28, "W")
        g.set(x + 1, 9, "w")
    for y in (14, 22):
        g.rect(0, y, 31, y + 2, "b")
        g.hline(0, 31, y + 2, "B")
    return g


def flowers(variant):
    g = Grid(32, 32)
    spots = [(6, 20), (14, 25), (22, 18), (26, 27), (10, 9), (19, 10)]
    colors = ["r", "u", "s", "r", "s", "u"]
    if variant:
        spots = [(x + 1, y) for x, y in spots]
    for (x, y), c in zip(spots, colors):
        g.rect(x, y + 2, x, y + 4, "G")
        g.pixels([(x - 1, y + 3)], "g")
        g.pixels([(x, y - 1), (x - 1, y), (x + 1, y), (x, y + 1)], c)
        g.set(x, y, "s" if c != "s" else "S")
    return g


def millwheel(turn):
    g = Grid(32, 64)
    cx, cy = 15.5, 38
    g.ellipse(cx, cy, 14, 14, "w")
    g.ellipse(cx, cy, 11, 11, ".", only="w")
    g.ellipse(cx, cy, 3, 3, "i")
    import math
    for k in range(8):
        a = (k + (0.5 if turn else 0)) * math.pi / 4
        for r in range(3, 14):
            g.set(int(cx + 0.5 + r * math.cos(a)), int(cy + 0.5 + r * math.sin(a)), "W")
        x = int(cx + 0.5 + 14 * math.cos(a))
        y = int(cy + 0.5 + 14 * math.sin(a))
        g.rect(x - 1, y - 1, x + 1, y + 1, "b")
    g.rect(13, 52, 18, 62, "W")  # post
    g.pixels([(6 + (3 if turn else 0), 55), (24 - (3 if turn else 0), 57), (12, 58)], "q")  # splashes
    return g


def bridge():
    g = Grid(32, 32)
    for y in range(0, 32, 6):
        g.rect(0, y, 31, y + 4, "b")
        g.hline(0, 31, y + 4, "B")
        g.pixels([(3, y + 2), (28, y + 2)], "W")
    return g


def well():
    g = Grid(32, 48)
    g.ellipse(15.5, 38, 12, 6, "c")
    g.ellipse(15.5, 36, 9, 3.5, "q")
    g.rect(4, 38, 27, 44, "c")
    g.rect(4, 42, 27, 44, "C")
    for x in range(5, 27, 5):
        g.rect(x, 39, x, 41, "C")
    g.rect(5, 14, 6, 37, "w")
    g.rect(25, 14, 26, 37, "w")
    for i, y in enumerate(range(8, 15)):
        g.rect(3 + i, y, 28 - i, y, "r" if i < 5 else "B")
    g.rect(14, 15, 17, 22, "W")
    return g


def _tall(g):
    """Pads a sprite to 32x64, the frame of a terrain one tile taller than its own."""
    out = Grid(32, 64)
    out.paste(g, 0, 64 - g.h)
    return out


SHEETS = {
    "crop-emberroot": {"kind": "crop", "frames": [emberroot(s) for s in range(5)], "palette": CROP_PALETTE},
    "crop-moonmelon": {"kind": "crop", "frames": [moonmelon(s) for s in range(5)], "palette": CROP_PALETTE},
    "item-emberroot": {"kind": "item", "frames": [item_emberroot()], "palette": ITEM_PALETTE},
    "item-moonmelon": {"kind": "item", "frames": [item_moonmelon()], "palette": ITEM_PALETTE},
    "item-seed-emberroot": {"kind": "item", "frames": [seed_packet("r", item_emberroot)], "palette": ITEM_PALETTE},
    "item-seed-moonmelon": {"kind": "item", "frames": [seed_packet("b", item_moonmelon)], "palette": ITEM_PALETTE},
    "prop-lantern": {"kind": "terrain", "frames": [lantern(True), lantern(False)], "palette": LIGHT_PALETTE},
    "prop-fence": {"kind": "terrain", "frames": [fence()], "palette": LIGHT_PALETTE},
    "prop-flowers": {"kind": "terrain", "frames": [flowers(0), flowers(1)], "palette": LIGHT_PALETTE, "outline": False},
    "prop-millwheel": {"kind": "terrain", "frames": [millwheel(False), millwheel(True)], "palette": LIGHT_PALETTE},
    "prop-bridge": {"kind": "terrain", "frames": [bridge()], "palette": LIGHT_PALETTE, "outline": False},
    "prop-well": {"kind": "terrain", "frames": [_tall(well())], "palette": LIGHT_PALETTE},
}
