#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_ROOT="${SOPHIA_ARTIFACT_ROOT:-$ROOT_DIR/.artifacts}"
PREFIX="${SOPHIA_INSTALL_PREFIX:-/opt/sophia}"

cd "$ROOT_DIR"
commit="$(git rev-parse HEAD)"
short_commit="${commit:0:12}"

if [[ -n "$(git status --short)" ]]; then
    echo "Refusing to install from a dirty worktree." >&2
    echo "Commit or remove the pending changes, then run this script again." >&2
    exit 1
fi

installed_commit="$(
    sed -n 's/^commit=//p' "$PREFIX/current/manifest" 2>/dev/null |
        head -n 1
)"
if [[ "$installed_commit" == "$commit" ]]; then
    echo "Sophia commit $short_commit is already installed."
    echo "Current release: $(readlink -f "$PREFIX/current")"
    exit 0
fi

mapfile -t artifacts < <(
    find "$ARTIFACT_ROOT" -mindepth 2 -maxdepth 2 -type f -name manifest \
        -exec grep -lFx "commit=$commit" {} \; 2>/dev/null |
        sed 's|/manifest$||' |
        sort
)
if (( ${#artifacts[@]} == 0 )); then
    echo "No immutable artifact exists for $short_commit; packaging it now."
    tools/package_live_session.sh
    mapfile -t artifacts < <(
        find "$ARTIFACT_ROOT" -mindepth 2 -maxdepth 2 -type f -name manifest \
            -exec grep -lFx "commit=$commit" {} \; 2>/dev/null |
            sed 's|/manifest$||' |
            sort
    )
fi
(( ${#artifacts[@]} == 1 )) || {
    echo "Expected one artifact for $commit; found ${#artifacts[@]}." >&2
    printf '  %s\n' "${artifacts[@]}" >&2
    exit 1
}
artifact="${artifacts[0]}"

(
    cd "$artifact"
    sha256sum -c SHA256SUMS
)

echo
echo "Installing immutable Sophia commit $short_commit"
echo "Artifact: $artifact"
echo "Prefix:   $PREFIX"
echo "sudo will prompt for your password."
sudo --preserve-env=SOPHIA_INSTALL_PREFIX \
    "$ROOT_DIR/tools/install_live_session.sh" "$artifact"

installed_commit="$(
    sed -n 's/^commit=//p' "$PREFIX/current/manifest" 2>/dev/null |
        head -n 1
)"
[[ "$installed_commit" == "$commit" ]] || {
    echo "Installation verification failed." >&2
    echo "Expected commit: $commit" >&2
    echo "Observed commit: ${installed_commit:-missing}" >&2
    exit 1
}

if [[ "$PREFIX" == /opt/sophia ]]; then
    for session_entry in \
        /usr/share/wayland-sessions/sophia.desktop \
        /usr/share/wayland-sessions/sophia-kitty.desktop; do
        [[ -r "$session_entry" ]] || {
            echo "Installed commit is correct, but a greetd session entry is missing:" >&2
            echo "  $session_entry" >&2
            exit 1
        }
    done
fi

echo
echo "Sophia $short_commit is installed and verified."
echo "Select “Sophia” in greetd for the next physical login run."
