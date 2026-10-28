#!/usr/bin/env bash
# Disposable macOS release runner only. Never print credential contents.
#
# With every Apple credential set, import them and set MACOS_SIGNING=true so pack-unix.sh signs
# and notarizes and smoke-macos.sh requires the signature. With none set, warn and set
# MACOS_SIGNING=false: the release ships an unsigned Mac app, as Windows ships unsigned without
# WINDOWS_SIGN_PARAMS. A partial set is a misconfiguration and fails.
set -euo pipefail
names=(MACOS_CERTIFICATE_BASE64 MACOS_CERTIFICATE_PASSWORD MACOS_SIGN_IDENTITY
  APPLE_API_KEY_BASE64 APPLE_API_KEY_ID APPLE_API_ISSUER)
missing=()
for name in "${names[@]}"; do
  [[ -n "${!name:-}" ]] || missing+=("$name")
done
if (( ${#missing[@]} == ${#names[@]} )); then
  echo "::warning::Apple signing credentials are not configured - the Mac editor is unsigned and Gatekeeper will block it until users allow it in System Settings."
  echo "MACOS_SIGNING=false" >> "$GITHUB_ENV"
  exit 0
fi
if (( ${#missing[@]} > 0 )); then
  echo "::error::Apple signing is partly configured; missing: ${missing[*]}"
  exit 1
fi
umask 077
keychain="$RUNNER_TEMP/tractr-signing.keychain-db"
password="$(openssl rand -hex 32)"
echo "::add-mask::$password"
printf '%s' "$MACOS_CERTIFICATE_BASE64" | base64 --decode > "$RUNNER_TEMP/tractr-signing.p12"
printf '%s' "$APPLE_API_KEY_BASE64" | base64 --decode > "$RUNNER_TEMP/tractr-notary.p8"
security create-keychain -p "$password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$password" "$keychain"
security import "$RUNNER_TEMP/tractr-signing.p12" -k "$keychain" -P "$MACOS_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$password" "$keychain" >/dev/null
xcrun notarytool store-credentials tractr-notary --keychain "$keychain" \
  --key "$RUNNER_TEMP/tractr-notary.p8" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER"
echo "MACOS_KEYCHAIN=$keychain" >> "$GITHUB_ENV"
echo "MACOS_SIGNING=true" >> "$GITHUB_ENV"
