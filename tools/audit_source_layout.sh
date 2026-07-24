#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
ledger="$root/docs/source-layout-exceptions.txt"
status=0

is_recorded() {
    category=$1
    path=$2
    grep -Fqx "$category $path" "$ledger"
}

find "$root/crates" -path '*/src/*.rs' -type f -print | sort | {
while IFS= read -r file; do
    relative=${file#"$root/"}
    lines=$(wc -l < "$file")
    if [ "$lines" -ge 800 ]; then
        printf '%s %s %s\n' source-lines "$lines" "$relative"
    fi
    if [ "$lines" -gt 1000 ] && ! is_recorded large-source "$relative"; then
        printf '%s\n' "error: $relative has $lines lines and no reviewed cohesion exception" >&2
        status=1
    fi
    if rg -q '#\[cfg\(test\)\]|#\[test\]' "$file" &&
        ! is_recorded inline-tests "$relative"; then
        printf '%s\n' "error: inline tests in $relative" >&2
        status=1
    fi
    case "$relative" in
        crates/sophia-cli/*|*/src/main.rs) ;;
        *)
            if rg -q '(^|[^[:alnum:]_])(eprintln!|println!)' "$file" &&
                ! is_recorded direct-printing "$relative"; then
                printf '%s\n' "error: direct library printing in $relative" >&2
                status=1
            fi
            ;;
    esac
done
exit "$status"
}

find "$root/crates" -path '*/tests/*.rs' -type f -print | sort |
while IFS= read -r file; do
    relative=${file#"$root/"}
    lines=$(wc -l < "$file")
    if [ "$lines" -ge 800 ]; then
        printf '%s %s %s\n' test-lines "$lines" "$relative"
    fi
done

status=0
while IFS=' ' read -r category relative; do
    [ -n "${category:-}" ] || continue
    case "$category" in \#*) continue ;; esac
    [ -f "$root/$relative" ] || {
        printf '%s\n' "error: stale source-layout exception: $category $relative" >&2
        status=1
    }
done < "$ledger"

exit "$status"
