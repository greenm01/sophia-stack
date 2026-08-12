fn session_shortcut_identity(
    shortcut: sophia_config::DesktopSessionShortcut,
) -> (u16, &'static str) {
    match shortcut {
        sophia_config::DesktopSessionShortcut::LaunchTerminal => (1, "spawn-terminal"),
        sophia_config::DesktopSessionShortcut::LaunchBrowser => (2, "spawn-browser"),
        sophia_config::DesktopSessionShortcut::CloseFocused => (3, "close-window"),
        sophia_config::DesktopSessionShortcut::Logout => (4, "logout"),
    }
}

fn resolve_public_shortcuts(
    candidate: &sophia_config::DesktopShortcutCandidate,
    configuration: &sophia_protocol::PolicyConfiguration,
) -> Result<sophia_engine::WmShortcutRegistry, &'static str> {
    if candidate.generation.raw() != configuration.generation {
        return Err("shortcut and policy generations differ");
    }
    let policy_actions = configuration
        .actions
        .iter()
        .filter(|action| action.session_operation_slot.is_none())
        .map(|action| (action.name.as_str(), action.action))
        .collect::<BTreeMap<_, _>>();
    let session_actions = configuration
        .actions
        .iter()
        .filter_map(|action| {
            action
                .session_operation_slot
                .map(|slot| ((slot, action.name.as_str()), action.action))
        })
        .collect::<BTreeMap<_, _>>();
    let mut bindings = Vec::with_capacity(candidate.bindings.len());
    for binding in &candidate.bindings {
        if binding.chord.kind == sophia_config::DesktopShortcutBindingKind::Pointer {
            let valid_engine_gesture = matches!(
                &binding.target,
                sophia_config::DesktopShortcutTarget::PolicyAction(action)
                    if (action == "move" && binding.chord.trigger == "left"
                        || action == "resize" && binding.chord.trigger == "right")
                        && binding.chord.modifiers.bits()
                            == sophia_config::DesktopShortcutModifiers::SUPER.bits()
            );
            if !valid_engine_gesture {
                return Err("unsupported pointer shortcut");
            }
            continue;
        }
        let action = match &binding.target {
            sophia_config::DesktopShortcutTarget::PolicyAction(name) => policy_actions
                .get(name.as_str())
                .copied()
                .ok_or("shortcut names an unregistered policy action")?,
            sophia_config::DesktopShortcutTarget::Session(shortcut) => session_actions
                .get(&session_shortcut_identity(*shortcut))
                .copied()
                .ok_or("shortcut names an unavailable session capability")?,
        };
        let keycode = sophia_config::desktop_shortcut_evdev_keycode(&binding.chord.trigger)
            .ok_or("shortcut trigger has no evdev identity")?;
        bindings.push(sophia_protocol::WmBindingRegistration {
            action,
            keycode,
            modifiers: sophia_protocol::WmModifierMask {
                bits: u32::from(binding.chord.modifiers.bits()),
            },
        });
    }
    // Built from configuration, so there is no protocol revision to declare and no
    // hello to fabricate. The public path never spoke API v7; it only borrowed its
    // constructor.
    sophia_engine::WmShortcutRegistry::new(
        &bindings,
        sophia_protocol::WmCapabilities::all_supported(),
        configuration.generation,
        configuration.chrome,
    )
    .map_err(|_| "resolved shortcut registry is invalid")
}
