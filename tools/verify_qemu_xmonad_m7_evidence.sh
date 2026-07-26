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
grep -q '^sophia_qemu_xmonad_pointer schema=1 status=sent source=qmp device=virtio-mouse action=focus_drag anchor=left_edge drag=96x24 commands=4$' "$evidence"
grep -q '^sophia_qemu_xmonad_pointer schema=2 status=sent source=qmp device=virtio-keyboard action=focused_key_probe events=2$' "$evidence"
grep -Eq '^sophia_live_session_pointer schema=5 status=focus_handoff_released surface=[0-9]+ count=[3-9][0-9]*$' "$evidence"
"$(dirname "$0")/verify_sophia_xmonad_pointer_focus.sh" "$evidence" >/dev/null
grep -Eq '^sophia_live_session schema=(14|15) status=bounded_complete .*wm_policy=external .*wm_requests=[1-9][0-9]* .*wm_committed=[1-9][0-9]* .*wm_degraded=false ' "$evidence"
grep -q '^sophia_qemu_guest schema=1 status=complete scenario=xmonad-m7$' "$evidence"
grep -q '^sophia_qemu_xmonad schema=1 status=restart_injected target=compatibility_bridge$' "$evidence"
grep -q '^sophia_live_wm schema=1 status=restarted .*preserved_layout=true' "$evidence"
if grep -q ' status=failed ' "$evidence"; then
    echo "xmonad evidence contains a failure marker" >&2
    exit 1
fi

echo "Milestone 7 xmonad QEMU evidence passed: $evidence"
