set dotenv-filename := x'.env.${ENV:-}'

# start two dash chat instances connected to a local mailbox server
mod dev 'scripts/dev.just'

# android development
mod android 'scripts/android.just'

# running tests
mod test 'scripts/test.just'

# running e2e tests
mod e2e 'e2e-tests'

# mailbox server
mod mailbox 'scripts/mailbox.just'

# push notifications
mod push 'scripts/push.just'

# ios development
mod ios 'scripts/ios.just'

# build server docker images and create the droplets that run them
mod infra 'scripts/infra.just'

# compile the messages, take the screenshots translators see and sync them to weblate
mod i18n 'scripts/i18n.just'

# Show available recipes.
_default:
    @just --list --list-submodules

# build dash chat as a binary
build:
    pnpm tauri build --no-bundle --debug

# Overrides so a local bundle needs none of the release-only setup CI provides:
bundle-config := '{"bundle":{"createUpdaterArtifacts":false}}'

# build dash chat as an installer (deb and rpm on linux)
bundle:
    pnpm tauri build -b appimage --config '{{ bundle-config }}'

# cut a new release (e.g. just release 0.11.0)
release version:
    ./scripts/release.sh {{version}}

# update the version in all version files without committing (e.g. just update-version 0.11.0)
update-version version:
    ./scripts/update-version.sh {{version}}

# format both UI and rust files
format:
    cargo fmt
    pnpm -r --if-present format
    pnpm exec prettier --write 'scripts/i18n-screenshots/*.ts'

# typecheck every TS package and the rust workspace
check:
    cargo check --workspace
    pnpm -r --if-present check
    pnpm exec tsc --noEmit -p scripts/i18n-screenshots
    pnpm exec prettier --check 'scripts/i18n-screenshots/*.ts'
