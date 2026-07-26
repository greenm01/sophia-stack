#!/usr/bin/env bash
set -euo pipefail

evidence=${1:-/tmp/sophia-qemu-xmonad-m7.log}
[[ -r "$evidence" ]] || { echo "missing xmonad evidence: $evidence" >&2; exit 1; }

required_chords=(
    meta_l+j meta_l+k meta_l+spc meta_l+2 meta_l+shift+1
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
grep -q '^sophia_qemu_xmonad_pointer schema=1 status=sent source=qmp device=virtio-mouse action=focus_drag anchor=left_edge drag=96x24 commands=4$' "$evidence"
grep -q '^sophia_qemu_xmonad_pointer schema=2 status=sent source=qmp device=virtio-keyboard action=focused_key_probe events=2$' "$evidence"
grep -q '^sophia_qemu_xmonad_pointer schema=3 status=passed source=qmp device=virtio-mouse action=output_edge_reverse edge=right reverse_delta=96$' "$evidence"
grep -Eq '^sophia_live_session_pointer schema=5 status=focus_handoff_released surface=[0-9]+ count=[3-9][0-9]*$' "$evidence"
grep -q '^sophia_live_session_pointer schema=7 status=output_edge_confined axis=horizontal side=maximum$' "$evidence"
grep -q '^sophia_live_session_pointer schema=7 status=edge_reverse_immediate axis=horizontal side=maximum$' "$evidence"
edge_line="$(grep -n -m1 '^sophia_live_session_pointer schema=7 status=output_edge_confined axis=horizontal side=maximum$' "$evidence" | cut -d: -f1)"
reverse_line="$(awk -v edge="$edge_line" 'NR > edge && /^sophia_live_session_pointer schema=7 status=edge_reverse_immediate axis=horizontal side=maximum$/ { print NR; exit }' "$evidence")"
sequence_line="$(awk -v reverse="$reverse_line" 'NR > reverse && /^sophia_qemu_xmonad_pointer schema=3 status=passed .* action=output_edge_reverse / { print NR; exit }' "$evidence")"
if [[ -z "$reverse_line" || -z "$sequence_line" ]]; then
    echo "xmonad pointer edge/reversal evidence is out of order" >&2
    exit 1
fi
"$(dirname "$0")/verify_sophia_xmonad_pointer_focus.sh" "$evidence" >/dev/null
awk '
    /^sophia_live_wm schema=3 status=focus_requested source=pointer surface=/ {
        split($0, fields, "surface=")
        target = fields[2]
        requested = 1
        next
    }
    requested && /^sophia_live_wm schema=1 status=focus_committed .* target=surface$/ {
        committed = 1
        next
    }
    committed && $0 ~ "^sophia_live_compositor_chrome schema=1 status=focused_border_composed surface=" target " generation=[0-9]+ primitives=4$" {
        border = 1
        next
    }
    committed && /sophia_live_compositor_damage schema=1 status=presented output=1 rects=[1-9][0-9]*$/ {
        damage = 1
        next
    }
    committed && /sophia_live_compositor_repaint schema=1 status=presented output=1 mode=partial rects=[1-9][0-9]* pixels=[1-9][0-9]*$/ {
        repaint = 1
        next
    }
    /^sophia_live_session_pointer schema=6 status=focused_key_routed / {
        exit !(requested && committed && border && damage && repaint)
    }
    END {
        if (!(requested && committed && border && damage && repaint)) {
            exit 1
        }
    }
' "$evidence" || {
    echo "focused border and bounded repaint did not follow committed pointer focus" >&2
    exit 1
}
border_surfaces="$(
    sed -n 's/^sophia_live_compositor_chrome schema=1 status=focused_border_composed surface=\([0-9][0-9]*\) generation=[0-9][0-9]* primitives=4$/\1/p' "$evidence" |
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
