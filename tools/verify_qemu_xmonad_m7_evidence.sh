#!/usr/bin/env bash
set -euo pipefail

evidence=${1:-/tmp/sophia-qemu-xmonad-m7.log}
[[ -r "$evidence" ]] || { echo "missing xmonad evidence: $evidence" >&2; exit 1; }

required_chords=(
    meta_l+j meta_l+k meta_l+spc meta_l+2 meta_l+1
    meta_l+ret meta_l+shift+c meta_l+shift+q
)
for chord in "${required_chords[@]}"; do
    grep -q "^sophia_qemu_xmonad_input schema=1 status=sent chord=$chord$" "$evidence" || {
        echo "missing xmonad chord evidence: $chord" >&2
        exit 1
    }
done

for action in LaunchTerminal CloseFocused Logout; do
    grep -q "status=session_action_committed .*action=$action" "$evidence" || {
        echo "missing committed session action: $action" >&2
        exit 1
    }
done

grep -q '^sophia_live_wm schema=1 status=ready adapter=external ' "$evidence"
grep -Eq '^sophia_live_wm schema=1 status=layout_committed .*surfaces=[3-9][0-9]* ' "$evidence"
grep -q 'sophia_live_compositor_damage schema=1 status=initial_presented output=1 rects=0$' "$evidence"
grep -q 'sophia_live_compositor_damage schema=1 status=initial_presented output=2 rects=0$' "$evidence"
grep -Eq 'sophia_live_output_repaint schema=1 status=initial_presented output=1 mode=full rects=1 pixels=[1-9][0-9]*$' "$evidence"
grep -Eq 'sophia_live_output_repaint schema=1 status=initial_presented output=2 mode=full rects=1 pixels=[1-9][0-9]*$' "$evidence"
grep -q '^sophia_qemu_xmonad_pointer schema=4 status=sent source=qmp device=virtio-mouse action=focus_click anchor=left_edge clicks=1 commands=3$' "$evidence"
grep -q '^sophia_qemu_xmonad_pointer schema=1 status=sent source=qmp device=virtio-mouse action=focus_drag anchor=left_edge drag=96x24 commands=4$' "$evidence"
grep -q '^sophia_qemu_xmonad_pointer schema=3 status=passed source=qmp device=virtio-mouse action=output_edge_reverse edge=right reverse_delta=96$' "$evidence"
grep -q '^sophia_live_session_pointer schema=7 status=output_edge_confined axis=horizontal side=maximum$' "$evidence"
grep -q '^sophia_live_session_pointer schema=7 status=edge_reverse_immediate axis=horizontal side=maximum$' "$evidence"
edge_line="$(grep -n -m1 '^sophia_live_session_pointer schema=7 status=output_edge_confined axis=horizontal side=maximum$' "$evidence" | cut -d: -f1)"
reverse_line="$(awk -v edge="$edge_line" 'NR > edge && /^sophia_live_session_pointer schema=7 status=edge_reverse_immediate axis=horizontal side=maximum$/ { print NR; exit }' "$evidence")"
sequence_line="$(awk -v reverse="$reverse_line" 'NR > reverse && /^sophia_qemu_xmonad_pointer schema=3 status=passed .* action=output_edge_reverse / { print NR; exit }' "$evidence")"
if [[ -z "$reverse_line" || -z "$sequence_line" ]]; then
    echo "xmonad pointer edge/reversal evidence is out of order" >&2
    exit 1
fi
awk '
    /^sophia_live_wm schema=2 status=workspace_projection_committed .* visible_surfaces=0 focus=none$/ {
        empty = NR
        next
    }
    empty && /^sophia_qemu_xmonad_pointer schema=5 status=begin action=empty_workspace_click$/ {
        probe = NR
        next
    }
    probe && /^sophia_live_session_pointer schema=8 status=button_suppressed reason=no_target count=[0-9]+ total=[2-9][0-9]*$/ {
        suppressed = NR
        next
    }
    probe && /^sophia_live_wm schema=3 status=focus_requested source=pointer surface=/ {
        invalid = 1
        exit
    }
    probe && /^sophia_live_session_pointer schema=2 status=button_routed count=/ {
        invalid = 1
        exit
    }
    probe && /^sophia_live_session_pointer schema=8 status=button_suppressed reason=policy / {
        invalid = 1
        exit
    }
    probe && /^sophia_qemu_xmonad_pointer schema=5 status=passed action=empty_workspace_click focus_requests=0 routed_buttons=0$/ {
        passed = NR
        exit
    }
    END {
        if (invalid || !empty || !probe || !suppressed || !passed ||
            !(empty < probe && probe < suppressed && suppressed < passed)) {
            exit 1
        }
    }
