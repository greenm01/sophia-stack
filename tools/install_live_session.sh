#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if (( $# == 0 )); then
    exec "$ROOT_DIR/tools/install_current_live_session.sh"
fi
(( $# == 1 )) || {
    echo "usage: tools/install_live_session.sh [ARTIFACT_DIR]" >&2
    exit 1
}
artifact="$1"
[[ -d "$artifact" ]] || {
    echo "Artifact directory does not exist: $artifact" >&2
    exit 1
}
artifact="$(cd "$artifact" && pwd)"
PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"

if [[ "$(id -u)" != 0 ]]; then
    writable_ancestor="$PREFIX"
    while [[ ! -e "$writable_ancestor" ]]; do
        parent="$(dirname "$writable_ancestor")"
        [[ "$parent" != "$writable_ancestor" ]] || break
        writable_ancestor="$parent"
    done
    [[ -d "$writable_ancestor" && -w "$writable_ancestor" ]] || {
        echo "Installation requires root for $PREFIX; run with sudo." >&2
        exit 1
    }
fi
release_id="$(sed -n 's/^release_id=//p' "$artifact/manifest" | head -n 1)"
[[ "$release_id" =~ ^[0-9A-Za-z._-]+$ ]] || {
    echo "Artifact has an invalid release_id." >&2
    exit 1
}
(
    cd "$artifact"
    sha256sum -c SHA256SUMS
)
"$artifact/tools/verify_packaged_policy.sh" "$artifact"

releases="$PREFIX/releases"
target="$releases/$release_id"
install -d -m 755 "$releases"
[[ ! -e "$target" ]] || {
    echo "Release is already installed: $target" >&2
    exit 1
}
staging="$releases/.install-$release_id-$$"
trap '[[ ! -d "$staging" ]] || mv "$staging" "$staging.failed"' EXIT
cp -a "$artifact" "$staging"
mv "$staging" "$target"
"$ROOT_DIR/tools/activate_live_session_release.sh" "$target"
trap - EXIT

echo "Installed Sophia release: $release_id"
echo "Operator guide: $PREFIX/current/share/doc/sophia/operations.md"
