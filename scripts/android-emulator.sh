#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_LINK="$ROOT/e2e-tests/.appium/emulator"

nix build "git+file:$ROOT#android-emulator" --out-link "$OUT_LINK"

# run-test-emulator boots the emulator on the next free port, waits until the
# device is ready, and exits — the emulator itself keeps running. Detach its
# stdio into a log file: the emulator inherits these fds, and holding a
# caller's pipe open would keep the caller's pipeline from ever seeing EOF.
LOG_DIR="$ROOT/.dbs/e2e"
mkdir -p "$LOG_DIR"
echo "Booting headless emulator (log: $LOG_DIR/emulator.log)..."
# run-test-emulator's boot-wait loop has no timeout, so if the emulator
# crashes at startup it hangs forever — bound it and surface the log.
if ! NIX_ANDROID_EMULATOR_FLAGS="${NIX_ANDROID_EMULATOR_FLAGS:--no-window -no-audio -no-boot-anim -gpu swiftshader_indirect}" \
    timeout 600 "$OUT_LINK/bin/run-test-emulator" < /dev/null >> "$LOG_DIR/emulator.log" 2>&1; then
    echo "Emulator failed to boot within 10 minutes. Log tail:" >&2
    tail -100 "$LOG_DIR/emulator.log" >&2
    exit 1
fi
echo "Emulator ready."
