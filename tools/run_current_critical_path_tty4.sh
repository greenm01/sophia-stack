#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HAGIA_ROOT="${SOPHIA_HAGIA_ROOT:-$ROOT_DIR/../hagia}"
TTY_REQUIRED="${SOPHIA_CRITICAL_PATH_TTY:-/dev/tty4}"
REMOVABLE_CONNECTOR="${SOPHIA_CRITICAL_PATH_REMOVABLE_CONNECTOR:-DP-3}"

if [[ ! -t 0 || "$(tty)" != "$TTY_REQUIRED" ]]; then
    echo "Switch to tty4 with Ctrl+Alt+F4, log in, and run:" >&2
    echo "  cd $ROOT_DIR && tools/run_current_critical_path_tty4.sh" >&2
    exit 1
fi
if [[ ! -d "$HAGIA_ROOT/.git" ]]; then
    echo "Hagia checkout not found at $HAGIA_ROOT" >&2
    exit 1
fi

sophia_commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
hagia_commit="$(git -C "$HAGIA_ROOT" rev-parse HEAD)"
mapfile -t three_head_topology < <(
    printf '%s\n' DP-1 DP-2 "$REMOVABLE_CONNECTOR" | sort -u
)
(( ${#three_head_topology[@]} == 3 )) || {
    echo "The removable connector must differ from DP-1 and DP-2." >&2
    exit 1
}

verify_identity() {
    local repo commit name upstream
    for name in Sophia Hagia; do
        if [[ "$name" == Sophia ]]; then
            repo="$ROOT_DIR"
            commit="$sophia_commit"
        else
            repo="$HAGIA_ROOT"
            commit="$hagia_commit"
        fi
        [[ -z "$(git -C "$repo" status --porcelain --untracked-files=all)" ]] || {
            echo "$name worktree changed; stopping before another physical gate." >&2
            exit 1
        }
        [[ "$(git -C "$repo" rev-parse HEAD)" == "$commit" ]] || {
            echo "$name HEAD changed; stopping before another physical gate." >&2
            exit 1
        }
        git -C "$repo" verify-commit "$commit" >/dev/null 2>&1 || {
            echo "$name HEAD lacks a valid cryptographic signature." >&2
            exit 1
        }
        upstream="$(git -C "$repo" rev-parse --verify refs/remotes/origin/master 2>/dev/null || true)"
        [[ -n "$upstream" && "$upstream" == "$commit" ]] || {
            echo "$name HEAD must equal the locally known origin/master." >&2
            echo "  HEAD:          $commit" >&2
            echo "  origin/master: ${upstream:-missing}" >&2
            exit 1
        }
    done
}

connected_connectors() {
    local status connector
    for status in /sys/class/drm/card*-*/status; do
        [[ -r "$status" && "$(<"$status")" == connected ]] || continue
        connector="${status%/status}"
        basename "$connector" | sed -E 's/^card[0-9]+-//'
    done | sort
}

wait_for_topology() {
    local instruction="$1"
    shift
    local -a expected=("$@") observed=()
    local reply
    while true; do
        echo
        echo "$instruction"
        echo "Expected: ${expected[*]}"
        echo "Press Enter after the cable state is stable, or type q to stop."
        IFS= read -r reply </dev/tty
        [[ "$reply" != q ]] || exit 1
        mapfile -t observed < <(connected_connectors)
        if [[ "${observed[*]}" == "${expected[*]}" ]]; then
            echo "Topology accepted: ${observed[*]}"
            return 0
        fi
        echo "Topology is not ready; observed: ${observed[*]:-none}" >&2
    done
}

verify_identity
echo "Sophia/Hagia critical-path physical proof"
echo "Sophia: $sophia_commit"
echo "Hagia:  $hagia_commit"
echo "The script stops at the first refusal or failed visual check."

wait_for_topology \
    "Disconnect $REMOVABLE_CONNECTOR for the two-head mirror gate." \
    DP-1 DP-2
verify_identity
"$ROOT_DIR/tools/run_mirror_group_gate_tty4.sh"

wait_for_topology \
    "Reconnect $REMOVABLE_CONNECTOR for the centered three-head mixed gate." \
    "${three_head_topology[@]}"
verify_identity
SOPHIA_MIXED_MIRROR_PRIMARY=DP-1 \
SOPHIA_MIXED_MIRROR_MEMBER="$REMOVABLE_CONNECTOR" \
SOPHIA_MIXED_EXTENDED=DP-2 \
    "$ROOT_DIR/tools/run_mixed_output_gate_tty4.sh" \
    --optimize-for=center-unscaled

wait_for_topology \
    "Disconnect $REMOVABLE_CONNECTOR again for the two-head Hagia/broker gate." \
    DP-1 DP-2
verify_identity
SOPHIA_HAGIA_ROOT="$HAGIA_ROOT" \
    "$ROOT_DIR/tools/run_current_hagia_policy_gate_tty4.sh"

verify_identity
"$ROOT_DIR/tools/verify_mirror_group_physical_archive.sh"
"$ROOT_DIR/tools/verify_mixed_output_physical_archive.sh"
SOPHIA_HAGIA_ROOT="$HAGIA_ROOT" \
    "$ROOT_DIR/tools/verify_hagia_policy_physical_archive.sh"

echo
echo "All three current critical-path physical gates passed and their archives verify."
echo "Reconnect $REMOVABLE_CONNECTOR before returning to the graphical session."
