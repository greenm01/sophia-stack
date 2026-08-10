#!/usr/bin/env bash

specula_require_tlc() {
    local expected_sha256="e22f8ffb4bacdea0a871f444dd94fe5fb0d8013b3388ae39e82e26f852c735d5"
    local jar_path="${SOPHIA_SPECULA_TLA2TOOLS_JAR:-}"

    if [[ -z "$jar_path" || "$jar_path" != /* || ! -f "$jar_path" ]]; then
        echo "SOPHIA_SPECULA_TLA2TOOLS_JAR must name the absolute pinned Specula TLA+ tools jar" >&2
        return 2
    fi
    if [[ "$(sha256sum "$jar_path" | awk '{print $1}')" != "$expected_sha256" ]]; then
        echo "Specula TLA+ tools jar checksum mismatch" >&2
        return 2
    fi
    if ! command -v java >/dev/null 2>&1; then
        echo "Java is required to check the TLA+ model" >&2
        return 2
    fi
}

specula_check_model() (
    local model_dir="$1"
    shift
    local check_dir
    check_dir="$(mktemp -d)"
    trap 'rm -rf "$check_dir"' EXIT
    cp "$model_dir"/*.tla "$model_dir"/*.cfg "$check_dir"/

    while (( $# > 0 )); do
        local module="$1"
        local config="$2"
        shift 2
        (
            cd "$check_dir"
            java -XX:+UseParallelGC -jar "$SOPHIA_SPECULA_TLA2TOOLS_JAR" \
                -deadlock -workers 1 -fp 0 -config "$config" "$module"
        )
    done
)
