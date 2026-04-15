alias d := dev

# start two dash chat instances connected to a local mailbox server
dev:
    pnpm exec mprocs

test:
    cd crates/dashchat-node && cargo nextest run

e2e-test:
    pnpm -F ./e2e-tests test

# run backwards compatibility tests against version tags (e.g. just compat-test v0.10.0 v0.10.1)
compat-test *TAGS:
    bash e2e-tests/compat/run.sh {{TAGS}}

# clean all paths used by the dev environment
clean-dev:
    -@adb uninstall studio.darksoil.dashchat 2>/dev/null
    rm -rf .dbs/dev

# builds dash chat as a CLI binary
build-binary:
    pnpm -F ./packages/stores build
    pnpm -F ./ui build
    cargo build --bins --release --locked --features tauri/custom-protocol,tauri/native-tls

# builds and runs dash chat as a binary
run-binary: build-binary
    ./target/release/dash-chat

# cut a new release (e.g. just release 0.11.0)
release version:
    ./scripts/release.sh {{version}}

# run the app in development mode
android-dev:
    nix develop git+file:.#androidDev --command pnpm tauri android dev

# build and install the android app via adb
android-install:
    nix develop git+file:.#androidDev --command \
        pnpm tauri android build --apk && \
        adb install src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk

# uninstall the android app via adb
android-uninstall:
    nix develop git+file:.#androidDev --command adb uninstall studio.darksoil.dashchat

# shows the logs for a connected android device running the app
android-logcat:
    nix develop git+file:.#androidDev --command adb logcat | grep -F "`adb shell ps | grep studio.darksoil.dashchat | tr -s [:space:] ' ' | cut -d' ' -f2`"
