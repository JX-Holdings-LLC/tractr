#!/usr/bin/env python3
"""Refuse incomplete or corrupt offline export templates before packaging an editor."""
import hashlib
import json
from pathlib import Path
import sys

root, version = Path(sys.argv[1]), sys.argv[2]
for target, executable in (("windows-x64", "farm-player.exe"), ("linux-x64", "farm-player"), ("web", "farm_wasm_bg.wasm")):
    folder = root / target
    manifest = json.loads((folder / "template.json").read_text(encoding="utf-8"))
    if manifest["target"] != target or manifest["version"] != version:
        raise SystemExit(f"{target}: wrong template target or version")
    files = manifest["files"]
    required = {"THIRD-PARTY.txt"}
    if target == "web":
        required |= {"farm_wasm.js", "index.html", "style.css", "game.js"}
    if not required <= files.keys():
        raise SystemExit(f"{target}: incomplete template inventory")
    if executable:
        files = {**files, executable: manifest["sha256"]}
    for name, expected in files.items():
        path = (folder / name).resolve()
        if not path.is_relative_to(folder.resolve()):
            raise SystemExit(f"{target}: invalid template path {name}")
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected.lower():
            raise SystemExit(f"{target}: checksum mismatch for {name}")
print("Offline templates verified: windows-x64, linux-x64, web")
