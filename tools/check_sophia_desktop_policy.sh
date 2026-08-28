#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
XMONAD_CONFIG="$ROOT_DIR/tools/config/sophia-xmonad/Main.hs"
CORE_CONFIG="$ROOT_DIR/tools/config/sophia-xmonad/core.kdl"
DESKTOP_PROFILE="$ROOT_DIR/tools/config/sophia-xmonad/desktop.kdl"

bash -n \
    "$ROOT_DIR/tools/build_sophia_xmonad.sh" \
    "$ROOT_DIR/tools/build_sophia_xmobar.sh" \
    "$ROOT_DIR/tools/resolve_sophia_xmonad.sh" \
    "$ROOT_DIR/tools/resolve_sophia_xmobar.sh" \
    "$ROOT_DIR/tools/verify_packaged_policy.sh"

for layout in ThreeColMid Tall Mirror Full spiral; do
    grep -Fq "$layout" "$XMONAD_CONFIG" || {
        echo "Sophia's configured xmonad is missing $layout." >&2
        exit 1
    }
done
if grep -Eq '\b(spawn|kill|className|title|doFloat|doShift|Tabbed|DynamicLog)\b' \
    "$XMONAD_CONFIG"; then
    echo "Sophia's configured xmonad contains forbidden application or metadata policy." >&2
    exit 1
fi
grep -Fq 'modMask = mod1Mask' "$XMONAD_CONFIG"
grep -Fq 'terminal = "/bin/false"' "$XMONAD_CONFIG"
for policy in focusMaster swapMaster swapDown swapUp Shrink Expand IncMasterN \
    StackSet.sink sophiaToggleFloat; do
    grep -Fq "$policy" "$XMONAD_CONFIG" || {
        echo "Sophia's configured xmonad is missing practical policy $policy." >&2
        exit 1
    }
done
grep -Fq 'focus-ring enabled=#false width=0 color="#ffb6b0"' "$CORE_CONFIG"
grep -Fq 'frame enabled=#true width=1 focused-color="#ffb6b0" unfocused-color="#7c7c7c"' \
    "$CORE_CONFIG"
cargo run --offline -q -p sophia-cli -- config check \
    "--config=$CORE_CONFIG" >/dev/null
cargo run --offline -q -p sophia-cli -- config check \
    "--desktop-profile=$DESKTOP_PROFILE" >/dev/null

xmonad_bin="$($ROOT_DIR/tools/build_sophia_xmonad.sh 2>/dev/null)"
xmobar_bin="$($ROOT_DIR/tools/build_sophia_xmobar.sh 2>/dev/null)"
[[ "$($xmonad_bin --version 2>&1 | head -n 1)" == 'xmonad 0.18.1' ]] || {
    echo "Sophia's configured xmonad has the wrong version." >&2
    exit 1
}
[[ "$($xmobar_bin --version 2>&1 | head -n 1)" == 'xmobar 0.51.1' ]] || {
    echo "Sophia's packaged xmobar has the wrong version." >&2
    exit 1
}

grep -Fq 'SOPHIA_XMONAD_BIN="$RELEASE_DIR/target/release/xmonad"' \
    "$ROOT_DIR/tools/installed/sophia-session"
grep -Fq 'SOPHIA_XMOBAR_BIN="$RELEASE_DIR/target/release/xmobar"' \
    "$ROOT_DIR/tools/installed/sophia-session"
grep -Fq 'SOPHIA_CORE_CONFIG="$RELEASE_DIR/share/sophia-policy/xmonad/core.kdl"' \
    "$ROOT_DIR/tools/installed/sophia-session"
grep -Fq 'SOPHIA_DESKTOP_PROFILE="${SOPHIA_DESKTOP_PROFILE:-$RELEASE_DIR/share/sophia-policy/xmonad/desktop.kdl}"' \
    "$ROOT_DIR/tools/installed/sophia-session"
grep -Fq -- '--wm-interface=sophia_wm_v1' \
    "$ROOT_DIR/tools/run_sophia_xmonad_session.sh"
grep -Fq -- '--wm-process-arg=serve-policy' \
    "$ROOT_DIR/tools/run_sophia_xmonad_session.sh"
grep -Fq -- '"--wm-process-executable-grant=$xmonad_bin"' \
    "$ROOT_DIR/tools/run_sophia_xmonad_session.sh"
if grep -Fq -- '--profile=xmonad' "$ROOT_DIR/tools/run_sophia_xmonad_session.sh"; then
    echo "Sophia's xmonad runner still selects the private API-v7 profile." >&2
    exit 1
fi

echo "Sophia desktop policy build checks passed."
