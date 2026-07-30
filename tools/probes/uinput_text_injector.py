#!/usr/bin/env python3
"""Create a bounded virtual keyboard and inject text through Linux uinput."""

from __future__ import annotations

import argparse
import fcntl
import os
from pathlib import Path
import signal
import struct
import sys
import time


EV_SYN = 0
EV_KEY = 1
SYN_REPORT = 0
BUS_USB = 0x03

KEY_CODES = {
    "a": 30,
    "b": 48,
    "c": 46,
    "d": 32,
    "e": 18,
    "f": 33,
    "g": 34,
    "h": 35,
    "i": 23,
    "j": 36,
    "k": 37,
    "l": 38,
    "m": 50,
    "n": 49,
    "o": 24,
    "p": 25,
    "q": 16,
    "r": 19,
    "s": 31,
    "t": 20,
    "u": 22,
    "v": 47,
    "w": 17,
    "x": 45,
    "y": 21,
    "z": 44,
    "\n": 28,
}

IOC_NRBITS = 8
IOC_TYPEBITS = 8
IOC_SIZEBITS = 14
IOC_NRSHIFT = 0
IOC_TYPESHIFT = IOC_NRSHIFT + IOC_NRBITS
IOC_SIZESHIFT = IOC_TYPESHIFT + IOC_TYPEBITS
IOC_DIRSHIFT = IOC_SIZESHIFT + IOC_SIZEBITS
IOC_NONE = 0
IOC_WRITE = 1
UINPUT_TYPE = ord("U")


def ioctl_code(direction: int, number: int, size: int = 0) -> int:
    return (
        (direction << IOC_DIRSHIFT)
        | (UINPUT_TYPE << IOC_TYPESHIFT)
        | (number << IOC_NRSHIFT)
        | (size << IOC_SIZESHIFT)
    )


UI_DEV_CREATE = ioctl_code(IOC_NONE, 1)
UI_DEV_DESTROY = ioctl_code(IOC_NONE, 2)
UI_DEV_SETUP = ioctl_code(IOC_WRITE, 3, 92)
UI_SET_EVBIT = ioctl_code(IOC_WRITE, 100, 4)
UI_SET_KEYBIT = ioctl_code(IOC_WRITE, 101, 4)
INPUT_EVENT = struct.Struct("llHHi")
UINPUT_SETUP = struct.Struct("HHHH80sI")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Create a virtual keyboard, publish its /dev/input/event path, "
            "then inject lowercase text after a trigger file appears."
        )
    )
    parser.add_argument("--text", default="sophia\n")
    parser.add_argument("--ready-file", type=Path)
    parser.add_argument("--trigger-file", type=Path)
    parser.add_argument("--result-file", type=Path)
    parser.add_argument("--timeout-seconds", type=float, default=30.0)
    parser.add_argument("--key-interval-ms", type=float, default=2.0)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def validate_text(text: str) -> list[int]:
    unsupported = sorted(set(text).difference(KEY_CODES))
    if unsupported:
        rendered = ", ".join(repr(character) for character in unsupported)
        raise ValueError(f"unsupported characters: {rendered}")
    if not text:
        raise ValueError("text must not be empty")
    return [KEY_CODES[character] for character in text]


def publish(path: Path | None, value: str) -> None:
    if path is None:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    with temporary.open("x", encoding="utf-8") as output:
        output.write(value)
        output.write("\n")
    os.chmod(temporary, 0o600)
    os.replace(temporary, path)


def find_event_device(name: str, deadline: float) -> Path:
    sysfs = Path("/sys/devices/virtual/input")
    while time.monotonic() < deadline:
        for input_path in sysfs.glob("input*"):
            try:
                if (input_path / "name").read_text(encoding="utf-8").strip() != name:
                    continue
            except (FileNotFoundError, PermissionError):
                continue
            event_paths = sorted(input_path.glob("event*"))
            if event_paths:
                return Path("/dev/input") / event_paths[0].name
        time.sleep(0.01)
    raise TimeoutError("uinput event node did not appear before the deadline")


