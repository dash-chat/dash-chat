#!/usr/bin/env bash
# Strip the host-managed graphics stack out of a freshly-bundled AppImage.
#
# linuxdeploy bundles libwayland-client.so.0 (and the rest of the GL/EGL stack)
# into the AppImage. Because AppRun puts the bundled libs ahead of the host's on
# LD_LIBRARY_PATH, the host Mesa libEGL ends up paired with a foreign
# libwayland-client. On Wayland sessions that mismatch makes eglGetPlatformDisplay
# return EGL_BAD_PARAMETER, WebKit's GPU process aborts, and the window renders
# blank. These libraries are part of the host compositor/driver ABI and must
# always come from the host, so we remove them and repackage.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE_DIR="$ROOT/src-tauri/target/release/bundle/appimage"
CACHE="$HOME/.cache/tauri"

# Library sonames that must resolve from the host, never from the AppImage.
STRIP_GLOBS=(
  'libwayland-client.so*'
  'libwayland-egl.so*'
  'libwayland-cursor.so*'
  'libwayland-server.so*'
  'libEGL.so*'
  'libGL.so*'
  'libGLX.so*'
  'libGLdispatch.so*'
  'libGLESv2.so*'
  'libOpenGL.so*'
  'libgbm.so*'
  'libdrm.so*'
)

APPIMAGE="${1:-}"
if [ -z "$APPIMAGE" ]; then
  APPIMAGE="$(find "$BUNDLE_DIR" -maxdepth 1 -name '*.AppImage' | head -1)"
fi
if [ -z "$APPIMAGE" ] || [ ! -f "$APPIMAGE" ]; then
  echo "Error: no AppImage found (looked in $BUNDLE_DIR)" >&2
  exit 1
fi
echo "Post-processing $APPIMAGE"

APPIMAGETOOL_SRC="$CACHE/linuxdeploy-plugin-appimage.AppImage"
if [ ! -f "$APPIMAGETOOL_SRC" ]; then
  echo "Error: $APPIMAGETOOL_SRC not found (run 'pnpm tauri build' first)" >&2
  exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# Extract appimagetool from the cached linuxdeploy appimage plugin.
mkdir -p "$WORK/tool"
( cd "$WORK/tool" && "$APPIMAGETOOL_SRC" --appimage-extract >/dev/null )
APPIMAGETOOL="$WORK/tool/squashfs-root/appimagetool-prefix/AppRun"
chmod +x "$APPIMAGETOOL"

# Unpack the bundled AppImage into an AppDir.
( cd "$WORK" && "$APPIMAGE" --appimage-extract >/dev/null )
APPDIR="$WORK/squashfs-root"

removed=0
for dir in "$APPDIR/usr/lib" "$APPDIR/usr/lib/x86_64-linux-gnu"; do
  [ -d "$dir" ] || continue
  for glob in "${STRIP_GLOBS[@]}"; do
    for f in "$dir"/$glob; do
      [ -e "$f" ] || continue
      echo "  removing $(basename "$f")"
      rm -f "$f"
      removed=$((removed + 1))
    done
  done
done
echo "Removed $removed bundled graphics libraries."

# Repackage in place. appimagetool fetches the AppImage runtime itself; bundle
# time already has network access for downloading the linuxdeploy tooling.
ARCH=x86_64 "$APPIMAGETOOL" --no-appstream "$APPDIR" "$APPIMAGE" >/dev/null
echo "Repackaged $APPIMAGE"
