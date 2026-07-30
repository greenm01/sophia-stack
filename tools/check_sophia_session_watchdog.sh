#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT="$(mktemp -d /tmp/sophia-session-watchdog.XXXXXX)"
trap 'rm -rf -- "$TEST_ROOT"' EXIT

mkdir -p "$TEST_ROOT/runtime" "$TEST_ROOT/state"
export XDG_RUNTIME_DIR="$TEST_ROOT/runtime"
export XDG_STATE_HOME="$TEST_ROOT/state"
export SOPHIA_BIN="$ROOT_DIR/tools/fixtures/fake_sophia_session_watchdog.sh"
export SOPHIA_NATIVE_WM_BIN=/bin/true
export SOPHIA_TTY_MODE_HELPER="$ROOT_DIR/tools/fixtures/fake_sophia_tty_mode.py"
export SOPHIA_STANDALONE_APP_BIN=/bin/true
export SOPHIA_TTY_PROFILE=standalone
export SOPHIA_STANDALONE_WORKLOAD=vkcube
export SOPHIA_BUILD_SESSION=false
export SOPHIA_MANAGE_KEYD=false
export SOPHIA_SESSION_WATCHDOG_SECONDS=1

set +e
script -qefc "$ROOT_DIR/tools/run_sophia_xmonad_session.sh" /dev/null \
    >"$TEST_ROOT/launcher.log" 2>&1
status=$?
set -e

if [[ "$status" -ne 124 ]]; then
    echo "Expected watchdog exit status 124; observed $status." >&2
    sed -n '1,220p' "$TEST_ROOT/launcher.log" >&2
    exit 1
fi

SESSION_DIR="$XDG_STATE_HOME/sophia/standalone-session"
rg -q 'result=deadline_exceeded .*action=terminate_process_group' \
    "$SESSION_DIR/session.log"
rg -q 'emergency=true session_shutdown=watchdog_term' \
    "$SESSION_DIR/recovery.log"
rg -q 'exit_status=124 emergency=true handoff=display_manager' \
    "$SESSION_DIR/lifecycle.log"
[[ ! -e "$XDG_RUNTIME_DIR/sophia-standalone-session-$UID/wrapper.pid" ]]

echo "Sophia external session watchdog regression passed"
