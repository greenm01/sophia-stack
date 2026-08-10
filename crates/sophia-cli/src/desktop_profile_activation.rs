use sophia_config::{
    DesktopAuthority, DesktopProfileActivationEffect, DesktopProfileActivationEffectKind,
    DesktopProfileActivationKey, DesktopProfileActivationMsg,
};

pub trait DesktopProfileAuthorityEffectExecutor {
    fn prepare_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool;

    fn activate_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool;

    fn rollback_authority(
        &mut self,
        authority: DesktopAuthority,
        key: DesktopProfileActivationKey,
    ) -> bool;
}

pub fn execute_desktop_profile_activation_effect<E>(
    executor: &mut E,
    effect: DesktopProfileActivationEffect,
) -> DesktopProfileActivationMsg
where
    E: DesktopProfileAuthorityEffectExecutor,
{
    let success = match effect.kind {
        DesktopProfileActivationEffectKind::PrepareAuthority => {
            executor.prepare_authority(effect.authority, effect.key)
        }
        DesktopProfileActivationEffectKind::ActivateAuthority => {
            executor.activate_authority(effect.authority, effect.key)
        }
        DesktopProfileActivationEffectKind::RollbackAuthority => {
            executor.rollback_authority(effect.authority, effect.key)
        }
    };
    match effect.kind {
        DesktopProfileActivationEffectKind::PrepareAuthority => {
            DesktopProfileActivationMsg::AuthorityPrepared {
                key: effect.key,
                authority: effect.authority,
                success,
            }
        }
        DesktopProfileActivationEffectKind::ActivateAuthority => {
            DesktopProfileActivationMsg::AuthorityActivated {
                key: effect.key,
                authority: effect.authority,
                success,
            }
        }
        DesktopProfileActivationEffectKind::RollbackAuthority => {
            DesktopProfileActivationMsg::RollbackCompleted {
                key: effect.key,
                authority: effect.authority,
                success,
            }
        }
    }
}
