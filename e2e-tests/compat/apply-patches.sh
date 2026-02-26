#!/usr/bin/env bash
# Apply patches to an old checkout so it supports E2E testing infrastructure.
#
# Patches applied:
#   1. filesystem.rs  — DATA_DIR env var override
#   2. lib.rs         — E2E_TEST bypass for single-instance plugin
#   3. +layout.svelte — Remove import.meta.env.DEV guard on registerTestUtils()
#   4. setup.rs       — MAILBOX_URL env var override
#
# Each patch is idempotent: if the target already contains the patched code
# (e.g. the current version), it's silently skipped.

set -euo pipefail

# ROOT can be passed as first argument (needed when script is copied to a temp dir)
ROOT="${1:-$(cd "$(dirname "$0")/../.." && pwd)}"

fail() { echo "PATCH FAILED: $1" >&2; exit 1; }

# ---------------------------------------------------------------------------
# 1. filesystem.rs — Add DATA_DIR env var check
# ---------------------------------------------------------------------------
FS="$ROOT/src-tauri/src/filesystem.rs"
if grep -q 'std::env::var("DATA_DIR")' "$FS" 2>/dev/null; then
    echo "  [skip] filesystem.rs already has DATA_DIR support"
else
    # Insert DATA_DIR check into local_data_dir(). We look for the line that
    # calls self.0.path().local_data_dir() and wrap it with an env check.
    if grep -q 'self\.0\.path()\.local_data_dir()' "$FS"; then
        sed -i '/pub fn local_data_dir/,/^    }/ {
            /let local_data_path/,/;/ {
                s|let local_data_path.*=.*self\.0\.path()\.local_data_dir()?;|let local_data_path = if let Ok(data_dir) = std::env::var("DATA_DIR") {\n            PathBuf::from(data_dir)\n        } else {\n            self.0.path().local_data_dir()?\n        };|
            }
        }' "$FS"
        grep -q 'std::env::var("DATA_DIR")' "$FS" || fail "filesystem.rs DATA_DIR patch"
        echo "  [ok] filesystem.rs patched with DATA_DIR support"
    else
        echo "  [skip] filesystem.rs — no recognizable local_data_dir pattern"
    fi
fi

# ---------------------------------------------------------------------------
# 2. lib.rs — Add E2E_TEST bypass before single-instance plugin
# ---------------------------------------------------------------------------
LIB="$ROOT/src-tauri/src/lib.rs"
if grep -q 'E2E_TEST' "$LIB" 2>/dev/null; then
    echo "  [skip] lib.rs already has E2E_TEST bypass"
else
    # Insert E2E_TEST check before the single-instance plugin block.
    # We look for the pattern that registers single_instance and add an
    # else-if branch before it.
    if grep -q 'tauri_plugin_single_instance' "$LIB"; then
        sed -i 's|} else {|} else if std::env::var("E2E_TEST").is_ok() {\n            // E2E tests run multiple built instances side-by-side;\n            // skip single-instance and production-only plugins.\n        } else {|' "$LIB"
        grep -q 'E2E_TEST' "$LIB" || fail "lib.rs E2E_TEST patch"
        echo "  [ok] lib.rs patched with E2E_TEST bypass"
    else
        echo "  [skip] lib.rs — no single-instance plugin found"
    fi
fi

# ---------------------------------------------------------------------------
# 3. +layout.svelte — Unconditionally register test utils
# ---------------------------------------------------------------------------
LAYOUT="$ROOT/ui/src/routes/+layout.svelte"
if grep -q "import\.meta\.env\.DEV" "$LAYOUT" 2>/dev/null; then
    # Remove the DEV guard so test utils are always registered
    sed -i "s|if (import\.meta\.env\.DEV) ||g" "$LAYOUT"
    # Also handle multi-line if blocks
    sed -i '/import\.meta\.env\.DEV/d' "$LAYOUT"
    echo "  [ok] +layout.svelte patched — test utils always registered"
elif grep -q 'registerTestUtils' "$LAYOUT" 2>/dev/null; then
    echo "  [skip] +layout.svelte already registers test utils unconditionally"
else
    # If registerTestUtils isn't imported at all, add it
    # Find the last import line and add after it
    sed -i '/^<\/script>/i\\timport("../../tests/setup-utils").then(({ registerTestUtils }) => registerTestUtils());' "$LAYOUT"
    grep -q 'registerTestUtils' "$LAYOUT" || fail "+layout.svelte registerTestUtils patch"
    echo "  [ok] +layout.svelte patched — added registerTestUtils"
fi

# ---------------------------------------------------------------------------
# 4. setup.rs — Add MAILBOX_URL env var override
# ---------------------------------------------------------------------------
SETUP="$ROOT/src-tauri/src/setup.rs"
if grep -q 'std::env::var("MAILBOX_URL")' "$SETUP" 2>/dev/null; then
    echo "  [skip] setup.rs already has MAILBOX_URL support"
else
    # Replace the mailbox URL construction with env var check.
    # Handle both the is_dev() pattern and direct URL assignment.
    if grep -q 'MAILBOX_PORT' "$SETUP"; then
        # Old pattern: uses MAILBOX_PORT + LOCAL_IP_ADDRESS
        sed -i '/let mailbox_url/,/};/ {
            c\    let mailbox_url = if let Ok(url) = std::env::var("MAILBOX_URL") {\
        url\
    } else {\
        "https://mailbox-server.production.dash-chat.dash-chat.garnix.me".to_string()\
    };
        }' "$SETUP"
        grep -q 'std::env::var("MAILBOX_URL")' "$SETUP" || fail "setup.rs MAILBOX_URL patch"
        echo "  [ok] setup.rs patched with MAILBOX_URL support"
    elif grep -q 'mailbox_url' "$SETUP"; then
        echo "  [skip] setup.rs — has mailbox_url but no MAILBOX_PORT to replace"
    else
        echo "  [skip] setup.rs — no recognizable mailbox URL pattern"
    fi
fi

echo "All patches applied."
