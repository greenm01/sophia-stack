#!/usr/bin/env bash
set -euo pipefail

# Verifies that direct scanout engaged, and engaged lawfully, across the
# session logs given.
#
# A wrapper. The verification is `cargo xtask verify direct-scanout`, which
# reads the verdict columns from `DirectScanoutVerdict::VERDICTS` rather than
# from a list written out beside them -- the drift that produced a nine-slot
# histogram against eleven verdicts, which built clean and would have panicked
# at the index. This file stays so every caller and self-test keeps working.
#
# Model: `validation/tla/PresentFlipOwnership.tla`.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
exec cargo --quiet xtask verify direct-scanout "$@"
