#!/usr/bin/env python3
"""Exercise release-files.py: the published set, DOWNLOADS.md and SHA256SUMS."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("release-files.py")
CONFIG = Path(__file__).resolve().parents[2] / ".release" / "config.json"
VERSION = "1.2.3"


def run(cwd, releases, *extra):
    return subprocess.run(
        [sys.executable, str(SCRIPT), VERSION, f"v{VERSION}", "owner/repo", str(releases), *extra],
        cwd=cwd, capture_output=True, text=True, check=False)


class ReleaseFilesTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.releases = self.root / "Releases"
        self.releases.mkdir()
        assets = json.loads(CONFIG.read_text())["apps"]["tractr"]["assets"]
        self.required = [a.replace("{version}", VERSION) for a in assets if a != "DOWNLOADS.md"]
        for name in self.required:
            (self.releases / name).write_bytes(name.encode() * 1000)
        # Written by vpk or downloaded from the previous release, never published.
        self.unpublished = ["RELEASES", "RELEASES-osx-x64", "assets.win.json", "Tractr-1.2.2-full.nupkg"]
        for name in self.unpublished:
            (self.releases / name).write_text(name)

    def sums(self):
        lines = (self.root / "SHA256SUMS").read_bytes().decode().split("\n")
        self.assertEqual(lines[-1], "")
        return dict(reversed(line.split("  ")) for line in lines[:-1])

    def test_publishes_exactly_the_configured_files(self):
        result = run(self.root, self.releases, "version: 1.2.3", "commit: abc")
        self.assertEqual(result.returncode, 0, result.stderr)
        sums = self.sums()
        self.assertEqual(sorted(sums), sorted(self.required + ["DOWNLOADS.md"]))
        for name, digest in sums.items():
            self.assertEqual(digest, hashlib.sha256((self.releases / name).read_bytes()).hexdigest())
        for name in self.unpublished:
            self.assertNotIn(name, sums)

    def test_downloads_names_one_installer_per_system(self):
        self.assertEqual(run(self.root, self.releases, "commit: abc").returncode, 0)
        text = (self.releases / "DOWNLOADS.md").read_text()
        for name in ("Tractr-win-Setup.exe", "Tractr-osx-arm64-Portable.zip", "Tractr-osx-x64-Portable.zip",
                     "Tractr-linux-x64.AppImage", "Tractr-win-Portable.zip"):
            self.assertIn(f"(https://github.com/owner/repo/releases/download/v{VERSION}/{name})", text)
        self.assertIn("https://github.com/owner/repo/blob/v1.2.3/docs/INSTALL.md", text)
        self.assertIn("commit: abc", text)
        self.assertNotIn("\r", text)

    def test_includes_the_delta_package_when_there_is_one(self):
        (self.releases / f"Tractr-{VERSION}-delta.nupkg").write_text("delta")
        self.assertEqual(run(self.root, self.releases).returncode, 0)
        self.assertIn(f"Tractr-{VERSION}-delta.nupkg", self.sums())

    def test_fails_when_a_published_file_is_missing(self):
        os.remove(self.releases / "Tractr-linux-x64.AppImage")
        result = run(self.root, self.releases)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stderr.strip(), "Missing release files: Tractr-linux-x64.AppImage")
        self.assertFalse((self.root / "SHA256SUMS").exists())


if __name__ == "__main__":
    unittest.main()
