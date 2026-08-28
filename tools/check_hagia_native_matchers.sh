#!/bin/sh
set -eu

# Offline proof that the native-session guide, verifier, archiver, and archive
# verifier work before any hardware is taken over. A physical gate costs a
# session and an operator; every mistake caught here is one that does not.
#
# The evidence below is synthesized rather than captured, so the mutations are
# the point: each required line is deleted in turn and the verifier must reject
# what remains, and each failure mode the gate exists to catch is injected and
# must be refused.

root_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
temp_dir=$(mktemp -d)
trap 'rm -rf -- "$temp_dir"' EXIT HUP INT TERM

proof_text=hagianativeproof
guide="$root_dir/tools/fixtures/hagia_native_session_guide.sh"
verifier="$root_dir/tools/verify_hagia_native_session.sh"

# Source invariants the guide's instructions depend on. A binding renamed in the
# compiled profile without the guide following it would produce a run whose
# keystrokes commit nothing, which reads on screen as Sophia ignoring the
# operator.
for binding in \
    'bind "Super+Return" "session:spawn-terminal"' \
    'bind "Super+j" "policy:focus-next"' \
    'bind "Super+q" "session:close-window"' \
    'bind "Ctrl+Alt+Delete" "session:logout"'; do
    grep -Fq "$binding" "$root_dir/crates/sophia-config/src/desktop_profile.rs" || {
        echo "the compiled desktop profile no longer binds: $binding" >&2
        exit 1
    }
done

# The gate must hand the session an explicit profile and refuse without one.
# `--no-config` runs the compiled profile while an exported digest still names a
# file on disk, which is how a gate comes to print an identity for a profile it
# never loaded. The runner turns SOPHIA_DESKTOP_PROFILE into --desktop-profile,
# so the gate's obligation is to bind it, not to spell the flag.
grep -Fq 'SOPHIA_DESKTOP_PROFILE="$desktop_profile"' \
    "$root_dir/tools/hagia_native_session_gate.sh" || {
    echo "the native gate does not pass an explicit desktop profile" >&2
    exit 1
}
grep -Fq 'set SOPHIA_DESKTOP_PROFILE to the absolute Hagia profile' \
    "$root_dir/tools/hagia_native_session_gate.sh" || {
    echo "the native gate does not refuse an unset desktop profile" >&2
    exit 1
}
if grep -v '^[[:space:]]*#' "$root_dir/tools/hagia_native_session_gate.sh" \
    | grep -Fq -- '--no-config'; then
    echo "the native gate must not run the compiled profile behind a bound digest" >&2
    exit 1
fi
# Exact TTY recovery is an exit criterion, and only the runner produces it.
grep -Fq 'SOPHIA_TTY_PROFILE=hagia' "$root_dir/tools/hagia_native_session_gate.sh" || {
    echo "the native gate does not route through the hagia runner profile" >&2
    exit 1
}
# `--expect-physical-text` without `--max-runtime-ms` or `--max-ticks` is
# refused during argument validation, before any window appears. The runner
# passes no runtime bound of its own because ordinary sessions have no
# lifetime, so the gate must supply one. This restates a rule that lives in
# `PersistentXtermSessionConfig::from_args`, which is duplication worth its
# keep: nothing else here reaches the real parser, and the omission already
# cost one physical attempt that ended before Kitty appeared.
if grep -Fq -- '--expect-physical-text' "$root_dir/tools/hagia_native_session_gate.sh" \
    && ! grep -Eq -- '--(max-runtime-ms|max-ticks)' "$root_dir/tools/hagia_native_session_gate.sh"; then
    echo "the native gate requests an input proof without a bounded runtime" >&2
    exit 1
fi
# The guide belongs to the startup terminal alone. One application id carries
# one argument list, so pointing the launch action at the same id gives every
# new window its own copy of the guide -- which finds its waits already
# satisfied, exits, and takes the window with it. A physical attempt spent two
# Super+Return presses discovering that, with both terminals reaching
# `surface_observed` and then `normal_exit_after_surface`.
grep -Fq -- '--session-action-app=terminal=workflow-terminal' \
    "$root_dir/tools/hagia_native_session_gate.sh" || {
    echo "the native gate launches its workflow terminals from the guide's application" >&2
    exit 1
}
if grep -Fq -- '--session-app-arg=workflow-terminal=$guide' \
    "$root_dir/tools/hagia_native_session_gate.sh"; then
    echo "the native gate gives its workflow terminals a copy of the guide" >&2
    exit 1
