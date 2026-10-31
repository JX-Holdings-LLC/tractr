#!/usr/bin/env bash
# Rebuilds Brambleford from its sources: the Vixl art, then the Tractr project folder, then its Problems.
#
#   examples/brambleford/build.sh
#
# Needs Vixl on the Python path (pip install vixl-engine), Pillow, and a Release build of farmc
# (dotnet build src/FarmEngine.Cli -c Release), whose FarmEngine.Authoring.dll the F# scripts load.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

(cd "$here/art" && python3 make_art.py)
dotnet fsi "$here/tools/template.fsx" quest "$scratch/base.json"
python3 "$here/tools/make_project.py" "$scratch/base.json" "$here/art/out" "$scratch/brambleford.json"
dotnet fsi "$here/tools/tofolder.fsx" "$scratch/brambleford.json" "$here/project"
dotnet fsi "$here/tools/check.fsx" "$here/project"
