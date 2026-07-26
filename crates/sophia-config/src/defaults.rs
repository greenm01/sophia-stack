pub const COMPILED_CORE_CONFIG: &str = r##"
/- kdl-version 2
schema 1
input {
    seat "seat0"
    keyboard rules="evdev" model="pc105" layout="us" variant="" options=""
    repeat delay-ms=660 interval-ms=25
}
compositor {
    chrome-fallback enabled=#true thickness=2 color="#70b7ff"
    chrome-limits max-thickness=64
}
namespace profile="classic-shared"
diagnostics verbose=#false
"##;

pub const COMPILED_WM_CONFIG: &str = r##"
/- kdl-version 2
schema 1
policy timeout-ms=300
workspace 1
workspace 2
workspace 3
workspace 4
workspace 5
workspace 6
workspace 7
workspace 8
workspace 9
layout "columns"
action "focus-next" id=1 behavior="focus-next"
action "workspace-two" id=2 behavior="activate-workspace" workspace=2
action "terminal" id=3 behavior="launch-application" application=1
binding action=1 keycode=57 modifiers="super"
binding action=2 keycode=3 modifiers="super"
binding action=3 keycode=28 modifiers="super"
chrome enabled=#true thickness=2 color="#70b7ff"
"##;