fi
for step in \
    'Press Super+Return once.' \
    'Press Super+J once.' \
    'Press Super+q once to close the focused terminal.' \
    'Press Ctrl+Alt+Delete once to log out normally.'; do
    grep -Fq "$step" "$guide" || {
        echo "the native guide no longer instructs: $step" >&2
        exit 1
    }
done

evidence="$temp_dir/evidence.log"
proof_result="$temp_dir/proof.result"
profile_sha256=dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd

printf '%s\n' \
    "sophia_live_desktop_profile schema=1 status=loaded mode=packaged-promotion generation=1 digest=7 root_sha256=$profile_sha256 sources=1" \
    'sophia_live_wm schema=4 status=ready adapter=sophia_wm_v1 socket=session_owned epoch=1 restarts=0' \
    'sophia_live_native_startup_output schema=1 status=presented output=1 proof=synchronous_modeset submission=1' \
    'sophia_live_metadata_broker schema=1 status=ready protected=true peer_pid=4321 revision=1' \
    'sophia_live_metadata_shell schema=1 status=ready protected=true peer_pid=4322 revision=1 connection_epoch=1' \
    'sophia_live_metadata_broker schema=1 status=descriptor_committed surface=1 content=redacted' \
    "sophia_live_session_input schema=2 status=complete source=physical text=$proof_text expected_events=34 matched_events=34 pixel_change=true" \
    'sophia_live_wm schema=1 status=physical_action_committed action=29' \
    'sophia_live_wm schema=1 status=session_action_committed transaction=11 action=LaunchTerminal' \
    'sophia_session_app schema=2 status=admitted source=action transaction=11 surface=2' \
    'sophia_live_wm schema=2 status=workspace_projection_committed transaction=11 output=1 workspace=1 visible_surfaces=2 focus=surface' \
    'sophia_live_wm schema=1 status=physical_action_committed action=29' \
    'sophia_live_wm schema=1 status=session_action_committed transaction=12 action=LaunchTerminal' \
    'sophia_session_app schema=2 status=admitted source=action transaction=12 surface=3' \
    'sophia_live_wm schema=2 status=workspace_projection_committed transaction=12 output=1 workspace=1 visible_surfaces=3 focus=surface' \
    'sophia_live_wm schema=1 status=physical_action_committed action=29' \
    'sophia_live_wm schema=1 status=session_action_committed transaction=13 action=LaunchTerminal' \
    'sophia_session_app schema=2 status=admitted source=action transaction=13 surface=4' \
    'sophia_live_wm schema=2 status=workspace_projection_committed transaction=13 output=1 workspace=1 visible_surfaces=4 focus=surface' \
    'sophia_live_wm schema=1 status=physical_action_committed action=1' \
    'sophia_live_wm schema=2 status=workspace_projection_committed transaction=14 output=1 workspace=1 visible_surfaces=4 focus=surface' \
    'sophia_live_wm schema=1 status=physical_action_committed action=31' \
    'sophia_live_wm schema=1 status=session_action_committed transaction=15 action=CloseFocused' \
    'sophia_live_wm schema=2 status=workspace_projection_committed transaction=15 output=1 workspace=1 visible_surfaces=3 focus=surface' \
    'sophia_live_wm schema=1 status=physical_action_committed action=32' \
    'sophia_live_wm schema=1 status=session_action_committed transaction=16 action=Logout' \
    'sophia_live_session_control schema=2 status=complete enqueued=9 dispatched=9 delivered=9 stale_retired=0 rejected=0 timed_out=0 unexpected=0 pending=0 max_queue_dwell_msec=4 max_ack_msec=7' \
    'sophia_live_session_native_suspend schema=2 outcome=drained drained=true abandoned_scanouts=0 skipped_present=none' \
    'sophia_live_session_keys schema=2 status=complete pending=0 release_barrier_pending=0 peak_pressed=2 synthetic_releases=0 state_only_releases=0 orphan_releases_suppressed=0 removed_surface_keys=0 repeat_active_seats=0 repeat_armed=0 repeat_routed=0 repeat_pulses=0 repeat_coalesced=0 repeat_cancelled=0 repeat_capacity_exhausted=0' \
    'sophia_live_session schema=16 status=bounded_complete display=:292 elapsed_msec=91000 input_pixel_change=true input_text_match=true input_queue_dwell_max_msec=3 native_submit_failures=0 native_retire_failures=0 native_callback_rejected=0 native_callback_queue_saturated=0 native_max_submit_to_page_flip_msec=9 native_max_upload_msec=4 native_max_render_msec=6 native_nonzero_exports=612 native_mixed_exports=612 native_in_flight=false native_cleanup_pending=false wm_restarts=0 wm_degraded=false present_disconnect_failures=0 present_live_sources=0 present_live_fences=0 present_live_transactions=0' \
    'sophia_live_native_resources schema=7 status=complete worker_requests=640 worker_completions=612 worker_failures=0 worker_soft_stalls=0 worker_hard_stalls=0 worker_release_enqueue_failures=0 frame_slot_acquisitions=612 frame_slot_reuses=609 frame_slot_deferrals=28 frame_slot_stale_releases=0 frame_slots_leased=0 frame_slots_high_watermark=3 max_worker_request_msec=8' \
    'sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false' \
    'sophia_live_output_topology_health schema=1 status=clean quarantined=false' \
    'sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed' \
    'sophia_live_session_protocol_errors schema=1 expected=0 unexpected=0' \
    'sophia_live_metadata_shell schema=1 status=stopped transport=disconnected process=terminated' \
    'sophia_live_metadata_broker schema=1 status=stopped transport=disconnected process=terminated' \
    'sophia_tty_recovery schema=3 profile=hagia kd_mode_before=0 kd_mode_after=0 termios_restored=true emergency=false session_shutdown=not_requested session_exit_status=none' \
    "sophia_hagia_native_identity schema=1 status=bound sophia_commit=1111111111111111111111111111111111111111 hagia_commit=2222222222222222222222222222222222222222 sophia_sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa hagia_sha256=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb hagia_shell_sha256=cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc desktop_profile_sha256=$profile_sha256" \
    >"$evidence"

