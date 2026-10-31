"""Composes the README's cast card in Vixl: each villager's portrait and sprite with a name and a role.

    python3 make_cast.py ../media/cast.png      # after make_art.py
"""

import io
import pathlib
import sys

from PIL import Image
from vixl import Project

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE / "out"

CAST = [
    ("odile", "Mayor Odile", "keeps the lanterns"),
    ("bram", "Bram", "the miller"),
    ("juniper", "Juniper", "seed shop"),
    ("marigold", "Marigold", "the Copper Kettle"),
    ("sela", "Sela", "fisher"),
    ("pip", "Pip", "treasure hunter"),
    ("rook", "Rook", "the wanderer"),
]


def nearest(path, box, scale):
    """PNG bytes of a crop of `path`, enlarged with nearest-neighbour sampling."""
    with Image.open(path) as image:
        part = image.crop(box)
        part = part.resize((part.width * scale, part.height * scale), Image.NEAREST)
        data = io.BytesIO()
        part.save(data, format="PNG")
        return data.getvalue()


def main(target):
    width, height, column = 1440, 500, 200
    project = Project(width, height, background="#1f2a24")
    project.apply([
        {"type": "gradient", "name": "sky", "start": "#24372c", "end": "#152019", "direction": "vertical"},
        {"type": "text", "name": "title", "text": "The people of Brambleford", "x": 40, "y": 28, "size": 40,
         "color": "#ffe9b8"},
    ], detail="compact")
    project.apply([
        {"type": "shape", "shape": "ellipse", "name": f"{key}-disc", "x": i * column + 38, "y": 100, "width": 160,
         "height": 160, "fill": "#33473a"}
        for i, (key, _, _) in enumerate(CAST)
    ], detail="compact")
    for i, (key, name, role) in enumerate(CAST):
        x = i * column + 20
        project.import_image(data=nearest(OUT / f"portrait-{key}.png", (48, 0, 96, 48), 3), name=f"{key}-portrait")
        project.import_image(data=nearest(OUT / f"npc-{key}.png", (3, 4, 29, 48), 3), name=f"{key}-sprite")
        project.apply([
            {"type": "move", "target": f"{key}-portrait", "x": x + 46, "y": 108},
            {"type": "move", "target": f"{key}-sprite", "x": x + 79, "y": 276},
            {"type": "text", "name": f"{key}-name", "text": name, "x": x + 10, "y": 424, "size": 22,
             "color": "#ffe9b8", "align": "center"},
            {"type": "text-layout", "target": f"{key}-name", "width": 180, "height": 30},
            {"type": "text", "name": f"{key}-role", "text": role, "x": x + 10, "y": 456, "size": 15,
             "color": "#b9cdb9", "align": "center"},
            {"type": "text-layout", "target": f"{key}-role", "width": 180, "height": 24},
        ], detail="compact")
    project.export(str(target), overwrite=True)


if __name__ == "__main__":
    main(pathlib.Path(sys.argv[1]))
