"""Writes hero.aseprite, the small Aseprite file the import tests read (python3 make-hero.py).

A 4x4 RGBA sprite with three frames (100, 150 and 200 ms): a visible layer "body" and a
hidden layer "hidden"; frame 0 has a raw cel, frame 1 a zlib-compressed cel, frame 2 a cel
linked to frame 0. Two tags (`walk_down`: frames 0-1 forward, `Idle Blink`: frames 1-2 pingpong)
and one slice (`icon`: 1,1 2x2). Layout follows Aseprite's file format specification.
"""

import pathlib
import struct
import zlib


def chunk(kind, data):
    return struct.pack("<IH", len(data) + 6, kind) + data


def string(text):
    raw = text.encode("utf-8")
    return struct.pack("<H", len(raw)) + raw


def layer(name, visible):
    flags = 1 if visible else 0
    return chunk(0x2004, struct.pack("<HHHHHHB3x", flags, 0, 0, 0, 0, 0, 255) + string(name))


def cel(layer_index, x, y, kind, body):
    return chunk(0x2005, struct.pack("<HhhBHh5x", layer_index, x, y, 255, kind, 0) + body)


def frame(duration, chunks):
    data = b"".join(chunks)
    return struct.pack("<IHHH2xI", 16 + len(data), 0xF1FA, len(chunks), duration, len(chunks)) + data


RED = bytes([255, 0, 0, 255])
GREEN = bytes([0, 255, 0, 255])
BLUE = bytes([0, 0, 255, 255])

tags = struct.pack("<H8x", 2)
tags += struct.pack("<HHBH6x3sx", 0, 1, 0, 0, b"\0\0\0") + string("walk_down")
tags += struct.pack("<HHBH6x3sx", 1, 2, 2, 0, b"\0\0\0") + string("Idle Blink")

slice_chunk = struct.pack("<III", 1, 0, 0) + string("icon") + struct.pack("<IiiII", 0, 1, 1, 2, 2)

frames = [
    frame(
        100,
        [
            layer("body", True),
            layer("hidden", False),
            chunk(0x2018, tags),
            chunk(0x2022, slice_chunk),
            cel(0, 0, 0, 0, struct.pack("<HH", 2, 2) + RED * 4),
            cel(1, 0, 0, 0, struct.pack("<HH", 4, 4) + BLUE * 16),
        ],
    ),
    frame(150, [cel(0, 2, 2, 2, struct.pack("<HH", 2, 2) + zlib.compress(GREEN * 4))]),
    frame(200, [cel(0, 0, 0, 1, struct.pack("<H", 0))]),
]

body = b"".join(frames)
header = struct.pack("<IHHHHHIH8xB3xHBBhhHH84x", 128 + len(body), 0xA5E0, 3, 4, 4, 32, 1, 100, 0, 0, 1, 1, 0, 0, 16, 16)
assert len(header) == 128
pathlib.Path(__file__).with_name("hero.aseprite").write_bytes(header + body)
