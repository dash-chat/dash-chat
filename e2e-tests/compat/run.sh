#!/usr/bin/env bash
# Backwards compatibility E2E test orchestrator.
#
# Usage: ./run.sh [version-tag ...]
#
# With no arguments, tests current-vs-current (smoke test).
# With version tags, checks out each tag, builds it, then:
#   Phase 1 (setup): creates data with the old binary
#   Phase 2 (verify): verifies data with the current binary
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
        xvfb-run "$@"
    else
        "$@"
    fi
}

# --- Helpers ---

die() { echo "ERROR: $*" >&2; exit 1; }

require_clean_tree() {
    if ! git diff --quiet HEAD 2>/dev/null; then
        die "Working tree is dirty. Commit or stash changes before running compat tests."
    fi
    if ! git diff --cached --quiet HEAD 2>/dev/null; then
        die "Staged changes found. Commit or stash before running compat tests."
    fi
}

allocate_port() {
    node -e "const s=require('net').createServer();s.listen(0,()=>{console.log(s.address().port);s.close()})"
}

run_wdio() {
    # wdio must be run from e2e-tests/ so it resolves node_modules correctly
    (cd "$E2E_DIR" && maybe_xvfb npx wdio run "$E2E_DIR/compat/wdio.compat.ts")
}

# --- Parse args ---

TAGS=("$@")
ORIGINAL_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$ORIGINAL_BRANCH" = "HEAD" ]; then
    ORIGINAL_BRANCH=$(git rev-parse HEAD)
fi

# If version tags are given, we need a clean tree for git checkout
if [ ${#TAGS[@]} -gt 0 ]; then
    require_clean_tree
fi

# --- Step 1: Build current version ---

echo "=== Building current version ==="
mkdir -p "$BINARIES_DIR/current"

pnpm install && pnpm --recursive build && pnpm tauri build --debug --no-bundle

BINARY_PATH="$ROOT/target/debug/dash-chat"
[ -f "$BINARY_PATH" ] || die "Current binary not found at $BINARY_PATH"
cp "$BINARY_PATH" "$BINARIES_DIR/current/dash-chat"
echo "Current binary: $BINARIES_DIR/current/dash-chat"

# --- If no tags, test current-vs-current ---

if [ ${#TAGS[@]} -eq 0 ]; then
    TAGS=("current")
fi

# --- Process each tag ---

FAILED_TAGS=()
PASSED_TAGS=()

for TAG in "${TAGS[@]}"; do
    echo ""
    echo "========================================"
    echo "=== Testing compatibility with $TAG ==="
    echo "========================================"

    TAG_BINARY_DIR="$BINARIES_DIR/$TAG"
    mkdir -p "$TAG_BINARY_DIR"

    if [ "$TAG" = "current" ]; then
        # Already built — nothing to do
        :
    else
        # --- Build old version ---

        echo "--- Checking out $TAG ---"
        git checkout "$TAG" 2>/dev/null || { echo "SKIP: tag $TAG not found"; FAILED_TAGS+=("$TAG"); continue; }

        echo "--- Building $TAG ---"
        (pnpm install && pnpm --recursive build && pnpm tauri build --debug --no-bundle) || {
            echo "SKIP: build failed for $TAG"
            git checkout "$ORIGINAL_BRANCH" 2>/dev/null
            FAILED_TAGS+=("$TAG")
            continue
        }

        [ -f "$BINARY_PATH" ] || { echo "SKIP: binary not found for $TAG"; git checkout "$ORIGINAL_BRANCH" 2>/dev/null; FAILED_TAGS+=("$TAG"); continue; }
        cp "$BINARY_PATH" "$TAG_BINARY_DIR/dash-chat"

        echo "--- Returning to $ORIGINAL_BRANCH ---"
        git checkout "$ORIGINAL_BRANCH" 2>/dev/null || die "Failed to return to $ORIGINAL_BRANCH"

        # Restore current node_modules after switching back
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
    for _ in $(seq 1 30); do
        if curl -s "$MAILBOX_URL" >/dev/null 2>&1; then
            break
        fi
        sleep 1
    done

    # --- Phase 1 — Setup with old binary ---

    echo "--- Phase 1: Creating data with $TAG ---"
    chmod +x "$TAG_BINARY_DIR/dash-chat"
    chmod +x "$E2E_DIR/compat/scripts/"*.sh

    PHASE1_OK=true
    COMPAT_BINARY="$TAG_BINARY_DIR/dash-chat" \
    COMPAT_PHASE=setup \
    MAILBOX_URL="$MAILBOX_URL" \
    SKIP_BUILD=1 \
        run_wdio || PHASE1_OK=false

    if [ "$PHASE1_OK" != "true" ]; then
        echo "FAIL: Phase 1 (setup) failed for $TAG"
        kill "$MAILBOX_PID" 2>/dev/null || true
        wait "$MAILBOX_PID" 2>/dev/null || true
        FAILED_TAGS+=("$TAG")
        continue
    fi

    # --- Phase 2 — Verify with current binary ---

    echo "--- Phase 2: Verifying with current version ---"
    PHASE2_OK=true
    COMPAT_BINARY="$BINARIES_DIR/current/dash-chat" \
    COMPAT_PHASE=verify \
    MAILBOX_URL="$MAILBOX_URL" \
    SKIP_BUILD=1 \
        run_wdio || PHASE2_OK=false

    # --- Cleanup ---

    kill "$MAILBOX_PID" 2>/dev/null || true
    wait "$MAILBOX_PID" 2>/dev/null || true

    if [ "$PHASE2_OK" = "true" ]; then
        echo "PASS: $TAG is backwards compatible"
        PASSED_TAGS+=("$TAG")
    else
        echo "FAIL: Phase 2 (verify) failed for $TAG"
        FAILED_TAGS+=("$TAG")
    fi

    rm -rf "$COMPAT_DB_DIR"
done

# --- Summary ---

echo ""
echo "========================================"
echo "=== Backwards Compatibility Results ==="
echo "========================================"

if [ ${#PASSED_TAGS[@]} -gt 0 ]; then
    echo "PASSED: ${PASSED_TAGS[*]}"
fi

if [ ${#FAILED_TAGS[@]} -gt 0 ]; then
    echo "FAILED: ${FAILED_TAGS[*]}"
    exit 1
fi

echo "All versions passed!"
