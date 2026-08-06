#!/usr/bin/env python3
"""Regression checks for the bounded QEMU RFB input encoder."""

import struct
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

import qemu_rfb_input


class CaptureSocket:
    def __init__(self, received: bytes = b"", timeout: bool = False):
        self.received = received
        self.timeout = timeout
        self.sent = []

    def settimeout(self, _seconds: float) -> None:
        pass

    def recv(self, size: int) -> bytes:
        if self.timeout:
            raise TimeoutError
        chunk, self.received = self.received[:size], self.received[size:]
        return chunk

    def sendall(self, payload: bytes) -> None:
        self.sent.append(payload)


def pointer_events(connection: CaptureSocket) -> list[tuple[int, int, int]]:
    return [struct.unpack("!BBHH", payload)[1:] for payload in connection.sent]


def main() -> None:
    relative = CaptureSocket(timeout=True)
    assert qemu_rfb_input.receive_pointer_mode(relative) is False

    update = struct.pack("!BBHHHHHi", 0, 0, 1, 1, 0, 1280, 800, -257)
    absolute = CaptureSocket(update)
    assert qemu_rfb_input.receive_pointer_mode(absolute) is True

    relative = CaptureSocket()
    qemu_rfb_input.send_pointer(relative, 40, -20, True, False, 1280, 800)
    assert pointer_events(relative) == [
        (0, 0x7FFF + 40, 0x7FFF - 20),
        (1, 0x7FFF, 0x7FFF),
        (0, 0x7FFF, 0x7FFF),
    ]

    absolute = CaptureSocket()
    qemu_rfb_input.send_pointer(absolute, 40, -20, True, True, 1280, 800)
    assert pointer_events(absolute) == [
        (0, 680, 380),
        (1, 680, 380),
        (0, 680, 380),
    ]

    print("QEMU RFB input encoder regressions passed.")


if __name__ == "__main__":
    main()
