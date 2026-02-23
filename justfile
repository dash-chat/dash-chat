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
