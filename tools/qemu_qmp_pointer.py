#!/usr/bin/env python3
"""Perform bounded pointer gestures through a QEMU virtio input device."""

import sys

sys.dont_write_bytecode = True

from qemu_qmp_type import QmpClient, fail


def relative(axis: str, value: int):
    return {"type": "rel", "data": {"axis": axis, "value": value}}


def button(down: bool, name: str):
    return {"type": "btn", "data": {"down": down, "button": name}}


def send(qmp, events):
    qmp.execute("input-send-event", {"events": events})


def main():
    if len(sys.argv) not in (2, 5, 6, 7, 8):
        fail(
            "usage: qemu_qmp_pointer.py QMP_SOCKET [DX DY CLICKS [BUTTON]] "
            "| QMP_SOCKET drag DX DY DRAG_DX DRAG_DY [BUTTON]"
        )
    socket_path = sys.argv[1]
    if len(sys.argv) in (7, 8):
        if sys.argv[2] != "drag":
            fail("the extended pointer gesture must be 'drag'")
        try:
            dx, dy, drag_dx, drag_dy = map(int, sys.argv[3:7])
        except ValueError:
            fail("DX, DY, DRAG_DX, and DRAG_DY must be integers")
        if any(not -4096 <= value <= 4096 for value in (dx, dy, drag_dx, drag_dy)):
            fail("pointer movement must be within +/-4096")
        button_name = sys.argv[7] if len(sys.argv) == 8 else "left"
        if button_name not in ("left", "middle", "right"):
            fail("drag BUTTON must be left, middle, or right")
        with QmpClient(socket_path) as qmp:
            send(qmp, [relative("x", dx), relative("y", dy)])
            send(qmp, [button(True, button_name)])
            send(qmp, [relative("x", drag_dx), relative("y", drag_dy)])
            send(qmp, [button(False, button_name)])
        return

    dx, dy, clicks = (40, 18, 2)
    button_name = "left"
    if len(sys.argv) >= 5:
        try:
            dx, dy, clicks = map(int, sys.argv[2:5])
        except ValueError:
            fail("DX, DY, and CLICKS must be integers")
        if not -4096 <= dx <= 4096 or not -4096 <= dy <= 4096 or not 0 <= clicks <= 4:
            fail("pointer movement must be within +/-4096 and clicks within 0-4")
    if len(sys.argv) == 6:
        button_name = sys.argv[5]
        if button_name not in ("left", "middle", "right", "wheel-up", "wheel-down"):
            fail("BUTTON must be left, middle, right, wheel-up, or wheel-down")
    with QmpClient(socket_path) as qmp:
        send(qmp, [relative("x", dx), relative("y", dy)])
        for _ in range(clicks):
            send(qmp, [button(True, button_name)])
            send(qmp, [button(False, button_name)])


if __name__ == "__main__":
    main()
