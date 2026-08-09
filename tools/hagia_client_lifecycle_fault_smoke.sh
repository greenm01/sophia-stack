#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
hagia_bin="${SOPHIA_HAGIA_BIN:-$(command -v hagia || true)}"
kitty_bin="${SOPHIA_TERMINAL_BIN:-$(command -v kitty || true)}"
firefox_bin="${SOPHIA_FIREFOX_BIN:-$(command -v firefox || true)}"
base_display="${SOPHIA_HAGIA_CLIENT_FAULT_DISPLAY:-:293}"
phase_list="${SOPHIA_HAGIA_CLIENT_FAULT_PHASES:-negotiated,snapshot_received}"

if [[ -z "$hagia_bin" || ! -x "$hagia_bin" ]]; then
    echo "set SOPHIA_HAGIA_BIN to a built Hagia executable" >&2
    exit 1
fi
if [[ -z "$kitty_bin" || ! -x "$kitty_bin" ]]; then
    echo "real Kitty is required; set SOPHIA_TERMINAL_BIN" >&2
    exit 1
fi
if [[ -z "$firefox_bin" || ! -x "$firefox_bin" ]]; then
    echo "the retained Hagia revision-1 profile requires Firefox" >&2
    exit 1
fi
if [[ ! "$base_display" =~ ^:([0-9]+)$ ]]; then
    echo "SOPHIA_HAGIA_CLIENT_FAULT_DISPLAY must be an X display such as :293" >&2
    exit 1
fi
base_display_number="${BASH_REMATCH[1]}"

proof_dir="$(mktemp -d)"
trap 'find "$proof_dir" -type f -delete; rmdir "$proof_dir"' EXIT
IFS=',' read -r -a phases <<<"$phase_list"
if (( ${#phases[@]} == 0 )); then
    echo "SOPHIA_HAGIA_CLIENT_FAULT_PHASES must select at least one phase" >&2
    exit 1
fi

cd "$ROOT_DIR"
cargo build --quiet --offline -p sophia-cli --features atomic-scanout-live

for index in "${!phases[@]}"; do
    phase="${phases[$index]}"
    case "$phase" in
        negotiated|snapshot_received) ;;
        *)
            echo "unsupported pre-projection Hagia fault phase: $phase" >&2
            exit 2
            ;;
    esac
    display=":$((base_display_number + index))"
    evidence="$proof_dir/$phase.log"
    marker="$proof_dir/$phase.marker"
    set +e
    SOPHIA_HAGIA_BIN="$hagia_bin" \
    SOPHIA_HAGIA_RESTART_MARKER="$marker" \
    SOPHIA_HAGIA_FAULT_AFTER="$phase" \
    target/debug/sophia sophia-live-session \
        --no-config \
        --session-mode=normal \
        "--session-app=terminal=$kitty_bin" \
        --session-start=terminal \
        --session-action-app=terminal=terminal \
        "--session-app=firefox=$firefox_bin" \
        --session-action-app=firefox=firefox \
        --session-app-arg=terminal=--config \
        --session-app-arg=terminal=NONE \
        --session-app-arg=terminal=--override \
        --session-app-arg=terminal=linux_display_server=x11 \
        "--display=$display" \
        --max-runtime-ms=10000 \
        --startup-ready-timeout-ms=6000 \
        "--wm-process=$ROOT_DIR/tools/fixtures/hagia_restart_once.sh" \
        --wm-interface=sophia_wm_v1 \
        >"$evidence" 2>&1
    status=$?
    set -e
    cat "$evidence"
    if (( status != 0 )); then
        exit "$status"
    fi

    grep -Eq "^hagia_policy_fault schema=1 phase=$phase$" "$evidence"
    grep -Eq '^sophia_live_wm schema=4 status=restarted adapter=sophia_wm_v1 epoch=2 restarts=1 preserved_layout=true$' "$evidence"
    grep -Eq '^sophia_live_session_startup schema=2 status=ready ' "$evidence"
    grep -Eq '^sophia_live_session_health schema=1 status=clean ' "$evidence"
    grep -Eq '^sophia_live_layout_health schema=2 status=clean ' "$evidence"
done
