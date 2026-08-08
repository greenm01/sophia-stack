#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PASS="$ROOT_DIR/tools/fixtures/installed_session_soak_pass.log"
IDENTITY_PASS="$ROOT_DIR/tools/fixtures/installed_runtime_identity_pass.log"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TEMP_DIR"' EXIT
TEMP_FILE="$TEMP_DIR/mutated.log"
CAPTURE_RELEASE="$TEMP_DIR/release"

install -d -m 755 "$CAPTURE_RELEASE/target/release"
printf 'schema=1\nversion=0.1.0-test\n' >"$CAPTURE_RELEASE/manifest"
printf 'sophia-test-binary\n' >"$CAPTURE_RELEASE/target/release/sophia"
printf 'xmonad-test-binary\n' >"$CAPTURE_RELEASE/target/release/xmonad"
printf 'xmobar-test-binary\n' >"$CAPTURE_RELEASE/target/release/xmobar"
"$ROOT_DIR/tools/installed/capture-runtime-identity.sh" \
    "$TEMP_DIR/captured.log" "$CAPTURE_RELEASE"
sophia_digest="$(sha256sum "$CAPTURE_RELEASE/target/release/sophia" | awk '{print $1}')"
grep -Fxq \
    "sophia_runtime_identity schema=2 kind=application name=sophia version=0.1.0-test digest=$sophia_digest" \
    "$TEMP_DIR/captured.log"

"$ROOT_DIR/tools/verify_installed_session_soak.sh" "$PASS" 7200000 2 2
progress="$({
    SOPHIA_SOAK_SESSION_LOG="$PASS" \
        SOPHIA_SOAK_IDENTITY_LOG="$TEMP_DIR/missing-identity.log" \
        "$ROOT_DIR/tools/installed/sophia-soak-progress"
} 2>&1)"
grep -Fq 'Apps: Kitty 2/10  Firefox 2/5  close 4/15' <<<"$progress"
grep -Fq 'Policy: 14/14  remaining: none' <<<"$progress"
grep -Fq 'Workspace: view 1  move 1    Pointer: move 1  resize 1' <<<"$progress"
sed 's/^sophia_live_session schema=14 status=bounded_complete /sophia_live_session schema=16 status=bounded_complete /' \
    "$PASS" >"$TEMP_FILE"
"$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2
"$ROOT_DIR/tools/verify_installed_runtime_identity.sh" "$IDENTITY_PASS"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$PASS" 7200001 2 2; then
    echo "installed soak verifier accepted an undersized duration" >&2
    exit 1
fi
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$PASS" 7200000 3 2; then
    echo "installed soak verifier accepted too few terminal actions" >&2
    exit 1
fi
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$PASS" 7200000 2 3; then
    echo "installed soak verifier accepted too few Firefox actions" >&2
    exit 1
fi
sed '/^sophia_live_selection schema=1 status=complete /d' "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted no clipboard summary" >&2
    exit 1
fi
sed '0,/^sophia_session_app schema=1 status=exited id=terminal /{/^sophia_session_app schema=1 status=exited id=terminal /d;}' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted too few clean terminal exits" >&2
    exit 1
fi
sed '0,/^sophia_session_app schema=1 status=exited id=firefox /{/^sophia_session_app schema=1 status=exited id=firefox /d;}' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted too few clean Firefox exits" >&2
    exit 1
fi
sed '/status=focus_committed /d' "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted no repeated focus changes" >&2
    exit 1
fi
sed '/status=workspace_projection_committed .* focus=none$/d' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted no workspace-away transitions" >&2
    exit 1
fi
sed '/^sophia_live_resize_epoch schema=3 status=visual_committed /d' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted no visually committed resizes" >&2
    exit 1
fi
sed '0,/action=CloseFocused$/{/action=CloseFocused$/d;}' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted too few close actions" >&2
    exit 1
fi
sed '/status=physical_action_committed action=7$/d' "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted a missing practical action" >&2
    exit 1
fi
sed '/status=physical_action_committed action=514$/d' "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted no workspace move" >&2
    exit 1
