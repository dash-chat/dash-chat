#!/usr/bin/env bash
# Guard: fail if a host-coupled library leaked into the built AppImage.
#
# Backstop for install-ci-linuxdeploy-wrapper.sh. If a future tauri-bundler
# changes the cached-tool path/filename or the skip-if-exists behaviour, the
# shim silently stops applying and the excluded libs leak back in. This
# converts that silent regression into a loud failure. Covers both the wayland
# client stack (blank-screen EGL mismatch) and glib (gvfs/gio module symbol
# mismatch, e.g. "undefined symbol: g_task_set_static_name").
set -euo pipefail

# Libraries that must resolve from the host, never be bundled. Matched against
# the basename of files under squashfs-root.
FORBIDDEN_GLOBS=(
  'libwayland-client.so*'
  'libglib-2.0.so*'
  'libgio-2.0.so*'
  'libgobject-2.0.so*'
  'libgmodule-2.0.so*'
)

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

leaked=""
for glob in "${FORBIDDEN_GLOBS[@]}"; do
  leaked+="$(find "$work/squashfs-root" -name "$glob")"$'\n'
done
leaked="$(printf '%s' "$leaked" | grep -v '^$' || true)"
if [ -n "$leaked" ]; then
  echo "assert-no-host-libs: forbidden libs leaked into $(basename "$appimage"):" >&2
  echo "$leaked" >&2
  exit 1
fi

echo "assert-no-host-libs: OK, no host-coupled libs bundled in $(basename "$appimage")"