# The guide drives this evidence to completion: every step it waits for is
# present, so it must finish rather than stall. A guide that cannot cross its own
# passing log would hang an operator in front of a working session.
set +e
printf '%s\n' "$proof_text" | timeout 30s env \
    SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$evidence" \
    SOPHIA_INPUT_PROOF_RESULT="$proof_result" \
    SOPHIA_HAGIA_NATIVE_TEXT="$proof_text" \
    "$guide" >/dev/null
guide_status=$?
set -e
if [ "$guide_status" -ne 0 ]; then
    echo "the native guide did not cross its own passing evidence: $guide_status" >&2
    exit 1
fi
if [ "$(cat "$proof_result" 2>/dev/null || true)" != "$proof_text" ]; then
    echo "the native guide did not record the typed proof phrase" >&2
    exit 1
fi

# The guide's waits must be bounded. An unbounded wait turns a session that can
# no longer produce a line into a hang instead of a failure, which is how the
# switcher gate became unrunnable without saying so.
if grep -Eq 'while .*grep -Eq .*; do$' "$guide"; then
    echo "the native guide contains an unbounded evidence wait" >&2
    exit 1
fi

"$verifier" "$evidence" "$proof_text" >/dev/null

# Every required line, deleted one at a time.
for missing in \
    'sophia_live_desktop_profile schema=1 status=loaded' \
    'sophia_live_wm schema=4 status=ready' \
    'sophia_live_native_startup_output schema=1 status=presented' \
    'sophia_live_metadata_broker schema=1 status=ready' \
    'sophia_live_metadata_broker schema=1 status=descriptor_committed' \
    'sophia_live_metadata_shell schema=1 status=ready' \
    'sophia_live_metadata_shell schema=1 status=stopped' \
    'sophia_live_metadata_broker schema=1 status=stopped' \
    'sophia_live_session_input schema=2 status=complete' \
    'sophia_live_wm schema=1 status=session_action_committed transaction=13 action=LaunchTerminal' \
    'sophia_live_wm schema=1 status=session_action_committed transaction=15 action=CloseFocused' \
    'sophia_live_wm schema=1 status=session_action_committed transaction=16 action=Logout' \
    'sophia_session_app schema=2 status=admitted source=action transaction=13' \
    'sophia_live_wm schema=1 status=physical_action_committed action=1' \
    'sophia_live_session_control schema=2 status=complete' \
    'sophia_live_session_native_suspend schema=2' \
    'sophia_live_session_keys schema=2 status=complete' \
    'sophia_live_session schema=16 status=bounded_complete' \
    'sophia_live_native_resources schema=7 status=complete' \
    'sophia_live_session_health schema=1 status=clean' \
    'sophia_live_output_topology_health schema=1 status=clean' \
    'sophia_live_session_cleanup schema=1 status=clean' \
    'sophia_live_session_protocol_errors schema=1' \
    'sophia_tty_recovery schema=3 profile=hagia' \
    'sophia_hagia_native_identity schema=1 status=bound'; do
    rejected="$temp_dir/rejected.log"
    grep -vF "$missing" "$evidence" >"$rejected"
    if "$verifier" "$rejected" "$proof_text" >/dev/null 2>&1; then
        echo "the native verifier accepted evidence without: $missing" >&2
        exit 1
    fi
