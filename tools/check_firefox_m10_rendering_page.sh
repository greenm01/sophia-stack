#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PAGE="$ROOT_DIR/tools/fixtures/firefox_m8_local_page.html"

require() {
    grep -Fq "$1" "$PAGE" || {
        echo "Firefox rendering page contract is missing: $1" >&2
        exit 1
    }
}

require "const renderingOnly = parameters.get('rendering_only') === '1';"
require "document.title = '0'.repeat(229);"
require "source.hidden = true;"
require "Firefox rendering canary ready"
require 'Watch the counter change, then press Ctrl+Alt+Delete to finish.'
require 'renderingTick += 1;'
require "document.body.style.background = renderingTick % 2 ? '#304860' : '#102038';"

echo 'Firefox M10 rendering page contract passed'
