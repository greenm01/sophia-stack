#!/bin/sh
set -eu

root_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
temp_dir=$(mktemp -d)
trap 'rm -rf -- "$temp_dir"' EXIT HUP INT TERM

grep -Fq 'bind "Super+Shift+f" "policy:toggle-fullscreen"' \
    "$root_dir/crates/sophia-config/src/desktop_profile.rs"
grep -Fq 'bind "Super+Shift+b" "policy:minimize"' \
    "$root_dir/crates/sophia-config/src/desktop_profile.rs"
grep -Fq 'bind "Super+Alt+b" "policy:restore-minimized"' \
    "$root_dir/crates/sophia-config/src/desktop_profile.rs"
grep -Fq 'bind "Super+Shift+Right" "policy:move-to-output-next"' \
    "$root_dir/crates/sophia-config/src/desktop_profile.rs"
grep -Fq 'bind "Super+Shift+Left" "policy:move-to-output-prev"' \
    "$root_dir/crates/sophia-config/src/desktop_profile.rs"
grep -Fq 'shell { enabled #true; }' \
    "$root_dir/crates/sophia-config/src/desktop_profile.rs"
grep -Fq 'bind "Super+p" "session:window-switcher"' \
    "$root_dir/crates/sophia-config/src/desktop_profile.rs"
grep -Fq "show_step 'Press Super+Shift+F once." \
    "$root_dir/tools/fixtures/hagia_physical_guide.sh"
grep -Fq '1. Press and release Super+Shift+B.' \
    "$root_dir/tools/fixtures/hagia_physical_guide.sh"
grep -Fq '3. Press and release Super+Alt+B anyway.' \
    "$root_dir/tools/fixtures/hagia_physical_guide.sh"

evidence="$temp_dir/evidence.log"
marker="$temp_dir/restart.marker"
proof_result="$temp_dir/proof.result"

