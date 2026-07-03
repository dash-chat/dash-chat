#!/usr/bin/env bash
# CI-only: install a compiled linuxdeploy shim into ~/.cache/tauri so that the
# subsequent `tauri build` runs the real linuxdeploy with --exclude-library
# flags that keep libwayland-* out of the AppImage.
#
# Why: the released AppImage bundles the CI runner's libwayland-client.so.0.
# AppRun puts bundled libs ahead of the host's, so a host whose wayland differs
# from the runner's gets the host Mesa libEGL paired with a foreign
# libwayland-client -> eglGetPlatformDisplay returns EGL_BAD_PARAMETER, WebKit's
# GPU process aborts, and the window renders blank. Excluding the wayland client
# stack forces those libraries to resolve from the host at runtime.
set -euo pipefail

ARCH="${ARCH:-x86_64}"
CACHE="$HOME/.cache/tauri"
REAL="$CACHE/linuxdeploy-$ARCH.real.AppImage"
SHIM="$CACHE/linuxdeploy-$ARCH.AppImage"
SRC="$(cd "$(dirname "$0")/.." && pwd)/scripts/linuxdeploy-exclude-wayland.c"

mkdir -p "$CACHE"

url="https://github.com/tauri-apps/binary-releases/releases/download/linuxdeploy/linuxdeploy-$ARCH.AppImage"
echo "Downloading real linuxdeploy -> $REAL"
curl -fsSL "$url" -o "$REAL"
chmod +x "$REAL"

echo "Compiling shim $SRC -> $SHIM"
cc -O2 -o "$SHIM" "$SRC"
chmod +x "$SHIM"

echo "Installed linuxdeploy exclude-wayland shim (real: $REAL)"
