#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
RUNTIME_ROOT="${XDG_RUNTIME_DIR:-/tmp}"
COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD)"
PROMOTION_ROOT="$STATE_HOME/sophia/m9-promotion/$COMMIT"

gates=(
    00-qemu-semantic
    01-native-chrome
    02-hardware-smoke
    03-xmobar
    04-emergency
)

fail() {
    echo "Milestone 9 promotion failed: $*" >&2
    exit 1
}

gate_label() {
    printf '%s\n' "${1#??-}"
}

gate_passed() {
    local gate result
    gate=$1
    result="$PROMOTION_ROOT/$gate/result.kdl"
    [[ -s "$result" ]] &&
        grep -Eq "^promotion-result schema=1 gate=\"$(gate_label "$gate")\" commit=\"$COMMIT\" status=\"passed\"( source-commit=\"[0-9a-f]{40}\")?$" \
            "$result"
}

print_status() {
    local gate status
    printf 'Milestone 9 promotion\n  commit: %s\n  evidence: %s\n' \
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
        00-qemu-semantic)
            "$ROOT_DIR/tools/verify_sophia_m9_semantic_gate.sh" \
                "$evidence" "$COMMIT"
            ;;
        01-native-chrome)
            "$ROOT_DIR/tools/verify_sophia_native_chrome.sh" \
                "$session" "$evidence/sequence.log"
            ;;
        02-hardware-smoke)
            require_archived_file "$guard"
            require_archived_file "$recovery"
            "$ROOT_DIR/tools/verify_sophia_xmonad_hardware_smoke.sh" \
                "$session" "$guard" "$recovery" &&
            "$ROOT_DIR/tools/verify_sophia_xmonad_four_kitty.sh" "$session"
            ;;
        03-xmobar)
            require_archived_file "$guard"
            require_archived_file "$recovery"
            "$ROOT_DIR/tools/verify_sophia_xmonad_xmobar_hardware_smoke.sh" \
                "$session" "$guard" "$recovery"
            ;;
        04-emergency)
            require_archived_file "$guard"
            require_archived_file "$recovery"
            "$ROOT_DIR/tools/verify_sophia_xmonad_emergency_tty3.sh" \
                "$session" "$guard" "$recovery"
            ;;
        *) fail "unknown gate: $gate" ;;
    esac
}

run_gate() {
    local gate=$1 evidence=$2
    case "$gate" in
        00-qemu-semantic)
            "$ROOT_DIR/tools/run_sophia_m9_semantic_gate.sh" "$evidence"
            ;;
        01-native-chrome)
            "$ROOT_DIR/tools/start_sophia_native_hot_reload_tty3.sh"
            ;;
        02-hardware-smoke)
            "$ROOT_DIR/tools/start_sophia_xmonad_hardware_smoke_tty3.sh"
            ;;
        03-xmobar)
            "$ROOT_DIR/tools/start_sophia_xmonad_xmobar_tty3.sh"
            ;;
        04-emergency)
            "$ROOT_DIR/tools/start_sophia_xmonad_emergency_tty3.sh"
            ;;
        *) fail "unknown gate: $gate" ;;
    esac
}

gate_launcher_status_accepted() {
    local gate=$1 status=$2
    ((status == 0)) || [[ "$gate" == 04-emergency && "$status" == 130 ]]
}

archive_gate_sequence() {
    local gate=$1 destination=$2 source=
    case "$gate" in
        01-native-chrome)
            source="$RUNTIME_ROOT/sophia-native-hot-reload-${UID}/sequence.log"
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
    [[ -z "$(git -C "$ROOT_DIR" status --porcelain)" ]] ||
        fail "commit or discard the dirty worktree before collecting physical evidence"

    local gate
    if ! gate="$(next_gate)"; then
        print_status
        echo "Milestone 9 promotion is complete for $COMMIT."
        return 0
    fi
    if [[ "$gate" != 00-qemu-semantic ]] &&
        [[ ! -t 0 || "$(tty)" != /dev/tty3 ]]; then
        fail "run 'tools/sophia_m9_promotion.sh next' from a logged-in tty3"
    fi

    mkdir -p "$PROMOTION_ROOT"
    chmod 700 "$STATE_HOME/sophia/m9-promotion" "$PROMOTION_ROOT"
    local temporary
    temporary="$(mktemp -d "$PROMOTION_ROOT/.${gate}.XXXXXX")"
    chmod 700 "$temporary"
    printf 'Running gate %s for commit %s.\n' "$(gate_label "$gate")" "$COMMIT"

    local run_status=0 profile=xmonad
    run_gate "$gate" "$temporary" || run_status=$?
    if [[ "$gate" != 00-qemu-semantic ]]; then
        [[ "$gate" == 01-native-chrome ]] && profile=native
        archive_session "$profile" "$temporary"
        archive_gate_sequence "$gate" "$temporary"
    fi

    if ! gate_launcher_status_accepted "$gate" "$run_status"; then
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

