#!/bin/sh
set -eu

export HOME=/root
export PATH=/usr/sbin:/usr/bin:/sbin:/bin
export LC_ALL=C
export XDG_RUNTIME_DIR=/tmp/sophia-runtime
export LIBGL_DRIVERS_PATH=/usr/lib/dri
# The minimal guest deliberately has neither logind nor a seatd daemon.
# Select libseat's direct no-op VT/device backend explicitly; production sessions
# retain normal libseat backend discovery.
export LIBSEAT_BACKEND=noop

mkdir -p /proc /sys /dev /run /run/udev /tmp /tmp/.X11-unix "$XDG_RUNTIME_DIR"
mount -t proc proc /proc 2>/dev/null || true
mount -t sysfs sysfs /sys 2>/dev/null || true
mount -t devtmpfs devtmpfs /dev 2>/dev/null || true
mkdir -p /dev/pts
mount -t devpts devpts /dev/pts
mount -t tmpfs tmpfs /run 2>/dev/null || true
chmod 700 "$XDG_RUNTIME_DIR"

scenario="session"
two_xterm=false
cmdline=""
IFS= read -r cmdline < /proc/cmdline || true
case " $cmdline " in
    *" sophia.scenario=emergency-recovery "*) scenario="emergency-recovery" ;;
    *" sophia.scenario=gtk-classic "*) scenario="gtk-classic" ;;
    *" sophia.scenario=gtk-confined "*) scenario="gtk-confined" ;;
    *" sophia.scenario=xmonad-m7 "*) scenario="xmonad-m7" ;;
    *" sophia.scenario=xmonad-idle-efficiency "*) scenario="xmonad-idle-efficiency" ;;
    *" sophia.scenario=xmonad-launch-burst "*) scenario="xmonad-launch-burst" ;;
    *" sophia.scenario=xmonad-producer-overload "*) scenario="xmonad-producer-overload" ;;
    *" sophia.scenario=xmonad-render-contention "*) scenario="xmonad-render-contention" ;;
    *" sophia.scenario=xmonad-resize-storm "*) scenario="xmonad-resize-storm" ;;
    *" sophia.scenario=xmonad-stale-response "*) scenario="xmonad-stale-response" ;;
    *" sophia.scenario=xmonad-m8-launcher "*) scenario="xmonad-m8-launcher" ;;
    *" sophia.scenario=xmonad-m8-mix "*) scenario="xmonad-m8-mix" ;;
    *" sophia.scenario=xmonad-m8-soak "*) scenario="xmonad-m8-soak" ;;
    *" sophia.scenario=xmonad-interactive "*) scenario="xmonad-interactive" ;;
esac
case " $cmdline " in
    *" sophia.two_xterm=1 "*) two_xterm=true ;;
esac

if [ "$scenario" = "emergency-recovery" ]; then
    echo "sophia_qemu_guest schema=1 status=booting gpu=virtio-gpu scenario=emergency-recovery"
elif [ "$scenario" = "gtk-classic" ] || [ "$scenario" = "gtk-confined" ]; then
    echo "sophia_qemu_guest schema=1 status=booting gpu=virtio-gpu scenario=$scenario"
elif [ "$scenario" = "xmonad-m7" ] || [ "$scenario" = "xmonad-idle-efficiency" ] || [ "$scenario" = "xmonad-launch-burst" ] || [ "$scenario" = "xmonad-producer-overload" ] || [ "$scenario" = "xmonad-render-contention" ] || [ "$scenario" = "xmonad-resize-storm" ] || [ "$scenario" = "xmonad-stale-response" ] || [ "$scenario" = "xmonad-m8-launcher" ] || [ "$scenario" = "xmonad-m8-mix" ] || [ "$scenario" = "xmonad-m8-soak" ] || [ "$scenario" = "xmonad-interactive" ]; then
    echo "sophia_qemu_guest schema=1 status=booting gpu=virtio-gpu scenario=$scenario"
else
    echo "sophia_qemu_guest schema=1 status=booting gpu=virtio-gpu ticks=300"
fi

udevd --daemon
udevadm control --log-priority=err

modprobe virtio_pci
modprobe virtio_gpu
modprobe virtio_input
modprobe evdev
udevadm trigger --action=add
udevadm settle --timeout=5

attempt=0
while [ ! -e /dev/dri/card0 ] && [ "$attempt" -lt 100 ]; do
    sleep 0.05
    attempt=$((attempt + 1))
