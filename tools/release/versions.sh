#!/usr/bin/env bash
# Checks that the version numbers in the repository agree (docs/dev/RELEASING.md "Versions").
#
#   tools/release/versions.sh                  release.json, the Cargo workspace version and the
#                                              Directory.Build.props default are the same X.Y.Z,
#                                              CHANGELOG.md starts with its "## X.Y.Z" section,
#                                              and VPK_VERSION in release.yml is the Velopack
#                                              package version
#   tools/release/versions.sh --release X.Y.Z  the same, and the version is X.Y.Z
#
# Every change to main advances the version (.release/config.json bumpPolicy), and only the
# versions dispatched to the release workflow are released. CI runs the first form; the release
# workflow runs the second before it builds anything, so crash logs, fe_version and the wasm
# version() report the version that shipped.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# version = "..." in the root Cargo.toml's [workspace.package] table.
cargo_version="$(awk '
  /^\[/ { section = $0 }
  section == "[workspace.package]" && /^version *=/ { gsub(/.*= *"|".*/, ""); print; exit }
' "$root/Cargo.toml")"
# <Version Condition="...">X</Version> in Directory.Build.props.
props_version="$(sed -n 's:.*<Version Condition="[^"]*">\([^<]*\)</Version>.*:\1:p' "$root/Directory.Build.props" | head -n1)"
# "version": "..." of the one app in release.json.
manifest_version="$(sed -n 's/^ *"version": *"\([^"]*\)".*/\1/p' "$root/release.json" | head -n1)"
changelog_heading="$(grep -m1 '^## ' "$root/CHANGELOG.md" || true)"

fail() {
  echo "versions.sh: $*" >&2
  exit 1
}

case "${1:-}" in
  "") ;;
  --release)
    release="${2:?usage: versions.sh --release X.Y.Z}"
    [[ "$cargo_version" == "$release" ]] ||
      fail "releasing $release, but Cargo.toml says $cargo_version; commit the version bump first"
    ;;
  *)
    fail "usage: versions.sh [--release X.Y.Z]"
    ;;
esac

[[ -n "$cargo_version" ]] || fail "no version in Cargo.toml [workspace.package]"
[[ -n "$props_version" ]] || fail "no default <Version> in Directory.Build.props"
[[ "$cargo_version" == "$props_version" ]] ||
  fail "Cargo.toml says $cargo_version, Directory.Build.props says $props_version"
[[ "$cargo_version" == "$manifest_version" ]] ||
  fail "Cargo.toml says $cargo_version, release.json says $manifest_version"

escaped="${cargo_version//./\\.}"
[[ "$changelog_heading" =~ ^##\ \[?v?$escaped\]?([[:space:]]|$) ]] ||
  fail "CHANGELOG.md should start with a \"## $cargo_version\" section, not \"$changelog_heading\""
[[ "$changelog_heading" != *unreleased* ]] ||
  fail "CHANGELOG.md marks $cargo_version unreleased (\"$changelog_heading\"); versions are not marked"

# The vpk tool that packs a release must be the Velopack version the app ships.
velopack="$(sed -n 's:.*<PackageVersion Include="Velopack" Version="\([^"]*\)".*:\1:p' "$root/Directory.Packages.props")"
vpk="$(sed -n 's/^ *VPK_VERSION: *\([^ #]*\).*/\1/p' "$root/.github/workflows/release.yml")"
[[ -n "$velopack" && "$velopack" == "$vpk" ]] ||
  fail "Velopack is '$velopack' in Directory.Packages.props but VPK_VERSION is '$vpk' in release.yml"

echo "versions: $cargo_version"
