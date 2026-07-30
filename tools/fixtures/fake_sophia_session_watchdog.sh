#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
    sophia-session-input-guard)
        armed_file=""
        for argument in "$@"; do
            case "$argument" in
                --armed-file=*)
                    armed_file="${argument#--armed-file=}"
                    ;;
            esac
        done
        [[ -n "$armed_file" ]] || exit 2
        printf 'armed\n' >"$armed_file"
        trap 'exit 0' INT TERM
        while true; do
            sleep 1
        done
        ;;
    sophia-live-session)
        trap '' TERM
        while true; do
            sleep 60
        done
        ;;
    *)
        exit 2
        ;;
esac
