#!/usr/bin/env bash
# Backwards compatibility E2E test orchestrator.
#
# Usage: ./run.sh <git-ref> [git-ref ...]
#
# Accepts any git ref (tag, branch, commit hash).
#
# For each ref:
#   1. Builds the current version
#   2. Checks out the ref, builds it
#   3. Returns to the original branch
#   4. Starts a local mailbox server
#   5. Runs Phase 1 (setup) with the ref's binary
#   6. Runs Phase 2 (verify) with the current binary
#   7. Cleans up
#
# Environment:
#   - Assumes all build tools (pnpm, cargo, etc.) are already available
#     (run inside `nix develop` on NixOS, or with tools installed on CI)
#   - Uses `xvfb-run` if DISPLAY is unset (headless CI)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

BINARIES_DIR="$ROOT/.e2e-binaries"
COMPAT_DB_DIR="$ROOT/.dbs/compat"
E2E_DIR="$ROOT/e2e-tests"

# --- Display wrapper: xvfb-run if headless ---

maybe_xvfb() {
    if [ -z "${DISPLAY:-}" ] && command -v xvfb-run &>/dev/null; then
        # dbus-run-session avoids ~35s/launch stalls on headless CI where
        # xdg-desktop-portal activation hangs on the systemd user bus.
        if command -v dbus-run-session &>/dev/null; then
            dbus-run-session -- xvfb-run "$@"
        else
            xvfb-run "$@"
        fi
    else
        "$@"
    fi
}

# --- Helpers ---

die() { echo "ERROR: $*" >&2; exit 1; }

