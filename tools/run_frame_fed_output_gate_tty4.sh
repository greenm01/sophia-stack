#!/usr/bin/env bash
set -euo pipefail

# Signed two-phase proof for the normal frame-fed startup output transaction.
# Run from a recovery-safe VT: both phases take exclusive DRM and input control.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HAGIA_ROOT="${SOPHIA_HAGIA_ROOT:-$ROOT_DIR/../hagia}"
CORE_CONFIG="$ROOT_DIR/tools/config/sophia-xmonad/core.kdl"
DESKTOP_PROFILE="$ROOT_DIR/tools/fixtures/frame_fed_output_proof.kdl"
KITTY_BIN="${SOPHIA_FRAME_FED_OUTPUT_KITTY:-$(command -v kitty || true)}"
SEAT="${SOPHIA_FRAME_FED_OUTPUT_SEAT:-seat0}"
TTY_REQUIRED="${SOPHIA_FRAME_FED_OUTPUT_TTY:-/dev/tty4}"
RUNTIME_MSEC="${SOPHIA_FRAME_FED_OUTPUT_RUNTIME_MSEC:-180000}"
SEQUENCE_TIMEOUT_MSEC="${SOPHIA_FRAME_FED_OUTPUT_SEQUENCE_TIMEOUT_MSEC:-120000}"
SUCCESS_TEXT="${SOPHIA_FRAME_FED_OUTPUT_SUCCESS_TEXT:-outputapply}"
ROLLBACK_TEXT="${SOPHIA_FRAME_FED_OUTPUT_ROLLBACK_TEXT:-outputrollback}"
EVIDENCE_ROOT="${SOPHIA_FRAME_FED_OUTPUT_EVIDENCE_ROOT:-/tmp/sophia-frame-fed-output}"

# shellcheck source=tools/lib/drm_master_guard.sh
. "$ROOT_DIR/tools/lib/drm_master_guard.sh"

refuse() {
    echo "frame-fed output gate refused: $*" >&2
    exit 2
}

[[ "${SOPHIA_FRAME_FED_OUTPUT_ARM:-0}" == 1 ]] \
    || refuse "set SOPHIA_FRAME_FED_OUTPUT_ARM=1 to acknowledge exclusive DRM/input use and real modesets"
[[ -t 0 && "$(tty)" == "$TTY_REQUIRED" ]] \
    || refuse "run this gate from $TTY_REQUIRED so another VT remains available for recovery"
