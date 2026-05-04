set working-directory := '..'

# run the app in development mode
dev:
    nix develop git+file:.#androidDev --command pnpm tauri android dev

# install the android app via adb
install:
    nix develop git+file:.#androidDev --command bash -c "adb install src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk"


# build and install the android app via adb
build-and-install:
    nix develop git+file:.#androidDev --command bash -c "pnpm tauri android build --apk && adb install src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk"

# build the android app
build:
    nix develop git+file:.#androidDev --command bash -c "pnpm tauri android build"

# uninstall the android app via adb
uninstall:
    nix develop git+file:.#androidDev --command adb uninstall studio.darksoil.dashchat

# shows the logs for a connected android device running the app
logcat:
    nix develop git+file:.#androidDev --command bash -c 'adb logcat | grep -F "$(adb shell ps | grep studio.darksoil.dashchat | tr -s " " " " | cut -d" " -f2)"'
