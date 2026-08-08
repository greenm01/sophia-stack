#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SPECULA_ROOT="${SOPHIA_SPECULA_ROOT:-$HOME/src/Specula}"
JAVA_21_HOME="${SOPHIA_SPECULA_JAVA_HOME:-/usr/lib/jvm/openjdk21}"
EXPECTED_SPECULA_COMMIT="3946f892cc078d5cfea3629f46bd826c246bf2a9"
EXPECTED_TLA_SHA256="e22f8ffb4bacdea0a871f444dd94fe5fb0d8013b3388ae39e82e26f852c735d5"
EXPECTED_COMMUNITY_SHA256="044e8ecdfbca92d51d7eb4469422c2a7da1fe25dc8ad39c4a90e6622d6da4d99"
MODE="install"

usage() {
    echo "usage: tools/install_specula_dev.sh [--verify-only]"
}

fail() {
    printf 'specula_dev_setup step=%s status=failed reason=%s\n' "$1" "$2" >&2
    exit 1
}

status() {
    printf 'specula_dev_setup step=%s status=%s\n' "$1" "$2"
}

if (( $# > 1 )); then
    usage >&2
    exit 2
fi
if (( $# == 1 )); then
    case "$1" in
        --verify-only) MODE="verify" ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; exit 2 ;;
    esac
fi

[[ "$EUID" -ne 0 ]] || fail environment do_not_run_as_root
[[ -r /etc/os-release ]] || fail environment missing_os_release
# shellcheck disable=SC1091
source /etc/os-release
[[ "${ID:-}" == "void" ]] || fail environment unsupported_distribution
[[ -d "$SPECULA_ROOT/.git" ]] || fail checkout missing_specula_checkout
[[ "$(git -C "$SPECULA_ROOT" rev-parse HEAD)" == "$EXPECTED_SPECULA_COMMIT" ]] ||
    fail checkout unexpected_specula_commit
[[ -z "$(git -C "$SPECULA_ROOT" status --porcelain)" ]] ||
    fail checkout dirty_specula_checkout
status checkout verified

packages=(openjdk21 apache-maven python3 python3-pip uv github-cli git curl)
missing_packages=()
for package in "${packages[@]}"; do
    if ! xbps-query -p pkgver "$package" >/dev/null 2>&1; then
        missing_packages+=("$package")
    fi
done

if [[ "$MODE" == "install" && ${#missing_packages[@]} -gt 0 ]]; then
    prior_java_provider="$(
        xbps-alternatives -l | awk '
            $0 == "java" { in_java = 1; next }
            in_java && /^[^ ]/ { in_java = 0 }
            in_java && /\(current\)$/ && provider == "" { provider = $2 }
            END { print provider }
        '
    )"
    sudo xbps-install -Sy "${missing_packages[@]}"
    if [[ -n "$prior_java_provider" ]]; then
        sudo xbps-alternatives -s "$prior_java_provider"
    fi
elif [[ "$MODE" == "verify" && ${#missing_packages[@]} -gt 0 ]]; then
    fail packages missing_packages
fi
status packages verified

[[ -x "$JAVA_21_HOME/bin/java" ]] || fail java missing_java_21
java_major="$("$JAVA_21_HOME/bin/java" -version 2>&1 |
    awk -F '"' 'NR == 1 { split($2, version, "."); print version[1] }')"
[[ "$java_major" == "21" ]] || fail java unexpected_java_version
export JAVA_HOME="$JAVA_21_HOME"
export PATH="$JAVA_HOME/bin:$PATH"
command -v mvn >/dev/null 2>&1 || fail maven missing_maven
status java verified

verify_sha256() {
    local expected="$1"
    local path="$2"
    local actual

    [[ -f "$path" ]] || fail jars missing_jar
    actual="$(sha256sum "$path" | awk '{print $1}')"
    [[ "$actual" == "$expected" ]] || fail jars checksum_mismatch
}

verify_sha256 "$EXPECTED_TLA_SHA256" "$SPECULA_ROOT/lib/tla2tools.jar"
verify_sha256 "$EXPECTED_COMMUNITY_SHA256" \
    "$SPECULA_ROOT/lib/CommunityModules-deps.jar"
tlc_help="$(
    "$JAVA_HOME/bin/java" -cp "$SPECULA_ROOT/lib/tla2tools.jar" \
        tlc2.TLC -help 2>&1 || true
)"
grep 'TLA+' <<<"$tlc_help" >/dev/null || fail jars tlc_invocation_failed
status jars verified

if [[ "$MODE" == "install" ]]; then
    (
        cd "$SPECULA_ROOT"
        uv tool install --force -e .
    )

    codex_bin="$(command -v codex || true)"
    [[ -n "$codex_bin" ]] || fail codex missing_codex
    setup_path="$(mktemp -d)"
    trap 'rm -rf "$setup_path"' EXIT
    ln -s "$codex_bin" "$setup_path/codex"
    printf 'plugin\ny\n' |
        env PATH="$setup_path:$JAVA_HOME/bin:/usr/bin:/usr/sbin:/bin:/sbin" \
            JAVA_HOME="$JAVA_HOME" \
            "$SPECULA_ROOT/specula" setup
    install -Dm755 "$ROOT_DIR/tools/specula_dev.sh" \
        "$HOME/.local/bin/specula-dev"
fi

[[ -x "$HOME/.local/bin/specula-dev" ]] || fail wrapper missing_wrapper
"$HOME/.local/bin/specula-dev" --help >/dev/null ||
    fail wrapper cli_invocation_failed
status wrapper verified

for python in \
    "$SPECULA_ROOT/tools/trace_debugger/.venv/bin/python" \
    "$SPECULA_ROOT/tools/spec_analyzer/.venv/bin/python" \
    "$SPECULA_ROOT/tools/inv_checking_tool/.venv/bin/python"
do
    [[ -x "$python" ]] && "$python" -I -c 'import mcp' ||
        fail helpers python_environment_failed
done
find "$SPECULA_ROOT/tools/cfa/target" -maxdepth 1 -type f -name '*.jar' \
    -print -quit | grep . >/dev/null || fail helpers missing_cfa_jar
status helpers verified

codex plugin list 2>/dev/null |
    grep -E 'specula-codex@specula[[:space:]]+installed' >/dev/null ||
    fail codex plugin_not_installed
status codex verified

printf 'specula_dev_setup status=ready wrapper=%s commit=%s\n' \
    "$HOME/.local/bin/specula-dev" "$EXPECTED_SPECULA_COMMIT"
