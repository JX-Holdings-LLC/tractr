"""Brambleford's villagers: walking sprite sheets and dialogue portraits, drawn as palette grids.

A sheet is 4 columns (walk frames) by 4 rows (down, left, right, up) of 32x48 frames, the layout Tractr
reads for a directional NPC sheet. A portrait is a 24x24 bust doubled to 48x48, one per expression.
"""

from pixels import Grid

FRAME_W, FRAME_H = 32, 48
DIRECTIONS = ["down", "left", "right", "up"]

# Each villager: colours for the palette symbols, a hair style, an outfit and extras.
PEOPLE = {
    "odile": dict(
        name="Mayor Odile Pennywhistle",
        skin="#f0c8a4", hair="#d9d4e3", shirt="#7a4ea3", pants="#3d2f5c", boots="#4a2e2a",
        acc="#e8b84a", acc2="#f4efe2", eyes="#3a2a3a",
        hair_style="bun", outfit="dress", extras=["glasses", "brooch"],
    ),
    "bram": dict(
        name="Bram Holloway",
        skin="#c98d63", hair="#6b4024", shirt="#4a6fa5", pants="#4c4a5e", boots="#3b2a20",
        acc="#f1ece0", acc2="#d8cfc0", eyes="#2b1d17",
        hair_style="beard", outfit="apron", extras=["burly"],
    ),
    "juniper": dict(
        name="Juniper Vale",
        skin="#f3d1b0", hair="#c4452f", shirt="#f2e6c9", pants="#5b4632", boots="#5a3a24",
        acc="#4f8a3c", acc2="#3c6b2e", eyes="#2c3b2a",
        hair_style="long", outfit="apron", extras=["freckles"],
    ),
    "pip": dict(
        name="Pip Marsh",
        skin="#f2cfa6", hair="#f0c64a", shirt="#f2a33a", pants="#3f6db0", boots="#7c4a2a",
        acc="#e05a3a", acc2="#b8432a", eyes="#2b2b3a",
        hair_style="messy", outfit="shorts", extras=["kid"],
    ),
    "sela": dict(
        name="Sela Marsh",
        skin="#8a5a3c", hair="#1f1a24", shirt="#2f8f8a", pants="#2c3e50", boots="#2a2a33",
        acc="#f2d24a", acc2="#c9a72e", eyes="#1a1414",
        hair_style="short", outfit="coat", extras=["earring"],
    ),
    "marigold": dict(
        name="Marigold Fenn",
        skin="#f5d3b8", hair="#e07a2e", shirt="#e8749a", pants="#6a3a52", boots="#6a3a2a",
        acc="#fff3e0", acc2="#e8d8c0", eyes="#3a2430",
        hair_style="curly", outfit="dress", extras=["flower"],
    ),
    "rook": dict(
        name="Rook",
        skin="#d9a27a", hair="#8d8a96", shirt="#5d6b4a", pants="#3d3a33", boots="#2e2620",
        acc="#4a3a5e", acc2="#33284a", eyes="#22202a",
        hair_style="hat", outfit="cloak", extras=["scarf"],
    ),
}


def shade(hex_color, factor):
    """Darker (factor < 1) or lighter (factor > 1) version of a #rrggbb colour."""
    r, g, b = (int(hex_color[i:i + 2], 16) for i in (1, 3, 5))
    if factor < 1:
        r, g, b = (int(v * factor) for v in (r, g, b))
    else:
        r, g, b = (int(v + (255 - v) * (factor - 1)) for v in (r, g, b))
    return "#%02x%02x%02x" % (r, g, b)


def palette(p):
    return {
        ".": "transparent",
        "k": p["skin"], "K": shade(p["skin"], 0.82),
        "h": p["hair"], "H": shade(p["hair"], 0.75),
        "s": p["shirt"], "S": shade(p["shirt"], 0.75),
        "p": p["pants"], "P": shade(p["pants"], 0.75),
        "b": p["boots"],
        "a": p["acc"], "A": shade(p["acc"], 0.8),
        "c": p["acc2"],
        "e": p["eyes"], "w": "#ffffff", "r": "#e8828a", "m": shade(p["skin"], 0.55),
        "g": "#2e2a33", "y": "#ffe27a", "f": "#ff6f91",
    }


# ---------------------------------------------------------------- sprites


