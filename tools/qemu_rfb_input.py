#!/usr/bin/env python3
"""Send one bounded input gesture through a local QEMU VNC socket."""

import socket
import struct
import sys
import time

sys.dont_write_bytecode = True

KEYSYMS = {
    "alt": 0xFFE9,
    "ctrl": 0xFFE3,
    "meta_l": 0xFFEB,
    "ret": 0xFF0D,
    "shift": 0xFFE1,
    "spc": 0x0020,
}


def fail(message: str) -> "NoReturn":
    raise SystemExit(message)


def receive_exact(connection: socket.socket, size: int) -> bytes:
    chunks = []
    remaining = size
    while remaining:
        chunk = connection.recv(remaining)
        if not chunk:
            fail("VNC connection closed during negotiation")
        chunks.append(chunk)
        remaining -= len(chunk)
    return b"".join(chunks)


def receive_pointer_mode(connection: socket.socket) -> bool:
    # QEMU reports its active pointer mode with the -257 pseudo-encoding.
    # Absence of a reply means the server remained in its initial relative mode.
    connection.settimeout(0.3)
    try:
        message_type = connection.recv(1)
    except TimeoutError:
        connection.settimeout(5)
        return False
    connection.settimeout(5)
    if message_type != b"\x00":
        fail("unexpected VNC server message during pointer negotiation")
    update = message_type + receive_exact(connection, 15)
    rectangles = struct.unpack("!H", update[2:4])[0]
    x, _y, _width, _height, encoding = struct.unpack("!HHHHi", update[4:16])
    if rectangles != 1 or encoding != -257 or x not in (0, 1):
        fail("invalid VNC pointer-mode update")
    return x == 1


def connect(socket_path: str) -> tuple[socket.socket, bool, int, int]:
    connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    connection.settimeout(5)
    connection.connect(socket_path)

    version = receive_exact(connection, 12)
    if not version.startswith(b"RFB 003.00"):
        fail("unsupported VNC protocol version")
    connection.sendall(b"RFB 003.008\n")
    security_count = receive_exact(connection, 1)[0]
    if security_count == 0:
        reason_size = struct.unpack("!I", receive_exact(connection, 4))[0]
        reason = receive_exact(connection, reason_size).decode(errors="replace")
        fail(f"VNC server rejected negotiation: {reason}")
    security_types = receive_exact(connection, security_count)
    if 1 not in security_types:
        fail("local VNC server does not offer no-authentication mode")
    connection.sendall(b"\x01")
    if struct.unpack("!I", receive_exact(connection, 4))[0] != 0:
        fail("VNC security negotiation failed")

    connection.sendall(b"\x01")
    server = receive_exact(connection, 24)
    width, height = struct.unpack("!HH", server[:4])
    name_size = struct.unpack("!I", server[20:24])[0]
    receive_exact(connection, name_size)
    # QEMU's -257 pseudo-encoding lets the server select relative or absolute
    # coordinates from the active guest pointer, matching ordinary viewers.
    connection.sendall(struct.pack("!BBHi", 2, 0, 1, -257))
    return connection, receive_pointer_mode(connection), width, height


def keysym(name: str) -> int:
    if name in KEYSYMS:
        return KEYSYMS[name]
    if len(name) == 1 and name.isascii() and name.islower():
        return ord(name)
    fail(f"unsupported key: {name}")


def send_key(connection: socket.socket, symbol: int, down: bool) -> None:
    connection.sendall(struct.pack("!BBHI", 4, int(down), 0, symbol))


def send_chord(connection: socket.socket, chord: str) -> None:
    keys = chord.split("+")
    if not keys or len(keys) > 4 or len(set(keys)) != len(keys):
        fail("chord must contain one through four unique keys")
    symbols = [keysym(key) for key in keys]
    for symbol in symbols:
        send_key(connection, symbol, True)
    time.sleep(0.08)
    for symbol in reversed(symbols):
        send_key(connection, symbol, False)
    time.sleep(0.2)


def send_text(connection: socket.socket, value: str) -> None:
    if not 1 <= len(value) <= 24 or not value.isascii() or not value.isalpha():
        fail("text must contain one through 24 ASCII letters")
    for character in value.lower():
        send_key(connection, ord(character), True)
        send_key(connection, ord(character), False)
        time.sleep(0.1)


def send_pointer(
    connection: socket.socket,
    dx: int,
    dy: int,
    click: bool,
    absolute: bool,
    width: int,
    height: int,
) -> None:
    if not -4096 <= dx <= 4096 or not -4096 <= dy <= 4096:
        fail("pointer deltas must be within plus or minus 4096")
    if absolute:
        x = max(0, min(width - 1, width // 2 + dx))
        y = max(0, min(height - 1, height // 2 + dy))
    else:
        center = 0x7FFF
        x, y = center + dx, center + dy
    connection.sendall(struct.pack("!BBHH", 5, 0, x, y))
    if click:
        # Absolute button events retain the selected position. Relative button
        # events use the protocol center so a click does not repeat the move.
        button_x, button_y = (x, y) if absolute else (0x7FFF, 0x7FFF)
        connection.sendall(struct.pack("!BBHH", 5, 1, button_x, button_y))
        connection.sendall(struct.pack("!BBHH", 5, 0, button_x, button_y))
    time.sleep(0.2)


def main() -> None:
    if len(sys.argv) < 4:
        fail("usage: qemu_rfb_input.py SOCKET chord KEYS | type TEXT | pointer X Y [click]")
    socket_path, action = sys.argv[1:3]
    connection, pointer_absolute, width, height = connect(socket_path)
    with connection:
        if action == "chord" and len(sys.argv) == 4:
            send_chord(connection, sys.argv[3])
        elif action == "type" and len(sys.argv) == 4:
            send_text(connection, sys.argv[3])
        elif action == "pointer" and len(sys.argv) in (5, 6):
            try:
                dx, dy = map(int, sys.argv[3:5])
            except ValueError:
                fail("pointer deltas must be integers")
            click = len(sys.argv) == 6 and sys.argv[5] == "click"
            if len(sys.argv) == 6 and not click:
                fail("the optional pointer action must be click")
            send_pointer(
                connection,
                dx,
                dy,
                click,
                pointer_absolute,
                width,
                height,
            )
        else:
            fail("usage: qemu_rfb_input.py SOCKET chord KEYS | type TEXT | pointer X Y [click]")


if __name__ == "__main__":
    main()
