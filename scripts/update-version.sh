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
IOS_PLIST="$ROOT/src-tauri/gen/apple/dash-chat_iOS/Info.plist"
IOS_PBXPROJ="$ROOT/src-tauri/gen/apple/dash-chat.xcodeproj/project.pbxproj"
IOS_PROJECT_YML="$ROOT/src-tauri/gen/apple/project.yml"

# Check that all files exist
for f in "$TAURI_CONF" "$CARGO_TOML" "$IOS_PLIST" "$IOS_PBXPROJ" "$IOS_PROJECT_YML"; do
  if [ ! -f "$f" ]; then
    echo "Error: $f not found"
    exit 1
  fi
done

# perl instead of sed: BSD sed (macOS) and GNU sed disagree on -i, 0,/re/ and \s
export VERSION

# 1. Update tauri.conf.json
perl -pi -e 's/"version": "[^"]*"/"version": "$ENV{VERSION}"/' "$TAURI_CONF"
echo "  Updated $TAURI_CONF"

# 2. Update src-tauri/Cargo.toml (only the package version, not dependency versions)
perl -pi -e '$done ||= s/^version = "[^"]*"/version = "$ENV{VERSION}"/' "$CARGO_TOML"
echo "  Updated $CARGO_TOML"

# 3. Update iOS Info.plist (CFBundleShortVersionString and CFBundleVersion)
perl -0pi -e 's|(<key>CFBundleShortVersionString</key>\s*<string>)[^<]*|$1$ENV{VERSION}|' "$IOS_PLIST"
perl -0pi -e 's|(<key>CFBundleVersion</key>\s*<string>)[^<]*|$1$ENV{VERSION}|' "$IOS_PLIST"
echo "  Updated $IOS_PLIST"

# 4. Update the Xcode project: the notification extension generates its Info.plist
# from these, and App Store validation requires them to match the app's.
perl -pi -e 's/CURRENT_PROJECT_VERSION = [^;]*;/CURRENT_PROJECT_VERSION = $ENV{VERSION};/' "$IOS_PBXPROJ"
perl -pi -e 's/MARKETING_VERSION = [^;]*;/MARKETING_VERSION = $ENV{VERSION};/' "$IOS_PBXPROJ"
echo "  Updated $IOS_PBXPROJ"

# 5. Update the xcodegen spec the Xcode project is regenerated from
perl -pi -e 's/^(\s*CFBundleShortVersionString:).*/$1 $ENV{VERSION}/' "$IOS_PROJECT_YML"
perl -pi -e 's/^(\s*CFBundleVersion:).*/$1 "$ENV{VERSION}"/' "$IOS_PROJECT_YML"
echo "  Updated $IOS_PROJECT_YML"

# 6. Update Cargo.lock to reflect the new version
(cd "$ROOT" && cargo update --workspace)
echo "  Updated Cargo.lock"
