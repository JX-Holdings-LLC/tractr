#!/usr/bin/env bash
# Native runner only: publish with all three prebuilt export templates, then package.
set -euo pipefail
rid="${1:?usage: pack-unix.sh RID VERSION}"
version="${2:?usage: pack-unix.sh RID VERSION}"
case "$rid:$(uname -s):$(uname -m)" in
  linux-x64:Linux:x86_64 | osx-x64:Darwin:x86_64 | osx-arm64:Darwin:arm64) ;;
  *) echo "The $rid editor must be built on a matching native runner." >&2; exit 1 ;;
esac
python3 tools/release/verify-templates.py players "$version"
dotnet publish src/Tractr.App -c Release -r "$rid" --self-contained \
  "-p:Version=$version" "-p:FarmPlayerTemplatesDir=$PWD/players" -o publish
case "$rid" in
  linux-*) library=libfarm_ffi.so ;;
  osx-*) library=libfarm_ffi.dylib ;;
esac
for file in Tractr "$library" licenses/LICENSE.txt licenses/THIRD-PARTY-dotnet.txt licenses/THIRD-PARTY-rust.txt; do
  test -s "publish/$file"
done
python3 tools/release/verify-templates.py publish/players "$version"
chmod +x publish/Tractr publish/players/linux-x64/farm-player
args=(pack --packId Tractr --packVersion "$version" --packDir publish
  --runtime "$rid" --channel "$rid" --mainExe Tractr --packTitle Tractr
  --packAuthors 'Tractr contributors' --releaseNotes release-notes.md --outputDir Releases)
if [[ "$rid" == linux-* ]]; then
  test -s publish/libfarm_ffi_silent.so
  if readelf -d publish/libfarm_ffi_silent.so | grep -q libasound; then
    echo "The silent engine must not depend on ALSA." >&2
    exit 1
  fi
  ldd "publish/$library" > native-dependencies.txt
  if grep -q 'not found' native-dependencies.txt; then
    cat native-dependencies.txt >&2
    exit 1
  fi
  args+=(--icon src/Tractr.App/Assets/app-icon-256.png --categories Development)
else
  # The .app ZIP is the installable artifact; no separate .pkg signing identity is needed.
  icon_dir="$(mktemp -d)"
  trap 'rm -rf "$icon_dir"' EXIT
  python3 tools/release/macos-icon.py src/Tractr.App/Assets/app-icon-256.png "$icon_dir/Tractr.icns"
  args+=(--noInst --bundleId com.jxholdings.tractr --icon "$icon_dir/Tractr.icns")
  # apple-signing.sh sets MACOS_SIGNING; dry runs and releases without Apple credentials are unsigned.
  if [[ "${MACOS_SIGNING:-false}" == true ]]; then
    : "${MACOS_SIGN_IDENTITY:?Signing requires a Developer ID Application identity}"
    : "${MACOS_KEYCHAIN:?Signing requires the signing keychain}"
    args+=(--signAppIdentity "$MACOS_SIGN_IDENTITY" --notaryProfile tractr-notary --keychain "$MACOS_KEYCHAIN")
  fi
fi
vpk --yes --skip-updates "${args[@]}"
