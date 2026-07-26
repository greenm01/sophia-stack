#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
RUNTIME_ROOT="${XDG_RUNTIME_DIR:-/tmp}"
COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD)"
PROMOTION_ROOT="$STATE_HOME/sophia/m9-promotion/$COMMIT"

gates=(
    01-native-chrome
    02-external-config
    03-normal
    04-pointer-focus
    05-four-kitty-1
    06-four-kitty-2
    07-four-kitty-3
    08-launch-burst
    09-keyboard-vt
    10-xmobar
    11-emergency
)

fail() {
    echo "Milestone 9 promotion failed: $*" >&2
    exit 1
}

gate_label() {
    printf '%s\n' "${1#??-}"
}

gate_passed() {
    local gate=$1 result="$PROMOTION_ROOT/$gate/result.kdl"
    [[ -s "$result" ]] &&
        grep -Eq "^promotion-result schema=1 gate=\"$(gate_label "$gate")\" commit=\"$COMMIT\" status=\"passed\"$" \
            "$result"
}

print_status() {
    local gate status
    printf 'Milestone 9 physical promotion\n  commit: %s\n  evidence: %s\n' \
        "$COMMIT" "$PROMOTION_ROOT"
    for gate in "${gates[@]}"; do
        if gate_passed "$gate"; then
            status=PASS
        else
            status=PENDING
        fi
        printf '  %-17s %s\n' "$(gate_label "$gate")" "$status"
    done
}

copy_if_present() {
    local source=$1 destination=$2
    [[ -s "$source" ]] && install -m 600 "$source" "$destination"
}

archive_session() {
    local profile=$1 destination=$2
    local source_dir="$STATE_HOME/sophia/${profile}-session"
    [[ -s "$source_dir/session.log" ]] ||
        fail "the $profile session did not produce session.log"
    install -m 600 "$source_dir/session.log" "$destination/session.log"
    copy_if_present "$source_dir/input-guard.log" "$destination/input-guard.log"
    copy_if_present "$source_dir/recovery.log" "$destination/recovery.log"
    copy_if_present "$source_dir/lifecycle.log" "$destination/lifecycle.log"
    copy_if_present "/tmp/sophia-${profile}-tty3-launch.log" "$destination/launch.log"
}

require_archived_file() {
    local path=$1
    [[ -s "$path" ]] || fail "missing archived evidence: $path"
}

verify_gate() {
    local gate=$1 evidence=$2
    local session="$evidence/session.log"
    local guard="$evidence/input-guard.log"
    local recovery="$evidence/recovery.log"
    case "$gate" in
        01-native-chrome)
            "$ROOT_DIR/tools/verify_sophia_native_chrome.sh" \
                "$session" "$evidence/sequence.log"
            ;;
        02-external-config)
            "$ROOT_DIR/tools/verify_sophia_xmonad_config_reload.sh" \
                "$session" "$evidence/sequence.log"
            ;;
        03-normal)
            require_archived_file "$guard"
            require_archived_file "$recovery"
            "$ROOT_DIR/tools/verify_sophia_xmonad_tty3.sh" \
                "$session" "$guard" "$recovery" &&
            "$ROOT_DIR/tools/verify_sophia_xmonad_focused_border.sh" "$session"
            ;;
        04-pointer-focus)
            "$ROOT_DIR/tools/verify_sophia_xmonad_pointer_focus_pair.sh" "$session"
            ;;
        05-four-kitty-1|06-four-kitty-2|07-four-kitty-3)
            "$ROOT_DIR/tools/verify_sophia_xmonad_four_kitty.sh" "$session"
            ;;
        08-launch-burst)
            "$ROOT_DIR/tools/verify_sophia_xmonad_launch_burst.sh" "$session"
            ;;
        09-keyboard-vt)
            "$ROOT_DIR/tools/verify_sophia_xmonad_keyboard_vt.sh" "$session"
            ;;
        10-xmobar)
            require_archived_file "$guard"
            require_archived_file "$recovery"
            "$ROOT_DIR/tools/verify_sophia_xmonad_xmobar.sh" \
                "$session" "$guard" "$recovery"
            ;;
        11-emergency)
            require_archived_file "$guard"
            require_archived_file "$recovery"
            "$ROOT_DIR/tools/verify_sophia_xmonad_emergency_tty3.sh" \
                "$session" "$guard" "$recovery"
            ;;
        *) fail "unknown gate: $gate" ;;
    esac
}