def _legs(g, view, frame, top, kid):
    """Trousers (or bare legs under a skirt) and boots, with the walk pose of `frame`."""
    bottom = 44
    lift = {0: (0, 0), 1: (2, 0), 2: (0, 0), 3: (0, 2)}[frame]
    if view in ("down", "up"):
        for i, (x0, x1) in enumerate(((11, 14), (17, 20))):
            up = lift[i]
            g.rect(x0, top, x1, bottom - 3 - up, "p")
            g.rect(x1, top + 2, x1, bottom - 3 - up, "P")
            g.rect(x0, bottom - 2 - up, x1, bottom - up, "b")
        g.rect(13, top, 18, top + 2, "p")
    else:  # left view; the right view is its mirror
        stride = {0: 0, 1: 3, 2: 0, 3: -3}[frame]
        back, front = (14 - stride // 2, 17 - stride // 2), (13 + stride // 2, 16 + stride // 2)
        if stride == 0:
            front = (13, 17)
            back = None
        if back:
            g.rect(back[0], top, back[1], bottom - 3, "P")
            g.rect(back[0] - 1, bottom - 2, back[1], bottom, "b")
        g.rect(front[0], top, front[1], bottom - 3, "p")
        g.rect(front[0] - 1, bottom - 2, front[1], bottom, "b")


def _torso(g, view, p, frame, top):
    """Shirt, arms and hands, plus the outfit over them."""
    outfit = p["outfit"]
    burly = "burly" in p["extras"]
    swing = {0: 0, 1: 1, 2: 0, 3: -1}[frame]
    hem = top + 10
    if view in ("down", "up"):
        x0, x1 = (9, 22) if burly else (10, 21)
        g.rect(x0, top, x1, hem, "s")
        g.rect(x1 - 1, top + 1, x1, hem, "S")
        # arms swing opposite each other
        for side, (ax0, ax1) in ((1, (x0 - 2, x0 - 1)), (-1, (x1 + 1, x1 + 2))):
            reach = hem - 1 + swing * side
            g.rect(ax0, top + 1, ax1, reach, "s" if side > 0 else "S")
            g.rect(ax0, reach + 1, ax1, reach + 2, "k")
        if outfit == "apron" and view == "down":
            g.rect(x0 + 2, top + 3, x1 - 2, hem + 6, "a")
            g.rect(x0 + 2, top + 3, x1 - 2, top + 3, "A")
            g.rect(x0 + 5, top + 6, x1 - 5, top + 7, "A")  # pocket
        if outfit == "apron" and view == "up":
            g.hline(x0, x1, top + 5, "a")  # apron strings
        if outfit == "dress":
            for i, y in enumerate(range(hem - 2, hem + 6)):
                g.rect(x0 - i // 3, y, x1 + i // 3, y, "s")
                g.set(x1 + i // 3, y, "S")
            g.hline(x0 - 2, x1 + 2, hem + 6, "S")
        if outfit == "coat":
            g.rect(x0, hem, x1, hem + 4, "s")
            g.rect(x1 - 1, hem, x1, hem + 4, "S")
            if view == "down":
                g.rect(15, top + 1, 16, hem + 4, "S")
                g.pixels([(14, top + 4), (14, top + 7)], "a")
        if outfit == "cloak":
            g.rect(x0 - 2, top, x1 + 2, hem + 3, "c" if view == "up" else "s")
            g.rect(x1 + 1, top + 1, x1 + 2, hem + 3, "S")
            if view == "down":
                g.rect(14, top + 1, 17, hem + 3, "S")
                g.rect(15, top + 1, 16, hem + 3, "c")
        if outfit == "shorts":
            g.rect(x0, hem - 1, x1, hem, "S")
    else:
        x0, x1 = (11, 20) if burly else (12, 19)
        g.rect(x0, top, x1, hem, "s")
        g.rect(x1 - 1, top + 1, x1, hem, "S")
        if outfit == "apron":
            g.rect(x0, top + 3, x0 + 2, hem + 6, "a")
            g.hline(x0, x1, top + 5, "a")
        if outfit == "dress":
            for i, y in enumerate(range(hem - 2, hem + 6)):
                g.rect(x0 - i // 3, y, x1 + i // 3, y, "s")
            g.hline(x0 - 2, x1 + 2, hem + 6, "S")
        if outfit == "coat":
            g.rect(x0, hem, x1, hem + 4, "s")
            g.rect(x1 - 1, hem, x1, hem + 4, "S")
        if outfit == "cloak":
            g.rect(x0 - 1, top, x1 + 2, hem + 3, "s")
            g.rect(x1, top + 1, x1 + 2, hem + 3, "S")
        # one arm, swinging forward and back
        ax = 15 - swing * 2
        g.rect(ax, top + 1, ax + 2, hem - 1, "S")
        g.rect(ax, hem, ax + 2, hem + 1, "k")


def _head(g, view, p, top):
    """Head, face and hair. `top` is the head's top row."""
    style = p["hair_style"]
    extras = p["extras"]
    cx, cy = 15.5, top + 7
    if style == "curly":
        g.ellipse(cx + (1 if view == "left" else 0), cy - 1, 9, 8.5, "h")
        g.ellipse(cx + (1 if view == "left" else 0) + 2, cy + 1, 7, 6, "H", only="h", y_min=int(cy) + 3)
    if style == "long" and view in ("down", "left", "up"):
        if view == "down":
            g.rect(7, top + 4, 9, top + 19, "h")
            g.rect(22, top + 4, 24, top + 19, "H")
        elif view == "left":
            g.rect(17, top + 4, 23, top + 19, "h")
            g.rect(21, top + 6, 23, top + 19, "H")
        else:
            g.rect(8, top + 4, 23, top + 19, "h")
            g.rect(20, top + 6, 23, top + 19, "H")
    # face
    g.ellipse(cx, cy, 7, 7.2, "k")
    if view == "left":
        g.ellipse(cx + 3, cy + 2, 4, 4, "K", only="k", y_min=int(cy) + 4)
        g.set(8, int(cy) + 1, "k")  # nose
    else:
        g.rect(9, int(cy) + 5, 22, int(cy) + 6, "K", only="k")
    # hair
    if style == "bun":
        g.ellipse(cx, cy - 3, 7.5, 4.5, "h", y_min=top - 1)
        g.ellipse(cx + (3 if view == "left" else 0), top - 1, 3, 3, "h")
        g.set(int(cx) + (3 if view == "left" else 0), top - 2, "H")
        if view == "down":
            g.rect(8, int(cy) - 2, 8, int(cy) + 2, "h")
            g.rect(23, int(cy) - 2, 23, int(cy) + 2, "H")
    elif style == "beard":
        g.ellipse(cx, cy - 5, 6, 2.5, "h", y_min=top - 1, y_max=top + 1)
        if view == "down":
            g.rect(8, int(cy) - 2, 9, int(cy) + 2, "h")
            g.rect(22, int(cy) - 2, 23, int(cy) + 2, "h")
            g.ellipse(cx, cy + 5, 6.5, 4, "h", y_min=int(cy) + 3)
            g.rect(13, int(cy) + 3, 18, int(cy) + 3, "H")  # moustache
        elif view == "left":
            g.rect(17, int(cy) - 3, 22, int(cy) + 2, "h")
            g.ellipse(cx - 2, cy + 5, 5, 4, "h", y_min=int(cy) + 3)
        else:
            g.rect(8, int(cy) - 3, 23, int(cy) + 3, "h")
    elif style == "long":
        g.ellipse(cx, cy - 3, 7.6, 5, "h", y_min=top - 1)
        if view == "down":
            g.rect(9, top + 3, 10, top + 6, "k")
            g.pixels([(12, top + 3), (13, top + 4)], "H")
    elif style == "messy":
        g.ellipse(cx, cy - 3, 7.6, 5, "h", y_min=top - 1)
        g.pixels([(10, top - 2), (13, top - 3), (16, top - 2), (19, top - 3), (21, top - 1),
                  (11, top - 1), (14, top - 2), (18, top - 2)], "h")
        if view == "down":
            g.pixels([(12, top + 4), (15, top + 5), (18, top + 4)], "h")
    elif style == "short":
        g.ellipse(cx, cy - 3, 7.6, 4.6, "h", y_min=top - 1)
        if view == "down":
            g.rect(8, int(cy) - 3, 8, int(cy) + 1, "h")
            g.rect(23, int(cy) - 3, 23, int(cy) + 1, "H")
        if view == "left":
            g.rect(17, int(cy) - 3, 23, int(cy) + 1, "h")
    elif style == "curly":
        g.ellipse(cx, cy - 4, 7.5, 3.5, "h")
    elif style == "hat":
        if view == "down":
            g.rect(8, int(cy) - 2, 9, int(cy) + 4, "h")
            g.rect(22, int(cy) - 2, 23, int(cy) + 4, "h")
        elif view == "left":
            g.rect(17, int(cy) - 2, 23, int(cy) + 5, "h")
        g.rect(9, top - 4, 22, top + 3, "a")
        g.rect(9, top + 1, 22, top + 2, "c")
        g.rect(20, top - 4, 22, top + 3, "A")
        g.rect(4, top + 3, 27, top + 4, "a")
        g.rect(4, top + 4, 27, top + 4, "A")
    if view == "up" and style not in ("hat",):
        g.ellipse(cx, cy, 7.4, 7.4, "h", y_max=int(cy) + 5)
        g.rect(9, int(cy) + 3, 22, int(cy) + 5, "H", only="h")
        if style == "bun":
            g.ellipse(cx, top, 3, 3, "H")
        if style == "beard":
            g.ellipse(cx, cy - 2, 5, 4, "k", only="h", y_max=int(cy) - 1)
    if view == "up" and style == "hat":
        g.ellipse(cx, cy + 1, 7.4, 6, "h", y_min=top + 5)
    if style == "curly" and view == "up":
        g.ellipse(cx, cy, 9, 8.5, "h")
        g.ellipse(cx + 2, cy + 2, 7, 6, "H", only="h", y_min=int(cy) + 3)
    # eyes and the rest of the face
    ey = int(cy) + 1
    if view == "down":
        for ex in (11, 19):
            g.rect(ex, ey, ex + 1, ey + 1, "e")
            g.set(ex, ey, "w")
        if "glasses" in extras:
            for ex in (10, 18):
                g.rect(ex, ey - 1, ex + 3, ey - 1, "g")
                g.rect(ex, ey + 2, ex + 3, ey + 2, "g")
                g.set(ex, ey, "g")
                g.set(ex + 3, ey, "g")
                g.set(ex, ey + 1, "g")
                g.set(ex + 3, ey + 1, "g")
            g.hline(14, 17, ey, "g")
        if p["hair_style"] != "beard":
            g.pixels([(10, ey + 3), (21, ey + 3)], "r")
            g.hline(15, 16, ey + 4, "m")
        if "freckles" in extras:
            g.pixels([(12, ey + 3), (13, ey + 4), (18, ey + 4), (19, ey + 3)], "K")
        if "flower" in extras:
            g.rect(21, top, 23, top + 2, "f")
            g.set(22, top + 1, "y")
        if "earring" in extras:
            g.set(8, ey + 4, "a")
            g.set(23, ey + 4, "a")
    elif view == "left":
        g.rect(10, ey, 11, ey + 1, "e")
        g.set(10, ey, "w")
        if "glasses" in extras:
            g.rect(9, ey - 1, 12, ey - 1, "g")
            g.rect(9, ey + 2, 12, ey + 2, "g")
            g.set(12, ey, "g")
            g.set(12, ey + 1, "g")
            g.hline(13, 17, ey, "g")
        if p["hair_style"] != "beard":
            g.set(12, ey + 3, "r")
            g.set(9, ey + 4, "m")
        if "flower" in extras:
            g.rect(18, top, 20, top + 2, "f")
            g.set(19, top + 1, "y")
        if "earring" in extras:
            g.set(17, ey + 4, "a")
    if "scarf" in extras and view != "up":
        g.rect(10, top + 14, 21, top + 16, "c")
        if view == "down":
            g.rect(18, top + 16, 20, top + 20, "c")
    if "scarf" in extras and view == "up":
        g.rect(10, top + 14, 21, top + 16, "c")
    if "brooch" in extras and view == "down":
        g.rect(15, top + 17, 16, top + 18, "a")


def frame(p, view, frame_index):
    """One 32x48 frame of `p` facing `view` ("right" is the mirror of "left")."""
    if view == "right":
        return frame(p, "left", frame_index).mirrored()
    g = Grid(FRAME_W, FRAME_H)
    kid = "kid" in p["extras"]
    bob = -1 if frame_index in (1, 3) else 0
    body_top = 24 + (5 if kid else 0)
    legs_top = body_top + (8 if kid else 11)
    _legs(g, view, frame_index, legs_top, kid)
    upper = Grid(FRAME_W, FRAME_H)
    _torso(upper, view, p, frame_index, body_top)
    _head(upper, view, p, body_top - 15)
    g.paste(upper.shifted(0, bob))
    return g


# ---------------------------------------------------------------- portraits

EXPRESSIONS = ["neutral", "happy", "sad"]


def portrait(p, expression):
    """A 24x24 bust for the dialogue box, doubled to 48x48 by the caller."""
    g = Grid(24, 24, oy=3)
    style = p["hair_style"]
    extras = p["extras"]
    cx, cy = 11.5, 10.5
    # shoulders and clothes
    g.ellipse(cx, 25, 10.5, 6, "s")
    g.ellipse(cx + 4, 25, 6, 5, "S", only="s")
    if p["outfit"] == "apron":
        g.rect(7, 20, 16, 23, "a")
    if p["outfit"] == "cloak":
        g.ellipse(cx, 25, 10.5, 6, "s")
        g.rect(10, 19, 13, 23, "c")
    if p["outfit"] == "coat":
        g.rect(11, 20, 12, 23, "S")
    # hair behind the face
    if style == "long":
        g.rect(3, 6, 20, 21, "h")
        g.rect(16, 8, 20, 21, "H")
    if style == "curly":
        g.ellipse(cx, cy - 1, 10, 9.5, "h")
        g.ellipse(cx + 3, cy + 2, 7, 6, "H", only="h", y_min=13)
    g.ellipse(cx, cy + 1, 7, 7.5, "k")
    g.rect(5, 15, 18, 17, "K", only="k")
    g.rect(10, 16, 13, 19, "k")  # neck
    g.rect(10, 18, 13, 19, "K")
    # hair on top
    if style == "bun":
        g.ellipse(cx, cy - 3, 7.6, 5, "h", y_min=1)
        g.ellipse(cx, 1.5, 3.2, 3, "h")
        g.rect(4, cy - 2, 5, cy + 3, "h")
        g.rect(18, cy - 2, 19, cy + 3, "H")
    elif style == "beard":
        g.ellipse(cx, cy - 6, 6, 2, "h", y_max=4)
        g.rect(4, cy - 2, 5, cy + 2, "h")
        g.rect(18, cy - 2, 19, cy + 2, "h")
        g.ellipse(cx, cy + 6, 7, 4.5, "h", y_min=int(cy) + 4)
        g.rect(9, int(cy) + 4, 14, int(cy) + 4, "H")
    elif style == "long":
        g.ellipse(cx, cy - 3, 7.8, 5, "h")
        g.pixels([(8, 6), (9, 7), (10, 7)], "H")
    elif style == "messy":
        g.ellipse(cx, cy - 3, 7.8, 5, "h")
        g.pixels([(5, 2), (8, 1), (11, 0), (14, 1), (17, 2), (7, 2), (13, 1), (16, 3)], "h")
        g.pixels([(8, 7), (12, 7), (15, 6)], "h")
    elif style == "short":
        g.ellipse(cx, cy - 3, 7.8, 4.6, "h")
        g.rect(4, cy - 3, 4, cy + 2, "h")
        g.rect(19, cy - 3, 19, cy + 2, "H")
    elif style == "curly":
        g.ellipse(cx, cy - 4, 7.8, 3.6, "h")
    elif style == "hat":
        g.rect(4, cy - 2, 5, cy + 4, "h")
        g.rect(18, cy - 2, 19, cy + 4, "h")
        g.rect(5, -1, 18, 4, "a")
        g.rect(5, 2, 18, 3, "c")
        g.rect(15, -1, 18, 4, "A")
        g.rect(0, 4, 23, 5, "a")
        g.hline(0, 23, 5, "A")
    ey = int(cy) + 2
    # eyes and brows by expression
    for ex in (7, 15):
        if expression == "happy":
            g.pixels([(ex, ey + 1), (ex + 1, ey), (ex + 2, ey + 1)], "e")
        else:
            g.rect(ex, ey, ex + 1, ey + 1, "e")
            g.set(ex, ey, "w")
    if expression == "sad":
        g.pixels([(6, ey - 1), (7, ey - 2), (17, ey - 2), (18, ey - 1)], "H" if style != "hat" else "e")
    if "glasses" in extras:
        for ex in (6, 14):
            g.rect(ex, ey - 1, ex + 3, ey - 1, "g")
            g.rect(ex, ey + 2, ex + 3, ey + 2, "g")
            g.rect(ex, ey - 1, ex, ey + 2, "g")
            g.rect(ex + 3, ey - 1, ex + 3, ey + 2, "g")
        g.hline(10, 13, ey, "g")
    if style != "beard":
        g.pixels([(5, ey + 3), (6, ey + 3), (17, ey + 3), (18, ey + 3)], "r")
    mouth_y = ey + 4 if style != "beard" else ey + 3
    if expression == "happy":
        g.pixels([(10, mouth_y), (11, mouth_y + 1), (12, mouth_y + 1), (13, mouth_y)], "m")
    elif expression == "sad":
        g.pixels([(10, mouth_y + 1), (11, mouth_y), (12, mouth_y), (13, mouth_y + 1)], "m")
    else:
        g.hline(11, 12, mouth_y, "m")
    if "freckles" in extras:
        g.pixels([(8, ey + 3), (9, ey + 4), (14, ey + 4), (15, ey + 3)], "K")
    if "flower" in extras:
        g.ellipse(18, 3, 2, 2, "f")
        g.set(18, 3, "y")
    if "earring" in extras:
        g.set(4, ey + 4, "a")
        g.set(19, ey + 4, "a")
    if "scarf" in extras:
        g.rect(7, 18, 16, 20, "c")
    if "brooch" in extras:
        g.rect(11, 21, 12, 22, "a")
    return g
