"""The Tractr editor screen (1600x1000) built from a theme."""
import math
from art import f, rect, ell, text, rivets

W, H = 1600, 1000


def dots(name, pts, r, color, **kw):
    d = []
    for cx, cy in pts:
        d.append(f"M {f(cx - r)} {f(cy)} A {r} {r} 0 1 0 {f(cx + r)} {f(cy)} A {r} {r} 0 1 0 {f(cx - r)} {f(cy)} Z")
    op = {"type": "shape", "shape": "path", "name": name, "x": 0, "y": 0, "path": " ".join(d), "fill": color}
    op.update(kw)
    return op


def lines(name, segs, color, w=1, **kw):
    d = " ".join(f"M {f(a)} {f(b)} L {f(c)} {f(e)}" for a, b, c, e in segs)
    op = {"type": "shape", "shape": "path", "name": name, "x": 0, "y": 0, "path": d, "stroke": color,
          "stroke_width": w}
    op.update(kw)
    return op


def gauge(p, cx, cy, r, frac, t, label, value):
    ops = []
    ops.append(ell(f"{p}-face", cx - r, cy - r, 2 * r, 2 * r, t["gauge_face"], stroke=t["line"], stroke_width=3))
    ops.append({"type": "shape", "shape": "arc", "name": f"{p}-track", "x": cx - r + 10, "y": cy - r + 10,
                "width": 2 * r - 20, "height": 2 * r - 20, "start_angle": 150, "end_angle": 390,
                "closed": False, "stroke": t["muted"], "stroke_width": 4, "line_cap": "butt"})
    ops.append({"type": "shape", "shape": "arc", "name": f"{p}-val", "x": cx - r + 10, "y": cy - r + 10,
                "width": 2 * r - 20, "height": 2 * r - 20, "start_angle": 150, "end_angle": 150 + 240 * frac,
                "closed": False, "stroke": t["accent"], "stroke_width": 6, "line_cap": "butt"})
    a = math.radians(150 + 240 * frac)
    ops.append(lines(f"{p}-needle", [(cx, cy, cx + math.cos(a) * (r - 18), cy + math.sin(a) * (r - 18))],
                     t["needle"], 4, line_cap="round"))
    ops.append(ell(f"{p}-pin", cx - 6, cy - 6, 12, 12, t["needle"]))
    ops.append(text(f"{p}-v", value, cx - r, cy + r * 0.28, int(r * 0.36), t["text"], t["mono"], align="center"))
    ops.append({"type": "text-layout", "target": f"{p}-v", "width": 2 * r})
    ops.append(text(f"{p}-l", label, cx - r, cy + r + 8, 13, t["muted"], t["body_b"], align="center", tracking=2))
    ops.append({"type": "text-layout", "target": f"{p}-l", "width": 2 * r})
    return ops


def panel(name, x, y, w, h, t, title=None):
    ops = [rect(name, x, y, w, h, t["panel"], r=t["radius"],
                **({"stroke": t["line"], "stroke_width": t["border"]} if t["border"] else {}))]
    if t.get("rivets"):
        ops += rivets(name, x, y, w, h, 120, t["rivet"], r=4, inset=10)
    if title:
        ops.append(rect(f"{name}-head", x + t["border"], y + t["border"], w - 2 * t["border"], 34, t["panel2"],
                        r=max(0, t["radius"] - 2)))
        ops.append(text(f"{name}-title", title, x + 18, y + 9, 15, t["head_text"], t["body_b"], tracking=3,
                        text_transform="uppercase"))
    return ops


