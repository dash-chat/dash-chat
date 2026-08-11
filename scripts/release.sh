#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: ENV=[staging|production] $0 <version>"
  echo ""
  echo "  version   Semver version string (e.g. 0.11.0). The 'v' prefix is added automatically for the git tag."
  echo ""
  echo "Example: $0 0.11.0"
  echo "Example: ENV=staging $0 0.11.0"
  exit 1
}

if [ $# -ne 1 ]; then
  usage
fi

VERSION="$1"
if [ "${ENV:-}" = "staging" ]; then
  TAG="v${VERSION}-staging"
elif [ -z "${ENV:-}" ] || [ "$ENV" = "production" ]; then
  TAG="v${VERSION}"
else
  echo "Error: Unknown environment '$ENV'. Only 'staging' or 'production' are supported."
  exit 1
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

"$ROOT/scripts/update-version.sh" "$VERSION"

# Commit, tag, and push
if [ "${ENV:-}" = "staging" ]; then
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