done

if [ ! -e /dev/dri/card0 ]; then
    echo "sophia_qemu_guest schema=1 status=failed reason=virtio_gpu_drm_missing"
    poweroff -f
fi

connector_count=0
connected_count=0
for connector in /sys/class/drm/card[0-9]-*; do
    if [ ! -f "$connector/status" ]; then
        continue
    fi
    connector_count=$((connector_count + 1))
    status=""
    IFS= read -r status < "$connector/status" || true
    if [ "$status" = "connected" ]; then
        connected_count=$((connected_count + 1))
    fi
done
echo "sophia_qemu_topology schema=1 status=observed requested_heads=2 connectors=$connector_count connected=$connected_count"

input_devices=""
for device in /dev/input/event*; do
    if [ -e "$device" ]; then
        if [ -z "$input_devices" ]; then
            input_devices="$device"
        else
            input_devices="$input_devices,$device"
        fi
    fi
done

guard_pid=""
guard_triggered_file="/tmp/sophia-input-guard.triggered"
if [ "$scenario" = "emergency-recovery" ]; then
    if [ -z "$input_devices" ]; then
        echo "sophia_qemu_guest_recovery schema=1 status=failed reason=input_devices_missing"
        sync
        poweroff -f
    fi
    guard_armed_file="/tmp/sophia-input-guard.armed"
    rm -f "$guard_armed_file" "$guard_triggered_file"
    /usr/bin/sophia sophia-session-input-guard \
        "--input-devices=$input_devices" \
        "--armed-file=$guard_armed_file" \
        "--triggered-file=$guard_triggered_file" \
        "--owner-pid=$$" &
    guard_pid=$!
    guard_armed=false
    attempt=0
    while [ "$attempt" -lt 600 ]; do
        if [ -s "$guard_armed_file" ]; then
            guard_armed=true
            break
        fi
        if ! kill -0 "$guard_pid" 2>/dev/null; then
            break
        fi
        sleep 0.05
        attempt=$((attempt + 1))
    done
    if [ "$guard_armed" != true ]; then
        echo "sophia_qemu_guest_recovery schema=1 status=failed reason=input_guard_arm_timeout"
        sync
        poweroff -f
    fi
    set -- sophia-live-session --display=:181 --native-scanout --max-runtime-ms=30000
    echo "sophia_qemu_guest_recovery schema=1 status=running chord=ctrl-alt-backspace"
elif [ "$scenario" = "gtk-classic" ] || [ "$scenario" = "gtk-confined" ]; then
    profile="classic"
    [ "$scenario" = "gtk-confined" ] && profile="confined"
    # Accessibility is outside this minimal image's GTK rendering/input proof.
    # Disable its bus lookup explicitly while retaining the real session bus.
    export GTK_A11Y=none
    expected_stdout="$(printf 'sophia\n.')"
    expected_stdout="${expected_stdout%.}"
    set -- sophia-live-session --display=:181 --native-scanout --max-runtime-ms=30000 \
        --namespace-profile="$profile" --software-client-rendering \
        --client=zenity --client-arg=--entry --client-arg=--title \
        --client-arg='Sophia GTK proof' --client-arg=--text \
        --client-arg='Type sophia, then click OK' \
        --expect-client-stdout="$expected_stdout" --require-client-normal-exit \
        --expect-physical-text=sophia --expect-physical-pointer \
        --inject-surface-resize=640x360 --exit-after-input-proof
    echo "sophia_qemu_gtk schema=1 status=running profile=$profile"
