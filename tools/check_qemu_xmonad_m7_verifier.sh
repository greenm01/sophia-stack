#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
verifier="$ROOT_DIR/tools/verify_qemu_xmonad_m7_evidence.sh"
fixture="$ROOT_DIR/tools/fixtures/qemu_xmonad_m7_pass.log"
tmp="$(mktemp /tmp/sophia-xmonad-m7-verifier.XXXXXX)"
trap 'rm -f "$tmp"' EXIT

"$verifier" "$fixture"

expect_failure() {
    local label=$1
    if "$verifier" "$tmp" >/dev/null 2>&1; then
        echo "xmonad M7 verifier accepted invalid evidence: $label" >&2
        exit 1
    fi
}

sed '/action=LaunchTerminal$/d' "$fixture" >"$tmp"
expect_failure missing_launch_action

sed '/status=layout_committed .*surfaces=3 /d' "$fixture" >"$tmp"
expect_failure missing_three_surface_layout

sed 's/wm_degraded=false/wm_degraded=true/' "$fixture" >"$tmp"
expect_failure degraded_wm

sed '/status=restarted /d' "$fixture" >"$tmp"
expect_failure missing_restart

sed '/sophia_qemu_xmonad_pointer /d' "$fixture" >"$tmp"
expect_failure missing_pointer_gesture

sed 's/status=focus_handoff_released surface=2 count=2/status=focus_handoff_released surface=2 count=1/' \
    "$fixture" >"$tmp"
expect_failure missing_click_release

sed 's/status=focus_handoff_released surface=2 count=3/status=focus_handoff_released surface=2 count=2/' \
    "$fixture" >"$tmp"
expect_failure missing_drag_motion

sed '/status=key_probe_begin gesture=click/d' "$fixture" >"$tmp"
expect_failure missing_click_key_probe_boundary

sed '/status=complete gesture=click/d' "$fixture" >"$tmp"
expect_failure incomplete_click_sequence

sed '/status=complete gesture=drag/d' "$fixture" >"$tmp"
expect_failure incomplete_drag_sequence

sed '/status=focused_key_routed /d' "$fixture" >"$tmp"
expect_failure missing_pointer_selected_key

sed '/sophia_live_compositor_chrome /d' "$fixture" >"$tmp"
expect_failure missing_focused_border

sed '/status=presented output=1 rects=8/d' "$fixture" >"$tmp"
expect_failure missing_retired_compositor_damage

sed '/sophia_live_output_repaint .*status=presented output=1 mode=partial/d' "$fixture" >"$tmp"
expect_failure missing_bounded_compositor_repaint

sed '/sophia_live_output_damage .*status=presented output=1/d' "$fixture" >"$tmp"
expect_failure missing_combined_output_damage

sed '/status=initial_presented output=2/d' "$fixture" >"$tmp"
expect_failure missing_secondary_damage_baseline

sed '/status=output_edge_confined /d' "$fixture" >"$tmp"
expect_failure missing_pointer_edge

sed '/status=edge_reverse_immediate /d' "$fixture" >"$tmp"
expect_failure missing_pointer_reverse

sed '/action=output_edge_reverse /d' "$fixture" >"$tmp"
expect_failure missing_qmp_pointer_edge_sequence

sed '/visible_surfaces=0 focus=none/d' "$fixture" >"$tmp"
expect_failure missing_empty_workspace_projection

sed '/status=passed action=empty_workspace_click/d' "$fixture" >"$tmp"
expect_failure incomplete_empty_workspace_click

sed '/status=button_suppressed reason=no_target/d' "$fixture" >"$tmp"
expect_failure missing_empty_workspace_suppression

sed '/status=begin action=empty_workspace_click/a sophia_live_wm schema=3 status=focus_requested source=pointer surface=2' \
    "$fixture" >"$tmp"
expect_failure hidden_surface_focus_request

sed '/status=begin action=empty_workspace_click/a sophia_live_session_pointer schema=2 status=button_routed count=1' \
    "$fixture" >"$tmp"
expect_failure hidden_surface_button_delivery

sed '/status=begin action=empty_workspace_click/a sophia_live_session_pointer schema=8 status=button_suppressed reason=policy mode=control_plane_only count=1' \
    "$fixture" >"$tmp"
expect_failure hidden_surface_policy_suppression

cp "$fixture" "$tmp"
printf '%s\n' 'sophia_qemu_guest schema=1 status=failed reason=test' >>"$tmp"
expect_failure guest_failure

echo "Milestone 7 xmonad verifier regressions passed."
