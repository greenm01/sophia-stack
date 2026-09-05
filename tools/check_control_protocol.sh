#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
cargo run --offline -q -p sophia-policy-protocol-gen -- --check
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s bindings/python/tests -p 'test_control_v1.py' -v