done

# A mutation rejected for the wrong reason proves nothing about the check it was
# written for, so each one states the reason it expects. Without this, a sed that
# silently stopped matching would still look like a passing negative case as long
# as the evidence failed some other way.
reject_mutation() {
    description="$1"
    mutated="$2"
    expected="$3"
    reason="$("$verifier" "$mutated" "$proof_text" 2>&1 >/dev/null || true)"
    if "$verifier" "$mutated" "$proof_text" >/dev/null 2>&1; then
        echo "the native verifier accepted $description" >&2
        exit 1
    fi
    case "$reason" in
        *"$expected"*) ;;
        *)
            echo "the native verifier rejected $description for the wrong reason:" >&2
            echo "  expected: $expected" >&2
            echo "  observed: $reason" >&2
            exit 1
            ;;
    esac
}

# A slot still leased after the session drained is a page flip that retired
# without releasing its buffer. Nothing else in the tree checks this today.
leaked="$temp_dir/leaked.log"
sed 's/frame_slots_leased=0/frame_slots_leased=1/' "$evidence" >"$leaked"
reject_mutation "a leaked frame-slot lease" "$leaked" \
    "a native frame slot was still leased at completion"

stale="$temp_dir/stale.log"
sed 's/frame_slot_stale_releases=0/frame_slot_stale_releases=2/' "$evidence" >"$stale"
reject_mutation "a refused stale slot release" "$stale" \
    "frame_slot_stale_releases must be zero"

# Requests must settle as completions or bounded deferrals. A request that did
# neither is a frame the renderer silently dropped.
unbalanced="$temp_dir/unbalanced.log"
sed 's/frame_slot_deferrals=28/frame_slot_deferrals=27/' "$evidence" >"$unbalanced"
reject_mutation "an unbalanced renderer-worker ledger" "$unbalanced" \
    "did not settle as completion or bounded deferral"

overcapacity="$temp_dir/overcapacity.log"
sed 's/frame_slots_high_watermark=3/frame_slots_high_watermark=4/' "$evidence" >"$overcapacity"
reject_mutation "a frame-slot pool above its three-slot capacity" "$overcapacity" \
    "exceeded its three-slot capacity"

slow="$temp_dir/slow.log"
sed 's/max_ack_msec=7/max_ack_msec=140/' "$evidence" >"$slow"
reject_mutation "session-control latency above its budget" "$slow" \
    "session-control latency exceeded 100ms"

diverged="$temp_dir/diverged.log"
sed 's/delivered=9 stale_retired=0/delivered=8 stale_retired=0/' "$evidence" >"$diverged"
reject_mutation "a session-control ledger that did not balance" "$diverged" \
    "enqueue, dispatch, and delivery counts diverged"

# Ordering, not just presence. A focus change committed before the launches, or
# a close before the focus change, is not the workflow this gate specifies.
reordered="$temp_dir/reordered.log"
grep -vF 'sophia_live_wm schema=1 status=physical_action_committed action=1' "$evidence" \
    | awk '
        { print }
        /^sophia_live_metadata_broker schema=1 status=descriptor_committed / {
            print "sophia_live_wm schema=1 status=physical_action_committed action=1"
        }
    ' >"$reordered"
reject_mutation "a focus change committed before its launches" "$reordered" \
    "focus-next was committed before the third terminal launch"

# A restarted or degraded WM is a different run than the one being promoted.
restarted="$temp_dir/restarted.log"
sed 's/wm_restarts=0/wm_restarts=1/' "$evidence" >"$restarted"
reject_mutation "a session whose WM restarted" "$restarted" \
    "completion does not contain wm_restarts=0"

# The compatibility bridge has its own gates.
bridged="$temp_dir/bridged.log"
cp "$evidence" "$bridged"
printf '%s\n' \
    'legacy WM did not configure all 4 synthetic windows within 3000 ms (configured 0)' \
    >>"$bridged"
reject_mutation "xmonad compatibility bridge activity" "$bridged" \
    "xmonad compatibility bridge activity"

# The identity must describe the profile that ran.
misreported="$temp_dir/misreported.log"
sed "s/root_sha256=$profile_sha256/root_sha256=eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee/" \
    "$evidence" >"$misreported"
reject_mutation "an identity naming a profile the session did not load" "$misreported" \
    "the loaded desktop profile is not the one bound to this run"

emergency="$temp_dir/emergency.log"
sed 's/emergency=false session_shutdown=not_requested/emergency=true session_shutdown=graceful/' \
    "$evidence" >"$emergency"
