#!/usr/bin/env bash
# Guard: fail if libwayland-client leaked into the built AppImage.
#
# Backstop for install-ci-linuxdeploy-wrapper.sh. If a future tauri-bundler
# changes the cached-tool path/filename or the skip-if-exists behaviour, the
# shim silently stops applying and wayland leaks back in. This converts that
# silent regression into a loud failure.
set -euo pipefail

BUNDLE_DIR="${1:-src-tauri/target/release/bundle/appimage}"
appimage="$(find "$BUNDLE_DIR" -maxdepth 1 -name '*.AppImage' | head -1)"
if [ -z "$appimage" ]; then
  echo "assert-no-wayland: no AppImage found in $BUNDLE_DIR" >&2
  exit 1
fi
appimage="$(readlink -f "$appimage")"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
( cd "$work" && "$appimage" --appimage-extract >/dev/null )

leaked="$(find "$work/squashfs-root" -name 'libwayland-client.so*')"
if [ -n "$leaked" ]; then
  echo "assert-no-wayland: libwayland-client leaked into $(basename "$appimage"):" >&2
  echo "$leaked" >&2
  exit 1
fi

echo "assert-no-wayland: OK, no libwayland-client bundled in $(basename "$appimage")"
