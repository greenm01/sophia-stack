#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_sophia_standalone_vkcube.sh"
PASS="$ROOT_DIR/tools/fixtures/standalone_vkcube_verifier_pass.log"
CPU_PASS="$ROOT_DIR/tools/fixtures/standalone_vkcube_verifier_cpu_pass.log"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

SOPHIA_VERIFY_WAIT_SECONDS=0 "$VERIFY" "$PASS" >/dev/null
SOPHIA_VERIFY_WAIT_SECONDS=0 "$VERIFY" "$CPU_PASS" >/dev/null

reject_mutation() {
    local name="$1" pattern="$2" replacement="$3"
    local candidate="$TEMP_DIR/$name.log"
    sed "s/$pattern/$replacement/" "$PASS" >"$candidate"
    if SOPHIA_VERIFY_WAIT_SECONDS=0 "$VERIFY" "$candidate" >/dev/null 2>&1; then
        echo "standalone vkcube verifier accepted mutation: $name" >&2
        exit 1
    fi
}

# This profile runs no window manager, so a session reporting one is not the
# session this proof describes.
reject_mutation managed 'wm_policy=disabled' 'wm_policy=external'
# The exit is the client exiting, not an operator shortcut: without a policy
# client there are no shortcuts to press.
reject_mutation unclean 'sophia_live_session_cleanup schema=1 status=clean' 'sophia_live_session_cleanup schema=1 status=leaked'
reject_mutation unfinished 'status=bounded_complete' 'status=timed_out'
reject_mutation app 'id=standalone' 'id=terminal'
reject_mutation evidence 'evidence=PresentedBuffer' 'evidence=BackingSnapshot'
reject_mutation retirement 'status=retired transaction=42' 'status=retired transaction=43'
reject_mutation cleanup 'native_cleanup_pending=false' 'native_cleanup_pending=true'

cpu_candidate="$TEMP_DIR/cpu-no-native-pixels.log"
sed 's/native_nonzero_exports=12/native_nonzero_exports=0/' "$CPU_PASS" >"$cpu_candidate"
if SOPHIA_VERIFY_WAIT_SECONDS=0 "$VERIFY" "$cpu_candidate" >/dev/null 2>&1; then
    echo "standalone vkcube verifier accepted CPU path without native pixels" >&2
    exit 1
fi

cpu_candidate="$TEMP_DIR/cpu-static-frame.log"
sed 's/authority_transactions=12/authority_transactions=1/' "$CPU_PASS" >"$cpu_candidate"
if SOPHIA_VERIFY_WAIT_SECONDS=0 "$VERIFY" "$cpu_candidate" >/dev/null 2>&1; then
    echo "standalone vkcube verifier accepted a static software frame" >&2
    exit 1
fi

cpu_candidate="$TEMP_DIR/cpu-no-present-feedback.log"
sed 's/present_complete_copy=12/present_complete_copy=0/' "$CPU_PASS" >"$cpu_candidate"
if SOPHIA_VERIFY_WAIT_SECONDS=0 "$VERIFY" "$cpu_candidate" >/dev/null 2>&1; then
    echo "standalone vkcube verifier accepted software Present without completion" >&2
    exit 1
fi

echo "standalone vkcube verifier regressions passed"
