use sophia_cli::desktop_profile_activation::{
    DesktopProfileAuthorityEffectExecutor, execute_desktop_profile_activation_effect,
};
use sophia_config::{
    ConfigDigest, ConfigGeneration, DesktopAuthority, DesktopProfileActivationEffect,
    DesktopProfileActivationEffectKind, DesktopProfileActivationKey, DesktopProfileActivationMsg,
};

#[derive(Default)]
struct FakeExecutor {
    calls: Vec<(
        DesktopProfileActivationEffectKind,
        DesktopAuthority,
        DesktopProfileActivationKey,
    )>,
    prepare_success: bool,
    activate_success: bool,
    rollback_success: bool,
}

impl DesktopProfileAuthorityEffectExecutor for FakeExecutor {
    fn prepare_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool {
        self.calls.push((
            DesktopProfileActivationEffectKind::PrepareAuthority,
            authority,
            key,
        ));
        self.prepare_success
    }

    fn activate_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool {
        self.calls.push((
            DesktopProfileActivationEffectKind::ActivateAuthority,
            authority,
            key,
        ));
        self.activate_success
    }

    fn rollback_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool {
        self.calls.push((
            DesktopProfileActivationEffectKind::RollbackAuthority,
            authority,
            key,
        ));
        self.rollback_success
    }
}

fn key() -> DesktopProfileActivationKey {
    DesktopProfileActivationKey::new(ConfigGeneration::from_raw(7), ConfigDigest::new([7; 32]))
}

fn effect(
    kind: DesktopProfileActivationEffectKind,
    authority: DesktopAuthority,
) -> DesktopProfileActivationEffect {
    DesktopProfileActivationEffect {
        kind,
        authority,
        key: key(),
    }
}

#[test]
fn executor_dispatches_each_effect_to_its_authority_handler() {
    let mut executor = FakeExecutor {
        prepare_success: true,
        activate_success: true,
        rollback_success: true,
        ..FakeExecutor::default()
    };

    let prepared = execute_desktop_profile_activation_effect(
        &mut executor,
        effect(
            DesktopProfileActivationEffectKind::PrepareAuthority,
            DesktopAuthority::Policy,
        ),
    );
    let activated = execute_desktop_profile_activation_effect(
        &mut executor,
        effect(
            DesktopProfileActivationEffectKind::ActivateAuthority,
            DesktopAuthority::Output,
        ),
    );
    let rolled_back = execute_desktop_profile_activation_effect(
        &mut executor,
        effect(
            DesktopProfileActivationEffectKind::RollbackAuthority,
            DesktopAuthority::Broker,
        ),
    );

    assert_eq!(
        executor.calls,
        vec![
            (
                DesktopProfileActivationEffectKind::PrepareAuthority,
                DesktopAuthority::Policy,
                key(),
            ),
            (
                DesktopProfileActivationEffectKind::ActivateAuthority,
                DesktopAuthority::Output,
                key(),
            ),
            (
                DesktopProfileActivationEffectKind::RollbackAuthority,
                DesktopAuthority::Broker,
                key(),
            ),
        ]
    );
    assert_eq!(
        prepared,
        DesktopProfileActivationMsg::AuthorityPrepared {
            key: key(),
            authority: DesktopAuthority::Policy,
            success: true,
        }
    );
    assert_eq!(
        activated,
        DesktopProfileActivationMsg::AuthorityActivated {
            key: key(),
            authority: DesktopAuthority::Output,
            success: true,
        }
    );
    assert_eq!(
        rolled_back,
        DesktopProfileActivationMsg::RollbackCompleted {
            key: key(),
            authority: DesktopAuthority::Broker,
            success: true,
        }
    );
}

#[test]
fn executor_preserves_failed_completion_identity() {
    let mut executor = FakeExecutor::default();
    let message = execute_desktop_profile_activation_effect(
        &mut executor,
        effect(
            DesktopProfileActivationEffectKind::PrepareAuthority,
            DesktopAuthority::Shell,
        ),
    );

    assert_eq!(
        message,
        DesktopProfileActivationMsg::AuthorityPrepared {
            key: key(),
            authority: DesktopAuthority::Shell,
            success: false,
        }
    );
}
