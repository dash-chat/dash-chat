# start two dash chat instances connected to a local mailbox server
mod dev 'scripts/dev.just'

# android development 
mod android 'scripts/android.just'

# running tests
mod test 'scripts/tests.just'

# building and running the tauri app
mod binary 'scripts/binary.just'

# cut a new release (e.g. just release 0.11.0)
release version:
    ./scripts/release.sh {{version}}
