#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_LINK="$ROOT/e2e-tests/.appium/emulator"

nix build --impure --expr '(import (builtins.getFlake "nixpkgs") { system = "x86_64-linux"; config = { allowUnfree = true; android_sdk.accept_license = true; }; }).androidenv.emulateApp { name = "dash-chat-e2e-emulator"; platformVersion = "35"; abiVersion = "x86_64"; systemImageType = "google_apis"; }' --out-link "$OUT_LINK"

# run-test-emulator boots the emulator on the next free port, waits until the
# device is ready, and exits — the emulator itself keeps running. Detach its
# stdio into a log file: the emulator inherits these fds, and holding a
# caller's pipe open would keep the caller's pipeline from ever seeing EOF.
LOG_DIR="$ROOT/.dbs/e2e"
mkdir -p "$LOG_DIR"
echo "Booting headless emulator (log: $LOG_DIR/emulator.log)..."
NIX_ANDROID_EMULATOR_FLAGS="${NIX_ANDROID_EMULATOR_FLAGS:--no-window -no-audio -no-boot-anim -gpu swiftshader_indirect}" \
    "$OUT_LINK/bin/run-test-emulator" < /dev/null >> "$LOG_DIR/emulator.log" 2>&1
echo "Emulator ready."