fi
sed '/status=pointer_gesture_committed mode=resize$/d' "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted no pointer resize" >&2
    exit 1
fi
cp "$PASS" "$TEMP_FILE"
printf 'sophia_live_wm schema=1 status=layout_timeout transaction=99 preserved_layout=true rollback_transaction=99 rollback_configures=1 resize_state=redacted\n' >>"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted a layout timeout" >&2
    exit 1
fi
cp "$PASS" "$TEMP_FILE"
printf 'sophia_live_resize_epoch schema=3 status=queue_aborted epoch=99 rejected_presents=1 recovery_extents=1\n' >>"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted an aborted resize queue" >&2
    exit 1
fi
sed 's/hidden_surface_commands=0/hidden_surface_commands=1/' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted a hidden surface command" >&2
    exit 1
fi
sed 's/pending=0 rejected=0/pending=0 rejected=1/' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted rejected WM transport work" >&2
    exit 1
fi
cp "$PASS" "$TEMP_FILE"
printf 'sophia_live_wm_transport schema=2 status=complete peak_depth=2 pending=0 rejected=0 action_coalesced=0 stale_responses=0 max_queue_dwell_msec=12 max_round_trip_msec=180\n' >>"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted duplicate WM transport summaries" >&2
    exit 1
fi
sed 's/owner_changes=2 conversions=2/owner_changes=1 conversions=1/' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted insufficient clipboard activity" >&2
    exit 1
fi
sed '/status=complete output=2 /d' "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted only one output" >&2
    exit 1
fi
sed 's/status=complete output=2 /status=complete output=1 /' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted duplicate output summaries" >&2
    exit 1
fi
sed 's/status=complete pending=0 /status=complete pending=1 /' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted a stuck key" >&2
    exit 1
fi
sed 's/hidden_updates=0 hardware_failures=0/hidden_updates=0 hardware_failures=1/' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted a cursor failure" >&2
    exit 1
fi
sed 's/timestamps=500 fallbacks=0 pending=0/timestamps=500 fallbacks=1 pending=0/' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted a page-flip clock fallback" >&2
    exit 1
fi
sed 's/input_events_flushed=100/input_events_flushed=99/' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted undrained input" >&2
    exit 1
fi
sed 's/native_callback_queue_saturated=0/native_callback_queue_saturated=1/' \
    "$PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted callback saturation" >&2
    exit 1
fi
cp "$PASS" "$TEMP_FILE"
printf 'free(): invalid pointer\n' >>"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_session_soak.sh" "$TEMP_FILE" 7200000 2 2; then
    echo "installed soak verifier accepted an allocator diagnostic" >&2
    exit 1
fi
sed '/name=firefox /d' "$IDENTITY_PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" "$TEMP_FILE"; then
    echo "runtime identity verifier accepted a missing Firefox identity" >&2
    exit 1
fi
sed '/name=sophia /d' "$IDENTITY_PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" "$TEMP_FILE"; then
    echo "runtime identity verifier accepted a missing Sophia identity" >&2
    exit 1
fi
sed 's/name=sophia version=0.1.0 digest=[0-9a-f]*/name=sophia version=0.1.0 digest=unavailable/' \
    "$IDENTITY_PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" "$TEMP_FILE"; then
    echo "runtime identity verifier accepted an unavailable Sophia digest" >&2
    exit 1
fi
wrong_sophia_digest=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
if "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" \
    "$IDENTITY_PASS" "$wrong_sophia_digest"; then
    echo "runtime identity verifier accepted the wrong expected Sophia digest" >&2
    exit 1
fi
sed 's/status=connected/status=disconnected/' "$IDENTITY_PASS" >"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" "$TEMP_FILE"; then
    echo "runtime identity verifier accepted no connected output" >&2
    exit 1
fi
cp "$IDENTITY_PASS" "$TEMP_FILE"
printf 'clipboard=forbidden\n' >>"$TEMP_FILE"
if "$ROOT_DIR/tools/verify_installed_runtime_identity.sh" "$TEMP_FILE"; then
    echo "runtime identity verifier accepted application content" >&2
    exit 1
fi

echo "installed session verifier checks passed"
