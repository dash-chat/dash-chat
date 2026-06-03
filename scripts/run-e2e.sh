#!/usr/bin/env bash

set -euo pipefail

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

cd e2e-tests
E2E_SPEC_FILE_RETRIES="$retry_attempts" pnpm wdio run wdio.conf.ts "${wdio_args[@]}"