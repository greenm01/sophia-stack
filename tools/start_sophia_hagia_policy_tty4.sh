#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"

export SOPHIA_TTY_PROFILE=hagia-policy
export SOPHIA_TTY_NUMBER=4
export SOPHIA_HAGIA_PHYSICAL_ARM=1
export SOPHIA_HAGIA_PHYSICAL_SEAT="${SOPHIA_HAGIA_PHYSICAL_SEAT:-seat0}"
export SOPHIA_HAGIA_BIN="${SOPHIA_HAGIA_BIN:-$PREFIX/current/target/release/hagia}"

exec "$ROOT_DIR/tools/start_sophia_tty3.sh" "$@"
