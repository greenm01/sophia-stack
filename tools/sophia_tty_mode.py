#!/usr/bin/env python3
"""Get or set Linux virtual-terminal display and keyboard modes."""

from __future__ import annotations

import array
import fcntl
import os
import sys

KDSETMODE = 0x4B3A
KDGETMODE = 0x4B3B
KD_TEXT = 0
KD_GRAPHICS = 1
KDGKBMODE = 0x4B44
KDSKBMODE = 0x4B45
K_OFF = 4
VT_ACTIVATE = 0x5606
VT_WAITACTIVE = 0x5607


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit(
            "usage: sophia_tty_mode.py get|text|graphics|MODE|"
            "get-keyboard|keyboard-off|keyboard-MODE|activate-vt-N"
        )
    action = sys.argv[1]
    inherited_fd = os.environ.get("SOPHIA_SESSION_TTY_FD")
    tty = None
    if inherited_fd is not None:
        tty_fd = int(inherited_fd, 10)
    else:
        tty_path = os.environ.get("SOPHIA_SESSION_TTY", "/dev/tty")
        tty = open(tty_path, "rb+", buffering=0)
        tty_fd = tty.fileno()
    try:
        if action == "get-keyboard":
            value = array.array("i", [0])
            fcntl.ioctl(tty_fd, KDGKBMODE, value, True)
            print(value[0])
            return 0
        if action == "keyboard-off":
            fcntl.ioctl(tty_fd, KDSKBMODE, K_OFF)
            return 0
        if action.startswith("keyboard-"):
            fcntl.ioctl(tty_fd, KDSKBMODE, int(action.removeprefix("keyboard-"), 10))
            return 0
        if action.startswith("activate-vt-"):
            terminal = int(action.removeprefix("activate-vt-"), 10)
            if terminal < 1 or terminal > 12:
                raise SystemExit("virtual terminal must be in 1..=12")
            fcntl.ioctl(tty_fd, VT_ACTIVATE, terminal)
            fcntl.ioctl(tty_fd, VT_WAITACTIVE, terminal)
            return 0
        if action == "get":
            value = array.array("i", [0])
            fcntl.ioctl(tty_fd, KDGETMODE, value, True)
            print(value[0])
            return 0
        if action == "text":
            mode = KD_TEXT
        elif action == "graphics":
            mode = KD_GRAPHICS
        else:
            mode = int(action, 10)
        fcntl.ioctl(tty_fd, KDSETMODE, mode)
    finally:
        if tty is not None:
            tty.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
