#!/usr/bin/env bash
set -euo pipefail

release="${1:-}"
[[ -n "$release" && -d "$release" ]] || {
    echo "usage: tools/verify_packaged_policy.sh RELEASE_DIR" >&2
    exit 1
}
release="$(cd "$release" && pwd)"
manifest="$release/manifest"
[[ -f "$manifest" ]] || {
    echo "Packaged policy manifest is missing: $manifest" >&2
    exit 1
}

field() {
    sed -n "s/^$1=//p" "$manifest" | head -n 1
}
require_sha256() {
    local name="$1" actual="$2" expected
    expected="$(field "$name")"
    [[ "$expected" =~ ^[0-9a-f]{64}$ && "$actual" == "$expected" ]] || {
        echo "Packaged policy has an invalid $name." >&2
        exit 1
    }
}

manifest_schema="$(field schema)"
[[ "$manifest_schema" == 6 ]] || {
    echo "Packaged policy requires native-only release manifest schema 6." >&2
    exit 1
}
for legacy_field in \
    xmonad_version xmonad_source_version xmonad_contrib_source_version \
    xmonad_config_sha256 xmonad_cabal_sha256 xmonad_project_sha256 \
    xmonad_core_config_sha256 xmonad_desktop_profile_sha256 \
    xmonad_binary_sha256 xmobar_version xmobar_source_commit \
    xmobar_config_sha256 xmobar_binary_sha256; do
    [[ -z "$(field "$legacy_field")" ]] || {
        echo "Native-only package contains legacy field: $legacy_field" >&2
        exit 1
    }
done
for legacy_path in \
    "$release/target/release/sophia-x11-wm-bridge" \
    "$release/target/release/xmonad" \
    "$release/target/release/xmobar" \
    "$release/share/sophia-policy/xmonad"; do
    [[ ! -e "$legacy_path" ]] || {
        echo "Native-only package contains a legacy policy artifact: $legacy_path" >&2
        exit 1
    }
done

case "$(field hagia_included)" in
    true)
        hagia="$release/target/release/hagia"
        hagia_shell="$release/target/release/narthex"
        for executable in "$hagia" "$hagia_shell"; do
            [[ -x "$executable" ]] || {
                echo "Packaged Hagia executable is missing: $executable" >&2
                exit 1
            }
        done
        require_sha256 hagia_binary_sha256 \
            "$(sha256sum "$hagia" | awk '{print $1}')"
        require_sha256 hagia_shell_binary_sha256 \
            "$(sha256sum "$hagia_shell" | awk '{print $1}')"
        hagia_profile="$release/share/sophia-policy/hagia/default.kdl"
        [[ -f "$hagia_profile" && ! -L "$hagia_profile" ]] || {
            echo "Packaged Hagia default profile is missing: $hagia_profile" >&2
            exit 1
        }
        [[ "$(field hagia_source_commit)" =~ ^[0-9a-f]{40}$ ]] || {
            echo "Packaged Hagia source commit is invalid." >&2
            exit 1
        }
        require_sha256 hagia_default_profile_sha256 \
            "$(sha256sum "$hagia_profile" | awk '{print $1}')"
        "$hagia" config check --config="$hagia_profile" >/dev/null
        "$release/target/release/sophia" config check \
            --desktop-profile="$hagia_profile" >/dev/null
        ;;
    false)
        for absent in \
            "$release/target/release/hagia" \
            "$release/target/release/narthex" \
            "$release/share/sophia-policy/hagia/default.kdl"; do
            [[ ! -e "$absent" ]] || {
                echo "Package declares hagia_included=false but contains: $absent" >&2
                exit 1
            }
        done
        ;;
    *)
        echo "Packaged policy has an invalid hagia_included field." >&2
        exit 1
        ;;
esac

echo "Native packaged policy executables verified."
