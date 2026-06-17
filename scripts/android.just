set working-directory := '..'

# run the app in development mode, pointing the app at UI_PORT if it is set
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -n "${UI_PORT:-}" ]; then
        nix develop git+file:.#androidDev --command pnpm tauri android dev --config "{\"build\":{\"devUrl\":\"http://localhost:$UI_PORT\"}}"
    else
        nix develop git+file:.#androidDev --command pnpm tauri android dev
    fi

# install the android app via adb
install:
    nix develop git+file:.#androidDev --command bash -c "adb install src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk"

# build the android app in an APK format
build TARGETS='aarch64 armv7 i686 x86_64':
    nix develop git+file:.#androidDev --command bash -c "pnpm tauri android build --apk -t {{TARGETS}}"

# build the android app in AAB format
bundle:
    nix develop git+file:.#androidDev --command bash -c "pnpm tauri android build"

# uninstall the android app via adb
uninstall:
    nix develop git+file:.#androidDev --command adb uninstall studio.darksoil.dashchat

# shows the logs for a connected android device running the app
logcat:
    nix develop git+file:.#androidDev --command bash -c 'adb logcat | grep -F "$(adb shell ps | grep studio.darksoil.dashchat | tr -s " " " " | cut -d" " -f2)"'
