#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_ROOT="${SOPHIA_ARTIFACT_ROOT:-$ROOT_DIR/.artifacts}"

cd "$ROOT_DIR"
[[ -z "$(git status --short)" ]] || {
    echo "Refusing to package a dirty worktree; commit the exact release first." >&2
    exit 1
}
commit="$(git rev-parse HEAD)"
version="$(awk -F'"' '$1 ~ /^version = / { print $2; exit }' Cargo.toml)"
[[ -n "$version" ]] || {
    echo "Could not resolve workspace version." >&2
    exit 1
}
release_id="${version}-${commit:0:12}"
artifact="$ARTIFACT_ROOT/sophia-$release_id"
[[ ! -e "$artifact" ]] || {
    echo "Release artifact already exists: $artifact" >&2
    exit 1
}

cargo build --offline --release -p sophia-cli --features atomic-scanout-live
cargo build --offline --release -p sophia-x11-wm-bridge
xmonad_bin="$(tools/resolve_sophia_xmonad.sh)"

install -d -m 755 \
    "$artifact/bin" \
    "$artifact/target/release" \
    "$artifact/tools/fixtures" \
    "$artifact/share/wayland-sessions"
install -m 755 target/release/sophia "$artifact/target/release/sophia"
install -m 755 target/release/sophia-x11-wm-bridge \
    "$artifact/target/release/sophia-x11-wm-bridge"
install -m 755 "$xmonad_bin" "$artifact/target/release/xmonad"
install -m 755 tools/installed/sophia-session "$artifact/bin/sophia-session"
install -m 755 tools/run_sophia_xmonad_session.sh \
    tools/resolve_sophia_xmonad.sh "$artifact/tools/"
install -m 755 tools/sophia_tty_mode.py "$artifact/tools/sophia_tty_mode.py"
install -m 644 tools/fixtures/firefox_m8_local_page.html \
    "$artifact/tools/fixtures/firefox_m8_local_page.html"

printf '%s\n' \
    '[Desktop Entry]' \
    'Name=Sophia' \
    'Comment=Sophia native X11 daily-driver candidate' \
    'Exec=@SOPHIA_INSTALL_PREFIX@/current/bin/sophia-session' \
    'Type=Application' \
    'DesktopNames=Sophia' \
    >"$artifact/share/wayland-sessions/sophia.desktop"
printf 'schema=1\nversion=%s\ncommit=%s\nrelease_id=%s\nbuilt_at_utc=%s\n' \
    "$version" "$commit" "$release_id" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    >"$artifact/manifest"
(
    cd "$artifact"
    find bin target tools share -type f -print0 |
        sort -z |
        xargs -0 sha256sum >SHA256SUMS
)

echo "Packaged immutable Sophia release: $artifact"
echo "Install with: sudo tools/install_live_session.sh $artifact"
