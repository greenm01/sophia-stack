use super::*;

/// One-shot proof control for the boundary between physical KMS acceptance and
/// candidate installation. A non-startup public proposal must never consume it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OutputProofRollbackAfterApply {
    requested: bool,
    fired: bool,
}

impl OutputProofRollbackAfterApply {
    pub(super) const fn new(requested: bool) -> Self {
        Self {
            requested,
            fired: false,
        }
    }

    pub(super) fn take_for_startup(&mut self, startup: bool) -> bool {
        if !self.requested || self.fired || !startup {
            return false;
        }
        self.fired = true;
        true
    }
}

pub(super) fn parse_output_proof_rollback_after_apply(
    args: &[String],
    native_scanout: bool,
    normal_session: bool,
    wm_interface: sophia_config::ExternalWmInterface,
    max_runtime: Option<Duration>,
    other_proof_control: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    let requested = args
        .iter()
        .any(|arg| arg == "--output-proof-rollback-after-apply");
    validate_output_proof_rollback_after_apply(
        requested,
        native_scanout,
        normal_session,
        wm_interface,
        max_runtime,
        std::env::var_os("SOPHIA_FRAME_FED_OUTPUT_ARM").as_deref()
            == Some(std::ffi::OsStr::new("1")),
        other_proof_control,
    )?;
    Ok(requested)
}

pub(super) fn validate_output_proof_rollback_after_apply(
    requested: bool,
    native_scanout: bool,
    normal_session: bool,
    wm_interface: sophia_config::ExternalWmInterface,
    max_runtime: Option<Duration>,
    hardware_armed: bool,
    other_proof_control: bool,
) -> Result<(), &'static str> {
    if !requested {
        return Ok(());
    }
    if !native_scanout
        || !normal_session
        || wm_interface != sophia_config::ExternalWmInterface::SophiaWmV1
        || max_runtime.is_none()
    {
        return Err(
            "--output-proof-rollback-after-apply requires --native-scanout, --session-mode=normal, --wm-interface=sophia_wm_v1, and --max-runtime-ms",
        );
    }
    if !hardware_armed {
        return Err(
            "set SOPHIA_FRAME_FED_OUTPUT_ARM=1 to arm --output-proof-rollback-after-apply",
        );
    }
    if other_proof_control {
        return Err(
            "--output-proof-rollback-after-apply is mutually exclusive with WM proof controls",
        );
    }
    Ok(())
}

pub(super) fn validate_prepared_output_proof_candidate(
    requested: bool,
    prepared: bool,
) -> Result<(), &'static str> {
    if requested && !prepared {
        Err("--output-proof-rollback-after-apply requires a prepared startup output candidate")
    } else {
        Ok(())
    }
}