printf '%s\n' \
    'sophia_live_metadata_broker schema=1 status=ready protected=true peer_pid=4321 revision=1' \
    'sophia_live_metadata_shell schema=1 status=ready protected=true peer_pid=4322 revision=1 connection_epoch=1' \
    'sophia_live_metadata_broker schema=1 status=descriptor_committed surface=7 content=redacted' \
    'sophia_live_wm schema=1 status=physical_action_committed action=37' \
    'sophia_live_wm schema=1 status=physical_action_committed action=66' \
    'sophia_live_wm schema=4 status=proof_restart_armed adapter=sophia_wm_v1 boundary=checkpoint_replace action=66' \
    '2026-08-09T00:00:00Z INF hagia event=checkpoint status=saved detail="candidate_nonempty=true"' \
    'sophia_live_wm schema=4 status=proof_restart_triggered adapter=sophia_wm_v1 phase=checkpoint_saved action=66 preserved_layout=true' \
    'sophia_live_wm schema=4 status=restarted adapter=sophia_wm_v1 epoch=2 restarts=1 preserved_layout=true' \
    '2026-08-09T00:00:01Z INF hagia event=checkpoint status=loaded detail="candidate_nonempty=true"' \
    '2026-08-09T00:00:01Z INF hagia event=checkpoint status=reconciled detail="candidate_nonempty=true"' \
    '2026-08-09T00:00:01Z INF hagia event=policy_refresh status=requested detail=checkpoint_reconciled' \
    'sophia_live_wm schema=1 status=physical_action_committed action=37' \
    'sophia_live_wm schema=1 status=physical_action_committed action=66' \
    'sophia_live_indicator_input schema=1 status=activated output=1 action=12' \
    'sophia_live_wm schema=1 status=physical_action_committed action=12' \
    'sophia_live_indicator_input schema=1 status=activated output=1 action=11' \
    'sophia_live_wm schema=1 status=physical_action_committed action=11' \
    'sophia_live_wm schema=1 status=physical_action_committed action=38' \
    'sophia_live_wm schema=1 status=physical_action_committed action=38' \
    'sophia_live_wm schema=1 status=physical_action_committed action=39' \
    'sophia_live_wm schema=1 status=physical_action_committed action=40' \
    '2026-08-09T00:00:01Z INF hagia event=checkpoint status=saved detail="candidate_nonempty=true"' \
    'sophia_live_wm schema=1 status=physical_action_committed action=5' \
    '2026-08-09T00:00:01Z INFO sophia_live_native_head_page_flip schema=2 status=submitted output=2 head=2 submission=9 content=Some(MixedPresent { frame: LiveProductionNativeFrameId(93), transaction: TransactionId(61), nonzero_rgb_pixels: 2246 }) frame=93' \
    'sophia_live_wm schema=1 status=physical_action_committed action=6' \
    'sophia_live_wm schema=1 status=physical_action_committed action=33' \
    'sophia_live_wm schema=1 status=physical_action_committed action=34' \
    'hagia_policy_projection schema=1 status=active_output_changed' \
    'sophia_live_metadata_broker schema=1 status=descriptor_committed surface=8 content=redacted' \
    'sophia_live_metadata_shell schema=1 status=shortcut_admitted action=descriptor_switcher' \
    '2026-08-09T00:00:02Z INFO sophia_live_native_head_page_flip schema=2 status=submitted output=1 head=1 submission=10 content=Some(HeadComposition { frame: LiveProductionNativeFrameId(94), transaction: TransactionId(62), nonzero_rgb_pixels: 1800 }) frame=94' \
    'sophia_live_metadata_shell schema=1 status=presented candidate_generation=1 presentation_epoch=10 output=1 visible=true' \
    'sophia_live_metadata_broker schema=1 status=issuer_validated activation=1 target=redacted' \
    'sophia_live_metadata_shell schema=1 status=activation_admitted activation=1 target=redacted' \
    'sophia_live_metadata_shell schema=1 status=presented candidate_generation=2 presentation_epoch=11 output=1 visible=false' \
    'sophia_live_metadata_shell schema=1 status=shortcut_admitted action=descriptor_switcher' \
    '2026-08-09T00:00:03Z INFO sophia_live_native_head_page_flip schema=2 status=submitted output=1 head=1 submission=11 content=Some(HeadComposition { frame: LiveProductionNativeFrameId(95), transaction: TransactionId(63), nonzero_rgb_pixels: 1800 }) frame=95' \
    'sophia_live_metadata_shell schema=1 status=presented candidate_generation=3 presentation_epoch=12 output=1 visible=true' \
    'sophia_live_metadata_shell schema=1 status=proof_restart_triggered visible_presentation=2 retained_pixels=true' \
    'sophia_live_metadata_shell schema=1 status=reconnected protected=true peer_pid=4323 revision=1 connection_epoch=2 reason=proof_visible_restart' \
    'sophia_live_metadata_shell schema=1 status=proof_inert_click observed=true activation=false' \
    'sophia_live_metadata_shell schema=1 status=shortcut_admitted action=descriptor_switcher' \
    '2026-08-09T00:00:04Z INFO sophia_live_native_head_page_flip schema=2 status=submitted output=1 head=1 submission=12 content=Some(HeadComposition { frame: LiveProductionNativeFrameId(96), transaction: TransactionId(64), nonzero_rgb_pixels: 1800 }) frame=96' \
    'sophia_live_metadata_shell schema=1 status=presented candidate_generation=4 presentation_epoch=13 output=1 visible=true' \
    'sophia_live_metadata_broker schema=1 status=issuer_validated activation=2 target=redacted' \
    'sophia_live_metadata_shell schema=1 status=activation_duplicate activation=2 target=redacted' \
    'sophia_live_metadata_shell schema=1 status=presented candidate_generation=5 presentation_epoch=14 output=1 visible=false' \
    'sophia_live_session_input schema=2 status=complete source=physical text=hagiapolicyproof expected_events=34 matched_events=34 pixel_change=true' \
    'sophia_live_session schema=16 status=bounded_complete physical_input=enabled native_in_flight=false native_cleanup_pending=false native_submit_failures=0 wm_restarts=1 wm_degraded=false complete=true' \
    'sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false' \
    'sophia_live_output_topology_health schema=1 status=clean quarantined=false' \
    'sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed' \
    'sophia_live_metadata_shell schema=1 status=stopped transport=disconnected process=terminated' \
    'sophia_live_metadata_broker schema=1 status=stopped transport=disconnected process=terminated' \
    'sophia_hagia_policy_identity schema=2 status=bound sophia_commit=1111111111111111111111111111111111111111 hagia_commit=2222222222222222222222222222222222222222 sophia_sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa hagia_sha256=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb hagia_shell_sha256=cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc' \
    >"$evidence"

set +e
env \
    SOPHIA_HAGIA_BIN=/usr/bin/sleep \
    SOPHIA_HAGIA_RESTART_MARKER="$marker" \
    SOPHIA_HAGIA_RESTART_AFTER_ACTION=66 \
    SOPHIA_HAGIA_RESTART_REQUIRES_ACTION=37 \
    SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$evidence" \
    "$root_dir/tools/fixtures/hagia_restart_once.sh" 30
wrapper_status=$?
set -e

