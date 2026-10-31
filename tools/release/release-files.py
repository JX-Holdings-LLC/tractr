#!/usr/bin/env python3
"""Write DOWNLOADS.md and SHA256SUMS for the files a release publishes.

    release-files.py VERSION TAG REPOSITORY RELEASES_DIR [BUILD_INFO_LINE ...]

The published files are the `assets` of .release/config.json, plus the optional
delta package. Each must be in RELEASES_DIR. Other files there (feeds of the
previous release, legacy RELEASES files, assets.*.json) are not published, so
they are left out.

DOWNLOADS.md is written to RELEASES_DIR. It is a published asset, and the
publisher appends it to the release notes (`notesAsset`), so every release page
says which single file to download. SHA256SUMS covers every published file,
DOWNLOADS.md included; the build provenance attestation is made from it.
"""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
CONFIG = ROOT / ".release" / "config.json"
APP = "tractr"
NOTES = "DOWNLOADS.md"

# What a user downloads to install Tractr, in the order the release page lists them.
INSTALLERS = (
    ("Windows (64-bit)", "Tractr-win-Setup.exe"),
    ("Mac with Apple silicon (M1 or newer)", "Tractr-osx-arm64-Portable.zip"),
    ("Mac with an Intel processor", "Tractr-osx-x64-Portable.zip"),
    ("Linux (64-bit)", "Tractr-linux-x64.AppImage"),
)
WINDOWS_PORTABLE = "Tractr-win-Portable.zip"


def published(version):
    app = json.loads(CONFIG.read_text(encoding="utf-8"))["apps"][APP]
    names = [name.replace("{version}", version) for name in app["assets"]]
    optional = [f"Tractr-{version}-delta.nupkg"]
    return names, optional


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def size(path):
    return f"{path.stat().st_size / 1_000_000:.0f} MB"


def downloads(version, tag, repository, releases, build_info):
    base = f"https://github.com/{repository}/releases/download/{tag}"
    guide = f"https://github.com/{repository}/blob/{tag}/docs/INSTALL.md"
    lines = [
        "## Download",
        "",
        "Download **one** file, the one for your computer:",
        "",
        "| Your computer | Download | Size |",
        "|---|---|---|",
    ]
    for computer, name in INSTALLERS:
        lines.append(f"| {computer} | [{name}]({base}/{name}) | {size(releases / name)} |")
    lines += [
        "",
        f"The [install guide]({guide}) explains the first launch on each system. "
        "After that, Tractr updates itself (**Help → Update Center**).",
        "",
        f"Rather not run an installer on Windows? [{WINDOWS_PORTABLE}]({base}/{WINDOWS_PORTABLE}) "
        f"({size(releases / WINDOWS_PORTABLE)}) is a portable copy: unzip it anywhere and run `Tractr.exe`.",
        "",
        "<details><summary>What are the other files?</summary>",
        "",
        "- `*.nupkg` and `releases.*.json`: update packages and the update feed of each system. "
        "Installed copies of Tractr download them; you don't need to.",
        "- `SHA256SUMS`: the SHA-256 of every file of this release, to check a download "
        "(`sha256sum --check --ignore-missing SHA256SUMS`).",
        f"- `{NOTES}`: this section.",
        "",
        "</details>",
    ]
    if build_info:
        lines += ["", "<details><summary>Build info</summary>", "", "```text", *build_info, "```", "", "</details>"]
    return "\n".join(lines) + "\n"


def main(argv):
    if len(argv) < 5:
        sys.exit(__doc__.strip().splitlines()[2].strip())
    version, tag, repository, releases = argv[1], argv[2], argv[3], Path(argv[4])
    required, optional = published(version)
    if NOTES not in required:
        sys.exit(f"{NOTES} must be a required asset in {CONFIG}")
    missing = [name for name in required if name != NOTES and not (releases / name).is_file()]
    if missing:
        sys.exit("Missing release files: " + ", ".join(missing))
    (releases / NOTES).write_text(downloads(version, tag, repository, releases, argv[5:]), encoding="utf-8", newline="\n")
    names = sorted(required + [name for name in optional if (releases / name).is_file()])
    sums = [f"{sha256(releases / name)}  {name}" for name in names]
    # LF line ends, as sha256sum --check expects.
    Path("SHA256SUMS").write_text("\n".join(sums) + "\n", encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main(sys.argv)
