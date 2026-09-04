#!/usr/bin/env bash
# Report whether Sophia, Hagia, and Narthex are all in the state a physical
# proof requires, before anyone switches to tty4 where recovery is expensive.
#
# This checks the same conditions as the tty4 gates and changes nothing. The
# policy gate requires all three; the native gate deliberately does not require
# origin/master, so that column is reported rather than enforced.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HAGIA_ROOT="${SOPHIA_HAGIA_ROOT:-$ROOT_DIR/../hagia}"
NARTHEX_ROOT="${SOPHIA_NARTHEX_ROOT:-$ROOT_DIR/../narthex}"

status=0
declare -a tuple=()

report() {
    local name="$1" repo="$2" clean=ok signed=ok upstream=ok commit=

    if [[ ! -d "$repo/.git" ]]; then
        printf '%-8s %s\n' "$name" "MISSING checkout: $repo"
        status=1
        return
    fi

    commit="$(git -C "$repo" rev-parse HEAD)"
    [[ -z "$(git -C "$repo" status --short)" ]] || { clean="DIRTY"; status=1; }
    git -C "$repo" verify-commit "$commit" >/dev/null 2>&1 || { signed="UNSIGNED"; status=1; }

    local remote
    remote="$(git -C "$repo" rev-parse --verify refs/remotes/origin/master 2>/dev/null || true)"
    if [[ -z "$remote" ]]; then
        upstream="NO origin/master"
        status=1
    elif [[ "$remote" != "$commit" ]]; then
        upstream="AHEAD/BEHIND origin/master"
        status=1
    fi

    printf '%-8s %s  clean=%s signed=%s upstream=%s\n' \
        "$name" "${commit:0:12}" "$clean" "$signed" "$upstream"
    tuple+=("$name=$commit")
}

echo "Physical-proof preconditions"
report Sophia "$ROOT_DIR"
report Hagia "$HAGIA_ROOT"
report Narthex "$NARTHEX_ROOT"

if (( status == 0 )); then
    echo
    echo "Commit tuple this proof would bind:"
    printf '  %s\n' "${tuple[@]}"
    printf '%s\n' 'sophia_proof_preconditions schema=1 status=ready repositories=3'
else
    echo
    echo "Resolve the repositories marked above before running a physical gate." >&2
    echo "The native gate does not require origin/master; the policy gate does." >&2
fi

exit "$status"