' "$evidence" || {
    echo "empty-workspace pointer input selected or routed to a hidden surface" >&2
    exit 1
}
"$(dirname "$0")/verify_sophia_xmonad_pointer_focus.sh" "$evidence" >/dev/null
awk '
    function invalidate() {
        invalid = 1
        exit 1
    }
    function reset_sequence() {
        target = ""
        phase = 1
        gesture_sent = 0
        key_probe_sent = 0
        border = 0
        compositor_damage = 0
        output_damage = 0
        repaint = 0
    }
    /^sophia_qemu_xmonad_pointer_focus schema=1 status=begin gesture=(click|drag)$/ {
        if (active) {
            invalidate()
        }
        gesture = $0
        sub(/^.*gesture=/, "", gesture)
        active = 1
        reset_sequence()
        next
    }
    active && $0 == "sophia_qemu_xmonad_pointer_focus schema=1 status=gesture_sent gesture=" gesture {
        gesture_sent = 1
        next
    }
    /^sophia_live_wm schema=3 status=focus_requested source=pointer surface=/ {
        if (!active || phase != 1) {
            next
        }
        split($0, fields, "surface=")
        target = fields[2]
        phase = 2
        next
    }
    active && phase == 2 && /^sophia_live_wm schema=1 status=focus_committed .* target=surface$/ {
        phase = 3
        next
    }
    active && phase >= 3 && $0 ~ "^sophia_live_compositor_chrome schema=2 status=focus_ring_composed surface=" target " generation=[0-9]+ primitives=4$" {
        border = 1
        next
    }
    active && phase >= 3 && /sophia_live_compositor_damage schema=1 status=presented output=1 rects=[1-9][0-9]*$/ {
        compositor_damage = 1
        next
    }
    active && phase >= 3 && compositor_damage && /sophia_live_output_damage schema=1 status=presented output=1 rects=[1-9][0-9]*$/ {
        output_damage = 1
        next
    }
    active && phase >= 3 && output_damage && /sophia_live_output_repaint schema=1 status=presented output=1 mode=(partial|full) rects=[1-9][0-9]* pixels=[1-9][0-9]*$/ {
        repaint = 1
        next
    }
    active && phase == 3 && /^sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control$/ {
        phase = 4
        next
    }
    active && phase == 4 && $0 ~ "^sophia_live_session_pointer schema=5 status=focus_handoff_released surface=" target " count=[0-9]+$" {
        count = $0
        sub(/^.*count=/, "", count)
        if ((gesture == "click" && count < 2) || (gesture == "drag" && count < 3)) {
            invalidate()
        }
        phase = 5
        next
    }
    active && phase == 5 && $0 == "sophia_qemu_xmonad_pointer_focus schema=1 status=key_probe_begin gesture=" gesture " events=2" {
        phase = 6
        next
    }
    active && $0 == "sophia_qemu_xmonad_pointer_focus schema=1 status=key_probe_sent gesture=" gesture " events=2" {
        key_probe_sent = 1
        next
    }
    active && phase == 6 && $0 == "sophia_live_session_pointer schema=6 status=focused_key_routed surface=" target {
        phase = 7
        next
    }
    active && $0 == "sophia_qemu_xmonad_pointer_focus schema=1 status=complete gesture=" gesture {
        if (phase != 7 || !gesture_sent || !key_probe_sent || !border || !compositor_damage || !output_damage || !repaint) {
            invalidate()
        }
        completed[gesture] = 1
        active = 0
        next
    }
    END {
        if (invalid || active || !completed["click"] || !completed["drag"]) {
            exit 1
        }
    }
' "$evidence" || {
    echo "plain-click and click-drag focus sequences were not independently committed, rendered, released, and keyboard-proven" >&2
    exit 1
}
border_surfaces="$(
    sed -n 's/^sophia_live_compositor_chrome schema=2 status=focus_ring_composed surface=\([0-9][0-9]*\) generation=[0-9][0-9]* primitives=4$/\1/p' "$evidence" |
        sort -u |
        wc -l
)"
if ((border_surfaces < 2)); then
    echo "focused border did not cover two focus targets" >&2
    exit 1
fi
grep -Eq '^sophia_live_session schema=(14|15) status=bounded_complete .*wm_policy=external .*wm_requests=[1-9][0-9]* .*wm_committed=[1-9][0-9]* .*wm_degraded=false ' "$evidence"
grep -q '^sophia_qemu_guest schema=1 status=complete scenario=xmonad-m7$' "$evidence"
grep -q '^sophia_qemu_xmonad schema=1 status=restart_injected target=compatibility_bridge$' "$evidence"
grep -q '^sophia_live_wm schema=1 status=restarted .*preserved_layout=true' "$evidence"
if grep -q ' status=failed ' "$evidence"; then
    echo "xmonad evidence contains a failure marker" >&2
    exit 1
fi

echo "Milestone 7 xmonad QEMU evidence passed: $evidence"