reject_mutation "an emergency exit as a normal logout" "$emergency" \
    "missing exact TTY recovery"

protocol="$temp_dir/protocol.log"
sed 's/^sophia_live_session_protocol_errors schema=1 expected=0 unexpected=0$/sophia_live_session_protocol_errors schema=1 expected=0 unexpected=3/' \
    "$evidence" >"$protocol"
reject_mutation "unexpected X protocol errors" "$protocol" \
    "missing zero unexpected protocol errors"

# An extra keypress the guide never asked for means the session that ran is not
# the session that was specified.
extra="$temp_dir/extra.log"
cp "$evidence" "$extra"
printf '%s\n' 'sophia_live_wm schema=1 status=physical_action_committed action=38' >>"$extra"
reject_mutation "an action the guide never requested" "$extra" \
    "the guide never asked for it"

# Archive and re-verify, then prove the archive verifier refuses a manifest that
# no longer matches its own evidence.
sophia_bin="$temp_dir/sophia"
hagia_bin="$temp_dir/hagia"
hagia_shell_bin="$temp_dir/hagia-shell"
cp /usr/bin/true "$sophia_bin"
cp /usr/bin/false "$hagia_bin"
cp /usr/bin/true "$hagia_shell_bin"
sophia_commit="$(git -C "$root_dir" rev-parse HEAD)"
hagia_root="${SOPHIA_HAGIA_ROOT:-$root_dir/../hagia}"
hagia_commit="$(git -C "$hagia_root" rev-parse HEAD)"
sophia_sha256="$(sha256sum "$sophia_bin" | awk '{ print $1 }')"
hagia_sha256="$(sha256sum "$hagia_bin" | awk '{ print $1 }')"
hagia_shell_sha256="$(sha256sum "$hagia_shell_bin" | awk '{ print $1 }')"
archive_evidence="$temp_dir/archive-evidence.log"
sed \
    -e "s/sophia_commit=1111111111111111111111111111111111111111/sophia_commit=$sophia_commit/" \
    -e "s/hagia_commit=2222222222222222222222222222222222222222/hagia_commit=$hagia_commit/" \
    -e "s/sophia_sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/sophia_sha256=$sophia_sha256/" \
    -e "s/hagia_sha256=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb/hagia_sha256=$hagia_sha256/" \
    -e "s/hagia_shell_sha256=cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc/hagia_shell_sha256=$hagia_shell_sha256/" \
    "$evidence" >"$archive_evidence"
archive_output="$(env \
    XDG_STATE_HOME="$temp_dir/state" \
    SOPHIA_HAGIA_ROOT="$hagia_root" \
    SOPHIA_HAGIA_NATIVE_SOPHIA_BIN="$sophia_bin" \
    SOPHIA_HAGIA_BIN="$hagia_bin" \
    SOPHIA_HAGIA_SHELL_BIN="$hagia_shell_bin" \
    "$root_dir/tools/archive_hagia_native_session_run.sh" \
    "$archive_evidence" "$proof_text")"
run_dir="${archive_output##*: }"
SOPHIA_HAGIA_ROOT="$hagia_root" \
    "$root_dir/tools/verify_hagia_native_session_archive.sh" "$run_dir" >/dev/null

# The same evidence must not become two proofs.
if env \
    XDG_STATE_HOME="$temp_dir/state" \
    SOPHIA_HAGIA_ROOT="$hagia_root" \
    SOPHIA_HAGIA_NATIVE_SOPHIA_BIN="$sophia_bin" \
    SOPHIA_HAGIA_BIN="$hagia_bin" \
    SOPHIA_HAGIA_SHELL_BIN="$hagia_shell_bin" \
    "$root_dir/tools/archive_hagia_native_session_run.sh" \
    "$archive_evidence" "$proof_text" >/dev/null 2>&1; then
    echo "the native archiver recorded the same evidence twice" >&2
    exit 1
fi

sed -i \
    's/^hagia_commit=.*/hagia_commit=ffffffffffffffffffffffffffffffffffffffff/' \
    "$run_dir/manifest"
(
    cd "$run_dir"
    sha256sum manifest result.kdl session.log >SHA256SUMS
)
if SOPHIA_HAGIA_ROOT="$hagia_root" \
    "$root_dir/tools/verify_hagia_native_session_archive.sh" \
    "$run_dir" >/dev/null 2>&1; then
    echo "the native archive verifier accepted an unknown Hagia commit" >&2
    exit 1
fi

printf '%s\n' 'Hagia native matchers accepted the bounded three-launch workflow.'
