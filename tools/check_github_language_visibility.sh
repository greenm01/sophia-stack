#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

for extension in rs tla c; do
    [[ -n "$(git ls-files "*.$extension" | head -n 1)" ]] || {
        echo "GitHub language allowlist has no .$extension source" >&2
        exit 1
    }
done

while IFS= read -r -d '' path; do
    case "$path" in
        *.rs | *.tla | *.c)
            expected_detectable=set
            ;;
        *)
            expected_detectable=unset
            ;;
    esac
    detectable="$(git check-attr linguist-detectable -- "$path" | sed 's/^.*: //')"
    [[ "$detectable" == "$expected_detectable" ]] || {
        echo "GitHub language visibility mismatch: $path is $detectable" >&2
        exit 1
    }
    if [[ "$expected_detectable" == set ]]; then
        for attribute in linguist-documentation linguist-vendored linguist-generated; do
            value="$(git check-attr "$attribute" -- "$path" | sed 's/^.*: //')"
            [[ "$value" == unset ]] || {
                echo "GitHub language source remains excluded: $path $attribute=$value" >&2
                exit 1
            }
        done
    fi
done < <(git ls-files -z)

echo "GitHub language visibility is restricted to Rust, TLA, and C."