def build(t, center="map"):
    o = []
    o.append(rect("chrome", 0, 0, W, H, t["bg"]))
    if t.get("bg_texture"):
        o.append({"type": "look", "target": "chrome", "look": t["bg_texture"]})
    # top bar
    o.append(rect("topbar", 0, 0, W, 60, t["bar"]))
    o += t["logo"](16, 10)
    mx = 240
    for m in ["File", "Edit", "Field", "Crops", "Critters", "Build"]:
        o.append(text(f"menu-{m}", m, mx, 21, 16, t["bar_text"], t["body"],
                      text_transform="uppercase" if t["upper"] else "none", tracking=1 if t["upper"] else 0))
        mx += len(m) * t.get("menu_char", 9) + 38
    # transport
    bx = 790
    o.append(rect("btn-play", bx, 12, 60, 36, t["accent"], r=t["radius"]))
    o.append({"type": "shape", "shape": "play", "name": "btn-play-ico", "x": bx + 21, "y": 20, "width": 20,
              "height": 20, "fill": t["on_accent"]})
    o.append(rect("btn-pause", bx + 68, 12, 44, 36, t["panel2"], r=t["radius"]))
    o.append({"type": "shape", "shape": "pause", "name": "btn-pause-ico", "x": bx + 80, "y": 21, "width": 20,
              "height": 18, "fill": t["text"]})
    o.append(rect("btn-step", bx + 120, 12, 44, 36, t["panel2"], r=t["radius"]))
    o.append({"type": "shape", "shape": "skip", "name": "btn-step-ico", "x": bx + 132, "y": 21, "width": 20,
              "height": 18, "fill": t["text"]})
    o.append(text("daylabel", "SPRING · DAY 14 · 06:40", 972, 21, 15, t["bar_text"], t["mono"], tracking=1))
    o.append(rect("btn-build", 1430, 12, 154, 36, t["accent2"], r=t["radius"]))
    o.append(text("btn-build-t", "HARVEST BUILD", 1430, 21, 14, t["on_accent2"], t["body_b"], tracking=2))
    o.append({"type": "text-layout", "target": "btn-build-t", "width": 154})
    o.append({"type": "text-set", "target": "btn-build-t", "align": "center"})

    # left: barn (scene tree)
    o += panel("barn", 12, 72, 290, 916, t, "Barn · Scene")
    rows = [("v", "Homestead", 0, "folder"), ("", "Farmhouse", 1, "house"), ("", "Red Barn", 1, "house"),
            ("v", "Fields", 0, "folder"), ("", "Field_North  24×16", 1, "grid"),
            ("", "Field_South  18×12", 1, "grid"), ("", "Orchard", 1, "grid"),
            ("v", "Machines", 0, "folder"), ("", "Tractor_01", 1, "gear"), ("", "Combine_02", 1, "gear"),
            ("", "Seed Drill", 1, "gear"), ("v", "Critters", 0, "folder"), ("", "Hen ×12", 1, "dot"),
            ("", "Dairy Cow ×3", 1, "dot"), ("", "Barn Cat", 1, "dot"), ("v", "Weather", 0, "folder"),
            ("", "Rain Cycle", 1, "dot"), ("", "Sun Path", 1, "dot")]
    y = 120
    for i, (arrow, label, depth, ico) in enumerate(rows):
        x = 30 + depth * 22
        if label.startswith("Field_North"):
            o.append(rect("barn-sel", 20, y - 6, 274, 32, t["select"], r=t["radius"]))
        col = t["text"] if depth else t["head_text"]
        if ico == "folder":
            o.append({"type": "shape", "shape": "chevron", "name": f"row{i}-ico", "x": x, "y": y + 4, "width": 10,
                      "height": 12, "fill": t["muted"], "rotation": 90})
        elif ico == "gear":
            o.append({"type": "shape", "shape": "cog", "name": f"row{i}-ico", "x": x, "y": y + 2, "width": 16,
                      "height": 16, "fill": t["accent"], "teeth": 8, "hole": 0.3})
        elif ico == "house":
            o.append({"type": "shape", "shape": "house", "name": f"row{i}-ico", "x": x, "y": y + 2, "width": 16,
                      "height": 16, "fill": t["barn"]})
        elif ico == "grid":
            o.append(rect(f"row{i}-ico", x, y + 3, 15, 15, t["crop"], r=2))
        else:
            o.append(ell(f"row{i}-ico", x + 3, y + 5, 10, 10, t["muted"]))
        o.append(text(f"row{i}", label, x + 24, y, 15, col, t["body_b"] if not depth else t["body"]))
        y += 38 if not (i + 1 < len(rows) and rows[i + 1][2] == 0) else 46

    # viewport
    vx, vy, vw, vh = 314, 72, 920, 680
    o += panel("view", vx, vy, vw, vh, t, None)
    o.append(rect("view-bar", vx + t["border"], vy + t["border"], vw - 2 * t["border"], 40, t["panel2"],
                  r=max(0, t["radius"] - 2)))
    tx = vx + 16
    for i, tl in enumerate(["Till", "Plant", "Water", "Fence", "Path", "Place"]):
        on = tl == "Plant"
        o.append(rect(f"tool{i}", tx, vy + 7, 78, 28, t["accent"] if on else t["panel"], r=t["radius"]))
        o.append(text(f"tool{i}-t", tl, tx, vy + 13, 14, t["on_accent"] if on else t["text"], t["body_b"],
                      tracking=1))
        o.append({"type": "text-layout", "target": f"tool{i}-t", "width": 78})
        o.append({"type": "text-set", "target": f"tool{i}-t", "align": "center"})
        tx += 86
    o.append(text("view-zoom", "Top view · 100% · grid 32", vx + vw - 230, vy + 14, 13, t["muted"], t["mono"]))
    mx0, my0, mw, mh = vx + 10, vy + 52, vw - 20, vh - 62
    if center == "map":
        o += farm_map(mx0, my0, mw, mh, t)
    else:
        o += node_graph(mx0, my0, mw, mh, t)

    # right: inspector
    ix, iy, iw = 1246, 72, 342
    o += panel("insp", ix, iy, iw, 916, t, "Inspector")
    o.append(text("insp-kind", "CROP", ix + 20, iy + 54, 12, t["accent"], t["body_b"], tracking=3))
    o.append(text("insp-name", "Sweet Corn", ix + 20, iy + 72, 30, t["text"], t["head"]))
    o.append(text("insp-id", "crops/corn.tcrop", ix + 20, iy + 112, 13, t["muted"], t["mono"]))
    # growth stages
    o.append(text("stages-l", "GROWTH STAGES", ix + 20, iy + 146, 12, t["muted"], t["body_b"], tracking=2))
    for k in range(5):
        sx = ix + 20 + k * 62
        o.append(rect(f"stage{k}", sx, iy + 166, 54, 64, t["well"], r=t["radius"],
                      **({"stroke": t["accent"], "stroke_width": 2} if k == 3 else {})))
        o.append(rect(f"stage{k}-soil", sx + 6, iy + 214, 42, 8, t["soil"], r=2))
        hgt = 8 + k * 9
        o.append({"type": "shape", "shape": "path", "name": f"stage{k}-sprout", "x": 0, "y": 0,
                  "path": f"M {sx + 27} {iy + 214} L {sx + 27} {iy + 214 - hgt}", "stroke": t["crop"],
                  "stroke_width": 3, "line_cap": "round"})
        if k:
            lw = 6 + k * 3
            o.append({"type": "shape", "shape": "lens", "name": f"stage{k}-leaf", "x": sx + 27 - lw,
                      "y": iy + 214 - hgt * 0.6 - 4, "width": lw * 2, "height": 8, "fill": t["crop"]})
        if k >= 3:
            o.append({"type": "shape", "shape": "capsule", "name": f"stage{k}-cob", "x": sx + 23,
                      "y": iy + 214 - hgt - 10, "width": 8, "height": 18, "fill": t["grain"]})
        o.append(text(f"stage{k}-d", f"D{[0, 3, 6, 9, 12][k]}", sx, iy + 234, 11, t["muted"], t["mono"],
                      align="center"))
        o.append({"type": "text-layout", "target": f"stage{k}-d", "width": 54})
    # fields
    fy = iy + 270
    for i, (lab, val, frac) in enumerate([("Days to harvest", "12", 0.5), ("Water need", "Medium", 0.55),
                                          ("Yield / tile", "3–5 ears", 0.7), ("Sell price", "$45", 0.38)]):
        o.append(text(f"f{i}-l", lab, ix + 20, fy, 14, t["text"], t["body"]))
        o.append(text(f"f{i}-v", val, ix + iw - 120, fy, 14, t["text"], t["mono"], align="right"))
        o.append({"type": "text-layout", "target": f"f{i}-v", "width": 100})
        o.append(rect(f"f{i}-track", ix + 20, fy + 26, iw - 40, 8, t["well"], r=4 if t["radius"] else 0))
        o.append(rect(f"f{i}-fill", ix + 20, fy + 26, (iw - 40) * frac, 8, t["accent"], r=4 if t["radius"] else 0))
        o.append(ell(f"f{i}-knob", ix + 20 + (iw - 40) * frac - 9, fy + 21, 18, 18, t["knob"],
                     stroke=t["accent"], stroke_width=3))
        fy += 62
    o.append(text("seasons-l", "SEASONS", ix + 20, fy + 4, 12, t["muted"], t["body_b"], tracking=2))
    for k, (s, on) in enumerate([("SPR", True), ("SUM", True), ("FAL", False), ("WIN", False)]):
        sx = ix + 20 + k * 78
        o.append(rect(f"sea{k}", sx, fy + 26, 70, 32, t["accent2"] if on else t["well"], r=t["radius"]))
        o.append(text(f"sea{k}-t", s, sx, fy + 33, 14, t["on_accent2"] if on else t["muted"], t["body_b"],
                      align="center", tracking=2))
        o.append({"type": "text-layout", "target": f"sea{k}-t", "width": 70})
    fy += 84
    o.append(text("script-l", "ON HARVEST", ix + 20, fy, 12, t["muted"], t["body_b"], tracking=2))
    o.append(rect("script", ix + 20, fy + 22, iw - 40, 112, t["code_bg"], r=t["radius"]))
    code = [("barn.store(", "corn, ", "yield)"), ("if ", "farmer.mood", " > 3:"), ("  ", "play(", "\"cluck\")"),
            ("soil.", "fertility ", "-= 1")]
    for i, (a, b, c) in enumerate(code):
        o.append(text(f"code{i}", a + b + c, ix + 34, fy + 34 + i * 24, 14, t["code_text"], t["mono"]))
    fy += 150
    o += gauge("g-fps", ix + 80, fy + 70, 58, 0.78, t, "FRAMES / SEC", "60")
    o += gauge("g-tick", ix + iw - 80, fy + 70, 58, 0.45, t, "SIM TICKS", "20")

    # bottom: season timeline
    bx, by, bw, bh = 314, 764, 920, 224
    o += panel("time", bx, by, bw, bh, t, "Season Clock")
    sx, sy, sw = bx + 20, by + 64, bw - 40
    cols = [t["spring"], t["summer"], t["fall"], t["winter"]]
    for k, nm in enumerate(["Spring", "Summer", "Fall", "Winter"]):
        o.append(rect(f"season{k}", sx + k * sw / 4, sy, sw / 4 - 4, 26, cols[k], r=t["radius"] // 2))
        o.append(text(f"season{k}-t", nm.upper(), sx + k * sw / 4 + 10, sy + 6, 12, t["season_text"], t["body_b"],
                      tracking=2))
    ticks = [(sx + i * sw / 112, sy + 34, sx + i * sw / 112, sy + 34 + (12 if i % 7 == 0 else 6)) for i in
             range(113)]
    o.append(lines("ticks", ticks, t["muted"], 1.5))
    # event tracks
    tracks = [("Rain", [(0.05, 0.12), (0.30, 0.36), (0.62, 0.70)], t["water"]),
              ("Market", [(0.10, 0.13), (0.35, 0.38), (0.60, 0.63), (0.85, 0.88)], t["accent2"]),
              ("Festival", [(0.22, 0.27), (0.72, 0.78)], t["accent"])]
    for k, (nm, spans, col) in enumerate(tracks):
        ty = sy + 62 + k * 30
        o.append(text(f"trk{k}", nm, sx, ty + 2, 13, t["muted"], t["body"]))
        for j, (a, b) in enumerate(spans):
            o.append(rect(f"trk{k}-{j}", sx + 80 + (sw - 80) * a, ty, (sw - 80) * (b - a), 20, col,
                          r=t["radius"] // 2))
    ph = sx + 80 + (sw - 80) * 0.115
    o.append(lines("playhead", [(ph, sy - 6, ph, sy + 150)], t["needle"], 2))
    o.append({"type": "shape", "shape": "triangle", "name": "playhead-cap", "x": ph - 8, "y": sy - 14,
              "width": 16, "height": 12, "fill": t["needle"], "rotation": 180})
    return o


def farm_map(x, y, w, h, t):
    o = [rect("map-grass", x, y, w, h, t["grass"], r=max(0, t["radius"] - 4))]
    if t.get("map_texture"):
        o.append({"type": "look", "target": "map-grass", "look": t["map_texture"]})
    # dirt road
    o.append(rect("map-road", x, y + 300, w, 44, t["road"]))
    o.append(rect("map-road2", x + 560, y, 44, 300, t["road"]))
    # fields
    plots = [("north", x + 40, y + 40, 480, 224, t["crop"]), ("south", x + 40, y + 380, 300, 220, t["crop2"]),
             ("east", x + 640, y + 380, 220, 220, t["grain"])]
    for nm, px, py, pw, ph, col in plots:
        o.append(rect(f"plot-{nm}", px, py, pw, ph, t["soil"], r=max(0, t["radius"] - 6)))
        rowsegs = [(px + 10, py + 16 + i * 32, px + pw - 10, py + 16 + i * 32) for i in range(int(ph // 32))]
        o.append(lines(f"plot-{nm}-furrows", rowsegs, t["soil_dark"], 6, line_cap="round"))
        pts = [(px + 22 + i * 32, py + 16 + j * 32) for i in range(int((pw - 20) // 32)) for j in
               range(int(ph // 32))]
        if nm == "north":
            pts = [p for p in pts if not (p[0] > px + 330 and p[1] > py + 120)]
        o.append(dots(f"plot-{nm}-crops", pts, 10 if nm != "east" else 8, col))
    # selection
    o.append(rect("sel", x + 34, y + 34, 492, 236, "transparent", stroke=t["select_line"], stroke_width=3,
                  dash=[10, 6], stroke_align="center"))
    o.append(rect("sel-tag", x + 34, y + 8, 150, 24, t["select_line"], r=max(0, t["radius"] - 4)))
    o.append(text("sel-tag-t", "Field_North", x + 44, y + 12, 13, t["on_select"], t["mono"]))
    # barn and house
    o.append(rect("barn-roof", x + 660, y + 60, 150, 110, t["barn"], r=max(0, t["radius"] - 6)))
    o.append(lines("barn-ridge", [(x + 660, y + 115, x + 810, y + 115)], t["barn_dark"], 6))
    o.append(rect("silo", x + 830, y + 70, 56, 56, t["metal"], r=28))
    o.append(ell("silo-cap", x + 842, y + 82, 32, 32, t["metal_dark"]))
    o.append(rect("house", x + 660, y + 200, 110, 80, t["house"], r=max(0, t["radius"] - 6)))
    o.append(lines("house-ridge", [(x + 715, y + 200, x + 715, y + 280)], t["barn_dark"], 4))
    # pond
    o.append({"type": "shape", "shape": "ellipse", "name": "pond", "x": x + 370, "y": y + 420, "width": 190,
              "height": 120, "fill": t["water"], "rotation": -12, "stroke": t.get("water_edge", t["water"]), "stroke_width": 6})
    # trees
    tp = [(x + 880, y + 200), (x + 860, y + 260), (x + 600, y + 420), (x + 590, y + 520), (x + 880, y + 330)]
    o.append(dots("trees-shadow", [(a + 4, b + 5) for a, b in tp], 22, t["tree_dark"]))
    o.append(dots("trees", tp, 22, t["tree"]))
    # tractor top-down
    tx, ty = x + 395, y + 205
    o.append(rect("tt-rw1", tx - 6, ty - 4, 26, 12, t["tire"], r=3))
    o.append(rect("tt-rw2", tx - 6, ty + 36, 26, 12, t["tire"], r=3))
    o.append(rect("tt-fw1", tx + 50, ty + 2, 16, 8, t["tire"], r=2))
    o.append(rect("tt-fw2", tx + 50, ty + 34, 16, 8, t["tire"], r=2))
    o.append(rect("tt-body", tx, ty + 8, 72, 28, t["tractor"], r=6))
    o.append(ell("tt-seat", tx + 6, ty + 14, 16, 16, t["tire"]))
    # grid
    gl = [(x + i * 32, y, x + i * 32, y + h) for i in range(1, int(w // 32) + 1)]
    gl += [(x, y + j * 32, x + w, y + j * 32) for j in range(1, int(h // 32) + 1)]
    o.append(lines("grid", gl, t["grid"], 1, opacity=t.get("grid_opacity", 0.18)))
    # cursor tool hint
    o.append(rect("hint", x + w - 236, y + h - 46, 224, 34, t["panel"], r=t["radius"], opacity=0.92))
    o.append(text("hint-t", "Plant · Sweet Corn · 3×3", x + w - 224, y + h - 37, 14, t["text"], t["body"]))
    return o


def node_graph(x, y, w, h, t):
    o = [rect("graph-bg", x, y, w, h, t["grass"])]
    gl = [(x + i * 24, y, x + i * 24, y + h) for i in range(1, int(w // 24) + 1)]
    gl += [(x, y + j * 24, x + w, y + j * 24) for j in range(1, int(h // 24) + 1)]
    o.append(lines("graph-grid", gl, t["grid"], 1, opacity=t.get("grid_opacity", 0.18)))
    gl2 = [(x + i * 120, y, x + i * 120, y + h) for i in range(1, int(w // 120) + 1)]
    gl2 += [(x, y + j * 120, x + w, y + j * 120) for j in range(1, int(h // 120) + 1)]
    o.append(lines("graph-grid2", gl2, t["grid"], 1.5, opacity=min(1, t.get("grid_opacity", 0.18) * 2)))
    cx = [x + 16 + i * 180 for i in range(5)]
    nodes = [("seed", "SEED", "planted", cx[0], y + 70, t["crop2"]),
             ("weather", "RAIN EVENT", "season.rain", cx[0], y + 370, t["water"]),
             ("sprout", "SPROUT", "day >= 3", cx[1], y + 70, t["crop"]),
             ("grow", "GROWING", "water > 0.4", cx[2], y + 220, t["crop"]),
             ("ripe", "RIPE", "day >= 12", cx[3], y + 70, t["grain"]),
             ("wilt", "WILTED", "dry 2 days", cx[3], y + 400, t["fall"]),
             ("harvest", "HARVEST", "barn.store()", cx[4], y + 70, t["accent"]),
             ("compost", "COMPOST", "soil += 1", cx[4], y + 400, t["muted"])]
    pos = {}
    for k, title, sub, nx, ny, col in nodes:
        nw, nh = 150, 100
        pos[k] = (nx, ny, nw, nh)
        o.append(rect(f"n-{k}", nx, ny, nw, nh, t["panel"], r=t["radius"], stroke=t["line"], stroke_width=2))
        o.append(rect(f"n-{k}-h", nx, ny, nw, 30, col, r=t["radius"]))
        o.append(text(f"n-{k}-t", title, nx + 12, ny + 7, 14, t["node_head_text"], t["body_b"], tracking=2))
        o.append(text(f"n-{k}-s", sub, nx + 12, ny + 46, 14, t["text"], t["mono"]))
        o.append(ell(f"n-{k}-in", nx - 7, ny + 58, 14, 14, t["panel2"], stroke=t["line"], stroke_width=2))
        o.append(ell(f"n-{k}-out", nx + nw - 7, ny + 58, 14, 14, t["accent"], stroke=t["line"], stroke_width=2))
    edges = [("seed", "sprout"), ("sprout", "grow"), ("weather", "grow"), ("grow", "ripe"), ("grow", "wilt"),
             ("ripe", "harvest"), ("wilt", "compost")]
    d = []
    for a, b in edges:
        ax, ay, aw, ah = pos[a]
        bx, by, bw, bh = pos[b]
        x1, y1 = ax + aw, ay + 65
        x2, y2 = bx, by + 65
        dx = max(20, abs(x2 - x1) * 0.5)
        d.append(f"M {x1} {y1} C {x1 + dx} {y1} {x2 - dx} {y2} {x2} {y2}")
    o.append({"type": "shape", "shape": "path", "name": "edges", "x": 0, "y": 0, "path": " ".join(d),
              "stroke": t["edge"], "stroke_width": 3})
    o.append({"type": "reorder", "target": "edges", "above": "graph-grid2"})
    o.append(text("graph-title", "CROP LIFECYCLE · corn.tcrop", x + 16, y + h - 36, 14, t["muted"], t["mono"]))
    return o
