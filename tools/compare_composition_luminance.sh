#!/usr/bin/env bash
set -euo pipefail

# Compare the luminance of composed regions across two evidence files.
#
# A filtering change is judged on where the light went, not on whether the
# bytes differ. `region_checksum` answers the second question and cannot answer
# the first: it moves for a correction and for a regression alike. This reads
# `region_luminance_mean_millis` and `region_luminance_histogram` instead, keyed
# by the region each belongs to, so the same rect in two runs is compared with
# itself.
#
# This reports; it does not pass or fail. Which regions were resampled is a
# property of the topology the run was given, not of the log, so the reader
# supplies that: a region the renderer sampled one-to-one is the control and
# must not have moved at all, and one that was downscaled is where a linear-light
# correction should raise the mean and pull the low-mid buckets toward the
# middle. A control that moved means the change reached further than intended.

before="${1:?usage: compare_composition_luminance.sh BEFORE_EVIDENCE AFTER_EVIDENCE}"
after="${2:?usage: compare_composition_luminance.sh BEFORE_EVIDENCE AFTER_EVIDENCE}"
(( $# == 2 )) || {
    echo "usage: compare_composition_luminance.sh BEFORE_EVIDENCE AFTER_EVIDENCE" >&2
    exit 2
}

for file in "$before" "$after"; do
    [[ -s "$file" ]] || {
        echo "missing or empty evidence: $file" >&2
        exit 1
    }
done

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

# One record per region, keyed by what identifies it across runs. The layer
# index joins the key because two layers can share a rect, and the stage joins
# it because a CPU and a DMA-BUF layer at the same place are different draws.
collect() {
    local file="$1" line stage layer target mean histogram checksum
    while IFS= read -r line; do
        stage="$(field "$line" source_stage)" || continue
        layer="$(field "$line" layer)" || continue
        target="$(field "$line" target)" || continue
        mean="$(field "$line" region_luminance_mean_millis)" || continue
        histogram="$(field "$line" region_luminance_histogram)" || continue
        checksum="$(field "$line" region_checksum)" || continue
        printf '%s|%s|%s\t%s\t%s\t%s\n' \
            "$stage" "$layer" "$target" "$mean" "$histogram" "$checksum"
    done < <(
        grep -E 'sophia_native_composition_region schema=(2|3) status=read ' "$file" || true
    )
}

before_records="$(collect "$before")"
after_records="$(collect "$after")"

if [[ -z "$before_records" ]]; then
    echo "no schema>=2 region evidence in $before -- was it produced before the" \
        "luminance fields landed, or without SOPHIA_NATIVE_COMPOSITION_PIXEL_TRACE?" >&2
    exit 1
fi
if [[ -z "$after_records" ]]; then
    echo "no schema>=2 region evidence in $after" >&2
    exit 1
fi

moved=0
held=0
unmatched=0

printf '%-34s %10s %10s %9s  %s\n' REGION BEFORE AFTER DELTA VERDICT
while IFS=$'\t' read -r key before_mean before_histogram before_checksum; do
    match="$(printf '%s\n' "$after_records" | grep -F -m1 "$(printf '%s\t' "$key")" || true)"
    if [[ -z "$match" ]]; then
        printf '%-34s %10s %10s %9s  %s\n' "$key" "$before_mean" - - "absent-after"
        unmatched=$(( unmatched + 1 ))
        continue
    fi
    after_mean="$(printf '%s' "$match" | cut -f2)"
    after_histogram="$(printf '%s' "$match" | cut -f3)"
    after_checksum="$(printf '%s' "$match" | cut -f4)"
    delta=$(( after_mean - before_mean ))

    # A checksum that held while the mean moved, or the reverse, is worth
    # seeing: the first means the parse found the wrong pair of lines, and the
    # second means pixels changed in a way that cancelled in the mean, which is
    # what the histogram is carried for.
    if [[ "$before_checksum" == "$after_checksum" ]]; then
        verdict="identical"
        held=$(( held + 1 ))
    elif (( delta > 0 )); then
        verdict="brighter"
        moved=$(( moved + 1 ))
    elif (( delta < 0 )); then
        verdict="darker"
        moved=$(( moved + 1 ))
    else
        verdict="redistributed"
        moved=$(( moved + 1 ))
    fi

    printf '%-34s %10s %10s %+9d  %s\n' \
        "$key" "$before_mean" "$after_mean" "$delta" "$verdict"
    if [[ "$before_histogram" != "$after_histogram" ]]; then
        printf '    before %s\n    after  %s\n' "$before_histogram" "$after_histogram"
    fi
done < <(printf '%s\n' "$before_records")

printf '\nregions moved=%d identical=%d absent-after=%d\n' \
    "$moved" "$held" "$unmatched"
echo "means are thousandths of a 0..255 luminance; histograms are 16 buckets, dark to light"
