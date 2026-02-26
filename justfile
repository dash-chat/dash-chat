alias d := dev

# start two dash chat instances connected to a local mailbox server
dev:
    mprocs

test:
    cd crates/dashchat-node && cargo nextest run

# uninstall the android app via adb
uninstall-android:
    adb uninstall studio.darksoil.dashchat

# clean all paths used by the dev environment
clean-dev:
    -@adb uninstall studio.darksoil.dashchat 2>/dev/null
    rm -rf .dev-dbs

# builds dash chat as a CLI binary
build-binary:
    pnpm -F ./packages/stores build
    pnpm -F ./ui build
    cargo build --bins --release --locked --features tauri/custom-protocol,tauri/native-tls    

# builds and runs dash chat as a binary
run-binary: build-binary
    ./target/release/dash-chat

# shows the logs for a connected android device running the app
logcat:
    adb logcat | grep -F "`adb shell ps | grep studio.darksoil.dashchat | tr -s [:space:] ' ' | cut -d' ' -f2`"

# cut a new release (e.g. just release 0.11.0)
release version:
    ./scripts/release.sh {{version}}
