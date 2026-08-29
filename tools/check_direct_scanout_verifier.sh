#!/usr/bin/env bash
set -euo pipefail

# Proves the direct-scanout session verifier rejects what it claims to reject.
#
# A verifier nobody has watched fail is a verifier that passes everything. Each
# mutation below removes exactly one of its rules and must be refused with the
# message that rule owns.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
verifier="$ROOT_DIR/tools/verify_direct_scanout_sessions.sh"
temp_dir="$(mktemp -d)"
trap 'rm -rf -- "$temp_dir"' EXIT

resources='sophia_live_native_resources schema=11 status=complete target_creations=6 renderer_workers=1 worker_result_misroutes=0 worker_max_service_skew=0 direct_scanout_attempts=44 direct_scanout_flips=43 direct_scanout_tests=2 direct_scanout_test_rejections=0 direct_scanout_refusals=0 direct_scanout_fallbacks=1'
verdicts='sophia_live_direct_scanout_verdicts schema=1 status=complete eligible=44 layer_count=6 layer_not_active=0 layer_resampled=0 layer_not_full_head=0 layer_not_dma_buf=2 layer_translucent=0 composition_required=0 composed_cursor=0'

write_pass() {
    local path="$1"
    {
        printf '%s\n' "$resources"
        printf '%s\n' "$verdicts"
        printf 'sophia_live_direct_scanout schema=1 status=exported output=1 scene_generation=11 reason=none\n'
        printf 'sophia_live_direct_scanout schema=1 status=test_passed output=1 scene_generation=11 reason=none\n'
        printf 'sophia_live_direct_scanout schema=1 status=flipped output=1 scene_generation=11 reason=none\n'
        printf 'sophia_live_direct_scanout schema=1 status=exported output=1 scene_generation=12 reason=none\n'
        printf 'sophia_live_direct_scanout schema=1 status=fell_back output=1 scene_generation=12 reason=none\n'
    } >"$path"
}

pass="$temp_dir/pass.log"
write_pass "$pass"
"$verifier" "$pass" >/dev/null || {
    echo "the verifier rejected a run that should pass" >&2
    exit 1
}

reject() {
    local name="$1" log="$2" expected="$3" output
    if output="$("$verifier" "$log" 2>&1)"; then
        echo "the verifier accepted $name" >&2
        exit 1
    fi
    printf '%s\n' "$output" | grep -Fq "$expected" || {
        echo "the verifier refused $name for the wrong reason:" >&2
        printf '%s\n' "$output" >&2
        exit 1
    }
}

# The whole point of the run: a session in which nothing was ever eligible has
# measured nothing, and must say so rather than pass on zeros.
quiet="$temp_dir/quiet.log"
sed -e 's/ direct_scanout_attempts=44/ direct_scanout_attempts=0/' \
    -e 's/ direct_scanout_flips=43/ direct_scanout_flips=0/' \
    -e 's/ direct_scanout_tests=2/ direct_scanout_tests=0/' \
    -e 's/ direct_scanout_fallbacks=1/ direct_scanout_fallbacks=0/' \
    -e 's/ eligible=44/ eligible=0/' \
    -e '/sophia_live_direct_scanout schema=1/d' \
    "$pass" >"$quiet"
reject "a run in which direct scanout never engaged" "$quiet" \
    "direct scanout never engaged"

# Engine's proof disagreeing with the pixels it was computed from.
disagreed="$temp_dir/disagreed.log"
sed 's/ direct_scanout_refusals=0/ direct_scanout_refusals=1/' "$pass" >"$disagreed"
reject "a run whose proof disagreed with its pixels" "$disagreed" \
    "disagreed with the frame it lowered"

# A client buffer on a plane the driver was never asked about.
untested="$temp_dir/untested.log"
sed 's/ direct_scanout_tests=2/ direct_scanout_tests=0/' "$pass" >"$untested"
reject "a direct flip with no validating commit" "$untested" \
    "no validating commit"

# Counters describing different frames.
overcounted="$temp_dir/overcounted.log"
sed 's/ direct_scanout_attempts=44/ direct_scanout_attempts=43/' "$pass" >"$overcounted"
reject "more settled attempts than attempts" "$overcounted" \
    "settled than were made"

refused_more="$temp_dir/refused_more.log"
sed 's/ direct_scanout_test_rejections=0/ direct_scanout_test_rejections=3/' "$pass" >"$refused_more"
reject "more refused validating commits than issued" "$refused_more" \
    "refused than issued"

# The episode order. A flip whose scene was never exported cannot have carried
# that scene's buffer, whatever the counters say.
unordered="$temp_dir/unordered.log"
grep -v 'status=exported output=1 scene_generation=11' "$pass" >"$unordered"
reject "a flip for a scene never exported" "$unordered" \
    "never exported"

# The flip's own guard, controlled separately: here a test did pass for an
# earlier scene, so the guard on `test_passed` is satisfied and only the guard
# on `flipped` can refuse this. Without a control of its own that guard could
# be deleted and every other case here would still pass.
orphan="$temp_dir/orphan.log"
{
    printf '%s\n' "$resources"
    printf '%s\n' "$verdicts"
    printf 'sophia_live_direct_scanout schema=1 status=exported output=1 scene_generation=11 reason=none\n'
    printf 'sophia_live_direct_scanout schema=1 status=test_passed output=1 scene_generation=11 reason=none\n'
    printf 'sophia_live_direct_scanout schema=1 status=fell_back output=1 scene_generation=11 reason=none\n'
    printf 'sophia_live_direct_scanout schema=1 status=flipped output=1 scene_generation=12 reason=none\n'
} >"$orphan"
reject "a flip whose own scene was never exported" "$orphan" \
    "a direct flip happened for a scene never exported"

# Evidence from a build that predates the row.
stale="$temp_dir/stale.log"
sed 's/sophia_live_native_resources schema=11/sophia_live_native_resources schema=10/' "$pass" >"$stale"
reject "evidence from a build without the direct path" "$stale" \
    "did not run this build"

# The verdict histogram is the diagnostic; a run without it cannot say why.
blind="$temp_dir/blind.log"
grep -v 'sophia_live_direct_scanout_verdicts' "$pass" >"$blind"
reject "a run that reported no eligibility verdicts" "$blind" \
    "no eligibility verdicts"

# Two sessions must describe the same histogram, or the totals are nonsense.
second="$temp_dir/second.log"
sed 's/ layer_not_dma_buf=2 / layer_not_dmabuf=2 /' "$pass" >"$second"
if output="$("$verifier" "$pass" "$second" 2>&1)"; then
    echo "the verifier accepted sessions disagreeing on the histogram shape" >&2
    exit 1
fi
printf '%s\n' "$output" | grep -Fq "disagree on the verdict histogram" || {
    echo "the verifier refused mismatched histograms for the wrong reason:" >&2
    printf '%s\n' "$output" >&2
    exit 1
}

echo "direct scanout session verifier checks passed"
