#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PAGE="$ROOT_DIR/tools/fixtures/firefox_m8_local_page.html"

require() {
    grep -Fq "$1" "$PAGE" || {
        echo "Firefox dialog page contract is missing: $1" >&2
        exit 1
    }
}
require_count() {
    [[ "$(grep -Fc "$1" "$PAGE")" == "$2" ]] || {
        echo "Firefox dialog page contract has the wrong count for: $1" >&2
        exit 1
    }
}

require "const dialogOnly = parameters.get('dialog_only') === '1';"
require "let stage = dialogOnly ? 6 : navigated ? 3 : 0;"
require_count "stage === 6 && (!dialogOnly || event.isTrusted)" 2
require "document.title = '0'.repeat(225);"
require "dialogOnly ? 226 : stages[6] + 1"
require "dialogOnly ? 227 : stages[7]"
require "Firefox dialog canary complete"

echo 'Firefox M10 dialog page contract passed'
