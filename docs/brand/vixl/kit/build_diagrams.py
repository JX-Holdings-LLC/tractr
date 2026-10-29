"""The explanatory figures in the docs, drawn as catalog pages in the Seed Catalog look.

    python build_diagrams.py            # every figure
    python build_diagrams.py workflow   # one figure

Writes out/NAME.png. Copy each to the folder in FIGURES (docs/media/...) and shrink it with
`python build_diagrams.py --install`, which quantizes the palette so the PNGs stay small: the guide's
pictures are embedded in the editor's in-app help.
"""
import sys
from vx import *

PAPER = "#f2e5c6"; CARD = "#f7eed6"; RED = "#b5332a"; INK = "#3a2a1e"; MUTED = "#6d5a40"; KRAFT = "#c9b48a"
SHADOW = "#6e1d17"; GREEN = "#4f6b3a"; MUSTARD = "#e3a83a"; SUN = "#f0c35e"; POND = "#7fa6a6"; SOIL = "#9b6b42"
SAND = "#e9dab6"
F = [("Alfa Slab One", 400), ("Oswald", 700), ("Courier Prime", 400), ("Courier Prime", 700)]
DISPLAY = "alfa-slab-one-400"; LABEL = "oswald-700"; BODY = "courier-prime-400"; BODY_BOLD = "courier-prime-700"

W = 1600

# Where each figure goes, relative to the repository's docs folder.
FIGURES = {
    "workflow": "media/guide", "edit-mode-tabs": "media/guide", "event-logic": "media/guide",
    "autotile": "media/guide", "music-priority": "media/guide", "export-targets": "media/guide",
    "playtest": "media/guide",
    "pack-anatomy": "media/plugins", "plugin-flow": "media/plugins", "minigame-canvas": "media/plugins",
    "architecture": "media/dev", "cartridge-flow": "media/dev",
}


def T(name, s, x, y, size, color=INK, font=BODY, **kw):
    op = {"type": "text", "name": name, "text": s, "x": x, "y": y, "size": size, "color": color, "font": font}
    op.update(kw)
    return op


def para(name, s, x, y, w, size=19, color=MUTED, font=BODY, **kw):
    """A wrapped paragraph in a box w wide."""
    return [T(name, s, x, y, size, color, font, line_height=1.25, **kw),
            {"type": "text-layout", "target": name, "width": w, "wrap": "pretty"}]


def box(name, x, y, w, h, fill=CARD, r=10, stroke=KRAFT, sw=2, shadow=KRAFT, off=6):
    ops = []
    if shadow:
        ops.append({"type": "shape", "shape": "rounded-rectangle", "name": name + "-shadow", "x": x + off,
                    "y": y + off, "width": w, "height": h, "fill": shadow, "radius": r})
    op = {"type": "shape", "shape": "rounded-rectangle", "name": name, "x": x, "y": y, "width": w, "height": h,
          "fill": fill, "radius": r}
    if stroke:
        op.update(stroke=stroke, stroke_width=sw)
    ops.append(op)
    return ops


def rect(name, x, y, w, h, fill, **kw):
    op = {"type": "shape", "shape": "rectangle", "name": name, "x": x, "y": y, "width": w, "height": h, "fill": fill}
    op.update(kw)
    return op


def seal(name, cx, cy, label, r=30, fill=RED, color=CARD, size=None):
    """A round number seal with a hard shadow."""
    size = size or int(r * 1.05)
    return [{"type": "shape", "shape": "ellipse", "name": name + "-shadow", "x": cx - r + 3, "y": cy - r + 3,
             "width": 2 * r, "height": 2 * r, "fill": SHADOW},
            {"type": "shape", "shape": "ellipse", "name": name, "x": cx - r, "y": cy - r, "width": 2 * r,
             "height": 2 * r, "fill": fill, "stroke": CARD, "stroke_width": 3},
            T(name + "-n", label, cx, cy - size * 0.62, size, color, DISPLAY, align="center")]


def arrow(name, pts, color=RED, sw=6, head=18):
    """A polyline arrow through pts, with a head at the last point."""
    d = "M " + " L ".join(f"{x:.1f} {y:.1f}" for x, y in pts)
    (x1, y1), (x2, y2) = pts[-2], pts[-1]
    import math
    a = math.atan2(y2 - y1, x2 - x1)
    tip = (x2, y2)
    back = (x2 - head * math.cos(a), y2 - head * math.sin(a))
    left = (back[0] + head * 0.62 * math.sin(a), back[1] - head * 0.62 * math.cos(a))
    right = (back[0] - head * 0.62 * math.sin(a), back[1] + head * 0.62 * math.cos(a))
    shaft = list(pts[:-1]) + [back]
    d = "M " + " L ".join(f"{x:.1f} {y:.1f}" for x, y in shaft)
    return [{"type": "shape", "shape": "path", "name": name, "x": 0, "y": 0, "path": d, "stroke": color,
             "stroke_width": sw},
            {"type": "shape", "shape": "path", "name": name + "-head", "x": 0, "y": 0,
             "path": f"M {tip[0]:.1f} {tip[1]:.1f} L {left[0]:.1f} {left[1]:.1f} L {right[0]:.1f} {right[1]:.1f} Z",
             "fill": color}]


