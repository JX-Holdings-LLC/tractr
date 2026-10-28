#!/usr/bin/env bash
# Disposable macOS release runner only. Never print credential contents.
set -euo pipefail
: "${MACOS_CERTIFICATE_BASE64:?Developer ID Application certificate is required}"
: "${MACOS_CERTIFICATE_PASSWORD:?Certificate password is required}"
: "${MACOS_SIGN_IDENTITY:?Developer ID Application identity is required}"
: "${APPLE_API_KEY_BASE64:?Notarization API key is required}"
: "${APPLE_API_KEY_ID:?Notarization API key ID is required}"
: "${APPLE_API_ISSUER:?Notarization API issuer is required}"
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
