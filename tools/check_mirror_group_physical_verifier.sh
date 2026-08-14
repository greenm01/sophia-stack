#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$ROOT_DIR/tools/fixtures/mirror_group_physical_pass.log"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT

"$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$fixture" >/dev/null

reject_mutation() {
    local expression="$1" description="$2"
    cp "$fixture" "$work/rejected.log"
    sed -i "$expression" "$work/rejected.log"
    if "$ROOT_DIR/tools/verify_mirror_group_physical.sh" "$work/rejected.log" >/dev/null 2>&1; then
        echo "mirror-group verifier accepted $description" >&2
        exit 1
    fi
}

reject_mutation '/connector=DP-2/d' 'a missing mirror head'
reject_mutation 's/mode=1920x1080/mode=2560x1440/' 'a downgraded secondary mode'
reject_mutation 's/native_cleanup_pending=false/native_cleanup_pending=true/' 'undrained native ownership'
reject_mutation '/status=visual_confirmed/d' 'missing visible-pixel confirmation'

printf '#!/usr/bin/env bash\nexit 0\n' >"$work/sophia"
printf 'schema 1\n' >"$work/profile.kdl"
chmod 755 "$work/sophia"
commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
sophia_sha256="$(sha256sum "$work/sophia" | awk '{ print $1 }')"
profile_sha256="$(sha256sum "$work/profile.kdl" | awk '{ print $1 }')"
sed \
    -e "s/source_commit=[0-9a-f]\{40\}/source_commit=$commit/" \
    -e "s/sophia_sha256=[0-9a-f]\{64\}/sophia_sha256=$sophia_sha256/" \
    -e "s/profile_sha256=[0-9a-f]\{64\}/profile_sha256=$profile_sha256/" \
    "$fixture" >"$work/archive.log"
archive="$(env \
    XDG_STATE_HOME="$work/state" \
    SOPHIA_MIRROR_SOPHIA_BIN="$work/sophia" \
    SOPHIA_MIRROR_PROFILE="$work/profile.kdl" \
    "$ROOT_DIR/tools/archive_mirror_group_physical_run.sh" "$work/archive.log")"
run_dir="${archive##*: }"
[[ -s "$run_dir/SHA256SUMS" ]] || {
    echo "mirror-group archive was not created" >&2
    exit 1
}
"$ROOT_DIR/tools/verify_mirror_group_physical_archive.sh" "$run_dir" >/dev/null
printf '\n' >>"$run_dir/session.log"
if "$ROOT_DIR/tools/verify_mirror_group_physical_archive.sh" "$run_dir" >/dev/null 2>&1; then
    echo "mirror-group archive accepted tampered evidence" >&2
    exit 1
fi

echo "mirror-group physical verifier checks passed"