def wait_for_trigger(path: Path, deadline: float, stopped: list[bool]) -> None:
    while not path.exists():
        if stopped[0]:
            raise InterruptedError("stopped before injection")
        if time.monotonic() >= deadline:
            raise TimeoutError("injection trigger did not appear before the deadline")
        time.sleep(0.005)


def emit(device: int, event_type: int, code: int, value: int) -> None:
    os.write(device, INPUT_EVENT.pack(0, 0, event_type, code, value))


def inject(device: int, keycodes: list[int], interval: float) -> None:
    for keycode in keycodes:
        emit(device, EV_KEY, keycode, 1)
        emit(device, EV_SYN, SYN_REPORT, 0)
        if interval:
            time.sleep(interval)
        emit(device, EV_KEY, keycode, 0)
        emit(device, EV_SYN, SYN_REPORT, 0)
        if interval:
            time.sleep(interval)


def self_test(text: str) -> int:
    keycodes = validate_text(text)
    if INPUT_EVENT.size != 24 or UINPUT_SETUP.size != 92:
        raise RuntimeError(
            f"unexpected Linux ABI sizes: input_event={INPUT_EVENT.size} "
            f"uinput_setup={UINPUT_SETUP.size}"
        )
    print(
        "sophia_uinput schema=1 status=self_test_passed "
        f"characters={len(text)} events={len(keycodes) * 4}"
    )
    return 0


def main() -> int:
    args = parse_args()
    if args.timeout_seconds <= 0:
        raise ValueError("--timeout-seconds must be positive")
    if args.key_interval_ms < 0 or args.key_interval_ms > 1000:
        raise ValueError("--key-interval-ms must be between 0 and 1000")
    if args.self_test:
        return self_test(args.text)
    if args.ready_file is None or args.trigger_file is None:
        raise ValueError("--ready-file and --trigger-file are required")

    keycodes = validate_text(args.text)
    stopped = [False]

    def stop(_signum: int, _frame: object) -> None:
        stopped[0] = True

    signal.signal(signal.SIGINT, stop)
    signal.signal(signal.SIGTERM, stop)
    name = f"Sophia Latency Keyboard {os.getpid()}"
    deadline = time.monotonic() + args.timeout_seconds
    device = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
    created = False
    try:
        fcntl.ioctl(device, UI_SET_EVBIT, EV_SYN)
        fcntl.ioctl(device, UI_SET_EVBIT, EV_KEY)
        for keycode in sorted(set(KEY_CODES.values())):
            fcntl.ioctl(device, UI_SET_KEYBIT, keycode)
        encoded_name = name.encode("utf-8")
        setup = UINPUT_SETUP.pack(
            BUS_USB,
            0x534F,
            0x5048,
            1,
            encoded_name.ljust(80, b"\0"),
            0,
        )
        fcntl.ioctl(device, UI_DEV_SETUP, setup)
        fcntl.ioctl(device, UI_DEV_CREATE)
        created = True
        event_path = find_event_device(name, deadline)
        publish(args.ready_file, str(event_path))
        print(
            "sophia_uinput schema=1 status=ready "
            f"device={event_path} characters={len(args.text)}",
            flush=True,
        )
        wait_for_trigger(args.trigger_file, deadline, stopped)
        injected_at_usec = time.monotonic_ns() // 1_000
        inject(device, keycodes, args.key_interval_ms / 1000.0)
        publish(args.result_file, str(injected_at_usec))
        print(
            "sophia_uinput schema=1 status=injected "
            f"characters={len(args.text)} events={len(keycodes) * 4}",
            flush=True,
        )
        while not stopped[0]:
            time.sleep(0.05)
    finally:
        if created:
            try:
                fcntl.ioctl(device, UI_DEV_DESTROY)
            except OSError:
                pass
        os.close(device)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, TimeoutError, ValueError) as error:
        print(f"sophia_uinput schema=1 status=failed reason={error}", file=sys.stderr)
        raise SystemExit(1) from error
