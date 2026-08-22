#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFIER="$ROOT_DIR/tools/verify_mixed_output_evidence.sh"
FIXTURE="$(mktemp)"
trap 'rm -f "$FIXTURE"' EXIT

write_fixture() {
    local active="${1:-1}"
    local include_retirement="${2:-yes}"
    {
        echo 'sophia_live_native_head schema=2 status=ready output=2 head=22 connector=DP-2 connector_id=4 mode=1920x1080 refresh_millihz=60000 mirrored=false'
        echo 'sophia_live_output_authority schema=1 status=effect_pending transaction=7 preserved_topology=true'
        echo 'sophia_live_head_composition_plan schema=2 status=ready output=2 head=22 scene_generation=9 target_generation=2 width=1920 height=1080 mapping=exact exact=0 downsampled=0 upsampled=0 mixed=0 active=0 fallback=0 unavailable=0 compositor_primitives=0 damage_rects=0 logical_content_checksum=1'
        echo 'sophia_live_head_composition_queue schema=1 status=queued output=2 head=22 frame=43 scene_generation=9 target_generation=2 mapping=exact width=1920 height=1080 logical_content_checksum=1 source=topology_candidate'
        echo 'sophia_live_output_authority schema=2 status=first_presented transaction=7 outputs=2 published=false rollback_retained=true'
        echo 'sophia_live_output_authority schema=2 status=committed transaction=7 topology_epoch=2 outputs=2 policy_required=true input=quarantined'
        echo 'sophia_output_v1_reference schema=1 status=settled kind=Committed topology_epoch=2 heads=3 groups=2'
        echo 'sophia_wm_v1_reference schema=1 status=settled outputs=2 surfaces=2 placement=1,1'
        echo "sophia_live_head_composition_plan schema=2 status=ready output=2 head=22 scene_generation=10 target_generation=2 width=1920 height=1080 mapping=exact exact=1 downsampled=0 upsampled=0 mixed=0 active=$active fallback=0 unavailable=0 compositor_primitives=1 damage_rects=1 logical_content_checksum=2"
        echo 'sophia_live_head_composition_queue schema=1 status=queued output=2 head=22 frame=44 scene_generation=10 target_generation=2 mapping=exact width=1920 height=1080 logical_content_checksum=2 source=head_plan'
        echo 'sophia_native_composition_sampling schema=3 status=active output=2 head=22 scene_generation=10 requested=exact_nearest effective=exact_nearest alpha_mode=opaque source=1920x1080 target=1920x1080 frame=1920x1080'
        echo 'sophia_live_native_head_page_flip schema=2 status=submitted output=2 head=22 submission=3 content=Some(HeadComposition) frame=44'
        echo 'sophia_live_native_head_page_flip schema=2 status=callback_accepted output=2 head=22 callbacks=1 kernel_sequence=3'
        if [[ "$include_retirement" == yes ]]; then
            echo 'sophia_live_native_head_page_flip schema=2 status=retired output=2 head=22 submission=3 frame=44'
        fi
        echo 'sophia_live_mirror_pacing schema=1 status=primary_presented output=1 primary=11 frame=45'
        echo 'sophia_live_mirror_pacing schema=1 status=released output=1 frame=45'
        echo 'sophia_live_native_head schema=3 status=complete output=1 head=11'
        echo 'sophia_live_native_head schema=3 status=complete output=1 head=13'
        echo 'sophia_live_native_head schema=3 status=complete output=2 head=22'
        echo 'sophia_live_output_topology_health schema=1 status=clean quarantined=false'
        echo 'sophia_live_session_health schema=1 status=clean result=complete'
    } >"$FIXTURE"
}

write_fixture
bash "$VERIFIER" "$FIXTURE" DP-2 >/dev/null

write_fixture 0
if bash "$VERIFIER" "$FIXTURE" DP-2 >/dev/null 2>&1; then
    echo 'Verifier accepted inactive extended content.' >&2
    exit 1
fi

write_fixture 1 no
if bash "$VERIFIER" "$FIXTURE" DP-2 >/dev/null 2>&1; then
    echo 'Verifier accepted an exact frame without retirement.' >&2
    exit 1
fi

write_fixture
sed -i '/status=released output=1 frame=45/d' "$FIXTURE"
if bash "$VERIFIER" "$FIXTURE" DP-2 >/dev/null 2>&1; then
    echo 'Verifier accepted a mirror generation without last-head release.' >&2
    exit 1
fi

echo 'Mixed-output evidence verifier fixtures passed.'
