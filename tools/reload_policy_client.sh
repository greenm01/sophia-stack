#!/usr/bin/env bash
# Replace the policy client of a running session and restart it in place.
#
# The window manager is a supervised child that reconnects and reloads its
# checkpoint, so swapping the binary and ending the process is a reload: the
# windows, the workspaces and the scroller camera survive it. That only works
# when the session was started against a developer path, because a release in
# /opt is checksummed and must stay byte-for-byte what was packaged.
#
# A reload that does not come back is rolled back to the binary it replaced,
# because the alternative is a desktop with no window manager.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
hagia_root="${SOPHIA_HAGIA_ROOT:-$ROOT_DIR/../hagia}"
hagia_built="${SOPHIA_HAGIA_BIN_SOURCE:-$hagia_root/hagia}"
dev_bin="${SOPHIA_HAGIA_DEV_BIN:-${XDG_STATE_HOME:-$HOME/.local/state}/sophia/dev/hagia}"
state_dir="${XDG_STATE_HOME:-$HOME/.local/state}/sophia/hagia-session"
log="$state_dir/session.log"

note() { printf 'reload: %s\n' "$1"; }
die() { printf 'reload: %s\n' "$1" >&2; exit 1; }

[[ -d "$hagia_root" ]] || die "no Hagia worktree at $hagia_root (set SOPHIA_HAGIA_ROOT)"
note "building Hagia from $hagia_root"
(cd "$hagia_root" && nimble build -d:release >/dev/null) || die "Hagia build failed"
[[ -x "$hagia_built" ]] || die "build produced no executable at $hagia_built"

mkdir -p "$(dirname "$dev_bin")"
chmod 700 "$(dirname "$dev_bin")"

# Install first, so a session that is not running still ends up on the new
# binary the next time it starts.
staged="$dev_bin.staged.$$"
backup=""
if [[ -e "$dev_bin" ]]; then
    backup="$dev_bin.previous"
    cp -p "$dev_bin" "$backup"
fi
cp "$hagia_built" "$staged"
chmod 700 "$staged"
mv -f "$staged" "$dev_bin"
note "installed $(cd "$hagia_root" && git rev-parse --short HEAD) to $dev_bin"

running="$(pgrep -x hagia || true)"
if [[ -z "$running" ]]; then
    note "no running policy client; the next session start will use it"
    exit 0
fi

current_exe="$(readlink -f "/proc/$running/exe" 2>/dev/null || true)"
if [[ "$current_exe" != "$(readlink -f "$dev_bin")" ]]; then
    note "the running session uses $current_exe"
    note "it was not started in developer mode, so it cannot be reloaded in place"
    note "log out and back in once; the session picks up $dev_bin by itself"
    exit 0
fi

before_lines="$(wc -l < "$log" 2>/dev/null || echo 0)"
note "restarting policy client $running"
kill -TERM "$running"

settled=""
for _ in $(seq 1 100); do
    sleep 0.1
    replacement="$(pgrep -x hagia || true)"
    if [[ -n "$replacement" && "$replacement" != "$running" ]]; then
        # Started is not the same as working. A committed layout is the
        # session's own word that the replacement negotiated the protocol and
        # produced a projection it accepted, which a binary that merely
        # launches cannot fake.
        if tail -n +"$before_lines" "$log" 2>/dev/null \
            | grep -qE "sophia_live_wm schema=[0-9]+ status=(layout_committed|focus_committed)"; then
            settled="$replacement"
            break
        fi
    fi
done

if [[ -n "$settled" ]]; then
    note "reloaded; policy client is now $settled"
    exit 0
fi

if [[ -z "$backup" ]]; then
    die "the new policy client did not come back and there is nothing to roll back to"
fi
note "the new policy client did not come back; rolling back"
cp -p "$backup" "$dev_bin"
survivor="$(pgrep -x hagia || true)"
[[ -n "$survivor" ]] && kill -TERM "$survivor" 2>/dev/null || true
for _ in $(seq 1 100); do
    sleep 0.1
    [[ -n "$(pgrep -x hagia || true)" ]] && { note "rolled back to the previous policy client"; exit 1; }
done
die "rolled back but no policy client is running; the session may need a restart"
