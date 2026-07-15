#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

spec_name="${1:-}"
shift || true

retry_attempts=1
wdio_args=()

for arg in "$@"; do
    case "$arg" in
        --no-rerun|--no-retry)
            retry_attempts=0
            ;;
        *)
            wdio_args+=("$arg")
            ;;
    esac
done

if [[ "$spec_name" != "-" && -n "$spec_name" ]]; then
    wdio_args+=(--spec "specs/${spec_name}.spec.ts")
fi

export AGENT_1="${AGENT_1:-desktop}"
export AGENT_2="${AGENT_2:-desktop}"

has_desktop=false
has_android=false
emulators_needed=0
for agent in "$AGENT_1" "$AGENT_2"; do
    case "$agent" in
        desktop) has_desktop=true ;;
        android) has_android=true ;;
        android-emulator)
            has_android=true
            emulators_needed=$((emulators_needed + 1))
            ;;
        *)
            echo "Invalid agent platform '$agent' (expected desktop, android or android-emulator)" >&2
            exit 1
            ;;
    esac
done

# Build only what the combo needs.
if $has_desktop; then
    just test e2e build
fi
if $has_android; then
    just test e2e android-build
fi

# The wdio config spawns this binary for the local mailbox. It must be built
# here, with the default toolchain: a `cargo` run inside the androidDev shell
# would rebuild everything with the android toolchain.
cargo build -p mailbox-server

# adb lives in the androidDev shell; the wrapper layers over the caller's
# default dev shell, so tauri-driver/pnpm stay on PATH for mixed combos.
android_shell() {
    nix develop "git+file:$ROOT#androidDev" --command "$@"
}

if $has_android; then
    devices="$(android_shell adb devices)"

    if grep -q "unauthorized$" <<< "$devices"; then
        echo "Note: an unauthorized device is connected — accept its USB" \
            "debugging prompt to use it."
    fi

    # Boot a headless emulator for each android-emulator agent that doesn't
    # have a running emulator yet. Emulators stay running across runs (kill
    # with `just android kill-emulator`).
    emulators_running="$(grep -c "^emulator-.*device$" <<< "$devices" || true)"
    while (( emulators_running < emulators_needed )); do
        bash "$ROOT/scripts/android-emulator.sh"
        emulators_running=$((emulators_running + 1))
    done
fi

cd "$ROOT/e2e-tests"

if $has_android; then
    export APPIUM_HOME="$PWD/.appium"

    # The uiautomator2 driver lives in APPIUM_HOME (not node_modules); install
    # it on first run. Pinned to the last version compatible with appium 2.x.
    if ! pnpm exec appium driver list --installed 2>&1 | grep -q uiautomator2; then
        pnpm exec appium driver install uiautomator2@4.2.9
    fi
fi

export E2E_SPEC_FILE_RETRIES="$retry_attempts"
if $has_android; then
    android_shell pnpm wdio run wdio.conf.ts "${wdio_args[@]}"
else
    pnpm wdio run wdio.conf.ts "${wdio_args[@]}"
fi
