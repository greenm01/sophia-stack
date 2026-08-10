#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
model_dir="$root_dir/validation/specula/profile-reattachment-protocol/spec"
source "$root_dir/tools/lib/specula_tlc.sh"

specula_require_tlc
specula_check_model "$model_dir" \
    base.tla base.cfg \
    MC.tla MC.cfg \
    MC.tla MC_hunt_configuration_before_active.cfg \
    MC.tla MC_hunt_stale_epoch.cfg \
    MC.tla MC_hunt_identity_mismatch.cfg \
    MC.tla MC_hunt_failure_preservation.cfg \
    MC.tla MC_hunt_fresh_correlation.cfg \
    Trace.tla Trace.cfg
