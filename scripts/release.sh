#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 <version> [staging]"
  echo ""
  echo "  version   Semver version string (e.g. 0.11.0). The 'v' prefix is added automatically for the git tag."
  echo "  staging   Optional. Tag as v{version}-staging and skip the public site update."
  echo ""
  echo "Example: $0 0.11.0"
  echo "Example: $0 0.11.0 staging"
  exit 1
}

if [ $# -lt 1 ] || [ $# -gt 2 ]; then
  usage
fi

VERSION="$1"
ENV="${2:-}"
if [ "$ENV" = "staging" ]; then
  TAG="v${VERSION}-staging"
elif [ -n "$ENV" ]; then
  echo "Error: Unknown environment '$ENV'. Only 'staging' is supported."
  exit 1
else
  TAG="v${VERSION}"
fi

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

# 3. Update site download links (production only — staging builds aren't on the public site)
if [ "$ENV" != "staging" ]; then
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

# 5. Update Cargo.lock to reflect the new version
(cd "$ROOT" && cargo update --workspace)
echo "  Updated Cargo.lock"

# 6. Commit, tag, and push
if [ "$ENV" = "staging" ]; then
  git -C "$ROOT" add "$TAURI_CONF" "$CARGO_TOML" "$IOS_PLIST" "$ROOT/Cargo.lock"
else
  git -C "$ROOT" add "$TAURI_CONF" "$CARGO_TOML" "$SITE_INDEX" "$IOS_PLIST" "$ROOT/Cargo.lock"
fi
if git -C "$ROOT" diff --cached --quiet; then
  echo "  No changes to commit (version files already up to date)"
else
  git -C "$ROOT" commit -m "Release $TAG"
  echo "  Created commit"
fi
git -C "$ROOT" tag "$TAG"
echo "  Created tag $TAG"

git -C "$ROOT" push
git -C "$ROOT" push origin "$TAG"
echo "  Pushed to remote"

echo ""
echo "Done! Released $TAG"

