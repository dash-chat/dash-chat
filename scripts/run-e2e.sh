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
for agent in "$AGENT_1" "$AGENT_2"; do
    case "$agent" in
        desktop) has_desktop=true ;;
        android|android-emulator) has_android=true ;;
        *)
            echo "Invalid agent platform '$agent' (expected desktop, android or android-emulator)" >&2
            exit 1
            ;;
    esac
done

# The desktop binary and the mailbox server must be built here, with the
# default toolchain: a `cargo` run inside the androidDev shell would rebuild
# everything with the android toolchain. Everything else Android-specific
# (device detection, APK builds, emulator boot, appium setup) lives in
# e2e-tests/setup/platforms/android.ts.
if $has_desktop; then
    just test e2e build
fi
cargo build -p mailbox-server

cd "$ROOT/e2e-tests"

export E2E_SPEC_FILE_RETRIES="$retry_attempts"
if $has_android; then
    # adb lives in the androidDev shell; it layers over the caller's default
    # dev shell, so tauri-driver/pnpm stay on PATH for mixed combos.
    nix develop "git+file:$ROOT#androidDev" --command pnpm wdio run wdio.conf.ts "${wdio_args[@]}"
else
    pnpm wdio run wdio.conf.ts "${wdio_args[@]}"
fi
