#!/usr/bin/env bash
set -euo pipefail

# The direct-scanout promotion gate.
#
# Binds the exact source and binary identity before any DRM takeover, runs the
# probe, verifies the evidence, and archives it. The archive is only as good as
# the identity bound here, so both happen in one script and the run refuses if
# either half cannot be established.
#
# What it proves: one GPU client filling one head with nothing else drawn, its
# own buffer reaching the plane through an accepted validating commit, and the
# session retiring and ending cleanly afterwards. See
# `validation/tla/PresentFlipOwnership.tla`.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
state_home="${XDG_STATE_HOME:-$HOME/.local/state}"
session_log="$state_home/sophia/standalone-session/session.log"
evidence="${SOPHIA_DIRECT_SCANOUT_EVIDENCE:-/tmp/sophia-direct-scanout.log}"
core_config="$ROOT_DIR/tools/fixtures/direct_scanout_core.kdl"
desktop_profile="$ROOT_DIR/tools/fixtures/direct_scanout_desktop.kdl"

if [[ ! -t 0 || "$(tty)" != /dev/tty3 ]]; then
    echo "Switch to tty3 with Ctrl+Alt+F3, log in, and run:" >&2
    echo "  just direct-scanout-gate" >&2
    exit 1
fi
if [[ -n "$(git -C "$ROOT_DIR" status --short)" ]]; then
    echo "Sophia worktree must be clean before the physical proof." >&2
    exit 1
fi

source_commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
git -C "$ROOT_DIR" verify-commit "$source_commit" >/dev/null 2>&1 || {
    echo "Physical-proof HEAD lacks a valid signature." >&2
    exit 1
}
upstream="$(git -C "$ROOT_DIR" rev-parse --verify refs/remotes/origin/master 2>/dev/null || true)"
if [[ -z "$upstream" || "$source_commit" != "$upstream" ]]; then
    echo "Physical-proof HEAD must equal the locally known origin/master." >&2
    echo "  HEAD:          $source_commit" >&2
    echo "  origin/master: ${upstream:-missing}" >&2
    exit 1
fi

client_bin="${SOPHIA_STANDALONE_APP_BIN:-$(command -v kitty || true)}"
[[ -n "$client_bin" && -x "$client_bin" ]] || {
    echo "The direct-scanout gate requires Kitty; set SOPHIA_STANDALONE_APP_BIN." >&2
    exit 1
}

echo "Building the exact physical-proof binary before DRM takeover..."
(
    cd "$ROOT_DIR"
    cargo build --quiet --release --offline -p sophia-cli --features atomic-scanout-live
)
if [[ -n "$(git -C "$ROOT_DIR" status --short)" \
    || "$(git -C "$ROOT_DIR" rev-parse HEAD)" != "$source_commit" ]]; then
    echo "Sophia source identity changed during the physical-proof build." >&2
    exit 1
fi

sophia_bin="$ROOT_DIR/target/release/sophia"
sophia_sha256="$(sha256sum "$sophia_bin" | awk '{ print $1 }')"
client_sha256="$(sha256sum "$client_bin" | awk '{ print $1 }')"
core_sha256="$(sha256sum "$core_config" | awk '{ print $1 }')"
desktop_sha256="$(sha256sum "$desktop_profile" | awk '{ print $1 }')"
echo "Sophia commit:  $source_commit"
echo "Sophia binary:  $sophia_sha256"
echo "Client:         $client_bin ($client_sha256)"

rm -f "$session_log"
status=0
# The probe recipe, not a copy of it: the session shape this gate promotes has
# to be the one anyone can run by hand, or the archive proves something nobody
# else can reproduce.
(
    cd "$ROOT_DIR"
    SOPHIA_STANDALONE_APP_BIN="$client_bin" just direct-scanout-probe
) || status=$?
if (( status != 0 )); then
    echo "The direct-scanout session did not return cleanly (exit $status); nothing is archived." >&2
    exit "$status"
fi
[[ -s "$session_log" ]] || {
    echo "The direct-scanout session produced no evidence: $session_log" >&2
    exit 1
}

# One artifact carries the run: the session's own evidence plus the identity it
# was produced under, which live in two places while the session runs.
install -m 600 "$session_log" "$evidence"
printf 'sophia_direct_scanout_identity schema=1 status=bound source_commit=%s sophia_sha256=%s client=%s client_sha256=%s core_sha256=%s desktop_sha256=%s\n' \
    "$source_commit" "$sophia_sha256" "$(basename "$client_bin")" "$client_sha256" \
    "$core_sha256" "$desktop_sha256" >>"$evidence"

"$ROOT_DIR/tools/verify_direct_scanout_standalone.sh" "$evidence"
SOPHIA_DIRECT_SCANOUT_CLIENT_BIN="$client_bin" \
    "$ROOT_DIR/tools/archive_direct_scanout_run.sh" "$evidence"
echo "Direct scanout gate passed"
