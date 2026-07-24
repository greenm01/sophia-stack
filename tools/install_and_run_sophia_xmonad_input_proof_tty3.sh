#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
if [[ ! -t 0 || "$(tty)" != /dev/tty3 ]]; then
    echo "Switch to TTY3 with Ctrl+Alt+F3, log in, and run:" >&2
    echo "  cd $ROOT_DIR" >&2
    echo "  tools/install_and_run_sophia_xmonad_input_proof_tty3.sh" >&2
    exit 1
fi
[[ -z "$(git status --short)" ]] || {
    echo "Refusing to install from a dirty worktree." >&2
    exit 1
}
commit="$(git rev-parse HEAD)"
version="$(awk -F'\"' '$1 ~ /^version = / { print $2; exit }' Cargo.toml)"
[[ -n "$version" ]] || {
    echo "Could not resolve the workspace version." >&2
    exit 1
}
artifact="$ROOT_DIR/.artifacts/sophia-$version-${commit:0:12}"
if [[ ! -d "$artifact" ]]; then
    tools/package_live_session.sh
fi
[[ -d "$artifact" ]] || {
    echo "Packaging did not produce the expected artifact: $artifact" >&2
    exit 1
}
artifact_commit="$(sed -n 's/^commit=//p' "$artifact/manifest" | head -n 1)"
[[ "$artifact_commit" == "$commit" ]] || {
    echo "Artifact commit does not match HEAD." >&2
    exit 1
}
(
    cd "$artifact"
    sha256sum -c SHA256SUMS
)

installed_commit=""
if [[ -f /opt/sophia/current/manifest ]]; then
    installed_commit="$(
        sed -n 's/^commit=//p' /opt/sophia/current/manifest |
            head -n 1
    )"
fi
if [[ "$installed_commit" == "$commit" ]]; then
    echo "Matching immutable release is already installed."
else
    echo "Installing immutable Sophia release $version-${commit:0:12}."
    sudo "$ROOT_DIR/tools/install_live_session.sh" "$artifact"
fi

echo
echo "The proof will start after the recovery guard is armed."
echo "Wait for Kitty to finish its initial xmonad resize and show a prompt."
echo "Then type exactly: sophia"
echo "Press Enter once, wait one second, then move the pointer and click once."
echo "Do not use Ctrl+Alt+Backspace unless recovery is needed."
echo
set +e
"$ROOT_DIR/tools/start_sophia_xmonad_input_proof_tty3.sh"
proof_status=$?
set -e
if (( proof_status != 0 )); then
    echo "Sophia input proof exited with status $proof_status." >&2
    echo "Evidence remains in ~/.local/state/sophia/xmonad-session/." >&2
    exit "$proof_status"
fi
"$ROOT_DIR/tools/verify_sophia_xmonad_input_proof_tty3.sh"
echo "Installed release and exact physical xmonad input proof both passed."
