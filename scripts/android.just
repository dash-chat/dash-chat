set working-directory := '..'

# run the app in development mode. --host makes Tauri set TAURI_DEV_HOST and
# serve the dev URL on the LAN IP so a physical device can reach Vite + HMR.
dev:
    nix develop git+file:.#androidDev --command pnpm tauri android dev --host

# install a release build via adb
install:
    nix develop git+file:.#androidDev --command bash -c "adb install src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk"

# install a debug build via adb
install-debug:
    nix develop git+file:.#androidDev --command bash -c "adb install src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk"

# build the android app in an APK format
build TARGETS='aarch64 armv7 i686 x86_64':
    nix develop git+file:.#androidDev --command bash -c "pnpm tauri android build --apk -t {{TARGETS}}"

# build the android app in an APK format in debug mode
build-debug TARGETS='aarch64 armv7 i686 x86_64':
    nix develop git+file:.#androidDev --command bash -c "pnpm tauri android build --debug --apk -t {{TARGETS}}"

# build the android app in AAB format
bundle:
    nix develop git+file:.#androidDev --command bash -c "pnpm tauri android build"

# boot a headless Android emulator (keeps running in the background); it then
# counts as a connected device, e.g. for `PLATFORMS=android-emulator just test e2e`
boot-emulator:
    nix run .#boot-emulator

# kill all running Android emulators
kill-emulator:
    nix develop git+file:.#androidDev --command bash -c 'for e in $(adb devices | grep "^emulator-" | cut -f1); do adb -s "$e" emu kill; done'

# uninstall the android app via adb
uninstall:
    nix develop git+file:.#androidDev --command adb uninstall studio.darksoil.dashchat

# shows the logs for a connected android device running the app
logcat:
    nix develop git+file:.#androidDev --command bash -c 'adb logcat | grep -F "$(adb shell ps | grep studio.darksoil.dashchat | tr -s " " " " | cut -d" " -f2)"'
