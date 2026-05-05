# start two dash chat instances connected to a local mailbox server
mod dev 'scripts/dev.just'

# android development 
mod android 'scripts/android.just'

# running tests
mod test 'scripts/test.just'

# push notifications
mod push 'scripts/push.just'

# ios development 
mod ios 'scripts/ios.just'

# build dash chat as a binary
build:
    pnpm tauri build --no-bundle

# build dash chat as an installer (AppImage on linux)
bundle:
    pnpm tauri build

# cut a new release (e.g. just release 0.11.0)
release version:
    ./scripts/release.sh {{version}}
