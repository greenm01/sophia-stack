#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PAGE="$ROOT_DIR/tools/fixtures/firefox_m8_local_page.html"

require() {
    grep -Fq "$1" "$PAGE" || {
        echo "Firefox PRIMARY page contract is missing: $1" >&2
        exit 1
    }
}

require "const primaryOnly = parameters.get('primary_only') === '1';"
require "let stage = dialogOnly ? 6 : primaryOnly ? 2 : navigated ? 3 : 0;"
require "if (!selectionPeer || stage !== 2 || !event.isTrusted) return;"
require "source.selectionStart !== 0 || source.selectionEnd !== source.value.length"
require "if (primaryOnly) document.title = '0'.repeat(231);"
require "document.title = '0'.repeat(230);"
require "document.title = '0'.repeat(232);"
require "source.value = 'sophia-firefox-primary';"
require "const expected = selectionPeer ? 'sophia-kitty-primary' : 'sophia';"
require "PRIMARY arrived with a token mismatch"
require "Firefox PRIMARY canary complete"

echo 'Firefox M10 PRIMARY page contract passed'
