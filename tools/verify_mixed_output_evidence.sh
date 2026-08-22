#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
    echo "usage: verify_mixed_output_evidence.sh EVIDENCE EXTENDED_CONNECTOR" >&2
    exit 2
fi

EVIDENCE="$1"
EXTENDED="$2"

if [[ ! -r "$EVIDENCE" || -z "$EXTENDED" ]]; then
    echo "Mixed-output evidence and the extended connector are required." >&2
    exit 2
fi

for required in \
    '^sophia_output_v1_reference schema=1 status=settled kind=Committed topology_epoch=[0-9]+ heads=3 groups=2$' \
    '^sophia_wm_v1_reference schema=1 status=settled outputs=2 surfaces=2 placement=1,1$' \
    'sophia_live_output_authority schema=2 status=committed .* outputs=2 ' \
    '^sophia_live_output_topology_health schema=1 status=clean quarantined=false$' \
    '^sophia_live_session_health schema=1 status=clean '; do
    grep -Eq "$required" "$EVIDENCE" || {
        echo "Mixed-output telemetry requirement is missing: $required" >&2
        exit 1
    }
done
if grep -Eq '(^Error:|panicked at|status=(failed|degraded|rolled_back)([[:space:]]|$))' \
    "$EVIDENCE"; then
    echo "Mixed-output evidence contains a failure, degradation, or rollback." >&2
    exit 1
fi
if grep -Eq 'sophia_native_composition_sampling schema=(2|3) status=(fallback|unavailable)([[:space:]]|$)' \
    "$EVIDENCE"; then
    echo "Mixed-output evidence contains a composition sampling fallback." >&2
    exit 1
fi

