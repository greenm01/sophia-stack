#!/usr/bin/env bash
set -euo pipefail

# Compile every GLSL source in the tree with a real GLSL front end.
#
# The renderer's own compile happens on a GPU at session startup, and a shader
# that fails there does not stop anything: the pipeline records
# `status=unavailable`, falls back to the direct program, and the session runs on
# with its filtering silently uncorrected. That is the right behaviour at runtime
# and a poor place to discover a typo. This is the check that discovers it
# instead, before the shader ever reaches hardware.
#
# It is a front-end check, not a substitute for the real one. glslang will not
# tell you a driver's limits were exceeded or that a uniform went unbound; it
# tells you the source is valid GLSL, which is the class of error a text editor
# cannot catch and a Rust compiler never sees.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VALIDATOR="${SOPHIA_GLSLANG:-}"

if [[ -z "$VALIDATOR" ]]; then
    for candidate in glslangValidator glslang; do
        if command -v "$candidate" >/dev/null 2>&1; then
            VALIDATOR="$(command -v "$candidate")"
            break
        fi
    done
fi

if [[ -z "$VALIDATOR" ]]; then
    echo "glslangValidator is required to check shader sources" >&2
    echo "install glslang, or set SOPHIA_GLSLANG to its path" >&2
    exit 2
fi
if [[ ! -x "$VALIDATOR" ]]; then
    echo "not executable: $VALIDATOR" >&2
    exit 2
fi

mapfile -t sources < <(
    find "$ROOT_DIR/crates" \
        -path '*/src/*' -type f \( -name '*.vert' -o -name '*.frag' \) -print | sort
)

# A run over nothing is the failure this guards against: a moved directory or a
# changed extension would otherwise report success having compiled no shaders.
if (( ${#sources[@]} == 0 )); then
    echo "no shader sources found under crates/*/src -- has the layout moved?" >&2
    exit 1
fi

failed=0
for source in "${sources[@]}"; do
    if ! output="$("$VALIDATOR" "$source" 2>&1)"; then
        echo "shader failed to compile: ${source#"$ROOT_DIR/"}" >&2
        printf '%s\n' "$output" >&2
        failed=$(( failed + 1 ))
    fi
done

if (( failed > 0 )); then
    echo "$failed of ${#sources[@]} shader sources failed" >&2
    exit 1
fi

echo "shader sources compiled: ${#sources[@]}"
