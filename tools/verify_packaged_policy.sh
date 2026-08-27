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
[[ "$manifest_schema" =~ ^(2|3|4|5)$ ]] || {
    echo "Packaged policy requires release manifest schema 2, 3, 4, or 5." >&2
    exit 1
}
[[ "$(field xmonad_version)" == xmonad_0.18.1 \
    && "$(field xmobar_version)" == xmobar_0.51.1 ]] || {
    echo "Packaged policy has unapproved executable versions." >&2
    exit 1
}
[[ "$(field xmonad_source_version)" == 0.18.1 \
    && "$(field xmonad_contrib_source_version)" == 0.18.2 ]] || {
    echo "Packaged policy has unapproved xmonad source versions." >&2
    exit 1
}
[[ "$(field xmobar_source_commit)" =~ ^[0-9a-f]{40}$ ]] || {
    echo "Packaged policy has no exact xmobar source commit." >&2
    exit 1
}

xmonad="$release/target/release/xmonad"
xmobar="$release/target/release/xmobar"
xmonad_config="$release/share/sophia-policy/xmonad/Main.hs"
xmonad_cabal="$release/share/sophia-policy/xmonad/sophia-xmonad.cabal"
xmonad_project="$release/share/sophia-policy/xmonad/cabal.project"
xmonad_core_config="$release/share/sophia-policy/xmonad/core.kdl"
xmonad_desktop_profile="$release/share/sophia-policy/xmonad/desktop.kdl"
xmobar_config="$release/tools/fixtures/xmobar_sophia.config"
for executable in "$xmonad" "$xmobar"; do
    [[ -x "$executable" ]] || {
        echo "Packaged policy executable is missing: $executable" >&2
        exit 1
    }
done
for config in "$xmonad_config" "$xmonad_cabal" "$xmonad_project" "$xmobar_config"; do
    [[ -f "$config" ]] || {
        echo "Packaged policy configuration is missing: $config" >&2
        exit 1
    }
done
if [[ "$manifest_schema" =~ ^(3|4|5)$ ]]; then
    [[ -f "$xmonad_core_config" ]] || {
        echo "Packaged Engine theme configuration is missing: $xmonad_core_config" >&2
        exit 1
    }
    require_sha256 xmonad_core_config_sha256 \
        "$(sha256sum "$xmonad_core_config" | awk '{print $1}')"
fi
if [[ "$manifest_schema" =~ ^(4|5)$ ]]; then
    [[ -f "$xmonad_desktop_profile" ]] || {
        echo "Packaged xmonad desktop profile is missing: $xmonad_desktop_profile" >&2
        exit 1
    }
    require_sha256 xmonad_desktop_profile_sha256 \
        "$(sha256sum "$xmonad_desktop_profile" | awk '{print $1}')"
fi

require_sha256 xmonad_binary_sha256 "$(sha256sum "$xmonad" | awk '{print $1}')"
require_sha256 xmobar_binary_sha256 "$(sha256sum "$xmobar" | awk '{print $1}')"
require_sha256 xmonad_config_sha256 "$(sha256sum "$xmonad_config" | awk '{print $1}')"
require_sha256 xmonad_cabal_sha256 "$(sha256sum "$xmonad_cabal" | awk '{print $1}')"
require_sha256 xmonad_project_sha256 "$(sha256sum "$xmonad_project" | awk '{print $1}')"
require_sha256 xmobar_config_sha256 "$(sha256sum "$xmobar_config" | awk '{print $1}')"
case "$(field hagia_included)" in
    true)
        hagia="$release/target/release/hagia"
        hagia_shell="$release/target/release/hagia-shell"
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
        if [[ "$manifest_schema" == 5 ]]; then
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
        fi
        ;;
    false) ;;
    *)
        echo "Packaged policy has an invalid hagia_included field." >&2
        exit 1
        ;;
esac
[[ "$($xmonad --version 2>&1 | head -n 1 | tr ' ' '_')" == "$(field xmonad_version)" \
    && "$($xmobar --version 2>&1 | head -n 1 | tr ' ' '_')" == "$(field xmobar_version)" ]] || {
    echo "Packaged policy executable version output does not match its manifest." >&2
    exit 1
}

echo "Packaged policy executables verified."