if [ "$wrapper_status" -ne 137 ] || [ ! -e "$marker" ]; then
    echo "Hagia restart matcher did not terminate its fixture process" >&2
    exit 1
fi

set +e
printf '%s\n' hagiapolicyproof | timeout 2s env \
    SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$evidence" \
    SOPHIA_INPUT_PROOF_RESULT="$proof_result" \
    "$root_dir/tools/fixtures/hagia_physical_guide.sh" >/dev/null
guide_status=$?
set -e

if [ "$guide_status" -ne 124 ]; then
    echo "Hagia physical guide exited unexpectedly: $guide_status" >&2
    exit 1
fi
if [ "$(cat "$proof_result" 2>/dev/null || true)" != hagiapolicyproof ]; then
    echo "Hagia physical guide did not cross its structured checkpoint matcher" >&2
    exit 1
fi

"$root_dir/tools/verify_hagia_policy_physical.sh" "$evidence" hagiapolicyproof >/dev/null

for missing in \
    'sophia_live_metadata_broker schema=1 status=ready' \
    'sophia_live_metadata_broker schema=1 status=descriptor_committed' \
    'sophia_live_metadata_shell schema=1 status=ready' \
    'sophia_live_metadata_shell schema=1 status=reconnected' \
    'sophia_live_metadata_broker schema=1 status=issuer_validated' \
    'sophia_live_metadata_shell schema=1 status=proof_inert_click' \
    'sophia_live_metadata_shell schema=1 status=stopped' \
    'sophia_live_metadata_broker schema=1 status=stopped' \
    'sophia_live_wm schema=1 status=physical_action_committed action=5' \
    'sophia_live_wm schema=1 status=physical_action_committed action=6' \
    'sophia_live_indicator_input schema=1 status=activated output=1 action=12' \
    'sophia_live_indicator_input schema=1 status=activated output=1 action=11' \
    'nonzero_rgb_pixels: 2246' \
    'nonzero_rgb_pixels: 1800' \
    'sophia_hagia_policy_identity schema=2 status=bound'; do
    rejected="$temp_dir/rejected.log"
    grep -vF "$missing" "$evidence" >"$rejected"
    if "$root_dir/tools/verify_hagia_policy_physical.sh" \
        "$rejected" hagiapolicyproof >/dev/null 2>&1; then
        echo "Hagia physical verifier accepted evidence without: $missing" >&2
        exit 1
    fi
done

failed="$temp_dir/failed.log"
cp "$evidence" "$failed"
printf '%s\n' \
    'sophia_live_metadata_broker schema=1 status=failed stage=shutdown transport=failed process=terminated' \
    >>"$failed"
if "$root_dir/tools/verify_hagia_policy_physical.sh" \
    "$failed" hagiapolicyproof >/dev/null 2>&1; then
    echo "Hagia physical verifier accepted a broker shutdown failure" >&2
    exit 1
fi

shell_failed="$temp_dir/shell-failed.log"
cp "$evidence" "$shell_failed"
printf '%s\n' \
    'sophia_live_metadata_shell schema=1 status=unavailable reason=retry retry_ms=250 error=fixture' \
    >>"$shell_failed"
if "$root_dir/tools/verify_hagia_policy_physical.sh" \
    "$shell_failed" hagiapolicyproof >/dev/null 2>&1; then
    echo "Hagia physical verifier accepted a Hagia Shell transport failure" >&2
    exit 1
fi

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
    SOPHIA_HAGIA_POLICY_SOPHIA_BIN="$sophia_bin" \
    SOPHIA_HAGIA_BIN="$hagia_bin" \
    SOPHIA_HAGIA_SHELL_BIN="$hagia_shell_bin" \
    "$root_dir/tools/archive_hagia_policy_physical_run.sh" \
    "$archive_evidence" hagiapolicyproof)"
run_dir="${archive_output##*: }"
SOPHIA_HAGIA_ROOT="$hagia_root" \
    "$root_dir/tools/verify_hagia_policy_physical_archive.sh" "$run_dir" >/dev/null

sed -i \
    's/^hagia_commit=.*/hagia_commit=ffffffffffffffffffffffffffffffffffffffff/' \
    "$run_dir/manifest"
(
    cd "$run_dir"
    sha256sum manifest result.kdl session.log >SHA256SUMS
)
if SOPHIA_HAGIA_ROOT="$hagia_root" \
    "$root_dir/tools/verify_hagia_policy_physical_archive.sh" \
    "$run_dir" >/dev/null 2>&1; then
    echo "Hagia physical archive verifier accepted an unknown Hagia commit" >&2
    exit 1
fi

printf '%s\n' 'Hagia physical matchers accepted structured checkpoint evidence.'
