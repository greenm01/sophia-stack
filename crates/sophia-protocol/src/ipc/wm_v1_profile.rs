use crate::{
    IpcCodecError, SOPHIA_WM_OUTCOME_PROFILE_ACCEPTED, SOPHIA_WM_OUTCOME_PROFILE_REJECTED_IDENTITY,
    SOPHIA_WM_OUTCOME_PROFILE_REJECTED_STATE, TransactionId, WmV1ProfileActivate,
    WmV1ProfileActive, WmV1ProfilePrepare, WmV1ProfilePrepared, WmV1ProfileRollback,
    WmV1ProfileRolledBack, decode_wm_v1_profile_activate_frame, decode_wm_v1_profile_active_frame,
    decode_wm_v1_profile_prepare_frame, decode_wm_v1_profile_prepared_frame,
    decode_wm_v1_profile_rollback_frame, decode_wm_v1_profile_rolled_back_frame,
    encode_wm_v1_profile_activate_frame, encode_wm_v1_profile_active_frame,
    encode_wm_v1_profile_prepare_frame, encode_wm_v1_profile_prepared_frame,
    encode_wm_v1_profile_rollback_frame, encode_wm_v1_profile_rolled_back_frame,
};

pub const WM_V1_PROFILE_DIGEST_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmV1ProfileIdentity {
    pub connection_epoch: u64,
    pub profile_generation: u64,
    pub profile_digest: [u8; WM_V1_PROFILE_DIGEST_BYTES],
}

impl WmV1ProfileIdentity {
    pub fn new(
        connection_epoch: u64,
        profile_generation: u64,
        profile_digest: [u8; WM_V1_PROFILE_DIGEST_BYTES],
    ) -> Result<Self, IpcCodecError> {
        if connection_epoch == 0 {
            return Err(IpcCodecError::InvalidProfileIdentity("connection_epoch"));
        }
        if profile_generation == 0 {
            return Err(IpcCodecError::InvalidProfileIdentity("profile_generation"));
        }
        if profile_digest == [0; WM_V1_PROFILE_DIGEST_BYTES] {
            return Err(IpcCodecError::InvalidProfileIdentity("profile_digest"));
        }
        Ok(Self {
            connection_epoch,
            profile_generation,
            profile_digest,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum WmV1ProfileOutcome {
    Accepted = SOPHIA_WM_OUTCOME_PROFILE_ACCEPTED,
    RejectedIdentity = SOPHIA_WM_OUTCOME_PROFILE_REJECTED_IDENTITY,
    RejectedState = SOPHIA_WM_OUTCOME_PROFILE_REJECTED_STATE,
}

impl TryFrom<u16> for WmV1ProfileOutcome {
    type Error = IpcCodecError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            SOPHIA_WM_OUTCOME_PROFILE_ACCEPTED => Ok(Self::Accepted),
            SOPHIA_WM_OUTCOME_PROFILE_REJECTED_IDENTITY => Ok(Self::RejectedIdentity),
            SOPHIA_WM_OUTCOME_PROFILE_REJECTED_STATE => Ok(Self::RejectedState),
            _ => Err(IpcCodecError::InvalidEnum {
                field: "profile_outcome",
                value: u32::from(value),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmV1ProfileCommand {
    pub transaction: TransactionId,
    pub identity: WmV1ProfileIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmV1ProfileCompletion {
    pub transaction: TransactionId,
    pub identity: WmV1ProfileIdentity,
    pub outcome: WmV1ProfileOutcome,
}

fn command(
    transaction: TransactionId,
    connection_epoch: u64,
    profile_generation: u64,
    profile_digest: [u8; WM_V1_PROFILE_DIGEST_BYTES],
) -> Result<WmV1ProfileCommand, IpcCodecError> {
    Ok(WmV1ProfileCommand {
        transaction,
        identity: WmV1ProfileIdentity::new(connection_epoch, profile_generation, profile_digest)?,
    })
}

fn completion(
    transaction: TransactionId,
    connection_epoch: u64,
    profile_generation: u64,
    profile_digest: [u8; WM_V1_PROFILE_DIGEST_BYTES],
    outcome: u16,
) -> Result<WmV1ProfileCompletion, IpcCodecError> {
    Ok(WmV1ProfileCompletion {
        transaction,
        identity: WmV1ProfileIdentity::new(connection_epoch, profile_generation, profile_digest)?,
        outcome: outcome.try_into()?,
    })
}

macro_rules! profile_command_codec {
    ($decode:ident, $encode:ident, $decode_frame:ident, $encode_frame:ident, $wire:ident) => {
        pub fn $decode(frame: &[u8]) -> Result<WmV1ProfileCommand, IpcCodecError> {
            let (transaction, wire) = $decode_frame(frame)?;
            command(
                transaction,
                wire.connection_epoch,
                wire.profile_generation,
                wire.profile_digest,
            )
        }

        pub fn $encode(command: WmV1ProfileCommand) -> Result<Vec<u8>, IpcCodecError> {
            $encode_frame(
                command.transaction,
                &$wire {
                    connection_epoch: command.identity.connection_epoch,
                    profile_generation: command.identity.profile_generation,
                    profile_digest: command.identity.profile_digest,
                },
            )
        }
    };
}

macro_rules! profile_completion_codec {
    ($decode:ident, $encode:ident, $decode_frame:ident, $encode_frame:ident, $wire:ident) => {
        pub fn $decode(frame: &[u8]) -> Result<WmV1ProfileCompletion, IpcCodecError> {
            let (transaction, wire) = $decode_frame(frame)?;
            completion(
                transaction,
                wire.connection_epoch,
                wire.profile_generation,
                wire.profile_digest,
                wire.outcome,
            )
        }

        pub fn $encode(completion: WmV1ProfileCompletion) -> Result<Vec<u8>, IpcCodecError> {
            $encode_frame(
                completion.transaction,
                &$wire {
                    connection_epoch: completion.identity.connection_epoch,
                    profile_generation: completion.identity.profile_generation,
                    profile_digest: completion.identity.profile_digest,
                    outcome: completion.outcome as u16,
                },
            )
        }
    };
}

profile_command_codec!(
    decode_wm_v1_profile_prepare,
    encode_wm_v1_profile_prepare,
    decode_wm_v1_profile_prepare_frame,
    encode_wm_v1_profile_prepare_frame,
    WmV1ProfilePrepare
);
profile_completion_codec!(
    decode_wm_v1_profile_prepared,
    encode_wm_v1_profile_prepared,
    decode_wm_v1_profile_prepared_frame,
    encode_wm_v1_profile_prepared_frame,
    WmV1ProfilePrepared
);
profile_command_codec!(
    decode_wm_v1_profile_activate,
    encode_wm_v1_profile_activate,
    decode_wm_v1_profile_activate_frame,
    encode_wm_v1_profile_activate_frame,
    WmV1ProfileActivate
);
profile_completion_codec!(
    decode_wm_v1_profile_active,
    encode_wm_v1_profile_active,
    decode_wm_v1_profile_active_frame,
    encode_wm_v1_profile_active_frame,
    WmV1ProfileActive
);
profile_command_codec!(
    decode_wm_v1_profile_rollback,
    encode_wm_v1_profile_rollback,
    decode_wm_v1_profile_rollback_frame,
    encode_wm_v1_profile_rollback_frame,
    WmV1ProfileRollback
);
profile_completion_codec!(
    decode_wm_v1_profile_rolled_back,
    encode_wm_v1_profile_rolled_back,
    decode_wm_v1_profile_rolled_back_frame,
    encode_wm_v1_profile_rolled_back_frame,
    WmV1ProfileRolledBack
);
