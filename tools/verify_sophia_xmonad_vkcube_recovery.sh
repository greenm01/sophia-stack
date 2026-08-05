#!/usr/bin/env bash
set -euo pipefail

STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
LOG_DIR="${SOPHIA_XMONAD_LOG_DIR:-$STATE_HOME/sophia/xmonad-session}"
SESSION_LOG="${1:-$LOG_DIR/session.log}"
WAIT_SECONDS="${SOPHIA_VERIFY_WAIT_SECONDS:-5}"

fail() {
    echo "vkcube xmonad verification failed: $*" >&2
    exit 1
}

field() {
    local line="$1" key="$2" token
    for token in $line; do
        if [[ "$token" == "$key="* ]]; then
            printf '%s\n' "${token#*=}"
            return 0
        fi
    done
    return 1
}

[[ -s "$SESSION_LOG" ]] || fail "missing session log: $SESSION_LOG"

deadline=$((SECONDS + WAIT_SECONDS))
while ! grep -Eq '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    ! grep -Eq '^sophia_live_session schema=(15|16) status=bounded_complete ' "$SESSION_LOG"; do
    ((SECONDS < deadline)) || fail "session log is incomplete"
    sleep 0.1
done

if grep -Eq \
    '(^Error:|panicked at|admission_group_(invalid|overflowed)|mismatched.transaction|status=(failed|degraded)([[:space:]]|$))' \
    "$SESSION_LOG"; then
    fail "session contains an error, invalid admission group, or degraded status"
fi

mapfile -t armed < <(
    grep -E \
        '^sophia_live_visual_admission schema=1 status=armed transaction=[0-9]+ surface=[0-9]+$' \
        "$SESSION_LOG"
)
proof_admissions=0
proof_retirements=0
declare -A proof_surfaces=()
diagnostic_feedback=false
if grep -Eq '^sophia_live_session_present_feedback schema=1 kind=complete ' "$SESSION_LOG"; then
    diagnostic_feedback=true
fi

for record in "${armed[@]}"; do
    transaction="$(field "$record" transaction)" ||
        fail "armed admission lacks a transaction"
    surface="$(field "$record" surface)" ||
        fail "armed admission lacks a surface"
    if ! grep -Eq \
        "^sophia_live_session_present schema=4 status=retired transaction=${transaction} surface=${surface} .* kind=software " \
        "$SESSION_LOG"; then
        continue
    fi
    proof_admissions=$((proof_admissions + 1))
    grep -Eq \
        "^sophia_live_visual_candidate schema=1 status=selected transaction=${transaction} surface=${surface} width=[0-9]+ height=[0-9]+ evidence=PresentedBuffer$" \
        "$SESSION_LOG" ||
        fail "surface $surface transaction $transaction was not selected from presented-buffer evidence"
    grep -Eq \
        "^sophia_live_visual_candidate_identity schema=1 status=selected transaction=${transaction} surface=${surface} source=(dma_buf|cpu_buffer) buffer=[1-9][0-9]*$" \
        "$SESSION_LOG" ||
        fail "surface $surface transaction $transaction was not selected by exact Present identity"
    if grep -Eq \
        "^sophia_live_visual_admission schema=1 status=committed transaction=${transaction} surface=${surface} " \
        "$SESSION_LOG"; then
        fail "surface $surface transaction $transaction bypassed native retirement"
    fi
    grep -Eq \
        "^sophia_live_visual_admission schema=1 status=presented transaction=${transaction} surface=${surface}$" \
        "$SESSION_LOG" ||
        fail "surface $surface transaction $transaction never completed visual admission"
    grep -Eq \
        "^sophia_live_session_present schema=4 status=retired transaction=${transaction} surface=${surface} .* kind=software " \
        "$SESSION_LOG" ||
        fail "surface $surface transaction $transaction has no matching page-flip retirement"

    mapfile -t retired < <(
        grep -E \
            "^sophia_live_session_present schema=4 status=retired transaction=[0-9]+ surface=${surface} .* kind=software " \
            "$SESSION_LOG"
    )
    ((${#retired[@]} >= 3)) ||
        fail "surface $surface retired only ${#retired[@]} Presents; animation requires at least 3"
    if [[ -z "${proof_surfaces[$surface]:-}" ]]; then
        proof_surfaces[$surface]=1
        proof_retirements=$((proof_retirements + ${#retired[@]}))
    fi
    previous=0
    for retired_record in "${retired[@]}"; do
        retired_transaction="$(field "$retired_record" transaction)" ||
            fail "retired Present lacks a transaction"
        ((retired_transaction > previous)) ||
            fail "surface $surface Present transactions did not advance"
        previous="$retired_transaction"
        frame="$(field "$retired_record" frame)" ||
            fail "software Present retirement lacks a native frame"
        native_submission="$(field "$retired_record" native_submission)" ||
            fail "software Present retirement lacks a native submission"
        ((frame > 0 && native_submission > 0)) ||
            fail "software Present retirement has an invalid native owner"
        ust="$(field "$retired_record" ust)" ||
            fail "software Present retirement lacks a display timestamp"
        msc="$(field "$retired_record" msc)" ||
            fail "software Present retirement lacks a display sequence"
        ((ust > 0 && msc > 0)) ||
            fail "software Present retirement has a zero display clock"
        grep -Eq \
            "sophia_live_native_page_flip schema=1 status=submitted output=[0-9]+ submission=${native_submission} content=Some\\((Cpu|RetainedMixed)(\\)| \\{.*\\}\\)) frame=${frame}$" \
            "$SESSION_LOG" ||
            fail "software Present frame $frame was not submitted by an independent native frame"
        grep -Eq \
            "sophia_live_native_page_flip schema=1 status=retired output=[0-9]+ submission=${native_submission} frame=${frame}$" \
            "$SESSION_LOG" ||
            fail "software Present frame $frame did not own its page-flip retirement"
        if [[ "$diagnostic_feedback" == true ]]; then
            grep -Eq \
                "^sophia_live_session_present_feedback schema=1 kind=complete transaction=${retired_transaction} routed=true mode=Copy ust=${ust} msc=${msc}$" \
                "$SESSION_LOG" ||
                fail "transaction $retired_transaction lacks matching Complete feedback"
            grep -Eq \
                "^sophia_live_session_present_feedback schema=1 kind=idle transaction=${retired_transaction} routed=true$" \
                "$SESSION_LOG" ||
                fail "transaction $retired_transaction lacks Idle feedback"
        fi
    done
done

((proof_admissions > 0)) ||
    fail "no exact software-Present visual admission was armed"

if [[ "$diagnostic_feedback" == false ]]; then
    completion="$(
        grep -E '^sophia_live_session schema=(15|16) status=bounded_complete ' "$SESSION_LOG" |
            tail -n 1
    )"
    complete_copy="$(field "$completion" present_complete_copy)" ||
        fail "session completion lacks aggregate Copy feedback"
    present_idle="$(field "$completion" present_idle)" ||
        fail "session completion lacks aggregate Idle feedback"
    idle_fences="$(field "$completion" present_idle_fence_triggers)" ||
        fail "session completion lacks aggregate idle-fence feedback"
    ((complete_copy >= proof_retirements)) ||
        fail "aggregate Copy feedback does not cover software retirements"
    ((present_idle >= proof_retirements && idle_fences >= proof_retirements)) ||
        fail "aggregate Idle feedback does not cover software retirements"
fi

grep -Eq '^sophia_live_session_cleanup schema=1 status=clean ' "$SESSION_LOG" ||
    fail "session cleanup was not clean"

echo "vkcube xmonad verification passed: $proof_admissions software visual admission(s) retired exactly"
