#!/usr/bin/env bash
set -euo pipefail

# Physical output-authority gate. Run it from tty4, not from the terminal you work in.
#
#   Ctrl+Alt+F4, log in, then:
#     ~/dev/sophia-stack/tools/run_native_output_gate_tty4.sh
#
# Three phases, each read-only unless the last is explicitly armed:
#
#   probe     Does a TEST_ONLY modeset need plane state and a valid FB_ID?
#   validate  Does the kernel accept the configured desktop as one atomic request?
#   apply     Drive the outputs for real. Armed only with SOPHIA_NATIVE_OUTPUT_APPLY=1.
#
# Why tty4. Apply is the only phase that can change what a monitor shows, and the
# failure it cannot rule out is a dark screen. Running here leaves the VT you work
# on untouched, so recovery is Ctrl+Alt+F2 rather than a reboot. This process holds
# DRM master only while it runs and always exits; switching VT away and back makes
# the kernel restore the console.
#
# Everything builds before the first DRM ioctl, so a compile error cannot strand
# the outputs mid-gate.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_ROOT="${SOPHIA_NATIVE_OUTPUT_GATE_ROOT:-/tmp/sophia-native-output-gate}"
APPLY="${SOPHIA_NATIVE_OUTPUT_APPLY:-0}"
REQUIRE_TTY="${SOPHIA_NATIVE_OUTPUT_GATE_TTY:-/dev/tty4}"

# shellcheck source=tools/lib/drm_master_guard.sh
. "$ROOT_DIR/tools/lib/drm_master_guard.sh"

if [[ ! -t 0 || "$(tty)" != "$REQUIRE_TTY" ]]; then
    echo "This gate runs on $REQUIRE_TTY so a failed apply cannot take your working VT." >&2
    echo "Switch with Ctrl+Alt+F4, log in, and run:" >&2
    echo "  $ROOT_DIR/tools/run_native_output_gate_tty4.sh" >&2
    echo >&2
    echo "Set SOPHIA_NATIVE_OUTPUT_GATE_TTY to override, but understand what you lose:" >&2
    echo "a dark screen on the VT you are reading this from has no second terminal." >&2
    exit 1
fi

# A physical result names the exact tree it came from, or it is not evidence.
if [[ -n "$(git -C "$ROOT_DIR" status --short)" ]]; then
    echo "Sophia worktree must be clean before a physical gate." >&2
    git -C "$ROOT_DIR" status --short >&2
    exit 1
fi
commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"

sophia_require_drm_master_available SOPHIA_NATIVE_OUTPUT_GATE_FORCE || exit 1

# Build before taking the card. A cargo invocation mid-gate would compete for the
# terminal at exactly the moment the outputs might be mid-change.
echo "Building sophia-cli at $commit before DRM takeover..."
(
    cd "$ROOT_DIR"
    cargo build --quiet --release --offline -p sophia-cli --features "atomic-scanout-live"
)
sophia_bin="$ROOT_DIR/target/release/sophia"
[[ -x "$sophia_bin" ]] || {
    echo "built binary is missing at $sophia_bin" >&2
    exit 1
}
binary_sha="$(sha256sum "$sophia_bin" | cut -d' ' -f1)"

evidence="$EVIDENCE_ROOT/${commit:0:12}"
mkdir -p "$evidence"

{
    echo "commit=$commit"
    echo "binary_sha256=$binary_sha"
    echo "tty=$(tty)"
    echo "apply_armed=$APPLY"
    echo "host=$(uname -n)"
    echo "kernel=$(uname -r)"
    echo "started_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$evidence/manifest.txt"

# Connector facts straight from sysfs, independent of anything Sophia reports.
# A transcript whose DRM claims disagree with this host's sysfs is not evidence
# about this host, whatever else it says.
for connector in /sys/class/drm/card*-*/; do
    name="$(basename "$connector")"
    printf '%s status=%s enabled=%s first_mode=%s\n' \
        "$name" \
        "$(cat "$connector/status" 2>/dev/null || echo unknown)" \
        "$(cat "$connector/enabled" 2>/dev/null || echo unknown)" \
        "$(head -1 "$connector/modes" 2>/dev/null || echo none)"
done > "$evidence/connectors.txt"

echo "Evidence: $evidence"
echo "Binary:   $binary_sha"
echo

phase_status=0
run_phase() {
    local name="$1" expect_mutation="$2"
    shift 2
    echo "== $name =="
    if [[ "$expect_mutation" == "mutates" ]]; then
        echo "-- this phase changes real output state --"
    fi
    set +e
    "$@" 2>&1 | tee "$evidence/$name.log"
    local status="${PIPESTATUS[0]}"
    set -e
    if [[ "$status" -ne 0 ]]; then
        echo "$name FAILED (exit $status)"
        phase_status=1
    else
        echo "$name passed"
    fi
    echo
    return 0
}

run_phase probe read-only "$sophia_bin" native-topology-probe
run_phase validate read-only "$sophia_bin" native-topology-validate

if [[ "$APPLY" == "1" ]]; then
    run_phase apply mutates env SOPHIA_NATIVE_OUTPUT_APPLY=1 "$sophia_bin" native-topology-apply
else
    echo "== apply =="
    echo "Not armed. Re-run with SOPHIA_NATIVE_OUTPUT_APPLY=1 to drive the outputs."
    echo "not_armed" > "$evidence/apply.log"
    echo
fi

reduce() {
    local file="$1" pattern="$2"
    grep -m1 "$pattern" "$evidence/$file" 2>/dev/null || echo "none"
}

echo "== summary =="
reduce probe.log '^sophia_native_topology_probe '
reduce validate.log '^sophia_native_topology_validate '
if [[ "$APPLY" == "1" ]]; then
    reduce apply.log '^sophia_native_topology_apply '
fi
echo
result="$([[ "$phase_status" -eq 0 ]] && echo passed || echo failed)"
echo "sophia_native_output_gate schema=1 commit=${commit:0:12} apply_armed=$APPLY result=$result"
echo "Evidence: $evidence"
echo

# One artifact to hand back. Everything in it is cross-checkable rather than
# self-asserted: the binary checksum must be what this commit builds, the connector
# facts must match this host's sysfs, and the phase lines must agree with both.
transcript="$evidence/transcript.txt"
{
    echo "=== sophia native output gate transcript ==="
    cat "$evidence/manifest.txt"
    echo "finished_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "result=$result"
    echo
    echo "--- sysfs connectors ---"
    cat "$evidence/connectors.txt"
    echo
    echo "--- probe ---"
    cat "$evidence/probe.log"
    echo
    echo "--- validate ---"
    cat "$evidence/validate.log"
    echo
    echo "--- apply ---"
    cat "$evidence/apply.log"
} > "$transcript"
# Digest last, over everything above it, so a truncated or edited paste stops
# matching. It proves internal consistency, not provenance.
printf 'transcript_sha256=%s\n' "$(sha256sum "$transcript" | cut -d' ' -f1)" >> "$transcript"

echo "================ paste everything below this line ================"
cat "$transcript"
echo "================ paste everything above this line ================"

exit "$phase_status"
