#!/usr/bin/env bash
# CI-only: install a compiled linuxdeploy shim AND a patched GTK plugin into
# ~/.cache/tauri so that the subsequent `tauri build` keeps libwayland-* out of
# the AppImage.
#
# Why: the released AppImage bundles the CI runner's libwayland-client.so.0.
# AppRun puts bundled libs ahead of the host's, so a host whose wayland differs
# from the runner's gets the host Mesa libEGL paired with a foreign
# libwayland-client -> eglGetPlatformDisplay returns EGL_BAD_PARAMETER, WebKit's
# GPU process aborts, and the window renders blank. Excluding the wayland client
# stack forces those libraries to resolve from the host at runtime.
#
# Two linuxdeploy invocations pull wayland in, so both must be intercepted:
#   1. The main call `tauri build` makes directly -> handled by the compiled
#      shim (tauri runs it because the file already exists at the cache path).
#   2. The call the GTK plugin makes internally (line ~296 of
#      linuxdeploy-plugin-gtk.sh) via its own $LINUXDEPLOY env var. That value
#      points at the *real* linuxdeploy (resolved from /proc/self/exe after the
#      shim's execv), so the shim never sees it. We instead pre-place a patched
#      copy of the plugin with the --exclude-library flags baked into that call.
#      tauri only downloads the plugin `if !gtk.exists()`, so our copy survives.
set -euo pipefail

ARCH="${ARCH:-x86_64}"
CACHE="$HOME/.cache/tauri"
REAL="$CACHE/linuxdeploy-$ARCH.real.AppImage"
SHIM="$CACHE/linuxdeploy-$ARCH.AppImage"
GTK="$CACHE/linuxdeploy-plugin-gtk.sh"
GSTREAMER="$CACHE/linuxdeploy-plugin-gstreamer.sh"
# Pinned: released tauri-cli downloads this script unpinned from master at
# bundle time; pre-placing a pinned copy also makes the release reproducible.
GSTREAMER_REV="2a2e67491c32995a3f279ad0ecbe77abd512b42a"
SRC="$(cd "$(dirname "$0")/.." && pwd)/scripts/linuxdeploy-exclude-wayland.c"

mkdir -p "$CACHE"

url="https://github.com/tauri-apps/binary-releases/releases/download/linuxdeploy/linuxdeploy-$ARCH.AppImage"
echo "Downloading real linuxdeploy -> $REAL"
curl -fsSL "$url" -o "$REAL"
chmod +x "$REAL"

echo "Compiling shim $SRC -> $SHIM"
cc -O2 -o "$SHIM" "$SRC"
chmod +x "$SHIM"

gtk_url="https://raw.githubusercontent.com/tauri-apps/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh"
echo "Downloading GTK plugin -> $GTK"
curl -fsSL "$gtk_url" -o "$GTK"

echo "Patching GTK plugin to exclude wayland from its linuxdeploy call"
python3 - "$GTK" <<'PY'
import sys

path = sys.argv[1]
with open(path) as f:
    text = f.read()

old = 'env LINUXDEPLOY_PLUGIN_MODE=1 "$LINUXDEPLOY" --appdir="$APPDIR" "${LIBRARIES[@]}"'
excludes = ' '.join(
    f'--exclude-library="{lib}.so*"'
    for lib in (
        "libwayland-client",
        "libwayland-egl",
        "libwayland-cursor",
        "libwayland-server",
    )
)
new = (
    'env LINUXDEPLOY_PLUGIN_MODE=1 "$LINUXDEPLOY" --appdir="$APPDIR" '
    f'{excludes} "${{LIBRARIES[@]}}"'
)

count = text.count(old)
if count != 1:
    sys.exit(
        f"expected exactly one linuxdeploy invocation to patch, found {count}; "
        "the upstream GTK plugin changed and the patch needs updating"
    )

with open(path, "w") as f:
    f.write(text.replace(old, new))
PY

chmod +x "$GTK"

# The gstreamer plugin (run because bundleMediaFramework is enabled) has the
# same internal-linuxdeploy-call problem as the GTK plugin: gst plugins from
# plugins-bad (waylandsink) would pull the wayland stack back in.
gstreamer_url="https://raw.githubusercontent.com/tauri-apps/linuxdeploy-plugin-gstreamer/$GSTREAMER_REV/linuxdeploy-plugin-gstreamer.sh"
echo "Downloading GStreamer plugin -> $GSTREAMER"
curl -fsSL "$gstreamer_url" -o "$GSTREAMER"

echo "Patching GStreamer plugin to exclude wayland from its linuxdeploy call"
python3 - "$GSTREAMER" <<'PY'
import sys

path = sys.argv[1]
with open(path) as f:
    text = f.read()

old = '"$LINUXDEPLOY" --appdir "$APPDIR"\n'
excludes = ' '.join(
    f'--exclude-library="{lib}.so*"'
    for lib in (
        "libwayland-client",
        "libwayland-egl",
        "libwayland-cursor",
        "libwayland-server",
    )
)
# waylandsink could never load with its library excluded, so drop it rather
# than ship a plugin that fails the registry scan on every launch.
new = (
    'rm -f "$plugins_target_dir"/libgstwaylandsink.so\n'
    f'"$LINUXDEPLOY" --appdir "$APPDIR" {excludes}\n'
)

count = text.count(old)
if count != 1:
    sys.exit(
        f"expected exactly one linuxdeploy invocation to patch, found {count}; "
        "the upstream GStreamer plugin changed and the patch needs updating"
    )

with open(path, "w") as f:
    f.write(text.replace(old, new))
PY

chmod +x "$GSTREAMER"

echo "Installed linuxdeploy exclude-wayland shim ($SHIM) and patched GTK ($GTK) and GStreamer ($GSTREAMER) plugins"
