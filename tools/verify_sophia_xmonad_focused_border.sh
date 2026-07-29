#!/usr/bin/env bash
set -euo pipefail

evidence=${1:-${XDG_STATE_HOME:-$HOME/.local/state}/sophia/xmonad-session/session.log}
[[ -r "$evidence" ]] || {
    echo "missing focused-border evidence: $evidence" >&2
    exit 1
}

if grep -Eq '(^Error:|panicked at|status=(failed|degraded)([[:space:]]|$))' "$evidence"; then
    echo "focused-border evidence contains an error, panic, or degraded status" >&2
    exit 1
fi

awk '
    function surface_index(record, fields, tail) {
        split(record, fields, "index: ")
        split(fields[2], tail, ",")
        return tail[1]
    }
    /^sophia_live_wm schema=1 status=focus_reconciled .* target=surface surface=SurfaceId / {
        target = surface_index($0)
        reconciled = 1
        next
    }
    reconciled && /^sophia_live_wm schema=1 status=focus_committed .* target=surface$/ {
        committed = 1
        next
    }
    committed &&
        $0 ~ "^sophia_live_compositor_chrome schema=2 status=focus_ring_composed surface=" target " generation=[0-9]+ primitives=4$" {
        pairs++
        seen[target] = 1
        reconciled = 0
        committed = 0
    }
    END {
        surfaces = 0
        for (surface in seen) {
            surfaces++
        }
        if (pairs < 2 || surfaces < 2 || reconciled || committed) {
            exit 1
        }
    }
' "$evidence" || {
    echo "focused borders did not follow committed focus on two surfaces" >&2
    exit 1
}

resize_proven="$(
    sed -n 's/^sophia_live_compositor_chrome schema=2 status=focus_ring_composed surface=\([0-9][0-9]*\) generation=\([0-9][0-9]*\) primitives=4$/\1 \2/p' "$evidence" |
        sort -u |
        awk '{ generations[$1]++ } END { for (surface in generations) if (generations[surface] >= 2) { print "yes"; exit } }'
)"
[[ "$resize_proven" == yes ]] || {
    echo "focused border did not change generation with focused geometry" >&2
    exit 1
}

awk '
    /^sophia_live_wm schema=2 status=workspace_projection_committed .* visible_surfaces=0 focus=none$/ {
        hidden = NR
    }
    hidden && /^sophia_live_wm schema=2 status=workspace_projection_committed .* visible_surfaces=[1-9][0-9]* focus=surface$/ {
        restored = NR
    }
    restored && /^sophia_live_compositor_chrome schema=2 status=focus_ring_composed / {
        border = NR
        exit
    }
    END { exit !(hidden && hidden < restored && restored < border) }
' "$evidence" || {
    echo "focused border was not restored after an empty workspace" >&2
    exit 1
}

awk '
    /^sophia_live_seat schema=1 status=suspended$/ { suspended = NR }
    suspended && /^sophia_live_seat schema=1 status=active source=resume$/ { resumed = NR }
    resumed && /^sophia_live_compositor_chrome schema=2 status=focus_ring_composed / {
        border = NR
        exit
    }
    END { exit !(suspended && suspended < resumed && resumed < border) }
' "$evidence" || {
    echo "focused border was not recomposed after VT resume" >&2
    exit 1
}

grep -Eq '^sophia_live_session schema=(15|16) status=bounded_complete .*native_mixed_exports=[1-9][0-9]* .*native_submit_failures=0 .*native_cleanup_pending=false' "$evidence"
grep -q '^sophia_live_session_health schema=1 status=clean ' "$evidence"
grep -q '^sophia_live_session_cleanup schema=1 status=clean ' "$evidence"

echo "Physical xmonad focused-border evidence passed: $evidence"