run_gate() {
    local gate=$1
    case "$gate" in
        01-native-chrome)
            "$ROOT_DIR/tools/start_sophia_native_hot_reload_tty3.sh"
            ;;
        02-external-config)
            "$ROOT_DIR/tools/start_sophia_xmonad_config_reload_tty3.sh"
            ;;
        03-normal)
            "$ROOT_DIR/tools/start_sophia_xmonad_tty3.sh"
            ;;
        04-pointer-focus)
            "$ROOT_DIR/tools/start_sophia_xmonad_pointer_focus_tty3.sh"
            ;;
        05-four-kitty-1|06-four-kitty-2|07-four-kitty-3)
            "$ROOT_DIR/tools/start_sophia_xmonad_four_kitty_tty3.sh"
            ;;
        08-launch-burst)
            "$ROOT_DIR/tools/start_sophia_xmonad_launch_burst_tty3.sh"
            ;;
        09-keyboard-vt)
            "$ROOT_DIR/tools/start_sophia_xmonad_keyboard_vt_tty3.sh"
            ;;
        10-xmobar)
            "$ROOT_DIR/tools/start_sophia_xmonad_xmobar_tty3.sh"
            ;;
        11-emergency)
            "$ROOT_DIR/tools/start_sophia_xmonad_emergency_tty3.sh"
            ;;
        *) fail "unknown gate: $gate" ;;
    esac
}

archive_gate_sequence() {
    local gate=$1 destination=$2 source=
    case "$gate" in
        01-native-chrome)
            source="$RUNTIME_ROOT/sophia-native-hot-reload-${UID}/sequence.log"
            ;;
        02-external-config)
            source="$RUNTIME_ROOT/sophia-xmonad-config-reload-${UID}/sequence.log"
            ;;
        *) return 0 ;;
    esac
    require_archived_file "$source"
    install -m 600 "$source" "$destination/sequence.log"
    grep -Fxq "commit=$COMMIT" "$destination/sequence.log" ||
        fail "sequence evidence belongs to another commit"
}

next_gate() {
    local gate
    for gate in "${gates[@]}"; do
        if ! gate_passed "$gate"; then
            printf '%s\n' "$gate"
            return 0
        fi
    done
    return 1
}

promote_next() {
    [[ -t 0 && "$(tty)" == /dev/tty3 ]] ||
        fail "run 'tools/sophia_m9_promotion.sh next' from a logged-in tty3"
    [[ -z "$(git -C "$ROOT_DIR" status --porcelain)" ]] ||
        fail "commit or discard the dirty worktree before collecting physical evidence"

    local gate
    if ! gate="$(next_gate)"; then
        print_status
        echo "Milestone 9 physical promotion is complete for $COMMIT."
        return 0
    fi

    mkdir -p "$PROMOTION_ROOT"
    chmod 700 "$STATE_HOME/sophia/m9-promotion" "$PROMOTION_ROOT"
    local temporary
    temporary="$(mktemp -d "$PROMOTION_ROOT/.${gate}.XXXXXX")"
    chmod 700 "$temporary"
    printf 'Running gate %s for commit %s.\n' "$(gate_label "$gate")" "$COMMIT"

    local run_status=0 profile=xmonad
    run_gate "$gate" || run_status=$?
    [[ "$gate" == 01-native-chrome ]] && profile=native
    archive_session "$profile" "$temporary"
    archive_gate_sequence "$gate" "$temporary"

    if ((run_status != 0)); then
        printf 'promotion-result schema=1 gate="%s" commit="%s" status="failed" launcher-status=%d\n' \
            "$(gate_label "$gate")" "$COMMIT" "$run_status" >"$temporary/result.kdl"
        mv "$temporary" "$PROMOTION_ROOT/${gate}.failed.$(date +%s).$$"
        fail "the launcher exited with status $run_status; failure evidence was retained"
    fi
    if ! verify_gate "$gate" "$temporary"; then
        printf 'promotion-result schema=1 gate="%s" commit="%s" status="failed" verifier-status=1\n' \
            "$(gate_label "$gate")" "$COMMIT" >"$temporary/result.kdl"
        mv "$temporary" "$PROMOTION_ROOT/${gate}.failed.$(date +%s).$$"
        fail "verification rejected the run; failure evidence was retained"
    fi

    printf 'promotion-result schema=1 gate="%s" commit="%s" status="passed"\n' \
        "$(gate_label "$gate")" "$COMMIT" >"$temporary/result.kdl"
    chmod 600 "$temporary/result.kdl"
    mv "$temporary" "$PROMOTION_ROOT/$gate"
    print_status
}

usage() {
    echo "Usage: tools/sophia_m9_promotion.sh {next|status}" >&2
    exit 2
}

case "${1:-}" in
    next) promote_next ;;
    status) print_status ;;
    *) usage ;;
esac
