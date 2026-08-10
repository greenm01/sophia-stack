#[derive(Debug)]
struct PreparedAuthorityFragment {
    slot: sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopAuthorityCandidate>,
}

impl PreparedAuthorityFragment {
    fn new(
        fragments: &sophia_config::DesktopProfileFragments,
        authority: sophia_config::DesktopAuthority,
        key: sophia_config::DesktopProfileActivationKey,
    ) -> Result<Self, sophia_config::DesktopProfileCandidateSlotError> {
        let slot = sophia_config::DesktopProfileCandidateSlot::new(authority);
        Ok(Self {
            slot: sophia_config::prepare_desktop_profile_candidate_slot_from_fragment(
                &slot,
                fragments.path(authority),
                key,
            )?,
        })
    }

    const fn slot_mut(
        &mut self,
    ) -> &mut sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopAuthorityCandidate>
    {
        &mut self.slot
    }

}

impl PreparedPublicPolicyLaunch {
    fn prepare_startup(
        &mut self,
        session_profile: &mut PreparedSessionProfile,
        input_profile: &mut PreparedInputProfile,
        output_profile: &mut PreparedOutputProfile,
        model: &sophia_config::DesktopProfileActivationModel,
        key: sophia_config::DesktopProfileActivationKey,
    ) -> Result<
        sophia_cli::desktop_profile_activation::DesktopProfileStartupPreparationReport,
        sophia_cli::desktop_profile_activation::DesktopProfileStartupActivationError,
    > {
        let mut executor = PublicProfilePreparationExecutor {
            policy: self.policy_profile.slot_mut(),
            shell: self.shell_profile.slot_mut(),
            shortcut: &mut self.shortcut_profile_slot,
            session: session_profile.slot_mut(),
            input: input_profile.slot_mut(),
            output: output_profile.slot_mut(),
            broker: self.broker_profile.slot_mut(),
        };
        sophia_cli::desktop_profile_activation::run_desktop_profile_startup_preparation(
            model,
            key,
            &mut executor,
        )
    }
}

struct PublicProfilePreparationExecutor<'a> {
    policy:
        &'a mut sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopAuthorityCandidate>,
    shell:
        &'a mut sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopAuthorityCandidate>,
    shortcut:
        &'a mut sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopShortcutCandidate>,
    session:
        &'a mut sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopSessionCandidate>,
    input: &'a mut sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopInputCandidate>,
    output:
        &'a mut sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopOutputCandidate>,
    broker:
        &'a mut sophia_config::DesktopProfileCandidateSlot<sophia_config::DesktopAuthorityCandidate>,
}

fn prepare_profile_slot<T>(
    slot: &mut sophia_config::DesktopProfileCandidateSlot<T>,
    key: sophia_config::DesktopProfileActivationKey,
) -> bool
where
    T: Clone + sophia_config::DesktopProfileCandidatePayload,
{
    let Some(candidate) = slot.candidate().cloned() else {
        return false;
    };
    if candidate.activation_key() != key {
        return false;
    }
    let Ok(next) = sophia_config::prepare_desktop_profile_candidate_slot(slot, candidate) else {
        return false;
    };
    *slot = next;
    true
}

fn rollback_profile_slot<T>(
    slot: &mut sophia_config::DesktopProfileCandidateSlot<T>,
    key: sophia_config::DesktopProfileActivationKey,
) -> bool
where
    T: Clone + sophia_config::DesktopProfileCandidatePayload,
{
    let Ok(next) = sophia_config::rollback_desktop_profile_candidate_slot(slot, key) else {
        return false;
    };
    *slot = next;
    true
}

impl PublicProfilePreparationExecutor<'_> {
    fn dispatch_prepare(
        &mut self,
        authority: sophia_config::DesktopAuthority,
        key: sophia_config::DesktopProfileActivationKey,
    ) -> bool {
        match authority {
            sophia_config::DesktopAuthority::Policy => prepare_profile_slot(self.policy, key),
            sophia_config::DesktopAuthority::Shell => prepare_profile_slot(self.shell, key),
            sophia_config::DesktopAuthority::Shortcut => prepare_profile_slot(self.shortcut, key),
            sophia_config::DesktopAuthority::Session => prepare_profile_slot(self.session, key),
            sophia_config::DesktopAuthority::Input => prepare_profile_slot(self.input, key),
            sophia_config::DesktopAuthority::Output => prepare_profile_slot(self.output, key),
            sophia_config::DesktopAuthority::Broker => prepare_profile_slot(self.broker, key),
        }
    }

    fn dispatch_rollback(
        &mut self,
        authority: sophia_config::DesktopAuthority,
        key: sophia_config::DesktopProfileActivationKey,
    ) -> bool {
        match authority {
            sophia_config::DesktopAuthority::Policy => rollback_profile_slot(self.policy, key),
            sophia_config::DesktopAuthority::Shell => rollback_profile_slot(self.shell, key),
            sophia_config::DesktopAuthority::Shortcut => {
                rollback_profile_slot(self.shortcut, key)
            }
            sophia_config::DesktopAuthority::Session => rollback_profile_slot(self.session, key),
            sophia_config::DesktopAuthority::Input => rollback_profile_slot(self.input, key),
            sophia_config::DesktopAuthority::Output => rollback_profile_slot(self.output, key),
            sophia_config::DesktopAuthority::Broker => rollback_profile_slot(self.broker, key),
        }
    }
}

impl sophia_cli::desktop_profile_activation::DesktopProfileAuthorityEffectExecutor
    for PublicProfilePreparationExecutor<'_>
{
    fn prepare_authority(
        &mut self,
        authority: sophia_config::DesktopAuthority,
        key: sophia_config::DesktopProfileActivationKey,
    ) -> bool {
        self.dispatch_prepare(authority, key)
    }

    fn activate_authority(
        &mut self,
        _authority: sophia_config::DesktopAuthority,
        _key: sophia_config::DesktopProfileActivationKey,
    ) -> bool {
        false
    }

    fn rollback_authority(
        &mut self,
        authority: sophia_config::DesktopAuthority,
        key: sophia_config::DesktopProfileActivationKey,
    ) -> bool {
        self.dispatch_rollback(authority, key)
    }
}