mapfile -t extended_ready < <(
    grep -E "sophia_live_native_head schema=2 status=ready .* connector=$EXTENDED " "$EVIDENCE"
)
if (( ${#extended_ready[@]} != 1 )); then
    echo "The extended connector did not map to exactly one opaque head." >&2
    exit 1
fi
extended_head="$(sed -n 's/.* head=\([0-9][0-9]*\) .*/\1/p' <<<"${extended_ready[0]}")"
committed_transaction="$(sed -n 's/.*sophia_live_output_authority schema=2 status=committed transaction=\([0-9][0-9]*\) .*/\1/p' "$EVIDENCE" | tail -n1)"
effect_line="$(grep -nE "sophia_live_output_authority schema=1 status=effect_pending transaction=$committed_transaction " "$EVIDENCE" | tail -n1 | cut -d: -f1)"
first_presented_line="$(grep -nEm1 "sophia_live_output_authority schema=2 status=first_presented transaction=$committed_transaction " "$EVIDENCE" | cut -d: -f1)"
placement_line="$(grep -nE '^sophia_wm_v1_reference schema=1 status=settled outputs=2 surfaces=2 placement=1,1$' "$EVIDENCE" | tail -n1 | cut -d: -f1)"
if [[ -z "$extended_head" || -z "$committed_transaction" || -z "$effect_line" \
    || -z "$first_presented_line" || -z "$placement_line" \
    || "$effect_line" -ge "$first_presented_line" \
    || "$first_presented_line" -ge "$placement_line" ]]; then
    echo "Mixed-output publication and policy-placement ordering is incomplete." >&2
    exit 1
fi

# The first topology frame is composed from the last committed three-output
# scene rather than from a fresh engine plan, so the artefact to read here is
# the queued candidate: DP-2 may be empty, but it must already be composed at
# its own mode with an exact mapping. A head plan cannot appear in this window
# -- plans are emitted where the engine binds scene layers to a head, which
# installing a prepared topology does not do.
extended_mode="$(sed -n 's/.* mode=\([0-9][0-9]*x[0-9][0-9]*\) .*/\1/p' <<<"${extended_ready[0]}")"
if [[ -z "$extended_mode" ]]; then
    echo "The extended connector did not report a mode." >&2
    exit 1
fi
sed -n "${effect_line},${first_presented_line}p" "$EVIDENCE" \
    | grep -Eq "sophia_live_head_composition_queue schema=1 status=queued .* head=$extended_head .* mapping=exact width=${extended_mode%x*} height=${extended_mode#*x} " || {
        echo "The extended head did not queue an exact native topology frame." >&2
        exit 1
    }

# Where a plan does exist -- every frame after the topology committed -- the
# extended head must still bind at its own scale: no sampling, no fallback.
if sed -n "$((first_presented_line + 1)),\$p" "$EVIDENCE" \
    | grep -E "sophia_live_head_composition_plan schema=2 status=ready .* head=$extended_head " \
    | grep -qvE " mapping=exact .* downsampled=0 upsampled=0 mixed=0 .* fallback=0 unavailable=0 "; then
    echo "The extended head composed a sampled or fallback frame after the topology committed." >&2
    exit 1
fi

exact_plan="$(awk -v start="$placement_line" -v head="$extended_head" '
    NR > start && $0 ~ "sophia_live_head_composition_plan schema=2 status=ready" \
        && $0 ~ (" head=" head " ") && $0 ~ " mapping=exact " \
        && $0 ~ " exact=1 " && $0 ~ " downsampled=0 " \
        && $0 ~ " upsampled=0 " && $0 ~ " mixed=0 " && $0 ~ " active=1 " \
        && $0 ~ " fallback=0 unavailable=0 " { print NR ":" $0; exit }
' "$EVIDENCE")"
if [[ -z "$exact_plan" ]]; then
    echo "The extended head never produced exact active content after policy placement." >&2
    exit 1
fi
plan_line="${exact_plan%%:*}"
plan_text="${exact_plan#*:}"
extended_output="$(sed -n 's/.* output=\([0-9][0-9]*\) .*/\1/p' <<<"$plan_text")"
scene_generation="$(sed -n 's/.* scene_generation=\([0-9][0-9]*\) .*/\1/p' <<<"$plan_text")"

queue="$(awk -v start="$plan_line" -v output="$extended_output" -v head="$extended_head" -v scene="$scene_generation" '
    NR > start && $0 ~ "sophia_live_head_composition_queue schema=1 status=queued" \
        && $0 ~ (" output=" output " ") && $0 ~ (" head=" head " ") \
        && $0 ~ (" scene_generation=" scene " ") && $0 ~ " mapping=exact " \
        { print NR ":" $0; exit }
' "$EVIDENCE")"
if [[ -z "$queue" ]]; then
    echo "The exact extended-head plan was not queued." >&2
    exit 1
fi
queue_line="${queue%%:*}"
queue_text="${queue#*:}"
frame="$(sed -n 's/.* frame=\([0-9][0-9]*\) .*/\1/p' <<<"$queue_text")"

submit_line="$(awk -v start="$queue_line" -v output="$extended_output" -v head="$extended_head" -v frame="$frame" '
    NR > start && $0 ~ "sophia_live_native_head_page_flip schema=2 status=submitted" \
        && $0 ~ (" output=" output " ") && $0 ~ (" head=" head " ") \
        && $0 ~ (" frame=" frame "($| )") { print NR; exit }
' "$EVIDENCE")"
callback_line="$(awk -v start="$submit_line" -v output="$extended_output" -v head="$extended_head" '
    NR > start && $0 ~ "sophia_live_native_head_page_flip schema=2 status=callback_accepted" \
        && $0 ~ (" output=" output " ") && $0 ~ (" head=" head " ") { print NR; exit }
' "$EVIDENCE")"
retire_line="$(awk -v start="$callback_line" -v output="$extended_output" -v head="$extended_head" -v frame="$frame" '
    NR > start && $0 ~ "sophia_live_native_head_page_flip schema=2 status=retired" \
        && $0 ~ (" output=" output " ") && $0 ~ (" head=" head " ") \
        && $0 ~ (" frame=" frame "($| )") { print NR; exit }
' "$EVIDENCE")"
if [[ -z "$frame" || -z "$submit_line" || -z "$callback_line" || -z "$retire_line" ]]; then
    echo "The exact extended-head frame did not complete queue-to-retirement." >&2
    exit 1
fi

sampling_line="$(awk -v start="$queue_line" -v stop="$submit_line" -v output="$extended_output" -v head="$extended_head" -v scene="$scene_generation" '
    NR > start && NR < stop \
        && $0 ~ "sophia_native_composition_sampling schema=3 status=active" \
        && $0 ~ (" output=" output " ") && $0 ~ (" head=" head " ") \
        && $0 ~ (" scene_generation=" scene " ") \
        && $0 ~ " requested=exact_nearest " \
        && $0 ~ " effective=exact_nearest " { print NR; exit }
' "$EVIDENCE")"
if [[ -z "$sampling_line" ]]; then
    echo "The extended head lacks realized exact-sampling evidence before submission." >&2
    exit 1
fi

mapfile -t completed_outputs < <(
    grep -E 'sophia_live_native_head schema=3 status=complete ' "$EVIDENCE" \
        | sed -n 's/.* output=\([0-9][0-9]*\) .*/\1/p' \
        | sort | uniq -c | awk '{ print $1 }' | sort -n
)
if [[ "${completed_outputs[*]}" != "1 2" ]]; then
    echo "Completed heads do not prove one singleton and one two-head logical output." >&2
    exit 1
fi

printf 'sophia_mixed_output_evidence schema=1 status=verified extended_head=%s output=%s scene_generation=%s frame=%s retired=true\n' \
    "$extended_head" "$extended_output" "$scene_generation" "$frame"
