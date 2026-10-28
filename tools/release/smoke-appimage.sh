#!/usr/bin/env bash
# Require the packaged editor to create a real X11 window. No FUSE device needed in CI.
set -euo pipefail
image="$(realpath "${1:?usage: smoke-appimage.sh FILE.AppImage}")"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export TRACTR_DATA_DIR="$work/data"
export APPIMAGE_EXTRACT_AND_RUN=1
export TRACTR_SMOKE_IMAGE="$image"
export TRACTR_SMOKE_LOG="$work/startup.log"
chmod +x "$image"
xvfb-run -a -s '-screen 0 1280x800x24' bash <<'INNER'
set -euo pipefail
"$TRACTR_SMOKE_IMAGE" > "$TRACTR_SMOKE_LOG" 2>&1 &
app_pid=$!
trap 'kill "$app_pid" 2>/dev/null || true; wait "$app_pid" 2>/dev/null || true' EXIT
for attempt in $(seq 1 30); do
  if ! kill -0 "$app_pid" 2>/dev/null; then
    cat "$TRACTR_SMOKE_LOG"
    exit 1
  fi
  if xwininfo -root -tree | grep -q '".*Tractr.*"'; then
    echo 'Packaged editor opened its window.'
    exit 0
  fi
  sleep 1
done
cat "$TRACTR_SMOKE_LOG"
echo 'Packaged editor did not open a window within 30 seconds.' >&2
exit 1
INNER
