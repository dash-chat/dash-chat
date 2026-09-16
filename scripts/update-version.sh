#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: ENV=[staging|production] $0 <version>"
  echo ""
  echo "  version   Semver version string (e.g. 0.11.0)."
  echo ""
  echo "Example: $0 0.11.0"
  echo "Example: ENV=staging $0 0.11.0"
  exit 1
}

if [ $# -ne 1 ]; then
  usage
fi

VERSION="$1"

# Validate semver format (basic check)
if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "Error: Version must be in semver format (e.g. 0.11.0)"
  exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

TAURI_CONF="$ROOT/src-tauri/tauri.conf.json"
CARGO_TOML="$ROOT/src-tauri/Cargo.toml"
SITE_INDEX="$ROOT/packages/site/index.html"
IOS_PLIST="$ROOT/src-tauri/gen/apple/dash-chat_iOS/Info.plist"
IOS_PBXPROJ="$ROOT/src-tauri/gen/apple/dash-chat.xcodeproj/project.pbxproj"
IOS_PROJECT_YML="$ROOT/src-tauri/gen/apple/project.yml"

# Check that all files exist
for f in "$TAURI_CONF" "$CARGO_TOML" "$SITE_INDEX" "$IOS_PLIST" "$IOS_PBXPROJ" "$IOS_PROJECT_YML"; do
  if [ ! -f "$f" ]; then
    echo "Error: $f not found"
    exit 1
  fi
done

# 1. Update tauri.conf.json
sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" "$TAURI_CONF"
echo "  Updated $TAURI_CONF"

# 2. Update src-tauri/Cargo.toml (only the package version, not dependency versions)
sed -i "0,/^version = \"[^\"]*\"/s//version = \"$VERSION\"/" "$CARGO_TOML"
echo "  Updated $CARGO_TOML"

# 3. Update site download links (production only — staging builds aren't on the public site)
if [ "${ENV:-}" != "staging" ]; then
  OLD_URL_PATTERN='releases/download/v[0-9]\+\.[0-9]\+\.[0-9]\+'
  NEW_URL_PATTERN="releases/download/v${VERSION}"
  sed -i "s|$OLD_URL_PATTERN|$NEW_URL_PATTERN|g" "$SITE_INDEX"

  OLD_FILE_PATTERN='Dash\.Chat_[0-9]\+\.[0-9]\+\.[0-9]\+'
  NEW_FILE_PATTERN="Dash.Chat_$VERSION"
  sed -i "s|$OLD_FILE_PATTERN|$NEW_FILE_PATTERN|g" "$SITE_INDEX"

  sed -i "s|darksoil-studio/dash-chat/v[0-9]\+\.[0-9]\+\.[0-9]\+|darksoil-studio/dash-chat/v${VERSION}|g" "$SITE_INDEX"

  echo "  Updated $SITE_INDEX"
fi

# 4. Update iOS Info.plist (CFBundleShortVersionString and CFBundleVersion)
sed -i "/<key>CFBundleShortVersionString<\/key>/{ n; s|<string>[^<]*</string>|<string>$VERSION</string>| }" "$IOS_PLIST"
sed -i "/<key>CFBundleVersion<\/key>/{ n; s|<string>[^<]*</string>|<string>$VERSION</string>| }" "$IOS_PLIST"
echo "  Updated $IOS_PLIST"

# 5. Update the Xcode project: the notification extension generates its Info.plist
# from these, and App Store validation requires them to match the app's.
sed -i "s/CURRENT_PROJECT_VERSION = [^;]*;/CURRENT_PROJECT_VERSION = $VERSION;/" "$IOS_PBXPROJ"
sed -i "s/MARKETING_VERSION = [^;]*;/MARKETING_VERSION = $VERSION;/" "$IOS_PBXPROJ"
echo "  Updated $IOS_PBXPROJ"

# 6. Update the xcodegen spec the Xcode project is regenerated from
sed -i "s/^\(\s*CFBundleShortVersionString:\).*/\1 $VERSION/" "$IOS_PROJECT_YML"
sed -i "s/^\(\s*CFBundleVersion:\).*/\1 \"$VERSION\"/" "$IOS_PROJECT_YML"
echo "  Updated $IOS_PROJECT_YML"

# 7. Update Cargo.lock to reflect the new version
(cd "$ROOT" && cargo update --workspace)
echo "  Updated Cargo.lock"
