#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PAGE="$ROOT_DIR/tools/fixtures/firefox_m8_local_page.html"

require() {
    grep -Fq "$1" "$PAGE" || {
        echo "Firefox selection page contract is missing: $1" >&2
        exit 1
    }
}

require "const selectionPeer = parameters.get('selection_peer') === 'kitty';"
require "source.value = 'sophia-firefox-primary';"
require "if (!selectionPeer || stage !== 2 || !event.isTrusted) return;"
require "source.selectionStart !== 0 || source.selectionEnd !== source.value.length"
require "const expected = selectionPeer ? 'sophia-kitty-clipboard' : 'sophia';"
require "const expected = selectionPeer ? 'sophia-kitty-primary' : 'sophia';"
require "if (selectionPeer) return;"
require "document.querySelector('#navigate').href = '?navigated=1&selection_peer=kitty';"

echo 'Firefox M10 selection page contract passed'
