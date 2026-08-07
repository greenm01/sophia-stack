#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
ROOT_DIR="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
TARGET_USER="${SUDO_USER:-${USER:-}}"

fail() {
    echo "Sophia uinput setup failed: $*" >&2
    exit 1
}

if [[ "${1:-}" == --help ]]; then
    cat <<EOF
Usage: tools/setup_sophia_uinput.sh

Install the uinput module-load entry and udev permissions required by Sophia's
bounded physical test runners. Run from an interactive terminal; the script
re-executes itself through sudo when necessary.
EOF
    exit 0
fi
[[ $# -eq 0 ]] || fail "unexpected arguments (use --help)"

if ((EUID != 0)); then
    exec sudo -- "$0"
fi

[[ -n "$TARGET_USER" && "$TARGET_USER" != root ]] ||
    fail "run this through sudo from the user that will execute the gate"
getent group input >/dev/null || fail "the input group does not exist"

install -o root -g root -m 0644 \
    "$ROOT_DIR/tools/config/99-sophia-uinput.rules" \
    /etc/udev/rules.d/99-sophia-uinput.rules
install -o root -g root -m 0644 \
    "$ROOT_DIR/tools/config/sophia-uinput.conf" \
    /etc/modules-load.d/sophia-uinput.conf

if ! id -nG "$TARGET_USER" | tr ' ' '\n' | grep -Fxq input; then
    usermod -aG input "$TARGET_USER"
    group_changed=true
else
    group_changed=false
fi

modprobe uinput
udevadm control --reload-rules
udevadm trigger --action=add --subsystem-match=misc --sysname-match=uinput
if [[ ! -e /dev/uinput ]]; then
    [[ -e /sys/class/misc/uinput/dev ]] ||
        fail "the kernel did not register the uinput misc device"
    device_number="$(</sys/class/misc/uinput/dev)"
    [[ "$device_number" == 10:223 ]] ||
        fail "unexpected uinput device number: $device_number"
    mknod /dev/uinput c 10 223
fi
chown root:input /dev/uinput
chmod 0660 /dev/uinput

printf 'sophia_uinput_setup schema=1 status=installed device=/dev/uinput mode=%s owner=%s group=%s user=%s group_changed=%s\n' \
    "$(stat -c %a /dev/uinput)" \
    "$(stat -c %U /dev/uinput)" \
    "$(stat -c %G /dev/uinput)" \
    "$TARGET_USER" \
    "$group_changed"
echo "Start a fresh login, or run: newgrp input"