def page(kicker, title, h, subtitle=None):
    """Paper, the double red frame of the catalog plates, a kicker and a title."""
    ops = [rect("paper", 0, 0, W, h, PAPER),
           rect("frame-out", 16, 16, W - 32, h - 32, "transparent", stroke=RED, stroke_width=8),
           rect("frame-in", 32, 32, W - 64, h - 64, "transparent", stroke=RED, stroke_width=2),
           T("kicker", kicker, 80, 66, 22, RED, LABEL, tracking=7),
           T("title", title, 80, 96, 46, INK, DISPLAY, tracking=2)]
    if subtitle:
        ops += para("subtitle", subtitle, 80, 160, W - 160, 21, MUTED)
    return ops


def card(name, x, y, w, h, n, heading, body, fill=CARD, body_size=19):
    ops = box(name, x, y, w, h, fill)
    ops += seal(name + "-seal", x + 46, y + 48, str(n), r=26)
    ops.append(T(name + "-h", heading, x + 86, y + 30, 22, INK, LABEL, tracking=2))
    ops += para(name + "-b", body, x + 24, y + 96, w - 48, body_size)
    return ops


def build(name, h, ops):
    """Make the figure. A text op with align center or right is anchored at its x: it gets a box around x."""
    out, laid = [], {o["target"] for o in ops if o.get("type") == "text-layout"}
    for o in ops:
        out.append(o)
        a = o.get("align")
        if o.get("type") == "text" and a in ("center", "right") and o["name"] not in laid:
            x = o["x"]
            bw = min(900, 2 * min(x, W - x)) if a == "center" else min(900, x)
            o["x"] = o["x"] - (bw / 2 if a == "center" else bw)
            out.append({"type": "text-layout", "target": o["name"], "width": bw})
    return make(name, W, h, PAPER, F, out)


# --- Creator guide ------------------------------------------------------------------------------------------


def workflow():
    h = 590
    ops = page("THE CREATOR'S YEAR", "FROM SEED TO SHIPPED GAME", h,
               "Every Tractr game grows the same way: build a little, play it, fix what Problems finds, and ship.")
    steps = [
        ("NEW PROJECT", "File → New Project. Start from Starter Farm, Cozy Garden, Quest RPG or Blank."),
        ("EDIT MODE", "F6. Paint maps, write people, quests and events, import art and sound."),
        ("PLAY MODE", "F5. Play it with the real game player. The world resets when you stop."),
        ("PROBLEMS", "The Problems tab lists what would break the game. Errors block export."),
        ("EXPORT GAME", "Windows, Linux and a web demo. Players need nothing else."),
    ]
    cw, gap, y, ch = 260, 44, 236, 236
    x0 = (W - (5 * cw + 4 * gap)) // 2
    for i, (head, body) in enumerate(steps):
        x = x0 + i * (cw + gap)
        ops += card(f"step{i}", x, y, cw, ch, i + 1, head, body, fill=SAND if i in (1, 2) else CARD)
        if i < 4:
            ops += arrow(f"to{i}", [(x + cw + 8, y + ch / 2), (x + cw + gap - 4, y + ch / 2)])
    # The loop: Play Mode and Problems send you back to Edit Mode.
    xl = x0 + 1 * (cw + gap) + cw / 2
    xr = x0 + 3 * (cw + gap) + cw / 2
    yb = y + ch + 54
    ops += arrow("loop", [(xr, y + ch + 12), (xr, yb), (xl, yb), (xl, y + ch + 16)], color=GREEN, sw=5)
    ops.append(T("loop-l", "REPEAT UNTIL IT'S FUN", (xl + xr) / 2, yb - 34, 20, GREEN, LABEL, tracking=4,
                 align="center"))
    return build("workflow", h, ops)


