#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mix="$ROOT_DIR/tools/fixtures/qemu_xmonad_m8_mix_pass.log"
soak="$ROOT_DIR/tools/fixtures/qemu_xmonad_m8_soak_pass.log"
grep -Fq 'require_command dbus-run-session' "$ROOT_DIR/tools/build_qemu_session_initramfs.sh"
grep -Fq '/usr/bin/dbus-run-session -- /usr/bin/sophia "$@"' "$ROOT_DIR/tools/qemu_guest_init.sh"
grep -Fq 'export GTK_A11Y=none' "$ROOT_DIR/tools/qemu_guest_init.sh"
grep -Fq 'export RUST_LOG=warn' "$ROOT_DIR/tools/qemu_guest_init.sh"
grep -Fq "status=admitted source=action transaction=[0-9]+ surface=[0-9]+" \
    "$ROOT_DIR/tools/qemu_session_harness.sh"
if grep -Fq 'reason=action_layout_timeout' "$ROOT_DIR/tools/qemu_session_harness.sh"; then
    echo "M8 harness still treats unrelated layout as launch admission" >&2
    exit 1
fi
"$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$mix"
"$ROOT_DIR/tools/verify_qemu_xmonad_m8_soak_evidence.sh" "$soak"

tmp="$(mktemp /tmp/sophia-m8-verifier.XXXXXX)"
trap 'rm -f "$tmp"' EXIT
sed '/id=firefox /d' "$mix" > "$tmp"
if "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 mix verifier accepted missing Firefox evidence" >&2
    exit 1
fi
sed '/stage=primary /d' "$mix" > "$tmp"
if "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 mix verifier accepted missing Firefox PRIMARY evidence" >&2
    exit 1
fi
sed '/status=scroll_complete source=wheel /d' "$mix" > "$tmp"
if "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 mix verifier accepted Firefox keyboard scroll fallback" >&2
    exit 1
fi
sed '/anchor=dialog_confirmation /d' "$mix" > "$tmp"
if "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 mix verifier accepted an unpositioned modal confirmation" >&2
    exit 1
fi
awk '!removed && /status=axis_batch / { removed=1; next } { print }' "$mix" > "$tmp"
if "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 mix verifier accepted only one routed wheel packet" >&2
    exit 1
fi
sed '/status=complete stages=8 /d' "$mix" > "$tmp"
if "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 mix verifier accepted missing Firefox selection evidence" >&2
    exit 1
fi
sed '/status=cleared reason=close_surface /d' "$mix" > "$tmp"
if "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 mix verifier accepted close actions without nonblocking key cleanup" >&2
    exit 1
fi
awk '!removed && /status=cleared reason=close_surface / { removed=1; next } { print }' "$mix" > "$tmp"
if ! "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 mix verifier rejected a close whose keys were already clear" >&2
    exit 1
fi
sed '/sophia_live_session_control schema=1 status=complete /d' "$mix" > "$tmp"
if "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 mix verifier accepted missing control-drain evidence" >&2
    exit 1
fi
sed '/cycle=20 /d' "$soak" > "$tmp"
if "$ROOT_DIR/tools/verify_qemu_xmonad_m8_soak_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 soak verifier accepted fewer than 20 cycles" >&2
    exit 1
fi
echo "Milestone 8 verifier regressions passed."
