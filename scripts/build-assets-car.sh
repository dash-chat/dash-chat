#!/usr/bin/env bash
# Compile src-tauri/icons/AppIcon.icon (Icon Composer source) into Assets.car
# so the macOS bundle's beforeBundleCommand can produce the file Tauri then
# copies into Resources/. No-op on non-macOS platforms (the file is only
# consumed by the macOS bundle).
set -euo pipefail

case "${TAURI_ENV_PLATFORM:-$(uname -s)}" in
  macos|darwin|Darwin) ;;
  *) exit 0 ;;
esac

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/src-tauri/icons/AppIcon.icon"
OUT="$ROOT/src-tauri/icons"

if [[ ! -d "$SRC" ]]; then
  echo "build-assets-car: $SRC missing" >&2
  exit 1
fi

xcrun actool "$SRC" \
  --compile "$OUT" \
  --output-format human-readable-text \
  --notices --warnings --errors \
  --output-partial-info-plist "$OUT/.partial-info.plist" \
  --app-icon AppIcon \
  --include-all-app-icons \
  --enable-on-demand-resources NO \
  --target-device mac \
  --minimum-deployment-target "${MACOSX_DEPLOYMENT_TARGET:-14.0}" \
  --platform macosx \
  >/dev/null

# actool drops AppIcon.icns + a partial Info.plist alongside Assets.car; we
# only need Assets.car (the legacy .icns is regenerated separately).
rm -f "$OUT/AppIcon.icns" "$OUT/.partial-info.plist"

echo "build-assets-car: wrote $OUT/Assets.car"
