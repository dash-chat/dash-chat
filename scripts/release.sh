#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 <version>"
  echo ""
  echo "  version   Semver version string (e.g. 0.11.0). The 'v' prefix is added automatically for the git tag."
  echo ""
  echo "Example: $0 0.11.0"
  exit 1
}

if [ $# -ne 1 ]; then
  usage
fi

VERSION="$1"
TAG="v${VERSION}"

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

# Check that all files exist
for f in "$TAURI_CONF" "$CARGO_TOML" "$SITE_INDEX" "$IOS_PLIST"; do
  if [ ! -f "$f" ]; then
    echo "Error: $f not found"
    exit 1
  fi
done

# Check for clean working tree
if [ -n "$(git -C "$ROOT" status --porcelain)" ]; then
  echo "Error: Working tree is not clean. Commit or stash changes first."
  exit 1
fi

# Check that tag doesn't already exist
if git -C "$ROOT" rev-parse "$TAG" >/dev/null 2>&1; then
  echo "Error: Tag $TAG already exists"
  exit 1
fi

echo "Releasing version $VERSION (tag: $TAG)..."

# 1. Update tauri.conf.json
sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" "$TAURI_CONF"
echo "  Updated $TAURI_CONF"

# 2. Update src-tauri/Cargo.toml (only the package version, not dependency versions)
sed -i "0,/^version = \"[^\"]*\"/s//version = \"$VERSION\"/" "$CARGO_TOML"
echo "  Updated $CARGO_TOML"

# 3. Update site download links
OLD_URL_PATTERN='releases/download/v[0-9]\+\.[0-9]\+\.[0-9]\+'
NEW_URL_PATTERN="releases/download/$TAG"
sed -i "s|$OLD_URL_PATTERN|$NEW_URL_PATTERN|g" "$SITE_INDEX"

# Update filenames in download URLs (Dash.Chat_X.Y.Z_...)
OLD_FILE_PATTERN='Dash\.Chat_[0-9]\+\.[0-9]\+\.[0-9]\+'
NEW_FILE_PATTERN="Dash.Chat_$VERSION"
sed -i "s|$OLD_FILE_PATTERN|$NEW_FILE_PATTERN|g" "$SITE_INDEX"

# Update nix command version
sed -i "s|darksoil-studio/dash-chat/v[0-9]\+\.[0-9]\+\.[0-9]\+|darksoil-studio/dash-chat/$TAG|g" "$SITE_INDEX"

echo "  Updated $SITE_INDEX"

# 4. Update iOS Info.plist (CFBundleShortVersionString and CFBundleVersion)
sed -i "/<key>CFBundleShortVersionString<\/key>/{ n; s|<string>[^<]*</string>|<string>$VERSION</string>| }" "$IOS_PLIST"
sed -i "/<key>CFBundleVersion<\/key>/{ n; s|<string>[^<]*</string>|<string>$VERSION</string>| }" "$IOS_PLIST"
echo "  Updated $IOS_PLIST"

# 5. Update Cargo.lock to reflect the new version
(cd "$ROOT" && cargo update --workspace)
echo "  Updated Cargo.lock"

# 6. Commit, tag, and push
git -C "$ROOT" add "$TAURI_CONF" "$CARGO_TOML" "$SITE_INDEX" "$IOS_PLIST" "$ROOT/Cargo.lock"
git -C "$ROOT" commit -m "Release $TAG"
git -C "$ROOT" tag "$TAG"
echo "  Created commit and tag $TAG"

git -C "$ROOT" push
git -C "$ROOT" push origin "$TAG"
echo "  Pushed to remote"

echo ""
echo "Done! Released $TAG"

