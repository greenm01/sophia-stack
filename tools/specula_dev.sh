#!/usr/bin/env bash
set -euo pipefail

SPECULA_ROOT="${SOPHIA_SPECULA_ROOT:-$HOME/src/Specula}"
JAVA_21_HOME="${SOPHIA_SPECULA_JAVA_HOME:-/usr/lib/jvm/openjdk21}"

[[ -x "$SPECULA_ROOT/specula" ]] || {
    echo "Specula checkout is missing its executable: $SPECULA_ROOT/specula" >&2
    exit 2
}
[[ -x "$JAVA_21_HOME/bin/java" ]] || {
    echo "OpenJDK 21 is missing: $JAVA_21_HOME/bin/java" >&2
    exit 2
}

export JAVA_HOME="$JAVA_21_HOME"
export PATH="$JAVA_HOME/bin:$PATH"
exec "$SPECULA_ROOT/specula" "$@"