elif [ "$scenario" = "xmonad-m7" ] || [ "$scenario" = "xmonad-idle-efficiency" ] || [ "$scenario" = "xmonad-launch-burst" ] || [ "$scenario" = "xmonad-producer-overload" ] || [ "$scenario" = "xmonad-render-contention" ] || [ "$scenario" = "xmonad-resize-storm" ] || [ "$scenario" = "xmonad-stale-response" ] || [ "$scenario" = "xmonad-m8-launcher" ] || [ "$scenario" = "xmonad-m8-mix" ] || [ "$scenario" = "xmonad-m8-soak" ] || [ "$scenario" = "xmonad-interactive" ]; then
    if [ ! -x /usr/bin/xmonad ]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=xmonad_missing"
        sync
        poweroff -f
    fi
    runtime_ms=60000
    [ "$scenario" != "xmonad-idle-efficiency" ] || runtime_ms=240000
    [ "$scenario" != "xmonad-launch-burst" ] || runtime_ms=240000
    [ "$scenario" != "xmonad-producer-overload" ] || runtime_ms=240000
    [ "$scenario" != "xmonad-render-contention" ] || runtime_ms=240000
    [ "$scenario" != "xmonad-resize-storm" ] || runtime_ms=180000
    [ "$scenario" != "xmonad-stale-response" ] || runtime_ms=120000
    [ "$scenario" != "xmonad-m8-mix" ] || runtime_ms=360000
    [ "$scenario" != "xmonad-m8-soak" ] || runtime_ms=2100000
    if [ "$scenario" = "xmonad-interactive" ]; then
        # This is an operator-owned development session, not an acceptance
        # clock. It exits only through the ordinary logout action.
        set -- sophia-live-session --display=:181 --native-scanout
    else
        set -- sophia-live-session --display=:181 --native-scanout --max-runtime-ms="$runtime_ms"
    fi
    if [ "$scenario" = "xmonad-producer-overload" ]; then
        if [ ! -x /usr/bin/sophia-present-overload-client ]; then
            echo "sophia_qemu_producer_overload schema=1 status=failed reason=application_missing program=/usr/bin/sophia-present-overload-client"
            sync
            poweroff -f
        fi
        export LANG=C.UTF-8
        export LC_ALL=C.UTF-8
        export SOPHIA_LIVE_SESSION_DIAGNOSTIC=1
        export SOPHIA_LIVE_SESSION_PRESENT_AGGREGATE=1
        export RUST_LOG=warn,sophia_backend_live::production_session::native_scanout::persistent_native_scanout=info
        set -- "$@" --session-mode=normal
        set -- "$@" --session-app=cpu=/usr/bin/xterm
        set -- "$@" --session-app-arg=cpu=-cm --session-app-arg=cpu=-dc
        set -- "$@" --session-app-arg=cpu=+bc
        set -- "$@" --session-app-arg=cpu=-e
        set -- "$@" --session-app-arg=cpu=/usr/bin/sleep
        set -- "$@" --session-app-arg=cpu=180
        set -- "$@" --session-start=cpu
        set -- "$@" --session-app=gpu=/usr/bin/sophia-present-overload-client
        set -- "$@" --session-action-app=launcher=gpu
        echo "sophia_qemu_xmonad schema=1 status=running windows=2 profile=xmonad mode=producer-overload producer=bounded-dri3-present interval_usec=5000 cpu_client=xterm"
    elif [ "$scenario" = "xmonad-idle-efficiency" ]; then
        if [ ! -x /usr/bin/sophia-idle-glxgears-client ]; then
            echo "sophia_qemu_idle_efficiency schema=1 status=failed reason=application_missing program=/usr/bin/sophia-idle-glxgears-client"
            sync
            poweroff -f
        fi
        export LANG=C.UTF-8
        export LC_ALL=C.UTF-8
        export RUST_LOG=warn,sophia_backend_live::production_session::native_scanout::persistent_native_scanout=info
        set -- "$@" --session-mode=normal
        set -- "$@" --session-app=cpu=/usr/bin/xterm
        set -- "$@" --session-app-arg=cpu=-cm --session-app-arg=cpu=-dc
        set -- "$@" --session-app-arg=cpu=+bc
        set -- "$@" --session-app-arg=cpu=-e
        set -- "$@" --session-app-arg=cpu=/usr/bin/sleep
        set -- "$@" --session-app-arg=cpu=180
        set -- "$@" --session-start=cpu
        set -- "$@" --session-app=gpu=/usr/bin/sophia-idle-glxgears-client
        set -- "$@" --session-action-app=launcher=gpu
        echo "sophia_qemu_xmonad schema=1 status=running windows=2 profile=xmonad mode=idle-efficiency producer=glxgears-static cpu_client=xterm"
    elif [ "$scenario" = "xmonad-render-contention" ]; then
        for program in /usr/bin/glxgears /usr/bin/xmobar; do
            if [ ! -x "$program" ]; then
                echo "sophia_qemu_render_contention schema=1 status=failed reason=application_missing program=$program"
                sync
                poweroff -f
            fi
        done
        export LANG=C.UTF-8
        export LC_ALL=C.UTF-8
        export RUST_LOG=warn
        set -- "$@" --session-mode=normal
        # Xmobar reserves fourteen rows before xmonad admits these windows.
        # Match the successive master dimensions inside that work area so
        # virgl spends startup rendering, not reallocating an oversize buffer.
        set -- "$@" --session-app=gpu1=/usr/bin/glxgears
        set -- "$@" --session-app-arg=gpu1=-geometry
        set -- "$@" --session-app-arg=gpu1=1276x786
        set -- "$@" --session-start=gpu1
        set -- "$@" --session-app=gpu2=/usr/bin/glxgears
        set -- "$@" --session-app-arg=gpu2=-geometry
        set -- "$@" --session-app-arg=gpu2=636x782
        set -- "$@" --session-app=gpu3=/usr/bin/glxgears
        set -- "$@" --session-app-arg=gpu3=-geometry
        set -- "$@" --session-app-arg=gpu3=636x782
        set -- "$@" --session-action-app=terminal=gpu2
        set -- "$@" --session-action-app=launcher=gpu3
        set -- "$@" --session-app=statusbar=/usr/bin/xmobar
        set -- "$@" --session-app-arg=statusbar=/usr/share/sophia/qemu_render_contention_xmobar.config
        set -- "$@" --session-start=statusbar
        echo "sophia_qemu_xmonad schema=1 status=running windows=3 profile=xmonad mode=render-contention producers=3 launch=serial cpu_bar=xmobar"
    elif [ "$scenario" = "xmonad-resize-storm" ]; then
        set -- "$@" --session-mode=normal
        set -- "$@" --session-app=renderer=/usr/bin/xterm
        set -- "$@" --session-app-arg=renderer=-cm --session-app-arg=renderer=-dc
        set -- "$@" --session-app-arg=renderer=-e
        set -- "$@" --session-app-arg=renderer=/usr/bin/sophia-resize-storm-client
        set -- "$@" --session-start=renderer
        set -- "$@" --inject-surface-resize-sequence=960x640,800x600,1024x700,720x540,960x640,800x600,1024x700,720x540,960x640,800x600,1024x700,720x540
        echo "sophia_qemu_xmonad schema=1 status=running windows=1 profile=xmonad mode=resize-storm steps=12"
    elif [ "$scenario" = "xmonad-stale-response" ]; then
        set -- "$@" --session-mode=normal
        set -- "$@" --session-app=primary=/usr/bin/xterm
        set -- "$@" --session-app-arg=primary=-cm --session-app-arg=primary=-dc
        set -- "$@" --session-app=secondary=/usr/bin/xterm
        set -- "$@" --session-app-arg=secondary=-cm --session-app-arg=secondary=-dc
        set -- "$@" --session-app=transient=/usr/bin/xterm
        set -- "$@" --session-app-arg=transient=-cm --session-app-arg=transient=-dc
        set -- "$@" --session-app-arg=transient=-e
        set -- "$@" --session-app-arg=transient=/usr/bin/sleep
        set -- "$@" --session-app-arg=transient=0.05
        set -- "$@" --session-start=primary --session-start=secondary
        set -- "$@" --session-action-app=terminal=transient
        echo "sophia_qemu_xmonad schema=1 status=running windows=2 profile=xmonad mode=stale-response"
    elif [ "$scenario" = "xmonad-launch-burst" ]; then
        set -- "$@" --session-mode=normal
        set -- "$@" --session-app=startup=/usr/bin/xterm
        set -- "$@" --session-app-arg=startup=-cm --session-app-arg=startup=-dc
        set -- "$@" --session-app=terminal=/usr/bin/xterm
        set -- "$@" --session-app-arg=terminal=-cm --session-app-arg=terminal=-dc
        set -- "$@" --session-start=startup --session-action-app=terminal=terminal
        # Twelve non-visual managed children leave four slots for the rapid
        # launch workload. This exercises the active-plus-pending limit without
        # turning the queue proof into a sixteen-pane resize benchmark.
        holder=1
        while [ "$holder" -le 12 ]; do
            set -- "$@" "--session-app=holder$holder=/usr/bin/sleep"
            set -- "$@" "--session-app-arg=holder$holder=20"
            set -- "$@" "--session-start=holder$holder"
            holder=$((holder + 1))
        done
        echo "sophia_qemu_xmonad schema=1 status=running windows=1 profile=xmonad mode=launch-burst"
    elif [ "$scenario" = "xmonad-m8-launcher" ]; then
        set -- "$@" --session-mode=normal
        set -- "$@" --session-app=terminal=/usr/bin/xterm
        set -- "$@" --session-app-arg=terminal=-cm --session-app-arg=terminal=-dc
        set -- "$@" --session-start=terminal --session-action-app=terminal=terminal
        echo "sophia_qemu_xmonad schema=1 status=running windows=1 profile=xmonad mode=normal"
    elif [ "$scenario" = "xmonad-m8-mix" ] || [ "$scenario" = "xmonad-m8-soak" ] || [ "$scenario" = "xmonad-interactive" ]; then
        for program in /usr/bin/firefox /usr/bin/vkcube /usr/bin/zenity; do
            if [ ! -x "$program" ]; then
                echo "sophia_qemu_xmonad schema=1 status=failed reason=m8_application_missing program=$program"
                sync
                poweroff -f
            fi
        done
        export MOZ_ENABLE_WAYLAND=0
        export MOZ_FORCE_DISABLE_E10S=1
        export MOZ_USE_XINPUT2=1
        export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json
        # The M8 verifier consumes reduced stdout records, not per-frame
        # tracing. Keep the emulated serial channel current so host input is
        # ordered against live guest state rather than a stale log backlog.
        export RUST_LOG=warn
        mkdir -p /tmp/firefox-profile
        printf '%s\n' \
            'user_pref("browser.tabs.remote.autostart", false);' \
            'user_pref("browser.tabs.remote.autostart.2", false);' \
            'user_pref("fission.autostart", false);' \
            'user_pref("middlemouse.paste", true);' \
            'user_pref("middlemouse.contentLoadURL", false);' \
            > /tmp/firefox-profile/user.js
        set -- "$@" --session-mode=normal
        set -- "$@" --session-app=terminal=/usr/bin/xterm
        set -- "$@" --session-app-arg=terminal=-cm --session-app-arg=terminal=-dc
        set -- "$@" --session-app=vulkan=/usr/bin/vkcube --session-app-arg=vulkan=--wsi --session-app-arg=vulkan=xcb
        # Match the deterministic two-column QEMU content allocation. Under
        # TCG, resizing Lavapipe during initial admission can exceed the
        # production two-second transaction budget; Firefox still exercises
        # the mixed-workload resize path after startup.
        set -- "$@" --session-app-arg=vulkan=--width --session-app-arg=vulkan=636
        set -- "$@" --session-app-arg=vulkan=--height --session-app-arg=vulkan=796
        set -- "$@" --session-app=launcher=/usr/bin/sophia-zenity-launcher
        set -- "$@" --session-app=firefox=/usr/bin/firefox --session-app-arg=firefox=--new-instance --session-app-arg=firefox=--no-remote
        set -- "$@" --session-app-arg=firefox=--profile --session-app-arg=firefox=/tmp/firefox-profile
        set -- "$@" --session-app-arg=firefox=file:///usr/share/sophia/firefox_m8_local_page.html
        if [ "$scenario" = "xmonad-interactive" ]; then
            set -- "$@" --session-start=terminal
        else
            set -- "$@" --session-start=terminal --session-start=vulkan
        fi
        set -- "$@" --session-action-app=terminal=terminal --session-action-app=launcher=launcher --session-action-app=firefox=firefox
        if [ "$scenario" = "xmonad-interactive" ]; then
            echo "sophia_qemu_xmonad schema=1 status=running windows=1 profile=xmonad mode=interactive proof_watchdog=off fault_injection=off"
        else
            set -- "$@" --firefox-m8-proof
            echo "sophia_qemu_xmonad schema=1 status=running windows=2 profile=xmonad mode=m8-app-mix"
        fi
    else
        set -- "$@" --secondary-terminal
        echo "sophia_qemu_xmonad schema=1 status=running windows=2 profile=xmonad"
    fi
    set -- "$@" --wm-process=/usr/bin/sophia-x11-wm-bridge
    set -- "$@" --wm-process-arg=--profile=xmonad
    set -- "$@" --wm-process-arg=--wm=/usr/bin/xmonad
    set -- "$@" --wm-process-arg=--wm-private-alias=xmonad/xmonad-x86_64-linux