adopt_parent_native_chrome() {
    [[ -z "$(git -C "$ROOT_DIR" status --porcelain)" ]] ||
        fail "commit or discard the dirty worktree before adopting evidence"
    local parent
    parent="$(git -C "$ROOT_DIR" rev-parse "${COMMIT}^")"
    local source_root="$STATE_HOME/sophia/m9-promotion/$parent/01-native-chrome"
    [[ -s "$source_root/result.kdl" ]] ||
        fail "the parent commit has no passing native-chrome evidence"
    grep -Eq "^promotion-result schema=1 gate=\"native-chrome\" commit=\"$parent\" status=\"passed\"" \
        "$source_root/result.kdl" ||
        fail "the parent native-chrome result is not passing"
    if ! git -C "$ROOT_DIR" diff --quiet "$parent" "$COMMIT" -- \
        crates \
        tools/start_sophia_tty3.sh \
        tools/run_sophia_xmonad_session.sh \
        tools/start_sophia_native_hot_reload_tty3.sh \
        tools/verify_sophia_native_chrome.sh \
        tools/config/proof_helpers.sh; then
        fail "native runtime or gate dependencies changed; physical evidence cannot be adopted"
    fi
    "$ROOT_DIR/tools/verify_sophia_native_chrome.sh" \
        "$source_root/session.log" "$source_root/sequence.log"
    mkdir -p "$PROMOTION_ROOT"
    chmod 700 "$STATE_HOME/sophia/m9-promotion" "$PROMOTION_ROOT"
    [[ ! -e "$PROMOTION_ROOT/01-native-chrome" ]] ||
        fail "current commit already has native-chrome evidence"
    local temporary
    temporary="$(mktemp -d "$PROMOTION_ROOT/.01-native-chrome.XXXXXX")"
    cp -a "$source_root/." "$temporary/"
    printf 'promotion-result schema=1 gate="native-chrome" commit="%s" status="passed" source-commit="%s"\n' \
        "$COMMIT" "$parent" >"$temporary/result.kdl"
    chmod 600 "$temporary/result.kdl"
    mv "$temporary" "$PROMOTION_ROOT/01-native-chrome"
    echo "Adopted byte-compatible native-chrome evidence from $parent."
}

adopt_parent_hardware_smoke() {
    [[ -z "$(git -C "$ROOT_DIR" status --porcelain)" ]] ||
        fail "commit or discard the dirty worktree before adopting evidence"
    local parent parent_root source_root=
    parent="$(git -C "$ROOT_DIR" rev-parse "${COMMIT}^")"
    parent_root="$STATE_HOME/sophia/m9-promotion/$parent"
    while IFS= read -r candidate; do
        if grep -Fxq \
            "promotion-result schema=1 gate=\"hardware-smoke\" commit=\"$parent\" status=\"failed\" verifier-status=1" \
            "$candidate/result.kdl" 2>/dev/null; then
            source_root=$candidate
            break
        fi
    done < <(
        find "$parent_root" -maxdepth 1 -mindepth 1 -type d \
            -name '02-hardware-smoke.failed.*' -printf '%T@ %p\n' 2>/dev/null |
            sort -nr |
            cut -d' ' -f2-
    )
    [[ -n "$source_root" ]] ||
        fail "the parent commit has no verifier-rejected hardware-smoke evidence"

    local changed_path
    while IFS= read -r changed_path; do
        case "$changed_path" in
            docs/research-log.md | docs/validation.md | \
            tools/sophia_m9_promotion.sh | \
            tools/verify_sophia_xmonad_hardware_smoke.sh | \
            tools/verify_sophia_xmonad_four_kitty.sh | \
            tools/verify_sophia_xmonad_pointer_focus.sh | \
            tools/check_sophia_xmonad_hardware_smoke_verifier.sh | \
            tools/check_sophia_xmonad_four_kitty_verifier.sh | \
            tools/fixtures/physical_xmonad_hardware_smoke_session_pass.log | \
            tools/fixtures/physical_xmonad_pointer_focus_pass.log)
                ;;
            *)
                fail "runtime or hardware-smoke dependency changed: $changed_path"
                ;;
        esac
    done < <(git -C "$ROOT_DIR" diff --name-only "$parent" "$COMMIT")

    for evidence in session.log input-guard.log recovery.log; do
        require_archived_file "$source_root/$evidence"
    done
    "$ROOT_DIR/tools/verify_sophia_xmonad_hardware_smoke.sh" \
        "$source_root/session.log" \
        "$source_root/input-guard.log" \
        "$source_root/recovery.log"
    "$ROOT_DIR/tools/verify_sophia_xmonad_four_kitty.sh" \
        "$source_root/session.log"

    mkdir -p "$PROMOTION_ROOT"
    chmod 700 "$STATE_HOME/sophia/m9-promotion" "$PROMOTION_ROOT"
    [[ ! -e "$PROMOTION_ROOT/02-hardware-smoke" ]] ||
        fail "current commit already has hardware-smoke evidence"
    local temporary
    temporary="$(mktemp -d "$PROMOTION_ROOT/.02-hardware-smoke.XXXXXX")"
    cp -a "$source_root/." "$temporary/"
    printf 'promotion-result schema=1 gate="hardware-smoke" commit="%s" status="passed" source-commit="%s"\n' \
        "$COMMIT" "$parent" >"$temporary/result.kdl"
    chmod 600 "$temporary/result.kdl"
    mv "$temporary" "$PROMOTION_ROOT/02-hardware-smoke"
    echo "Adopted reverified hardware-smoke evidence from $parent."
}

