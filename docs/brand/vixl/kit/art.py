"""Reusable Tractr motifs, emitted as Vixl operations."""
import math


def f(v):
    return f"{v:.1f}"


def tractor(p, x0, y0, s, c, outline=None, lw=3, bg="transparent"):
    """Vintage open-seat tractor facing right in a 600x420 unit box at (x0, y0) scaled by s.

    c: body, dark, tire, rim, hub, light. outline: stroke-only line-art color (blueprint)."""
    X = lambda u: x0 + u * s
    Y = lambda v: y0 + v * s
    ops = []

    def shp(name, shape, ux, uy, uw, uh, fill, **kw):
        op = {"type": "shape", "shape": shape, "name": f"{p}-{name}", "x": X(ux), "y": Y(uy),
              "width": uw * s, "height": uh * s}
        if outline:
            op["fill"] = bg if fill != "transparent" else "transparent"
            op["stroke"] = outline
            op["stroke_width"] = lw
        else:
            op["fill"] = fill
        op.update(kw)
        ops.append(op)

    def path(name, d, fill=None, stroke=None, sw=None, **kw):
        op = {"type": "shape", "shape": "path", "name": f"{p}-{name}", "x": 0, "y": 0, "path": d}
        if outline:
            op["fill"] = bg if fill else None
            op["stroke"] = outline
            op["stroke_width"] = lw if not sw else max(lw, sw * 0.4)
            if op["fill"] is None:
                del op["fill"]
        else:
            if fill:
                op["fill"] = fill
            if stroke:
                op["stroke"] = stroke
                op["stroke_width"] = sw * s if sw else 4
        op.update(kw)
        ops.append(op)

    def P(*pts):
        out = []
        for i, pt in enumerate(pts):
            if isinstance(pt, str):
                out.append(pt)
            else:
                out.append(f"{f(X(pt[0]))} {f(Y(pt[1]))}")
        return " ".join(out)

    # fender behind rear wheel
    shp("fender", "arc", 0, 100, 340, 340, c["body"], start_angle=180, end_angle=360, inner_radius=0.86)
    # transmission / frame
    shp("frame", "rounded-rectangle", 160, 215, 170, 60, c["body"], radius=10 * s)
    # engine block with fins
    shp("engine", "rounded-rectangle", 300, 245, 225, 60, c["dark"], radius=6 * s)
    if not outline:
        for i in range(6):
            shp(f"fin{i}", "rectangle", 320 + i * 32, 255, 10, 40, c["hub"])
    # hood
    path("hood", "M " + P((250, 255), "L", (250, 175), "Q", (250, 160), (268, 160), "L", (530, 160),
                          "Q", (562, 160), (565, 195), "L", (565, 255), "Z"), fill=c["body"])
    # hood stripe
    if not outline:
        shp("stripe", "rectangle", 262, 188, 290, 12, c["light"])
    # grill
    shp("grill", "rounded-rectangle", 552, 158, 30, 108, c["dark"], radius=6 * s)
    if not outline:
        for i in range(5):
            shp(f"grillbar{i}", "rectangle", 557, 170 + i * 18, 20, 5, c["hub"])
    # exhaust stack and intake
    shp("stack", "rectangle", 452, 78, 16, 86, c["dark"])
    shp("cap", "rounded-rectangle", 446, 68, 28, 14, c["dark"], radius=3 * s)
    shp("intake", "rounded-rectangle", 395, 118, 18, 46, c["dark"], radius=4 * s)
    # seat
    shp("seatpost", "rectangle", 152, 160, 14, 60, c["dark"])
    shp("seat", "rounded-rectangle", 115, 148, 90, 18, c["dark"], radius=8 * s)
    shp("seatback", "rounded-rectangle", 108, 106, 18, 56, c["dark"], radius=8 * s)
    # steering
    path("column", "M " + P((292, 165), "L", (246, 104)), stroke=c["dark"], sw=9, line_cap="round")
    shp("wheel", "ellipse", 212, 94, 70, 20, "transparent", stroke=outline or c["dark"],
        stroke_width=(lw if outline else 7 * s), rotation=-18)
    # rear tire
    shp("rtire", "cog", 25, 125, 290, 290, c["tire"], teeth=26, depth=0.05, hole=0)
    shp("rrim", "ellipse", 82, 182, 176, 176, c["rim"])
    shp("rring", "ring", 102, 202, 136, 136, c["hub"], thickness=0.12)
    shp("rhub", "ellipse", 140, 240, 60, 60, c["body"])
    shp("rbolt", "ellipse", 160, 260, 20, 20, c["dark"])
    # front tire
    shp("ftire", "cog", 418, 268, 148, 148, c["tire"], teeth=16, depth=0.07, hole=0)
    shp("frim", "ellipse", 450, 300, 84, 84, c["rim"])
    shp("fhub", "ellipse", 476, 326, 32, 32, c["body"])
    # headlight
    shp("lamp", "ellipse", 520, 132, 26, 26, c.get("lamp", c["light"]))
    return ops


