"""Draws Brambleford's art with Vixl.

Every sprite is a Vixl `pixel-art` layer (rows of palette symbols, a 1 px outline added by Vixl) laid out
on a sheet document; each sheet is saved as an editable `.vixl` master and exported as a PNG for Tractr.

    python3 make_art.py            # writes masters/*.vixl and out/*.png next to this file

Needs Vixl (`pip install vixl-engine`, or an editable install of the Vixl repository).
"""

import json
import pathlib
import sys

from vixl import Project

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import people  # noqa: E402
import props  # noqa: E402

OUT = HERE / "out"
MASTERS = HERE / "masters"


def sheet(name, width, height, sprites, palette, outline=True):
    """One Vixl document holding `sprites` ([(layer name, Grid, x, y)]) on a transparent canvas."""
    project = Project(width, height, background="transparent")
    operations = [
        {
            "type": "pixel-art",
            "name": layer,
            "rows": grid.rows(),
            "palette": palette,
            "outline": outline,
            "shading": False,
            "x": x,
            "y": y,
        }
        for layer, grid, x, y in sprites
    ]
    project.apply(operations, detail="compact")
    project.save(str(MASTERS / f"{name}.vixl"), overwrite=True)
    project.export(str(OUT / f"{name}.png"), overwrite=True)
    return OUT / f"{name}.png"


def people_art(manifest):
    for key, person in people.PEOPLE.items():
        palette = people.palette(person)
        sprites = []
        for row, view in enumerate(people.DIRECTIONS):
            for column in range(4):
                grid = people.frame(person, view, column)
                sprites.append((f"{view}-{column}", grid, column * people.FRAME_W, row * people.FRAME_H))
        sheet(f"npc-{key}", 4 * people.FRAME_W, 4 * people.FRAME_H, sprites, palette)
        portraits = [
            (expression, people.portrait(person, expression).scaled(2), i * 48, 0)
            for i, expression in enumerate(people.EXPRESSIONS)
        ]
        sheet(f"portrait-{key}", 48 * len(portraits), 48, portraits, palette)
        manifest[f"npc-{key}"] = {"kind": "npc", "frame": [people.FRAME_W, people.FRAME_H], "frames": 4}
        manifest[f"portrait-{key}"] = {"kind": "portrait", "expressions": people.EXPRESSIONS, "size": 48}


def props_art(manifest):
    for name, spec in props.SHEETS.items():
        sprites = []
        for i, grid in enumerate(spec["frames"]):
            sprites.append((f"{name}-{i}", grid, i * grid.w, 0))
        w = sum(grid.w for grid in spec["frames"])
        h = max(grid.h for grid in spec["frames"])
        sheet(name, w, h, sprites, spec["palette"], outline=spec.get("outline", True))
        manifest[name] = {k: v for k, v in spec.items() if k not in ("frames", "palette")}
        manifest[name]["frame"] = [spec["frames"][0].w, spec["frames"][0].h]
        manifest[name]["frames"] = len(spec["frames"])


def main():
    OUT.mkdir(exist_ok=True)
    MASTERS.mkdir(exist_ok=True)
    manifest = {}
    people_art(manifest)
    props_art(manifest)
    icon = props.item_moonmelon().scaled(8)
    sheet("icon-brambleford", 256, 256, [("icon", icon, 0, 0)], props.ITEM_PALETTE)
    manifest["icon-brambleford"] = {"kind": "icon", "frame": [256, 256], "frames": 1}
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"wrote {len(manifest)} sheets to {OUT}")


if __name__ == "__main__":
    main()
