#!/usr/bin/env bash
# Validate and launch the app extracted from the ZIP that will be distributed.
set -euo pipefail
archive="${1:?usage: smoke-macos.sh ZIP [VERSION [--require-signature]]}"
version="${2:-}"
signature="${3:-}"
case "$signature" in
  '' | --require-signature) ;;
  *) echo "Unknown signature option: $signature" >&2; exit 1 ;;
esac
work="$(mktemp -d)"
app_pid=''
cleanup() {
  if [[ -n "$app_pid" ]]; then
    kill "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT
ditto -xk "$archive" "$work"
app="$work/Tractr.app"
binary="$app/Contents/MacOS/Tractr"
test -x "$binary"
test -s "$app/Contents/MacOS/libfarm_ffi.dylib"
if [[ -n "$version" ]]; then
  python3 "$(dirname "$0")/verify-templates.py" "$app/Contents/MacOS/players" "$version"
fi
if [[ "$signature" == --require-signature ]]; then
  codesign --verify --deep --strict --verbose=2 "$app"
  xcrun stapler validate "$app"
  spctl --assess --type execute --verbose=2 "$app"
fi
export TRACTR_DATA_DIR="$work/data"
"$binary" > "$work/startup.log" 2>&1 &
app_pid=$!
for attempt in $(seq 1 10); do
  sleep 1
  if ! kill -0 "$app_pid" 2>/dev/null; then
    cat "$work/startup.log"
    echo 'Packaged Mac editor exited during startup.' >&2
    exit 1
  fi
done
echo 'Packaged Mac editor stayed running through startup.'