MAILBOX_PID=""
cleanup() {
    if [ -n "$MAILBOX_PID" ]; then
        kill "$MAILBOX_PID" 2>/dev/null || true
        wait "$MAILBOX_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

require_clean_tree() {
    if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
        die "Working tree is dirty (modified or untracked files). Commit or stash changes before running compat tests."
    fi
}

allocate_port() {
    node -e "const s=require('net').createServer();s.listen(0,()=>{console.log(s.address().port);s.close()})"
}

run_wdio() {
    # wdio must be run from e2e-tests/ so it resolves node_modules correctly
    (cd "$E2E_DIR" && maybe_xvfb npx wdio run "$E2E_DIR/compat/wdio.compat.ts")
}

# --- Validate args ---

if [ $# -eq 0 ]; then
    echo "Usage: $0 <git-ref> [git-ref ...]"
    echo "Example: $0 v0.10.0"
    echo "Example: $0 HEAD"
    exit 1
fi

REFS=("$@")
ORIGINAL_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$ORIGINAL_BRANCH" = "HEAD" ]; then
    ORIGINAL_BRANCH=$(git rev-parse HEAD)
fi

# Check if any ref requires a checkout (i.e., resolves to a different commit than HEAD)
CURRENT_SHA=$(git rev-parse HEAD)
NEEDS_CHECKOUT=false
for REF in "${REFS[@]}"; do
    REF_SHA=$(git rev-parse "$REF" 2>/dev/null || echo "")
    if [ "$REF_SHA" != "$CURRENT_SHA" ]; then
        NEEDS_CHECKOUT=true
        break
    fi
done

if [ "$NEEDS_CHECKOUT" = "true" ]; then
    require_clean_tree
fi

# --- Step 1: Build current version ---

echo "=== Building current version ==="
mkdir -p "$BINARIES_DIR/current"

pnpm install && pnpm --recursive build && pnpm tauri build --debug --no-bundle --features e2e-tests

BINARY_PATH="$ROOT/target/debug/dash-chat"
[ -f "$BINARY_PATH" ] || die "Current binary not found at $BINARY_PATH"
cp "$BINARY_PATH" "$BINARIES_DIR/current/dash-chat"
echo "Current binary: $BINARIES_DIR/current/dash-chat"

# --- Process each ref ---

FAILED_REFS=()
PASSED_REFS=()

for REF in "${REFS[@]}"; do
    # Resolve to short label for display (short hash for commits, name for tags/branches)
    REF_LABEL=$(git log -1 --format='%h' "$REF" 2>/dev/null || echo "$REF")

    echo ""
    echo "========================================"
    echo "=== Testing compatibility with $REF_LABEL ==="
    echo "========================================"

    REF_BINARY_DIR="$BINARIES_DIR/$REF_LABEL"
    mkdir -p "$REF_BINARY_DIR"

    # --- Step 2: Build ref version (or reuse current binary for HEAD) ---

    REF_SHA=$(git rev-parse "$REF" 2>/dev/null || echo "")
    if [ "$REF_SHA" = "$CURRENT_SHA" ]; then
        echo "--- Ref $REF_LABEL is HEAD, reusing current binary ---"
        cp "$BINARIES_DIR/current/dash-chat" "$REF_BINARY_DIR/dash-chat"
    else
        echo "--- Checking out $REF ---"
        git checkout "$REF" 2>/dev/null || { echo "SKIP: ref $REF not found"; FAILED_REFS+=("$REF_LABEL"); continue; }

        echo "--- Building $REF_LABEL ---"
        (pnpm install && pnpm --recursive build && pnpm tauri build --debug --no-bundle --features e2e-tests) || {
            echo "SKIP: build failed for $REF_LABEL"
            git checkout -f "$ORIGINAL_BRANCH" 2>/dev/null
            FAILED_REFS+=("$REF_LABEL")
            continue
        }

        [ -f "$BINARY_PATH" ] || { echo "SKIP: binary not found for $REF_LABEL"; git checkout -f "$ORIGINAL_BRANCH" 2>/dev/null; FAILED_REFS+=("$REF_LABEL"); continue; }
        cp "$BINARY_PATH" "$REF_BINARY_DIR/dash-chat"

        # Return to original branch and clean stale Rust artifacts.
        # Force checkout: pnpm install on the old ref may have modified
        # tracked files (e.g. pnpm-lock.yaml), making a normal checkout fail.
        git checkout -f "$ORIGINAL_BRANCH" 2>/dev/null
        cargo clean -p dash-chat -p dashchat-node
        pnpm install
    fi

    # --- Clean compat data dir ---

    rm -rf "$COMPAT_DB_DIR"
    mkdir -p "$COMPAT_DB_DIR"

    # --- Start mailbox server ---

    MAILBOX_PORT=$(allocate_port)
    MAILBOX_URL="http://localhost:$MAILBOX_PORT"
    MAILBOX_DB="$COMPAT_DB_DIR/mailbox-server/mailbox.db"
    mkdir -p "$(dirname "$MAILBOX_DB")"

    echo "--- Starting mailbox server on $MAILBOX_URL ---"
    cargo run -p mailbox-server -- --db-path "$MAILBOX_DB" --addr "0.0.0.0:$MAILBOX_PORT" &
    MAILBOX_PID=$!

    # Wait for mailbox server to be ready
    MAILBOX_READY=false
    for _ in $(seq 1 30); do
        if curl -s "$MAILBOX_URL" >/dev/null 2>&1; then
            MAILBOX_READY=true
            break
        fi
        sleep 1
    done

    if [ "$MAILBOX_READY" != "true" ]; then
        echo "FAIL: Mailbox server failed to start for $REF_LABEL"
        kill "$MAILBOX_PID" 2>/dev/null || true
        wait "$MAILBOX_PID" 2>/dev/null || true
        MAILBOX_PID=""
        FAILED_REFS+=("$REF_LABEL")
        continue
    fi

    # --- Step 5: Phase 1 — Setup with ref binary ---

    echo "--- Phase 1: Creating data with $REF_LABEL ---"
    chmod +x "$REF_BINARY_DIR/dash-chat"
    chmod +x "$E2E_DIR/compat/scripts/"*.sh

    PHASE1_OK=true
    COMPAT_BINARY="$REF_BINARY_DIR/dash-chat" \
    COMPAT_PHASE=setup \
    MAILBOX_URL="$MAILBOX_URL" \
        run_wdio || PHASE1_OK=false

    if [ "$PHASE1_OK" != "true" ]; then
        echo "FAIL: Phase 1 (setup) failed for $REF_LABEL"
        kill "$MAILBOX_PID" 2>/dev/null || true
        wait "$MAILBOX_PID" 2>/dev/null || true
        MAILBOX_PID=""
        FAILED_REFS+=("$REF_LABEL")
        continue
    fi

    # --- Phase 2 — Verify with current binary ---

    echo "--- Phase 2: Verifying with current version ---"
    PHASE2_OK=true
    COMPAT_BINARY="$BINARIES_DIR/current/dash-chat" \
    COMPAT_PHASE=verify \
    MAILBOX_URL="$MAILBOX_URL" \
        run_wdio || PHASE2_OK=false

    # --- Cleanup ---

    kill "$MAILBOX_PID" 2>/dev/null || true
    wait "$MAILBOX_PID" 2>/dev/null || true
    MAILBOX_PID=""

    if [ "$PHASE2_OK" = "true" ]; then
        echo "PASS: $REF_LABEL is backwards compatible"
        PASSED_REFS+=("$REF_LABEL")
    else
        echo "FAIL: Phase 2 (verify) failed for $REF_LABEL"
        FAILED_REFS+=("$REF_LABEL")
    fi

    rm -rf "$COMPAT_DB_DIR"
done

# --- Summary ---

echo ""
echo "========================================"
echo "=== Backwards Compatibility Results ==="
echo "========================================"

if [ ${#PASSED_REFS[@]} -gt 0 ]; then
    echo "PASSED: ${PASSED_REFS[*]}"
fi

if [ ${#FAILED_REFS[@]} -gt 0 ]; then
    echo "FAILED: ${FAILED_REFS[*]}"
    exit 1
fi

echo "All versions passed!"
