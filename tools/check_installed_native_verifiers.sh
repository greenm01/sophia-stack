#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

checks=(
    tools/check_installed_login_cycle_verifier.sh
    tools/check_installed_xterm_verifier.sh
    tools/check_truecolor_verifier.sh
    tools/check_installed_fallback_verifier.sh
    tools/check_installed_native_chrome_verifier.sh
    tools/check_installed_session_lifecycle_verifier.sh
    tools/check_installed_watchdog_recovery.sh
    tools/check_installed_hagia_ledger.sh
)
for check in "${checks[@]}"; do
    "$check"
done

echo "installed native verifier fixtures passed"
