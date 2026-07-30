#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

signing_key="$(git config --get user.signingkey || true)"
[[ -n "$signing_key" ]] || {
    echo "Git user.signingkey is not configured." >&2
    exit 1
}

active_tty="$(tty)"
[[ "$active_tty" == /dev/* ]] || {
    echo "Run this script from an interactive local terminal." >&2
    exit 1
}

export GPG_TTY="$active_tty"
gpg-connect-agent updatestartuptty /bye >/dev/null
printf 'unlock\n' |
    gpg --pinentry-mode loopback \
        --local-user "$signing_key" \
        --clearsign >/dev/null

echo "Git signing key is unlocked in gpg-agent."
