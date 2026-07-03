#!/usr/bin/env bash
# Guard: fail if libwayland-client leaked into the built AppImage.
#
# Backstop for install-ci-linuxdeploy-wrapper.sh. If a future tauri-bundler
# changes the cached-tool path/filename or the skip-if-exists behaviour, the
# shim silently stops applying and wayland leaks back in. This converts that
# silent regression into a loud failure.
set -euo pipefail

# The bundle lands in the Cargo workspace target dir (repo-root `target/`), but
# fall back to the per-crate `src-tauri/target/` in case the layout changes.
if [ -n "${1:-}" ]; then
  BUNDLE_DIRS=("$1")
else
  BUNDLE_DIRS=(
    "target/release/bundle/appimage"
    "src-tauri/target/release/bundle/appimage"
  )
fi

appimage=""
for dir in "${BUNDLE_DIRS[@]}"; do
  [ -d "$dir" ] || continue
  appimage="$(find "$dir" -maxdepth 1 -name '*.AppImage' | head -1)"
  [ -n "$appimage" ] && break
done
if [ -z "$appimage" ]; then
  echo "assert-no-wayland: no AppImage found in ${BUNDLE_DIRS[*]}" >&2
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