else
    set -- sophia-live-session --display=:181 --native-scanout --max-ticks=300 \
        --expect-physical-text=sophia --expect-physical-pointer
    if [ "$two_xterm" = true ]; then
        set -- "$@" --secondary-terminal
    fi
fi

if [ -n "$input_devices" ]; then
    set -- "$@" "--input-devices=$input_devices"
if [ "$scenario" = "xmonad-m7" ] || [ "$scenario" = "xmonad-m8-launcher" ] || [ "$scenario" = "xmonad-m8-mix" ] || [ "$scenario" = "xmonad-m8-soak" ]; then
    (
        while ! pidof sophia-x11-wm-bridge >/dev/null 2>&1; do sleep 0.05; done
        # Start the fault clock after the scenario's startup clients exist.
        # Bridge startup precedes client admission by an unbounded amount on
        # slower guests, so it cannot safely anchor this recovery boundary.
        while ! pidof xterm >/dev/null 2>&1; do sleep 0.05; done
        if [ "$scenario" = "xmonad-m8-mix" ] || [ "$scenario" = "xmonad-m8-soak" ]; then
            while ! pidof vkcube >/dev/null 2>&1; do sleep 0.05; done
        fi
        sleep 30
        while :; do
            wm_pid="$(pidof xmonad 2>/dev/null || true)"
            bridge_pid="$(pidof sophia-x11-wm-bridge 2>/dev/null || true)"
            [ -z "$wm_pid" ] || kill -TERM $wm_pid 2>/dev/null || true
            [ -z "$bridge_pid" ] || kill -TERM $bridge_pid 2>/dev/null || true
            echo "sophia_qemu_xmonad schema=1 status=restart_injected target=compatibility_bridge"
            [ "$scenario" = "xmonad-m8-soak" ] || break
            sleep 180
        done
    ) &