[[ -d "$HAGIA_ROOT/.git" ]] || refuse "Hagia checkout is unavailable: $HAGIA_ROOT"
[[ "$KITTY_BIN" == /* && -x "$KITTY_BIN" ]] \
    || refuse "SOPHIA_FRAME_FED_OUTPUT_KITTY must name an absolute executable Kitty path"
[[ "$SEAT" =~ ^[A-Za-z0-9_-]{1,64}$ ]] || refuse "the libinput seat name is invalid"
[[ "$RUNTIME_MSEC" =~ ^[0-9]+$ && "$RUNTIME_MSEC" -ge 60000 ]] \
    || refuse "SOPHIA_FRAME_FED_OUTPUT_RUNTIME_MSEC must be at least 60000"
[[ "$SEQUENCE_TIMEOUT_MSEC" =~ ^[0-9]+$ \
    && "$SEQUENCE_TIMEOUT_MSEC" -ge 1000 \
    && "$SEQUENCE_TIMEOUT_MSEC" -le 600000 ]] \
    || refuse "SOPHIA_FRAME_FED_OUTPUT_SEQUENCE_TIMEOUT_MSEC must be 1000-600000"
for proof_text in "$SUCCESS_TEXT" "$ROLLBACK_TEXT"; do
    [[ "$proof_text" =~ ^[a-z]{1,24}$ ]] \
        || refuse "proof text must contain 1-24 lowercase ASCII letters"
done
[[ "$SUCCESS_TEXT" != "$ROLLBACK_TEXT" ]] || refuse "success and rollback proof text must differ"
for file in "$CORE_CONFIG" "$DESKTOP_PROFILE"; do
    [[ -r "$file" ]] || refuse "checked-in proof configuration is missing: $file"
done

verify_repo() {
    local repo="$1" name="$2" head upstream
    [[ -z "$(git -C "$repo" status --porcelain --untracked-files=all)" ]] \
        || refuse "$name worktree must be clean"
    head="$(git -C "$repo" rev-parse HEAD)"
    git -C "$repo" verify-commit "$head" >/dev/null 2>&1 \
        || refuse "$name HEAD lacks a valid cryptographic signature"
    upstream="$(git -C "$repo" rev-parse --verify refs/remotes/origin/master 2>/dev/null || true)"
    [[ -n "$upstream" && "$head" == "$upstream" ]] \
        || refuse "$name HEAD must equal the locally known origin/master"
}
verify_repo "$ROOT_DIR" Sophia
verify_repo "$HAGIA_ROOT" Hagia
sophia_commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
hagia_commit="$(git -C "$HAGIA_ROOT" rev-parse HEAD)"

check_reference_connectors() {
    local facts="$1" connector status name mode
    : >"$facts"
    for status in /sys/class/drm/card*-*/status; do
        [[ -r "$status" && "$(<"$status")" == connected ]] || continue
        connector="${status%/status}"
        name="$(basename "$connector" | sed -E 's/^card[0-9]+-//')"
        mode="$(head -n 1 "$connector/modes" 2>/dev/null || true)"
        printf '%s status=connected preferred=%s\n' "$name" "${mode:-none}" >>"$facts"
    done
    sort -o "$facts" "$facts"
    [[ "$(wc -l <"$facts")" == 2 ]] \
        || refuse "the reference proof requires exactly two connected outputs"
    grep -Fxq 'DP-1 status=connected preferred=2560x1440' "$facts" \
        || refuse "DP-1 must be connected with preferred mode 2560x1440"
    grep -Fxq 'DP-2 status=connected preferred=1920x1080' "$facts" \
        || refuse "DP-2 must be connected with preferred mode 1920x1080"
}

mkdir -p "$EVIDENCE_ROOT"
run_dir="$EVIDENCE_ROOT/${sophia_commit:0:12}-$(date -u +%Y%m%dT%H%M%SZ)-$$"
mkdir -m 700 "$run_dir"
preserve_failed_run() {
    local status=$?
    trap - ERR HUP INT TERM
    printf 'Frame-fed output gate failed (exit %s); diagnostic evidence remains at %s\n' \
        "$status" "$run_dir" >&2
    exit "$status"
}
trap preserve_failed_run ERR HUP INT TERM
connectors="$run_dir/connectors.txt"
check_reference_connectors "$connectors"

if ! drm_refusal="$(sophia_require_drm_master_available SOPHIA_FRAME_FED_OUTPUT_FORCE 2>&1)"; then
    refuse "$drm_refusal"
fi

echo "Building exact signed Sophia and Hagia binaries before DRM takeover..."
hagia_bin="${TMPDIR:-/tmp}/hagia-frame-fed-${hagia_commit:0:12}"
hagia_nimcache="${TMPDIR:-/tmp}/hagia-frame-fed-nimcache-${hagia_commit:0:12}"
(
    cd "$HAGIA_ROOT"
    nim c -d:release --path:src --nimcache:"$hagia_nimcache" \
        -o:"$hagia_bin" src/hagia.nim
)
(
    cd "$ROOT_DIR"
    cargo build --quiet --release --offline -p sophia-cli --features atomic-scanout-live
)
sophia_bin="$ROOT_DIR/target/release/sophia"
[[ -x "$sophia_bin" && -x "$hagia_bin" ]] || refuse "a proof binary is missing after build"
"$sophia_bin" config check --config="$CORE_CONFIG" >/dev/null
"$sophia_bin" config check --desktop-profile="$DESKTOP_PROFILE" >/dev/null

verify_repo "$ROOT_DIR" Sophia
verify_repo "$HAGIA_ROOT" Hagia
[[ "$(git -C "$ROOT_DIR" rev-parse HEAD)" == "$sophia_commit" \
    && "$(git -C "$HAGIA_ROOT" rev-parse HEAD)" == "$hagia_commit" ]] \
    || refuse "source identity changed during the proof build"

sophia_sha256="$(sha256sum "$sophia_bin" | awk '{ print $1 }')"
hagia_sha256="$(sha256sum "$hagia_bin" | awk '{ print $1 }')"
core_sha256="$(sha256sum "$CORE_CONFIG" | awk '{ print $1 }')"
profile_sha256="$(sha256sum "$DESKTOP_PROFILE" | awk '{ print $1 }')"
connectors_sha256="$(sha256sum "$connectors" | awk '{ print $1 }')"
identity_fields="source_commit=$sophia_commit hagia_commit=$hagia_commit sophia_sha256=$sophia_sha256 hagia_sha256=$hagia_sha256 core_sha256=$core_sha256 profile_sha256=$profile_sha256 connectors_sha256=$connectors_sha256"

echo "Frame-fed output proof: $sophia_commit with Hagia $hagia_commit"
echo "Evidence directory: $run_dir"
echo "Phase 1 applies and publishes the candidate. When both displays are correct, type '$SUCCESS_TEXT' and press Enter."

run_phase() {
    local phase="$1" proof_text="$2" display="$3" evidence="$4"
    shift 4
    local runtime_evidence="$run_dir/$phase.runtime.log"
    check_reference_connectors "$run_dir/connectors-$phase.txt"
    cmp -s "$connectors" "$run_dir/connectors-$phase.txt" \
        || refuse "connector facts changed before the $phase phase"
    if ! drm_refusal="$(sophia_require_drm_master_available SOPHIA_FRAME_FED_OUTPUT_FORCE 2>&1)"; then
        refuse "$drm_refusal"
    fi

    set +e
    RUST_LOG="${RUST_LOG:-sophia=info,sophia_backend_live=info}" \
    SOPHIA_FRAME_FED_OUTPUT_ARM=1 \
    SOPHIA_LIVE_SESSION_SKIP_BUILD=1 \
    SOPHIA_LIVE_SESSION_VERIFY_MODE=caller \
    SOPHIA_LIVE_SESSION_DISPLAY="$display" \
    SOPHIA_LIVE_SESSION_RUNTIME_MSEC="$RUNTIME_MSEC" \
    SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$runtime_evidence" \
        "$ROOT_DIR/tools/live_session_persistent_hardware_proof.sh" \
        --config="$CORE_CONFIG" \
        --desktop-profile="$DESKTOP_PROFILE" \
        --session-mode=normal \
        --session-app=terminal="$KITTY_BIN" \
        --session-start=terminal \
        --session-action-app=terminal=terminal \
        --session-action-app=browser=terminal \
        --session-app-arg=terminal=--config \
        --session-app-arg=terminal=NONE \
        --session-app-arg=terminal=--override \
        --session-app-arg=terminal=linux_display_server=x11 \
        --session-app-arg=terminal=--override \
        --session-app-arg=terminal=remember_window_size=no \
        --wm-process="$hagia_bin" \
        --wm-interface=sophia_wm_v1 \
        --input-seat="$SEAT" \
        --expect-physical-text="$proof_text" \
        --physical-sequence-timeout-ms="$SEQUENCE_TIMEOUT_MSEC" \
        --exit-after-input-proof \
        "$@"
    status=$?
    set -e

    {
        printf 'sophia_frame_fed_output_gate schema=1 status=phase_started phase=%s %s\n' \
            "$phase" "$identity_fields"
        cat "$runtime_evidence"
    } >"$evidence"
    rm -f -- "$runtime_evidence"
    if (( status != 0 )); then
        printf 'sophia_frame_fed_output_gate schema=1 status=phase_failed phase=%s exit=%s\n' \
            "$phase" "$status" >>"$evidence"
        return "$status"
    fi
    printf 'sophia_frame_fed_output_gate schema=1 status=phase_passed phase=%s exit=0\n' \
        "$phase" >>"$evidence"
}

success_log="$run_dir/success.log"
rollback_log="$run_dir/rollback.log"
run_phase success "$SUCCESS_TEXT" :296 "$success_log"

echo "Phase 1 passed. Phase 2 applies the same candidate, rolls it back immediately after KMS acceptance, and never installs or publishes it."
echo "After the original display topology recovers, type '$ROLLBACK_TEXT' and press Enter."
run_phase rollback "$ROLLBACK_TEXT" :297 "$rollback_log" \
    --output-proof-rollback-after-apply

check_reference_connectors "$run_dir/connectors-final.txt"
cmp -s "$connectors" "$run_dir/connectors-final.txt" \
    || refuse "connector facts changed during the two-phase proof"

"$ROOT_DIR/tools/verify_frame_fed_output_evidence.sh" \
    "$success_log" "$rollback_log" "$SUCCESS_TEXT" "$ROLLBACK_TEXT"
archive_output="$(
    SOPHIA_FRAME_FED_OUTPUT_SOPHIA_BIN="$sophia_bin" \
    SOPHIA_FRAME_FED_OUTPUT_HAGIA_BIN="$hagia_bin" \
    SOPHIA_HAGIA_ROOT="$HAGIA_ROOT" \
        "$ROOT_DIR/tools/archive_frame_fed_output_physical_run.sh" \
        "$success_log" "$rollback_log" "$connectors" "$SUCCESS_TEXT" "$ROLLBACK_TEXT"
)"
trap - ERR HUP INT TERM
echo "$archive_output"
echo "Frame-fed output apply/rollback gate passed. Raw evidence: $run_dir"