pre_emergency_adoption_path_allowed() {
    case "$1" in
        crates/sophia-session/src/live_session/owner_loop_state.rs | \
        crates/sophia-session/src/live_session/owner_loop/lifecycle.rs | \
        crates/sophia-session/src/live_session/tests/input_policy_tests.rs | \
        docs/research-log.md | docs/validation.md | \
        tools/sophia_m9_promotion.sh | tools/check_sophia_m9_promotion.sh)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

adopt_parent_pre_emergency() {
    [[ -z "$(git -C "$ROOT_DIR" status --porcelain)" ]] ||
        fail "commit or discard the dirty worktree before adopting evidence"
    gate_passed 00-qemu-semantic ||
        fail "run the current commit's unattended QEMU semantic gate before adoption"

    local parent parent_root
    parent="$(git -C "$ROOT_DIR" rev-parse "${COMMIT}^")"
    parent_root="$STATE_HOME/sophia/m9-promotion/$parent"
    local changed_path
    while IFS= read -r changed_path; do
        pre_emergency_adoption_path_allowed "$changed_path" ||
            fail "non-emergency runtime or gate dependency changed: $changed_path"
    done < <(git -C "$ROOT_DIR" diff --name-only "$parent" "$COMMIT")

    local gate source_root
    for gate in 01-native-chrome 02-hardware-smoke 03-xmobar; do
        source_root="$parent_root/$gate"
        grep -Eq \
            "^promotion-result schema=1 gate=\"$(gate_label "$gate")\" commit=\"$parent\" status=\"passed\"( source-commit=\"[0-9a-f]{40}\")?$" \
            "$source_root/result.kdl" 2>/dev/null ||
            fail "the parent commit has no passing $(gate_label "$gate") evidence"
        verify_gate "$gate" "$source_root"
        [[ ! -e "$PROMOTION_ROOT/$gate" ]] ||
            fail "current commit already has $(gate_label "$gate") evidence"
    done

    local staging destination
    staging="$(mktemp -d "$PROMOTION_ROOT/.pre-emergency-adoption.XXXXXX")"
    chmod 700 "$staging"
    for gate in 01-native-chrome 02-hardware-smoke 03-xmobar; do
        source_root="$parent_root/$gate"
        destination="$staging/$gate"
        mkdir "$destination"
        cp -a "$source_root/." "$destination/"
        printf 'promotion-result schema=1 gate="%s" commit="%s" status="passed" source-commit="%s"\n' \
            "$(gate_label "$gate")" "$COMMIT" "$parent" >"$destination/result.kdl"
        chmod 600 "$destination/result.kdl"
    done
    for gate in 01-native-chrome 02-hardware-smoke 03-xmobar; do
        mv "$staging/$gate" "$PROMOTION_ROOT/$gate"
    done
    rmdir "$staging"
    echo "Adopted reverified native, hardware-smoke, and xmobar evidence from $parent."
}

usage() {
    echo "Usage: tools/sophia_m9_promotion.sh {next|status|adopt-parent-native|adopt-parent-hardware|adopt-parent-pre-emergency}" >&2
    exit 2
}

case "${1:-}" in
    next) promote_next ;;
    status) print_status ;;
    adopt-parent-native) adopt_parent_native_chrome ;;
    adopt-parent-hardware) adopt_parent_hardware_smoke ;;
    adopt-parent-pre-emergency) adopt_parent_pre_emergency ;;
    *) usage ;;
esac