fi

fi

set +e
# Give every application in the guest one session-scoped bus. Modern GTK
# acquires the bus before opening X, so a bus-less image can strand launchers
# without ever reaching the authority listener.
SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
    /usr/bin/dbus-run-session -- /usr/bin/sophia "$@"
status=$?
set -e

if [ "$scenario" = "emergency-recovery" ]; then
    guard_done=false
    attempt=0
    while [ "$attempt" -lt 100 ]; do
        if ! kill -0 "$guard_pid" 2>/dev/null; then
            guard_done=true
            break
        fi
        sleep 0.05
        attempt=$((attempt + 1))
    done
    set +e
    if [ "$guard_done" = true ]; then
        wait "$guard_pid"
        guard_status=$?
    else
        kill -TERM "$guard_pid" 2>/dev/null || true
        wait "$guard_pid" 2>/dev/null || true
        guard_status=124
    fi
    set -e
    guard_pid=""
else
    guard_status=0
fi

if [ "$scenario" = "emergency-recovery" ]; then
    if [ "$status" -eq 0 ] && [ "$guard_status" -eq 0 ] \
        && [ -s "$guard_triggered_file" ]; then
        echo "sophia_qemu_guest_recovery schema=1 status=complete exit_status=0 guard_exit_status=0"
    else
        echo "sophia_qemu_guest_recovery schema=1 status=failed reason=recovery_exit exit_status=$status guard_exit_status=$guard_status"
    fi
