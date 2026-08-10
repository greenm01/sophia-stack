#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
model_dir="$root_dir/validation/specula/profile-activation-protocol/spec"
jar_path="${SOPHIA_SPECULA_TLA2TOOLS_JAR:-}"
expected_sha256="e22f8ffb4bacdea0a871f444dd94fe5fb0d8013b3388ae39e82e26f852c735d5"

if [[ -z "$jar_path" || "$jar_path" != /* || ! -f "$jar_path" ]]; then
    echo "SOPHIA_SPECULA_TLA2TOOLS_JAR must name the absolute pinned Specula TLA+ tools jar" >&2
    exit 2
fi
if [[ "$(sha256sum "$jar_path" | awk '{print $1}')" != "$expected_sha256" ]]; then
    echo "Specula TLA+ tools jar checksum mismatch" >&2
    exit 2
fi
if ! command -v java >/dev/null 2>&1; then
    echo "Java is required to check the profile activation protocol model" >&2
    exit 2
fi

check_dir="$(mktemp -d)"
trap 'rm -rf "$check_dir"' EXIT
cp "$model_dir"/*.tla "$model_dir"/*.cfg "$check_dir"/

run_tlc() {
    local module="$1"
    local config="$2"
    (
        cd "$check_dir"
        java -XX:+UseParallelGC -jar "$jar_path" \
            -deadlock -workers 1 -fp 0 -config "$config" "$module"
    )
}

run_tlc base.tla base.cfg
for config in \
    MC.cfg \
    MC_hunt_local_proxy.cfg \
    MC_hunt_stale_epoch.cfg \
    MC_hunt_identity_mismatch.cfg \
    MC_hunt_configuration_order.cfg \
    MC_hunt_partial_failure.cfg \
    MC_hunt_restart_scope.cfg; do
    run_tlc MC.tla "$config"
done
run_tlc Trace.tla Trace.cfg
