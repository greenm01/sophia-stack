#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
hagia_bin="${SOPHIA_HAGIA_BIN:-$(command -v hagia || true)}"
kitty_bin="${SOPHIA_TERMINAL_BIN:-$(command -v kitty || true)}"
firefox_bin="${SOPHIA_FIREFOX_BIN:-$(command -v firefox || true)}"
display="${SOPHIA_HAGIA_LIVE_DISPLAY:-:288}"
evidence="${SOPHIA_HAGIA_LIVE_EVIDENCE:-/tmp/sophia-hagia-live-session.log}"

if [[ -z "$hagia_bin" || ! -x "$hagia_bin" ]]; then
    echo "set SOPHIA_HAGIA_BIN to a built Hagia executable" >&2
    exit 1
fi
if [[ -z "$kitty_bin" || ! -x "$kitty_bin" ]]; then
    echo "real Kitty is required; set SOPHIA_TERMINAL_BIN" >&2
    exit 1
fi
if [[ -z "$firefox_bin" || ! -x "$firefox_bin" ]]; then
    echo "the retained Hagia revision-1 profile requires Firefox" >&2
    exit 1
fi

proof_dir="$(mktemp -d)"
trap 'rm -f "$proof_dir/restarted"; rmdir "$proof_dir"' EXIT

cd "$ROOT_DIR"
cargo build --quiet --offline -p sophia-cli --features native-session

set +e
SOPHIA_HAGIA_BIN="$hagia_bin" \
SOPHIA_HAGIA_RESTART_MARKER="$proof_dir/restarted" \
SOPHIA_HAGIA_FAULT_AFTER=checkpoint_saved \
SOPHIA_HAGIA_FAULT_OCCURRENCE=3 \
SOPHIA_HAGIA_FAULT_DELAY_MSEC=200 \
target/debug/sophia session run \
    --session-mode=normal \
    "--session-app=terminal=$kitty_bin" \
    --session-start=terminal \
    --session-action-app=terminal=terminal \
    "--session-app=firefox=$firefox_bin" \
    --session-action-app=browser=firefox \
    --session-app-arg=terminal=--config \
    --session-app-arg=terminal=NONE \
    --session-app-arg=terminal=--override \
    --session-app-arg=terminal=linux_display_server=x11 \
    --session-app-arg=terminal=--override \
    --session-app-arg=terminal=remember_window_size=no \
    "--display=$display" \
    --max-runtime-ms=10000 \
    --startup-ready-timeout-ms=6000 \
    --wm-process="$ROOT_DIR/tools/fixtures/hagia_restart_once.sh" \
    --wm-interface=sophia_wm_v1 \
    >"$evidence" 2>&1
status=$?
set -e
cat "$evidence"
if (( status != 0 )); then
    exit "$status"
fi

grep -Eq '^sophia_live_wm schema=4 status=restarted adapter=sophia_wm_v1 epoch=2 restarts=1 preserved_layout=true$' "$evidence"
grep -Eq '^hagia_policy_checkpoint schema=1 status=saved candidate_nonempty=true$' "$evidence"
grep -Eq '^hagia_policy_checkpoint schema=1 status=loaded candidate_nonempty=true$' "$evidence"
grep -Eq '^hagia_policy_checkpoint schema=1 status=reconciled candidate_nonempty=true$' "$evidence"
grep -Eq '^hagia_policy_refresh schema=1 status=requested reason=checkpoint_reconciled policy_generation=2 outputs=[1-9][0-9]*$' "$evidence"
grep -Eq '^sophia_live_session_startup schema=2 status=ready ' "$evidence"
grep -Eq '^sophia_live_session_health schema=1 status=clean ' "$evidence"
grep -Eq '^sophia_live_layout_health schema=2 status=clean ' "$evidence"

constraint_reconciliations="$(grep -Ec '^sophia_live_wm schema=1 status=constraints_reconciled ' "$evidence")"
layout_commits="$(grep -Ec '^sophia_live_wm schema=1 status=layout_committed ' "$evidence")"
checkpoint_saves="$(grep -Ec '^hagia_policy_checkpoint schema=1 status=saved ' "$evidence")"
if (( constraint_reconciliations != 1 || layout_commits > 8 || checkpoint_saves > 8 )); then
    echo "Hagia admission/restart cycle did not remain bounded: constraints=$constraint_reconciliations layouts=$layout_commits checkpoints=$checkpoint_saves" >&2
    exit 1
fi