elif [ "$scenario" = "gtk-classic" ] || [ "$scenario" = "gtk-confined" ]; then
    if [ "$status" -eq 0 ]; then
        echo "sophia_qemu_guest schema=1 status=complete scenario=$scenario"
    else
        echo "sophia_qemu_guest schema=1 status=failed reason=gtk_session_exit scenario=$scenario exit_status=$status"
    fi
elif [ "$scenario" = "xmonad-m7" ] || [ "$scenario" = "xmonad-idle-efficiency" ] || [ "$scenario" = "xmonad-launch-burst" ] || [ "$scenario" = "xmonad-producer-overload" ] || [ "$scenario" = "xmonad-render-contention" ] || [ "$scenario" = "xmonad-resize-storm" ] || [ "$scenario" = "xmonad-stale-response" ] || [ "$scenario" = "xmonad-m8-launcher" ] || [ "$scenario" = "xmonad-m8-mix" ] || [ "$scenario" = "xmonad-m8-soak" ] || [ "$scenario" = "xmonad-interactive" ]; then
    if [ "$status" -eq 0 ]; then
        echo "sophia_qemu_guest schema=1 status=complete scenario=$scenario"
    else
        echo "sophia_qemu_guest schema=1 status=failed reason=xmonad_session_exit exit_status=$status"
    fi
elif [ "$status" -eq 0 ]; then
    echo "sophia_qemu_guest schema=1 status=complete ticks=300"
else
    echo "sophia_qemu_guest schema=1 status=failed reason=session_exit exit_status=$status"
fi

sync
poweroff -f
