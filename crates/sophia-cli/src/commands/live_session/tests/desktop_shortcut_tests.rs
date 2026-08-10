use super::*;

fn shortcut_candidate(
    bindings: Vec<sophia_config::DesktopShortcutBinding>,
) -> sophia_config::DesktopShortcutCandidate {
    sophia_config::DesktopShortcutCandidate {
        generation: sophia_config::ConfigGeneration::INITIAL,
        digest: sophia_config::ConfigDigest::new([7; 32]),
        profile: "test".to_owned(),
        bindings,
    }
}

fn key_shortcut(
    trigger: &str,
    target: sophia_config::DesktopShortcutTarget,
) -> sophia_config::DesktopShortcutBinding {
    sophia_config::DesktopShortcutBinding {
        chord: sophia_config::DesktopShortcutChord {
            kind: sophia_config::DesktopShortcutBindingKind::Key,
            modifiers: sophia_config::DesktopShortcutModifiers::SUPER,
            trigger: trigger.to_owned(),
        },
        target,
    }
}

#[test]
fn desktop_shortcuts_resolve_against_the_policy_action_catalog() {
    let target = sophia_config::DesktopShortcutTarget::PolicyAction("focus-next".to_owned());
    let candidate = shortcut_candidate(vec![
        key_shortcut("j", target.clone()),
        key_shortcut("l", target),
        key_shortcut(
            "return",
            sophia_config::DesktopShortcutTarget::Session(
                sophia_config::DesktopSessionShortcut::LaunchTerminal,
            ),
        ),
    ]);
    let configuration = sophia_protocol::PolicyConfiguration {
        connection_epoch: 1,
        generation: 1,
        actions: vec![
            sophia_protocol::PolicyActionRegistration {
                action: WmActionId::from_raw(1),
                name: "focus-next".to_owned(),
                session_operation_slot: None,
            },
            sophia_protocol::PolicyActionRegistration {
                action: WmActionId::from_raw(2),
                name: "spawn-terminal".to_owned(),
                session_operation_slot: Some(1),
            },
        ],
        chrome: sophia_protocol::WmChromePolicy::default(),
    };

    let mut registry = resolve_public_shortcuts(&candidate, &configuration).unwrap();
    assert_eq!(registry.binding_count(), 3);
    assert_eq!(
        registry.handle_key(
            36,
            WmModifierMask {
                bits: WmModifierMask::SUPER,
            },
            true,
        ),
        sophia_engine::WmShortcutDecision {
            action: Some(WmActionId::from_raw(1)),
            consumed: true,
        }
    );
}

#[test]
fn desktop_shortcuts_reject_unregistered_policy_semantics() {
    let candidate = shortcut_candidate(vec![key_shortcut(
        "j",
        sophia_config::DesktopShortcutTarget::PolicyAction("unknown".to_owned()),
    )]);
    let configuration = sophia_protocol::PolicyConfiguration {
        connection_epoch: 1,
        generation: 1,
        actions: Vec::new(),
        chrome: sophia_protocol::WmChromePolicy::default(),
    };

    assert_eq!(
        resolve_public_shortcuts(&candidate, &configuration),
        Err("shortcut names an unregistered policy action")
    );
}
