#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PAGE="$ROOT_DIR/tools/fixtures/firefox_m8_local_page.html"

require() {
    grep -Fq "$1" "$PAGE" || {
        echo "Firefox promotion page contract is missing: $1" >&2
        exit 1
    }
}

require "const promotionOnly = parameters.get('promotion_only') === '1';"
require "if (!promotionOnly) return nextSteps[stage];"
require "case 0: return 'Step 1/6: type sophia.';"
require "case 7: return 'Step 6/6 complete: press Ctrl+Q for a normal Firefox close.';"
require "(promotionOnly && stage === 1)"
require "document.querySelector('#clipboard').hidden = true;"
require "document.querySelector('#primary').hidden = true;"
require "document.querySelector('#navigate').href = '?navigated=1&promotion_only=1';"
require "if (selectionPeer || promotionOnly) return;"
require "document.body.classList.remove('dialog-proof');"
require "next.textContent = nextStep(7);"

echo 'Firefox M10 promotion page contract passed'
