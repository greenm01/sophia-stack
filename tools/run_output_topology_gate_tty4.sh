#!/usr/bin/env bash
set -euo pipefail

# One-command signed physical proof for two-output loss and return. This gate
# performs a real modeset and therefore runs only from a recovery-safe TTY.

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
ROOT_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
TTY_REQUIRED="${SOPHIA_OUTPUT_TOPOLOGY_TTY:-/dev/tty4}"
SEAT="${SOPHIA_OUTPUT_TOPOLOGY_SEAT:-seat0}"
INSTALL_PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"
EVIDENCE="${SOPHIA_OUTPUT_TOPOLOGY_EVIDENCE:-/tmp/sophia-output-topology-$(date +%Y%m%d-%H%M%S).log}"
EVIDENCE_LATEST="${SOPHIA_OUTPUT_TOPOLOGY_EVIDENCE_LATEST:-/tmp/sophia-output-topology-physical.log}"

usage() {
    cat <<USAGE
usage: tools/run_output_topology_gate_tty4.sh

From tty4, run the complete signed two-output disconnect/reconnect gate with
safe defaults. Hagia is discovered from PATH, $INSTALL_PREFIX/current, or the
adjacent Hagia checkout. Environment overrides remain available through the
SOPHIA_OUTPUT_TOPOLOGY_*, SOPHIA_HAGIA_BIN, SOPHIA_TERMINAL_BIN, and
SOPHIA_FIREFOX_BIN variables.
USAGE
}

case "${1:-}" in
    -h | --help)
        usage
        exit 0
        ;;
    "") ;;
    *)
        echo "Unknown argument: $1" >&2
        usage >&2
        exit 2
        ;;
esac
if (( $# > 1 )); then
    echo "This gate accepts no positional arguments." >&2
    usage >&2
    exit 2
fi

refuse() {
    printf 'Output-topology gate refused: %s\n' "$*" >&2
    exit 2
}

current_tty="$(tty 2>/dev/null || true)"
if [[ ! -t 0 || "$current_tty" != "$TTY_REQUIRED" ]]; then
    refuse "switch to $TTY_REQUIRED with Ctrl+Alt+F4, log in, and run $SCRIPT_PATH"
fi

if [[ -n "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=all)" ]]; then
    refuse "Sophia worktree must be clean before a signed physical gate."
fi
source_commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
git -C "$ROOT_DIR" verify-commit "$source_commit" >/dev/null 2>&1 ||
    refuse "Sophia HEAD must have a valid cryptographic signature."

if [[ -n "${SOPHIA_HAGIA_BIN:-}" ]]; then
    HAGIA_BIN="$SOPHIA_HAGIA_BIN"
else
    HAGIA_BIN="$(command -v hagia || true)"
    for candidate in \
        "$INSTALL_PREFIX/current/target/release/hagia" \
        "$ROOT_DIR/../hagia/target/release/hagia" \
        "$ROOT_DIR/../hagia/hagia"; do
        if [[ -z "$HAGIA_BIN" && -x "$candidate" ]]; then
            HAGIA_BIN="$candidate"
        fi
    done
fi
if [[ -z "$HAGIA_BIN" || ! -x "$HAGIA_BIN" ]]; then
    refuse "Hagia was not found; set SOPHIA_HAGIA_BIN to its executable path."
fi
HAGIA_BIN="$(readlink -f "$HAGIA_BIN")"

mapfile -t connected_outputs < <(
    for status in /sys/class/drm/card*-*/status; do
        [[ -r "$status" && "$(<"$status")" == connected ]] || continue
        connector="${status%/status}"
        basename "$connector" | sed -E 's/^card[0-9]+-//'
    done | sort
)
if (( ${#connected_outputs[@]} != 2 )); then
    refuse "connect exactly two physical outputs (observed ${#connected_outputs[@]}: ${connected_outputs[*]:-none})."
fi

# shellcheck source=tools/lib/drm_master_guard.sh
. "$ROOT_DIR/tools/lib/drm_master_guard.sh"
if ! drm_master_refusal="$(sophia_require_drm_master_available SOPHIA_OUTPUT_TOPOLOGY_FORCE 2>&1)"; then
    refuse "$drm_master_refusal"
fi

echo "Building signed Sophia source $source_commit before DRM takeover..."
(
    cd "$ROOT_DIR"
    cargo build --quiet --release --offline -p sophia-cli \
        --features atomic-scanout-live
)
if [[ -n "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=all)" \
    || "$(git -C "$ROOT_DIR" rev-parse HEAD)" != "$source_commit" ]]; then
    refuse "Sophia source identity changed during the physical-gate build."
fi
git -C "$ROOT_DIR" verify-commit "$source_commit" >/dev/null 2>&1 ||
    refuse "Sophia HEAD signature no longer verifies after the build."

mkdir -p "$(dirname "$EVIDENCE")"
if [[ -n "$EVIDENCE_LATEST" && "$EVIDENCE_LATEST" != "$EVIDENCE" ]]; then
    ln -sfn "$EVIDENCE" "$EVIDENCE_LATEST"
fi

echo "Hagia:   $HAGIA_BIN"
echo "Evidence: $EVIDENCE"
export SOPHIA_OUTPUT_TOPOLOGY_ARM=1
export SOPHIA_OUTPUT_TOPOLOGY_SEAT="$SEAT"
export SOPHIA_OUTPUT_TOPOLOGY_EVIDENCE="$EVIDENCE"
export SOPHIA_HAGIA_BIN="$HAGIA_BIN"
export SOPHIA_LIVE_SESSION_SKIP_BUILD=1

"$ROOT_DIR/tools/output_topology_physical_gate.sh"

if [[ -n "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=all)" \
    || "$(git -C "$ROOT_DIR" rev-parse HEAD)" != "$source_commit" ]]; then
    refuse "Sophia source identity changed during the physical gate."
fi
git -C "$ROOT_DIR" verify-commit "$source_commit" >/dev/null 2>&1 ||
    refuse "Sophia HEAD signature no longer verifies after the gate."

echo "Signed output-topology gate passed for $source_commit"
