#!/usr/bin/env python3
"""Exercise release template validation, including corrupt and incomplete bundles."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

VERIFY = Path(__file__).with_name("verify-templates.py")


class TemplatesTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for target, exe in (("windows-x64", "farm-player.exe"), ("linux-x64", "farm-player"), ("web", "farm_wasm_bg.wasm")):
            folder = self.root / target
            folder.mkdir()
            names = [exe, "THIRD-PARTY.txt"]
            if target == "web":
                names += ["farm_wasm.js", "index.html", "style.css", "game.js"]
            hashes = {}
            for name in names:
                (folder / name).write_bytes(name.encode())
                hashes[name] = hashlib.sha256(name.encode()).hexdigest()
            manifest = {"target": target, "version": "1.0.0", "sha256": hashes.pop(exe), "files": hashes}
            (folder / "template.json").write_text(json.dumps(manifest))

    def verify(self):
        return subprocess.run([sys.executable, str(VERIFY), str(self.root), "1.0.0"], capture_output=True).returncode

    def test_complete_bundle(self):
        self.assertEqual(0, self.verify())

    def test_corrupt_native_and_web_binaries(self):
        for target, name in (("linux-x64", "farm-player"), ("web", "farm_wasm_bg.wasm")):
            with self.subTest(target=target):
                path = self.root / target / name
                original = path.read_bytes()
                path.write_bytes(b"corrupt")
                self.assertNotEqual(0, self.verify())
                path.write_bytes(original)

    def test_missing_platform(self):
        (self.root / "windows-x64" / "template.json").unlink()
        self.assertNotEqual(0, self.verify())

    def test_wrong_version_and_missing_inventory(self):
        path = self.root / "web" / "template.json"
        original = json.loads(path.read_text())
        for change in ({"version": "0.9.0"}, {"files": {}}):
            with self.subTest(change=change):
                path.write_text(json.dumps({**original, **change}))
                self.assertNotEqual(0, self.verify())

    def test_path_escape(self):
        path = self.root / "web" / "template.json"
        manifest = json.loads(path.read_text())
        manifest["files"]["../outside"] = "0" * 64
        path.write_text(json.dumps(manifest))
        self.assertNotEqual(0, self.verify())


if __name__ == "__main__":
    unittest.main()
