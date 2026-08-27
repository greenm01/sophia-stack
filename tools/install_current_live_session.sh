#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_ROOT="${SOPHIA_ARTIFACT_ROOT:-$ROOT_DIR/.artifacts}"
PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"

cd "$ROOT_DIR"
commit="$(git rev-parse HEAD)"
version="$(awk -F'"' '$1 ~ /^version = / { print $2; exit }' Cargo.toml)"
[[ -n "$version" ]] || {
    echo "Could not resolve workspace version." >&2
    exit 1
}
release_id="${version}-${commit:0:12}"
artifact="$ARTIFACT_ROOT/sophia-$release_id"

if [[ ! -d "$artifact" ]]; then
    [[ -z "$(git status --short)" ]] || {
        echo "The current commit has no artifact and the worktree is dirty." >&2
        echo "Commit or discard the changes before packaging an exact release." >&2
        exit 1
    }
    "$ROOT_DIR/tools/package_live_session.sh"
fi

artifact_commit="$(sed -n 's/^commit=//p' "$artifact/manifest" | head -n 1)"
[[ "$artifact_commit" == "$commit" ]] || {
    echo "Artifact commit does not match the current Git commit." >&2
    echo "Expected: $commit" >&2
    echo "Found:    ${artifact_commit:-missing}" >&2
    exit 1
}
(
    cd "$artifact"
    sha256sum -c SHA256SUMS
)

echo "Installing current Sophia commit: $commit"
echo "Release artifact: $artifact"
installed_release="$PREFIX/releases/$release_id"
installer="$ROOT_DIR/tools/install_live_session.sh"
installer_argument="$artifact"
if [[ -d "$installed_release" ]]; then
    echo "Immutable release already exists; verifying and re-activating it."
    installer="$ROOT_DIR/tools/activate_live_session_release.sh"
    installer_argument="$installed_release"
fi
if [[ "$(id -u)" == 0 ]]; then
    exec "$installer" "$installer_argument"
fi
if [[ "$PREFIX" != /opt/sophia ]]; then
    exec "$installer" "$installer_argument"
fi
exec sudo "$installer" "$installer_argument"