def edit_mode_tabs():
    h = 740
    ops = page("EDIT MODE · F6", "EIGHT TABS, ONE GAME", h,
               "Every part of a game has a tab. Undo and redo (Ctrl+Z, Ctrl+Y) work in all of them.")
    tabs = [
        ("MAP", "Paint terrain, place people and things, add doors between scenes."),
        ("CONTENT", "People, dialogue, items, crops, quests, events, shops and every other list."),
        ("PROBLEMS", "Checks the whole game. Go to jumps to whatever needs a fix."),
        ("SETTINGS", "Name, clock and calendar, farm, people, combat and export settings."),
        ("MODS", "Install, review, order and switch content packs and their plugins."),
        ("ART", "Import pictures and sounds, slice animation clips, assign artwork."),
        ("WORKSHOP", "Ready-made patterns: a romance, a festival, a fishing spot, in one step."),
        ("INTERFACE", "Panels on the game screen: counters, story status, action buttons."),
    ]
    cw, ch, gx, gy = 340, 196, 32, 36
    x0 = (W - (4 * cw + 3 * gx)) // 2
    y0 = 236
    for i, (head, body) in enumerate(tabs):
        x = x0 + (i % 4) * (cw + gx)
        y = y0 + (i // 4) * (ch + gy)
        ops += box(f"tab{i}", x, y, cw, ch)
        # A drawer label plate.
        ops += box(f"tab{i}-plate", x + 20, y + 22, cw - 40, 54, fill=RED, r=6, stroke=None, shadow=SHADOW, off=4)
        ops.append(T(f"tab{i}-h", head, x + cw / 2, y + 34, 28, CARD, LABEL, tracking=6, align="center"))
        ops += para(f"tab{i}-b", body, x + 24, y + 100, cw - 48, 19)
    return build("edit-mode-tabs", h, ops)


def event_logic():
    h = 760
    ops = page("CONTENT → EVENTS", "WHEN · IF · THEN", h,
               "An event runs its outcomes when its trigger happens and every condition holds.")
    cols = [
        ("WHEN", "THE TRIGGER", RED, ["A crop is harvested", "Each morning", "The player steps on a tile",
                                     "A quest changes", "A monster is slain"]),
        ("IF", "ALL CONDITIONS HOLD", GREEN, ["Season is spring", "Flag met-elder is set", "Hearts with Mae ≥ 4",
                                              "any / all / not groups", "chance 0.25"]),
        ("THEN", "THE OUTCOMES, IN ORDER", SOIL, ["Give 100 gold", "Set a flag or a variable",
                                                  "Start a cutscene", "Send a letter", "Warp the player"]),
    ]
    cw, gap, y, ch = 420, 70, 236, 370
    x0 = (W - (3 * cw + 2 * gap)) // 2
    for i, (word, sub, color, items) in enumerate(cols):
        x = x0 + i * (cw + gap)
        ops += box(f"col{i}", x, y, cw, ch)
        ops += box(f"col{i}-band", x, y, cw, 104, fill=color, r=10, stroke=None, shadow=None)
        ops.append(T(f"col{i}-w", word, x + 28, y + 18, 40, CARD, DISPLAY, tracking=2))
        ops.append(T(f"col{i}-s", sub, x + 30, y + 70, 17, CARD, LABEL, tracking=3))
        for j, item in enumerate(items):
            yy = y + 128 + j * 46
            ops.append({"type": "shape", "shape": "ellipse", "name": f"col{i}-d{j}", "x": x + 30, "y": yy + 7,
                        "width": 12, "height": 12, "fill": color})
            ops.append(T(f"col{i}-i{j}", item, x + 56, yy, 21, INK, BODY))
        if i < 2:
            ops += arrow(f"to{i}", [(x + cw + 10, y + ch / 2), (x + cw + gap - 6, y + ch / 2)])
    ops += para("foot", "An event that names a scene fires only while the player is there. Outcomes that cause "
                "events of their own (a harvest, a sale) run them too, four levels deep at most.", 80, 650, W - 160,
                19, MUTED)
    return build("event-logic", h, ops)


def autotile():
    h = 860
    ops = page("TERRAINS → AUTOTILE", "WHICH FRAME AN AUTOTILE DRAWS", h,
               "Each neighbour of the same terrain adds its number. The sum is the frame of the 16-frame "
               "autotile clip: 0 stands alone, 15 is surrounded.")
    # The rule: a cross of neighbours around a tile.
    s, cx, cy = 96, 300, 470
    cells = [(0, -1, "1", "ABOVE"), (1, 0, "2", "RIGHT"), (0, 1, "4", "BELOW"), (-1, 0, "8", "LEFT")]
    ops.append(rect("c-mid", cx - s / 2, cy - s / 2, s, s, GREEN, stroke=INK, stroke_width=3))
    ops.append(T("c-mid-t", "TILE", cx, cy - 13, 22, CARD, LABEL, tracking=3, align="center"))
    for dx, dy, n, lab in cells:
        x, y = cx + dx * s - s / 2, cy + dy * s - s / 2
        ops.append(rect(f"c-{lab}", x, y, s, s, CARD, stroke=KRAFT, stroke_width=3))
        ops.append(T(f"c-{lab}-n", n, x + s / 2, y + 14, 46, RED, DISPLAY, align="center"))
        ops.append(T(f"c-{lab}-l", lab, x + s / 2, y + s - 30, 16, MUTED, LABEL, tracking=3, align="center"))
    ops.append(T("ex", "EXAMPLE", cx, cy + 1.5 * s + 30, 18, RED, LABEL, tracking=5, align="center"))
    ops += para("ex-b", "The same terrain above and to the right: 1 + 2 = frame 3.", cx - 170, cy + 1.5 * s + 62, 340, 19, INK,
                align="center")
    # The 16 frames.
    gx0, gy0, t, g = 600, 270, 104, 24
    for n in range(16):
        x = gx0 + (n % 4) * (t + g)
        y = gy0 + (n // 4) * (t + g + 4)
        ops.append(rect(f"f{n}", x, y, t, t, CARD, stroke=KRAFT, stroke_width=2))
        c = 36
        ops.append(rect(f"f{n}-c", x + (t - c) / 2, y + (t - c) / 2, c, c, GREEN))
        arms = {1: (x + (t - c) / 2, y, c, (t - c) / 2), 2: (x + (t + c) / 2, y + (t - c) / 2, (t - c) / 2, c),
                4: (x + (t - c) / 2, y + (t + c) / 2, c, (t - c) / 2), 8: (x, y + (t - c) / 2, (t - c) / 2, c)}
        for bit, (ax, ay, aw, ah) in arms.items():
            if n & bit:
                ops.append(rect(f"f{n}-a{bit}", ax, ay, aw, ah, GREEN))
        ops.append(T(f"f{n}-n", str(n), x + 8, y + 4, 20, RED, DISPLAY))
    nx = 1160
    ops.append(T("note-h", "WITH ART", nx, 280, 20, RED, LABEL, tracking=5))
    ops += para("note-a", "Give the terrain's asset an animation clip named autotile with these 16 frames, in this "
                "order.", nx, 316, 340, 19, INK)
    ops.append(T("note-h2", "WITHOUT ART", nx, 470, 20, RED, LABEL, tracking=5))
    ops += para("note-b", "The game draws a border where the terrain meets another: a shore round a pond, an edge "
                "along a path.", nx, 506, 340, 19, INK)
    ops.append(T("note-h3", "MAP EDGES", nx, 660, 20, RED, LABEL, tracking=5))
    ops += para("note-c", "Off the map counts as the same terrain, so a field that reaches the edge has no seam.",
                nx, 696, 340, 19, INK)
    return build("autotile", h, ops)


def music_priority():
    h = 790
    ops = page("SOUND AND MUSIC", "WHICH MUSIC PLAYS", h,
               "The game checks these from the top every moment. The first one that has music wins.")
    rungs = [
        ("EVENT OR CUTSCENE", "playMusic, or a cutscene's music step, until the player changes scene."),
        ("TODAY'S FESTIVAL", "The festival's music, all day."),
        ("THE WEATHER", "Today's weather's music, outdoors only."),
        ("THE SCENE", "The scene's own music."),
        ("THE SEASON", "The season's music."),
        ("BUILT-IN", "Tractr's own day and night music. Birdsong, crickets and rain play under any of them."),
    ]
    y0, rh, x, w = 236, 86, 220, W - 440
    for i, (head, body) in enumerate(rungs):
        y = y0 + i * rh
        inset = i * 18
        fill = SAND if i < 5 else CARD
        ops += box(f"r{i}", x + inset, y, w - 2 * inset, rh - 14, fill=fill, r=8)
        ops += seal(f"r{i}-s", x + inset + 44, y + (rh - 14) / 2, str(i + 1), r=24, fill=RED if i < 5 else GREEN)
        ops.append(T(f"r{i}-h", head, x + inset + 90, y + 10, 22, INK, LABEL, tracking=2))
        ops.append(T(f"r{i}-b", body, x + inset + 90, y + 40, 19, MUTED, BODY))
    ops += arrow("down", [(150, 250), (150, 236 + 6 * rh - 30)], color=KRAFT, sw=8, head=26)
    ops.append(T("down-l", "FIRST MATCH WINS", 128, 520, 18, MUTED, LABEL, tracking=4, rotation=-90,
                 align="center"))
    return build("music-priority", h, ops)


def export_targets():
    h = 900
    ops = page("FILE → EXPORT GAME", "WHAT EXPORT GAME BUILDS", h,
               "Export checks the game, compiles it into one game.cart file and puts it beside a ready-made player "
               "for each target. Nothing is compiled on your machine.")
    # Project.
    ops += box("proj", 80, 290, 330, 420, fill=SAND)
    ops.append(T("proj-h", "YOUR PROJECT", 245, 314, 24, INK, LABEL, tracking=3, align="center"))
    for j, item in enumerate(["maps and scenes", "people and quests", "items, crops, shops", "events, cutscenes",
                              "art and sounds", "Settings → Export"]):
        ops.append(T(f"proj-i{j}", "· " + item, 106, 372 + j * 52, 19, MUTED, BODY))
    ops += arrow("a1", [(422, 500), (470, 500)])
    # Export.
    ops += box("exp", 482, 380, 290, 240, fill=RED, stroke=None, shadow=SHADOW)
    ops.append(T("exp-h", "EXPORT GAME", 627, 404, 26, CARD, LABEL, tracking=3, align="center"))
    ops += para("exp-b", "1. Run Problems: errors stop it.\n2. Compile game.cart.\n3. Copy a player, name it, "
                "give it your icon.", 506, 456, 250, 19, CARD)
    # Targets.
    targets = [
        ("WINDOWS X64", "WillowCreek/", "WillowCreek.exe, game.cart, licenses/", "and WillowCreek-windows-x64.zip"),
        ("LINUX X64 · STEAM DECK", "WillowCreek/", "WillowCreek, game.cart, WillowCreek.png, "
         "WillowCreek.desktop, licenses/", "and WillowCreek-linux-x64.tar.gz"),
        ("WEB DEMO", "WillowCreek-web.zip", "index.html, game.js, farm_wasm_bg.wasm, game.cart, icon.png, "
         "licenses/", "for an itch.io page"),
    ]
    tx, tw, th = 900, 620, 176
    for i, (head, folder, files, note) in enumerate(targets):
        y = 236 + i * (th + 22)
        ops += box(f"t{i}", tx, y, tw, th)
        ops.append(T(f"t{i}-h", head, tx + 24, y + 16, 22, RED, LABEL, tracking=3))
        ops.append(T(f"t{i}-f", folder, tx + 24, y + 54, 20, INK, BODY_BOLD))
        ops += para(f"t{i}-x", files, tx + 24, y + 84, tw - 48, 17, MUTED)
        ops.append(T(f"t{i}-n", note, tx + 24, y + th - 34, 17, GREEN, BODY))
        ops += arrow(f"a2-{i}", [(784, 500), (840, 500), (840, y + th / 2), (tx - 10, y + th / 2)], sw=5, head=16)
    ops += para("foot", "Players unzip and play. Saves, settings and screenshots go in their own user folder.",
                80, 830, W - 160, 19, MUTED)
    return build("export-targets", h, ops)


def playtest():
    h = 580
    ops = page("PLAY MODE · F5", "PLAYTESTS ARE SAFE", h,
               "Play Mode remembers the world when it starts. What happens next is yours to keep or throw away.")
    y, ch = 250, 236
    ops += box("start", 80, y, 300, ch, fill=SAND)
    ops += seal("start-s", 126, y + 48, "1", r=26)
    ops.append(T("start-h", "F5: PLAY", 166, y + 30, 24, INK, LABEL, tracking=2))
    ops += para("start-b", "Starts in the game, no title screen. Click the game to give it the keyboard.",
                104, y + 96, 252, 19)
    ops += arrow("a1", [(392, y + ch / 2), (470, y + ch / 2)])
    ops += box("play", 482, y, 420, ch)
    ops += seal("play-s", 528, y + 48, "2", r=26)
    ops.append(T("play-h", "PLAY AND TEST", 568, y + 30, 24, INK, LABEL, tracking=2))
    ops += para("play-b", "Restart starts over from that moment. Debug adds money, skips days, teleports and "
                "sets flags.", 506, y + 96, 372, 19)
    ops += arrow("a2", [(914, y + 60), (1000, y + 60)])
    ops += arrow("a3", [(914, y + ch - 60), (1000, y + ch - 60)])
    ops += box("back", 1012, y - 10, 508, 116, fill=CARD)
    ops.append(T("back-h", "F6: BACK TO EDIT MODE", 1036, y + 6, 22, GREEN, LABEL, tracking=2))
    ops += para("back-b", "The world is as it was before you played.", 1036, y + 44, 460, 19)
    ops += box("keep", 1012, y + ch - 106, 508, 116, fill=CARD)
    ops.append(T("keep-h", "KEEP CHANGES ON, THEN F6", 1036, y + ch - 90, 22, RED, LABEL, tracking=2))
    ops += para("keep-b", "The world, the player, flags and the date stay as you left them.", 1036, y + ch - 52,
                460, 19)
    return build("playtest", h, ops)


# --- Plugins and packs -----------------------------------------------------------------------------------


def pack_anatomy():
    h = 900
    ops = page("CONTENT PACKS", "WHAT'S IN A PACK", h,
               "A pack is one JSON file. The player reads its manifest before installing it; everything else "
               "joins the game under the pack's own name.")
    # The packet.
    px, py, pw, ph = 80, 250, 520, 600
    ops += box("packet", px, py, pw, ph, fill=RED, r=14, stroke=None, shadow=SHADOW, off=8)
    ops += box("packet-win", px + 30, py + 30, pw - 60, ph - 60, fill=CARD, r=10, stroke=None, shadow=None)
    ops.append(T("pk-k", "SEED No. 01", px + pw / 2, py + 56, 20, RED, LABEL, tracking=6, align="center"))
    ops.append(T("pk-t", "glow-farm.json", px + pw / 2, py + 90, 34, INK, DISPLAY, align="center"))
    code = ('{\n  "manifest": {\n    "id": "glow-farm",\n    "name": "Glow Farm",\n    "version": "1.0.0",\n'
            '    "engineCompatibility": "*",\n    "dependencies": [ … ],\n    "overrides": [ … ],\n'
            '    "permissions": { … }\n  },\n  "content": { … },\n  "assets": [ … ],\n  "plugins": [ … ]\n}')
    ops.append(T("pk-code", code, px + 54, py + 150, 20, INK, BODY, line_height=1.3))
    parts = [
        ("MANIFEST", RED, "Who the pack is, what it needs (an engine version, other packs) and what it asks for: the "
         "hooks its plugins hear, the changes they may make, minigames and game panels. The Mods tab shows it all."),
        ("CONTENT", GREEN, "Items, crops, scenes, NPCs, quests, cutscenes, buildings… Ids become glow-farm:seed. "
         "Terrains, tools and item kinds keep their ids and replace a built-in only when listed in overrides."),
        ("ASSETS", SOIL, "The pictures and sounds the content uses, embedded in the file. Export selection as pack "
         "adds them for you."),
        ("PLUGINS", INK, "Small JavaScript programs that answer the game's hooks with changes, or run a minigame. "
         "They run in a sandbox, inside the permissions above."),
    ]
    x, w = 680, 840
    for i, (head, color, body) in enumerate(parts):
        y = 250 + i * 152
        ops += box(f"p{i}", x, y, w, 134)
        ops.append(rect(f"p{i}-bar", x, y, 14, 134, color))
        ops.append(T(f"p{i}-h", head, x + 36, y + 14, 24, color, LABEL, tracking=4))
        ops += para(f"p{i}-b", body, x + 36, y + 50, w - 64, 18)
    return build("pack-anatomy", h, ops)


def plugin_flow():
    h = 900
    ops = page("PACK PLUGINS", "HOW A PLUGIN CHANGES A GAME", h,
               "A plugin never touches the game itself. It answers with a list of changes, and the game decides "
               "which of them happen.")
    steps = [
        ("THE GAME STEPS", "A morning comes, a crop is harvested, the player enters a scene.", CARD),
        ("A HOOK FIRES", "Your handler for onDayStart gets { day, season, year }.", CARD),
        ("YOUR HANDLER RUNS", "In its own sandbox, with a fuel budget. It returns mutations:\n"
         "[{ type: 'giveItem', itemId: 'seed' }]", SAND),
        ("PERMISSIONS ARE CHECKED", "Each mutation must be one the manifest declares. A refused one is dropped "
         "and reported; the rest still apply.", CARD),
        ("APPLIED BEFORE THE NEXT TICK", "As pluginMutation commands, in the order they arrived, into the "
         "command log like a player's own.", CARD),
    ]
    cw = 560
    pos = [(80, 250, 130), (80, 430, 130), (500, 610, 150), (960, 430, 130), (960, 250, 130)]
    for i, ((head, body, fill), (x, y, ch)) in enumerate(zip(steps, pos)):
        w = cw + 40 if i == 2 else cw
        ops += box(f"s{i}", x, y, w, ch, fill=fill)
        ops += seal(f"s{i}-s", x + 44, y + 44, str(i + 1), r=24)
        ops.append(T(f"s{i}-h", head, x + 84, y + 28, 22, INK, LABEL, tracking=2))
        ops += para(f"s{i}-b", body, x + 84, y + 64, w - 108, 17)
    ops += arrow("a1", [(360, 388), (360, 424)])
    ops += arrow("a2", [(360, 568), (360, 685), (492, 685)])
    ops += arrow("a3", [(1108, 685), (1240, 685), (1240, 568)])
    ops += arrow("a4", [(1240, 422), (1240, 388)])
    ops += arrow("a5", [(952, 315), (648, 315)], color=GREEN)
    ops.append(T("a5-l", "THE NEXT STEP", 800, 280, 18, GREEN, LABEL, tracking=4, align="center"))
    ops += para("foot", "Replays need no plugins: the command log already holds their answers. A plugin's "
                "JavaScript variables start over when a game loads; keep what must last with saveData.",
                80, 800, W - 160, 19, MUTED)
    return build("plugin-flow", h, ops)


def minigame_canvas():
    h = 960
    ops = page("PACK MINIGAMES", "THE MINIGAME CANVAS", h,
               "draw() answers a list of commands in canvas units: 400 × 240, origin top left, y down. The game "
               "scales it into the minigame window.")
    s = 2.2
    ox, oy = 130, 300
    cw, chh = 400 * s, 240 * s
    ops.append(rect("cv", ox, oy, cw, chh, "#1b2140", stroke=INK, stroke_width=4))
    for gx in range(50, 400, 50):
        ops.append(rect(f"gx{gx}", ox + gx * s, oy, 1, chh, "#2c3560"))
    for gy in range(50, 240, 50):
        ops.append(rect(f"gy{gy}", ox, oy + gy * s, cw, 1, "#2c3560"))
    ops.append(T("o-l", "(0, 0)", ox, oy - 34, 18, INK, BODY_BOLD))
    ops += arrow("ax", [(ox + 90, oy - 24), (ox + 230, oy - 24)], color=RED, sw=4, head=14)
    ops.append(T("ax-l", "x", ox + 240, oy - 44, 26, RED, DISPLAY))
    ops += arrow("ay", [(ox - 30, oy + 10), (ox - 30, oy + 150)], color=RED, sw=4, head=14)
    ops.append(T("ay-l", "y", ox - 42, oy + 158, 26, RED, DISPLAY))
    ops.append(T("e-l", "(400, 240)", ox + cw, oy + chh + 8, 18, INK, BODY_BOLD, align="right"))
    # A: text, with its baseline.
    tx, ty = 12, 26
    ops.append(T("txt", "Caught 3", ox + tx * s, oy + ty * s - 14 * s * 0.95, 14 * s, "#ffe9a8", BODY_BOLD))
    ops.append(rect("base", ox + tx * s - 6, oy + ty * s, 150, 2, RED))
    ops += seal("ta", ox + tx * s + 176, oy + ty * s - 10, "A", r=18, fill=RED)
    # B: a sprite.
    sx, sy, sw_, sh = 300, 70, 48, 48
    ops.append(rect("spr", ox + sx * s, oy + sy * s, sw_ * s, sh * s, "transparent", stroke=POND, stroke_width=3))
    ops.append({"type": "shape", "shape": "ellipse", "name": "lantern", "x": ox + (sx + 10) * s,
                "y": oy + (sy + 8) * s, "width": 28 * s, "height": 32 * s, "fill": SUN})
    ops += seal("tb", ox + (sx + sw_) * s + 26, oy + sy * s + 10, "B", r=18, fill=RED)
    # C: a rectangle.
    rx, ry, rw, rh = 160, 204, 80, 16
    ops.append({"type": "shape", "shape": "rounded-rectangle", "name": "paddle", "x": ox + rx * s, "y": oy + ry * s,
                "width": rw * s, "height": rh * s, "fill": MUSTARD, "radius": 5 * s})
    ops += seal("tc", ox + (rx + rw) * s + 30, oy + ry * s + 16, "C", r=18, fill=RED)
    # The commands.
    cmds = [
        ("A", "TEXT", "{ type: 'text', x: 12, y: 26, text: 'Caught 3', size: 14 }",
         "y is the baseline; align is left, center or right of x."),
        ("B", "SPRITE", "{ type: 'sprite', x: 300, y: 70, w: 48, h: 48, item: 'lantern' }",
         "An item's art, fitted into the box."),
        ("C", "RECT", "{ type: 'rect', x: 160, y: 204, w: 80, h: 16, radius: 5, color: 'accent' }",
         "With outline: n it draws a line n wide instead of filling."),
    ]
    cx, cwid = 1070, 450
    for i, (letter, head, code, note) in enumerate(cmds):
        y = 300 + i * 180
        ops += box(f"cmd{i}", cx, y, cwid, 160)
        ops += seal(f"cmd{i}-s", cx + 36, y + 36, letter, r=18, fill=RED)
        ops.append(T(f"cmd{i}-h", head, cx + 66, y + 22, 22, INK, LABEL, tracking=4))
        ops += para(f"cmd{i}-c", code, cx + 22, y + 62, cwid - 44, 16, INK, BODY_BOLD)
        ops += para(f"cmd{i}-n", note, cx + 22, y + 116, cwid - 44, 16, MUTED)
    ops += para("side", "Colours are #rgb, #rrggbb, #rrggbbaa, or a theme colour (text, muted, primary, accent, "
                "track, panel, background, error). input.pointer reports x and y in the same units.", 80, 868,
                W - 160, 18, MUTED)
    return build("minigame-canvas", h, ops)


# --- Contributors ------------------------------------------------------------------------------------------


def architecture():
    h = 1040
    ops = page("UNDER THE HOOD", "HOW TRACTR IS BUILT", h,
               "Rust runs the game. F# understands the project. C# is the desktop app.")
    bands = [
        ("C#", "TRACTR.APP · AVALONIA", "The desktop editor: windows, forms, file pickers, the project store, "
         "image import, the Update Center. It decides nothing about games or projects itself.", SAND,
         ["Tractr.App", "Tractr.Updates", "FarmEngine.Interop"]),
        ("F#", "FARMENGINE.AUTHORING", "Everything about the project: schema records, edits and undo, loading, "
         "Problems, packs, the content compiler and Export Game. farmc runs it from the command line.", CARD,
         ["FarmEngine.Authoring", "FarmEngine.Authoring.Net", "FarmEngine.Export", "FarmEngine.Cli"]),
        ("RUST", "THE ENGINE AND THE PLAYER", "Everything that runs while a game is played: the deterministic "
         "simulation, saves, the plugin sandbox, in-game UI, rendering, audio, and the player.", SAND,
         ["farm-sim", "farm-runtime", "farm-plugins", "farm-render", "farm-ui", "farm-gpu", "farm-player",
          "farm-cart", "farm-ffi", "farm-wasm"]),
    ]
    x, w, bh = 80, 1000, 200
    for i, (lang, head, body, fill, crates) in enumerate(bands):
        y = 236 + i * (bh + 60)
        ops += box(f"b{i}", x, y, w, bh, fill=fill)
        ops += box(f"b{i}-l", x + 20, y + 20, 130, bh - 40, fill=(RED, GREEN, SOIL)[i], r=8, stroke=None,
                   shadow=SHADOW, off=4)
        size = 48 if len(lang) < 3 else 34
        ops.append(T(f"b{i}-lt", lang, x + 85, y + bh / 2 - size * 0.62, size, CARD, DISPLAY, align="center"))
        ops.append(T(f"b{i}-h", head, x + 176, y + 24, 22, INK, LABEL, tracking=3))
        ops += para(f"b{i}-b", body, x + 176, y + 60, w - 210, 18)
        names = ", ".join(crates)
        ops += para(f"b{i}-c", names, x + 176, y + bh - 56, w - 210, 15, RED, BODY_BOLD)
    # Contracts between the bands.
    ops += arrow("c1", [(400, 442), (400, 490)], color=INK, sw=5, head=16)
    ops.append(T("c1-l", "every project change is an F# Edit", 420, 452, 17, INK, BODY))
    ops += arrow("c2", [(400, 702), (400, 750)], color=INK, sw=5, head=16)
    ops.append(T("c2-l", "game.cart: the compiled cartridge", 420, 712, 17, INK, BODY))
    ops += arrow("c3", [(1100, 336), (1140, 336), (1140, 856), (1100, 856)], color=INK, sw=5, head=16)
    ops.append(T("c3-l", "C ABI (farm-ffi):", 1160, 536, 18, INK, BODY_BOLD))
    ops += para("c3-b", "Play Mode, map previews, headless sessions", 1160, 564, 330, 17, MUTED)
    # Where the player runs.
    ops += box("runs", 1160, 236, 360, 196, fill=CARD)
    ops.append(T("runs-h", "ONE PLAYER, THREE PLACES", 1182, 254, 20, RED, LABEL, tracking=2))
    for j, item in enumerate(["the editor's Play Mode", "exported desktop games",
                              "the web demo (farm-wasm)"]):
        ops += para(f"runs-{j}", "· " + item, 1182, 296 + j * 52, 320, 18, INK)
    return build("architecture", h, ops)


def cartridge_flow():
    h = 560
    ops = page("BUILD PIPELINE", "FROM PROJECT FOLDER TO RUNNING GAME", h)
    nodes = [
        ("PROJECT FOLDER", "project.json, scenes/, npcs/, dialogue/, items/, assets/", SAND),
        ("F# LOAD + CHECK", "ProjectLoad, Problems.collect: errors stop here", CARD),
        ("F# COMPILE", "ContentCompiler, PackMerge, Builtin → CartridgeCompiler", CARD),
        ("GAME.CART", "FlatBuffers: GameInfo, content, start state, presentation, assets, plugins", SAND),
        ("RUST PLAYER", "farm-cart reads it, farm-sim runs it, farm-player draws it", CARD),
    ]
    cw, gap, y, ch = 262, 42, 200, 200
    x0 = (W - (5 * cw + 4 * gap)) // 2
    for i, (head, body, fill) in enumerate(nodes):
        x = x0 + i * (cw + gap)
        ops += box(f"n{i}", x, y, cw, ch, fill=fill)
        ops.append(T(f"n{i}-h", head, x + 22, y + 22, 21, RED if i in (0, 3) else INK, LABEL, tracking=2))
        ops += para(f"n{i}-b", body, x + 22, y + 66, cw - 44, 17)
        if i < 4:
            ops += arrow(f"a{i}", [(x + cw + 6, y + ch / 2), (x + cw + gap - 4, y + ch / 2)])
    ops += para("foot", "The same cartridge drives Play Mode (CartridgeCompiler.CompileForPlaytest), Export Game and "
                "farmc, so a playtest runs exactly what ships. Saves are Rust's alone (farm-cart::save).",
                80, 440, W - 160, 18, MUTED)
    return build("cartridge-flow", h, ops)


ALL = {"workflow": workflow, "edit-mode-tabs": edit_mode_tabs, "event-logic": event_logic, "autotile": autotile,
       "music-priority": music_priority, "export-targets": export_targets, "playtest": playtest,
       "pack-anatomy": pack_anatomy, "plugin-flow": plugin_flow, "minigame-canvas": minigame_canvas,
       "architecture": architecture, "cartridge-flow": cartridge_flow}


def install():
    """Quantize each built figure and copy it into docs/media."""
    from PIL import Image
    docs = os.path.normpath(os.path.join(D, "..", "..", ".."))
    for name, folder in FIGURES.items():
        src = os.path.join(OUT, name + ".png")
        if not os.path.exists(src):
            continue
        dst = os.path.join(docs, folder, name + ".png")
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        im = Image.open(src).convert("RGB")
        im.quantize(colors=64, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE).save(dst, optimize=True)
        print(dst, os.path.getsize(dst) // 1024, "KB")


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    args = sys.argv[1:]
    if args == ["--install"]:
        install()
    else:
        for n in args or ALL:
            ALL[n]()
