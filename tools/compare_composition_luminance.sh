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

# The centroid of the lit population, in thousandths of a bucket index.
#
# Bucket zero is background, and on a dark desktop it holds well over 97% of
# every region -- a whole-region mean is therefore a measurement of how much
# black is on screen, which no filter changes. The pixels a filter does change
# are the anti-aliased edges spread through the buckets above it, and their
# centroid is where those edges sit on the dark-to-light axis.
#
# It is deliberately a ratio. Two runs of a scripted gate need not put exactly
# the same number of glyphs on screen, and a count would move for that reason
# alone; the centroid does not care how much text there is, only how bright its
# edges came out. That is the quantity a linear-light correction is predicted to
# raise.
centroid_millis() {
    printf '%s' "$1" | awk -F: '
        {
            weighted = 0
            lit = 0
            for (slot = 2; slot <= NF; slot++) {
                bucket = slot - 1
                weighted += bucket * $slot
                lit += $slot
            }
            if (lit == 0) { print "0"; exit }
            printf "%d\n", (weighted * 1000) / lit
        }'
}

# The lit population, ignoring background.
lit_pixels() {
    printf '%s' "$1" | awk -F: '
        {
            lit = 0
            for (slot = 2; slot <= NF; slot++) { lit += $slot }
            print lit
        }'
}

# One record per region, keyed by what identifies it across runs, keeping the
# last.
#
# The layer index joins the key because two layers can share a rect, and the
# stage joins it because a CPU and a DMA-BUF layer at the same place are
# different draws. The last record wins because the earlier ones are a session
# starting up: a real run emitted a region nineteen times, eighteen of them
# while the terminal was still empty or half-drawn, and only the last held the
# content the gate exists to look at.
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
        grep -E 'sophia_native_composition_region schema=(2|3|4) status=read ' "$file" || true
    ) | awk -F'\t' '{ record[$1] = $0 } END { for (key in record) print record[key] }' | sort
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

printf '%-32s %8s %9s %9s %8s  %s\n' \
    REGION LIT CENTROID 'AFTER' DELTA VERDICT
while IFS=$'\t' read -r key before_mean before_histogram before_checksum; do
    match="$(printf '%s\n' "$after_records" | grep -F -m1 "$(printf '%s\t' "$key")" || true)"
    before_lit="$(lit_pixels "$before_histogram")"
    before_centroid="$(centroid_millis "$before_histogram")"
    if [[ -z "$match" ]]; then
        printf '%-32s %8s %9s %9s %8s  %s\n' \
            "$key" "$before_lit" "$before_centroid" - - "absent-after"
        unmatched=$(( unmatched + 1 ))
        continue
    fi
    after_histogram="$(printf '%s' "$match" | cut -f3)"
    after_checksum="$(printf '%s' "$match" | cut -f4)"
    after_centroid="$(centroid_millis "$after_histogram")"
    delta=$(( after_centroid - before_centroid ))

    # A region with no lit pixels has no edges to judge, so it is neither
    # evidence for the change nor against it. Saying so beats printing a
    # centroid of zero that looks like a measurement.
    if (( before_lit == 0 )); then
        verdict="unlit"
    elif [[ "$before_checksum" == "$after_checksum" ]]; then
        verdict="identical"
        held=$(( held + 1 ))
    elif (( delta > 0 )); then
        verdict="edges-brighter"
        moved=$(( moved + 1 ))
    elif (( delta < 0 )); then
        verdict="edges-darker"
        moved=$(( moved + 1 ))
    else
        verdict="changed-not-in-centroid"
        moved=$(( moved + 1 ))
    fi

    printf '%-32s %8s %9s %9s %+8d  %s\n' \
        "$key" "$before_lit" "$before_centroid" "$after_centroid" "$delta" "$verdict"
    if [[ "$before_histogram" != "$after_histogram" ]]; then
        printf '    before %s\n    after  %s\n' "$before_histogram" "$after_histogram"
    fi
done < <(printf '%s\n' "$before_records")

printf '\nregions moved=%d identical=%d absent-after=%d\n' \
    "$moved" "$held" "$unmatched"
cat <<'NOTE'
LIT counts pixels outside the background bucket -- the anti-aliased edges a
filter acts on. CENTROID is where those edges sit on the dark-to-light axis, in
thousandths of a bucket, and is a ratio so it does not move when a run happens
to draw more text. Histograms are 16 buckets, dark to light; bucket zero is
background and is excluded from both figures.
NOTE