def wheat(p, x, y, h, color, lean=0.0, grains=7, stem=None, sw=3):
    """A wheat stalk rising from (x, y) to height h; lean bends the tip sideways (fraction of h)."""
    tipx, tipy = x + lean * h, y - h
    cx, cy = x + lean * h * 0.2, y - h * 0.55
    ops = [{"type": "shape", "shape": "path", "name": f"{p}-stem", "x": 0, "y": 0,
            "path": f"M {f(x)} {f(y)} Q {f(cx)} {f(cy)} {f(tipx)} {f(tipy)}",
            "stroke": stem or color, "stroke_width": sw, "line_cap": "round"}]

    def bez(t):
        return ((1 - t) ** 2 * x + 2 * (1 - t) * t * cx + t * t * tipx,
                (1 - t) ** 2 * y + 2 * (1 - t) * t * cy + t * t * tipy)

    L, W = h * 0.085, h * 0.016
    d = []
    for i in range(grains):
        t = 0.6 + 0.37 * i / max(1, grains - 1)
        px, py = bez(t)
        qx, qy = bez(min(1, t + 0.02))
        ang = math.atan2(qy - py, qx - px)
        for side in (-1, 1):
            if i == grains - 1 and side == 1:
                a = ang
            else:
                a = ang + side * 0.6
            dx, dy = math.cos(a), math.sin(a)
            nx, ny = -dy, dx
            gx, gy = px + dx * L * 0.5, py + dy * L * 0.5
            t1 = (gx + dx * L / 2, gy + dy * L / 2)
            t2 = (gx - dx * L / 2, gy - dy * L / 2)
            d.append(f"M {f(t2[0])} {f(t2[1])} Q {f(gx + nx * W * 2)} {f(gy + ny * W * 2)} {f(t1[0])} {f(t1[1])} "
                     f"Q {f(gx - nx * W * 2)} {f(gy - ny * W * 2)} {f(t2[0])} {f(t2[1])} Z")
            if i == grains - 1:
                break
    ops.append({"type": "shape", "shape": "path", "name": f"{p}-grain", "x": 0, "y": 0, "path": " ".join(d),
                "fill": color})
    return ops


def rivets(p, x, y, w, h, step, color, r=6, inset=14):
    """Rivet dots around a rectangle's edge as one path layer."""
    d = []

    def dot(cx, cy):
        d.append(f"M {f(cx - r)} {f(cy)} A {r} {r} 0 1 0 {f(cx + r)} {f(cy)} A {r} {r} 0 1 0 {f(cx - r)} {f(cy)} Z")

    nx = max(1, int((w - 2 * inset) // step))
    ny = max(1, int((h - 2 * inset) // step))
    for i in range(nx + 1):
        cx = x + inset + (w - 2 * inset) * i / nx
        dot(cx, y + inset)
        dot(cx, y + h - inset)
    for j in range(1, ny):
        cy = y + inset + (h - 2 * inset) * j / ny
        dot(x + inset, cy)
        dot(x + w - inset, cy)
    return [{"type": "shape", "shape": "path", "name": f"{p}-rivets", "x": 0, "y": 0, "path": " ".join(d),
             "fill": color}]


def text(name, s, x, y, size, color, font, **kw):
    op = {"type": "text", "name": name, "text": s, "x": x, "y": y, "size": size, "color": color, "font": font}
    op.update(kw)
    return op


def rect(name, x, y, w, h, fill, r=0, **kw):
    op = {"type": "shape", "shape": "rounded-rectangle" if r else "rectangle", "name": name, "x": x, "y": y,
          "width": w, "height": h, "fill": fill}
    if r:
        op["radius"] = r
    op.update(kw)
    return op


def ell(name, x, y, w, h, fill, **kw):
    op = {"type": "shape", "shape": "ellipse", "name": name, "x": x, "y": y, "width": w, "height": h, "fill": fill}
    op.update(kw)
    return op


def center_x(name, dy=None):
    c = {"center-x": "canvas.center-x"}
    return {"type": "constrain", "target": name, "constraints": c}
