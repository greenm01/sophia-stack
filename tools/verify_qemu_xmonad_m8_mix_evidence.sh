#!/usr/bin/env bash
set -euo pipefail

evidence=${1:-/tmp/sophia-qemu-xmonad-m8-mix.log}
[[ -s "$evidence" ]] || { echo "missing M8 mix evidence: $evidence" >&2; exit 1; }

for app in terminal vulkan firefox launcher; do
    grep -q "^sophia_session_app schema=1 status=started id=$app " "$evidence" || {
        echo "M8 mix did not start $app" >&2
        exit 1
    }
done
for app in terminal firefox launcher; do
    grep -q "^sophia_session_app schema=1 status=started id=$app source=action$" "$evidence" || {
        echo "M8 mix did not action-launch $app" >&2
        exit 1
    }
    grep -Eq "^sophia_session_app schema=1 status=exited id=$app source=managed exit_status=exit status: 0$" "$evidence" || {
        echo "M8 mix did not normally close $app"
        exit 1
    }
done
close_actions=$(grep -c '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$' "$evidence" || true)
(( close_actions >= 3 )) || {
    echo "M8 mix observed only $close_actions committed close actions" >&2
    exit 1
}
close_key_clears=$(grep -c '^sophia_live_session_keys schema=1 status=cleared reason=close_surface .* count=[1-9][0-9]*$' "$evidence" || true)
(( close_key_clears >= 2 )) || {
    echo "M8 mix observed only $close_key_clears nonblocking close key clears" >&2
    exit 1
}
for stage in loaded keyboard clipboard primary resize dialog; do
    grep -q "^sophia_firefox_m8 schema=1 status=stage_complete stage=$stage " "$evidence" || {
        echo "M8 mix is missing Firefox stage $stage" >&2
        exit 1
    }
done
grep -q '^sophia_qemu_xmonad_input schema=1 status=sent pointer=left phase=firefox-navigation$' "$evidence"
grep -q '^sophia_firefox_m8 schema=1 status=navigation_ready content=redacted$' "$evidence"
navigation_ready_line=$(grep -n '^sophia_firefox_m8 schema=1 status=navigation_ready content=redacted$' "$evidence" | head -n 1 | cut -d: -f1)
scroll_line=$(grep -n '^sophia_firefox_m8 schema=1 status=stage_complete stage=scroll ' "$evidence" | head -n 1 | cut -d: -f1)
scroll_axis_routes=$(sed -n "${navigation_ready_line},${scroll_line}p" "$evidence" | awk '
    /^sophia_live_session_pointer schema=9 status=axis_batch / {
        for (i = 1; i <= NF; i++) {
            if ($i ~ /^routed=[0-9]+$/) {
                split($i, field, "=")
                total += field[2]
            }
        }
    }
    END { print total + 0 }
')
(( scroll_axis_routes >= 2 )) || {
    echo "M8 mix observed only $scroll_axis_routes routed wheel packets after navigation readiness" >&2
    exit 1
}
grep -q '^sophia_qemu_firefox_m8 schema=3 status=interactions_complete keyboard=true clipboard=true primary=true navigation=true scroll=true resize=true refocus=true pointer=true dialog=true$' "$evidence"
grep -q '^sophia_live_session_pointer schema=3 status=axis_observed$' "$evidence"
axis_routes=$(awk '
    /^sophia_live_session_pointer schema=9 status=axis_batch / {
        for (i = 1; i <= NF; i++) {
            if ($i ~ /^routed=[0-9]+$/) {
                split($i, field, "=")
                total += field[2]
            }
        }
    }
    END { print total + 0 }
' "$evidence")
(( axis_routes >= 2 )) || {
    echo "M8 mix observed only $axis_routes routed wheel packets" >&2
    exit 1
}
grep -q '^sophia_qemu_firefox_m8 schema=4 status=scroll_complete source=wheel axis_routes=2 keyboard_fallback=false$' "$evidence"
grep -q '^sophia_firefox_m8 schema=1 status=dialog_ready content=redacted$' "$evidence"
grep -q '^sophia_qemu_firefox_m8 schema=7 status=dialog_open surface_snapshot=false modality=dom$' "$evidence"
grep -q '^sophia_qemu_xmonad_pointer schema=3 status=positioned anchor=dialog_confirmation source=isolated_page movement=none$' "$evidence"
grep -q '^sophia_qemu_xmonad_input schema=1 status=sent pointer=left phase=firefox-dialog-confirmation$' "$evidence"
grep -q '^sophia_qemu_firefox_m8 schema=7 status=dialog_closed surface_snapshot=false modality=dom$' "$evidence"
for stage in loaded keyboard clipboard primary scroll resize refocus dialog; do
    grep -q "^sophia_firefox_m8 schema=1 status=stage_complete stage=$stage " "$evidence"
done
grep -Eq '^sophia_firefox_m8 schema=1 status=complete stages=8 selection_owner_changes=[2-9][0-9]* selection_conversions=[2-9][0-9]* content=redacted$' "$evidence"
for chord in meta_l+j meta_l+k meta_l+spc meta_l+shift+2 meta_l+3 meta_l+f meta_l+p; do
    grep -q "^sophia_qemu_xmonad_input schema=1 status=sent chord=$chord$" "$evidence" || {
        echo "M8 mix is missing chord $chord" >&2
        exit 1
    }
done
grep -q '^sophia_live_outputs schema=2 status=ready discovered=2 presentation=2 native_owned=2 ' "$evidence"
[[ "$(grep -c '^sophia_live_output schema=1 status=complete ' "$evidence" || true)" -eq 2 ]]
grep -q '^sophia_live_wm schema=1 status=restarted .*preserved_layout=true' "$evidence"
grep -q '^sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false$' "$evidence"
grep -Eq '^sophia_live_session_control schema=1 status=complete .*timed_out=0 unexpected=0 pending=0 ' "$evidence"
grep -Eq '^sophia_live_session_keys schema=2 status=complete pending=0 release_barrier_pending=0 .*state_only_releases=[1-9][0-9]* ' "$evidence"
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' "$evidence"
grep -q '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed$' "$evidence"
grep -q '^sophia_qemu_guest schema=1 status=complete scenario=xmonad-m8-mix$' "$evidence"
if grep -q ' status=failed ' "$evidence"; then
    echo "M8 mix evidence contains a failure marker" >&2
    exit 1
fi
echo "Milestone 8 mixed-application evidence passed: $evidence"
