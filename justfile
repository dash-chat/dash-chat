alias d := dev

# start two dash chat instances connected to a local mailbox server
dev:
    mprocs
 
# uninstall the android app via adb
uninstall-android:
    adb uninstall studio.darksoil.dashchat

# clean all paths used by the dev environment
clean-dev:
    -just uninstall-android
    rm -rf .dev-dbs
