#!/usr/bin/env python3
"""Wrap the existing 256px PNG in an ICNS container without changing its pixels."""
from pathlib import Path
import struct
import sys

png = Path(sys.argv[1]).read_bytes()
if png[:8] != b"\x89PNG\r\n\x1a\n" or struct.unpack(">II", png[16:24]) != (256, 256):
    raise SystemExit("The macOS icon source must be a 256x256 PNG")
entry = struct.pack(">4sI", b"ic08", len(png) + 8) + png
Path(sys.argv[2]).write_bytes(struct.pack(">4sI", b"icns", len(entry) + 8) + entry)
