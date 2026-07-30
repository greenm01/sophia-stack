#!/usr/bin/env python3
import sys


def main() -> int:
    operation = sys.argv[1] if len(sys.argv) > 1 else ""
    if operation == "get":
        print("0")
    elif operation == "get-keyboard":
        print("xlate")
    return 0


raise SystemExit(main())
