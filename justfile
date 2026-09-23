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

# build digital ocean droplet images and create droplets
mod droplet 'scripts/droplet.just'

# build docker images for the mailbox, LAN mailbox and push notifications servers
mod docker 'scripts/docker.just'

# Show available recipes.
_default:
    @just --list --list-submodules

# build dash chat as a binary
build:
    pnpm tauri build --no-bundle --debug

# run the binary produced by `just build`
run:
    ./target/debug/dash-chat

# delete the data dirs of the binary run by `just run`, after confirmation
wipe:
    #!/usr/bin/env bash
    set -euo pipefail
    id=studio.darksoil.dashchat
    if [ -n "${DATA_DIR:-}" ]; then
        candidates=("$DATA_DIR")
    elif [ "$(uname)" = Darwin ]; then
        candidates=("$HOME/Library/Application Support/$id" "$HOME/Library/Caches/$id" "$HOME/Library/WebKit/$id")
    else
        candidates=("${XDG_DATA_HOME:-$HOME/.local/share}/$id" "${XDG_CACHE_HOME:-$HOME/.cache}/$id" "${XDG_CONFIG_HOME:-$HOME/.config}/$id")
    fi
    dirs=()
    for dir in "${candidates[@]}"; do
        [ -e "$dir" ] && dirs+=("$dir")
    done
    if [ ${#dirs[@]} -eq 0 ]; then
        echo "No Dash Chat data dirs found."
        exit 0
    fi
    echo "Found Dash Chat data dirs:"
    printf '  %s\n' "${dirs[@]}"
    read -rp "Delete them? [y/N] " answer
    if [[ "$answer" =~ ^[Yy]([Ee][Ss])?$ ]]; then
        rm -rf "${dirs[@]}"
        echo "Deleted."
    else
        echo "Aborted."
    fi

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

# typecheck every TS package and the rust workspace
check:
    cargo check --workspace
    pnpm -r --if-present check

# regenerate paraglide message exports from source translation files
paraglide:
    pnpm --filter ui paraglide
