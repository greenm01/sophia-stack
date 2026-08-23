use crate::{
    AttentionState, BrokerToplevelActionGrant, BrokerV1ClientHello, BrokerV1Rejection,
    BrokerV1Request, BrokerV1Response, BrokerV1ServerWelcome, DisplayLabel, IconTokenId,
    MAX_CHROME_LABEL_LEN, MetadataDisclosure, MetadataDisclosureRule, NamespaceProfile,
    ReducedMetadataCandidate, SanitizedChromeMetadata, TransactionId, TrustLevel,
};

use super::cursor::{Cursor, push_u8, push_u16, push_u32, push_u64};
use super::frame::{decode_frame, encode_frame};
use super::primitives::{
    decode_optional_text, decode_surface_id, encode_optional_text, encode_surface_id,
};
use super::types::{IpcCodecError, IpcMessageKind};

pub fn encode_broker_v1_client_hello_frame(
    hello: BrokerV1ClientHello,
) -> Result<Vec<u8>, IpcCodecError> {
    let mut payload = Vec::with_capacity(4);
    push_u16(&mut payload, hello.minimum_revision);
    push_u16(&mut payload, hello.maximum_revision);
    encode_frame(
        IpcMessageKind::BrokerV1ClientHello,
        TransactionId::INVALID,
        &payload,
    )
}

pub fn decode_broker_v1_client_hello_frame(
    frame: &[u8],
) -> Result<BrokerV1ClientHello, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    require_kind(header.message_kind, IpcMessageKind::BrokerV1ClientHello)?;
    require_handshake_transaction(header.transaction)?;
    let mut cursor = Cursor::new(payload);
    let hello = BrokerV1ClientHello {
        minimum_revision: cursor.u16()?,
        maximum_revision: cursor.u16()?,
    };
    cursor.finish()?;
    Ok(hello)
}

pub fn encode_broker_v1_server_welcome_frame(
    welcome: BrokerV1ServerWelcome,
) -> Result<Vec<u8>, IpcCodecError> {
    let mut payload = Vec::with_capacity(16);
    push_u16(&mut payload, welcome.selected_revision);
    push_u16(&mut payload, 0);
    push_u64(&mut payload, welcome.connection_epoch);
    push_u32(&mut payload, welcome.max_surfaces);
    push_u16(&mut payload, welcome.max_label_bytes);
    push_u16(&mut payload, 0);
    encode_frame(
        IpcMessageKind::BrokerV1ServerWelcome,
        TransactionId::INVALID,
        &payload,
    )
}

pub fn decode_broker_v1_server_welcome_frame(
    frame: &[u8],
) -> Result<BrokerV1ServerWelcome, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    require_kind(header.message_kind, IpcMessageKind::BrokerV1ServerWelcome)?;
    require_handshake_transaction(header.transaction)?;
    let mut cursor = Cursor::new(payload);
    let selected_revision = cursor.u16()?;
    require_zero(cursor.u16()?, "broker_welcome_reserved")?;
    let welcome = BrokerV1ServerWelcome {
        selected_revision,
        connection_epoch: cursor.u64()?,
        max_surfaces: cursor.u32()?,
        max_label_bytes: cursor.u16()?,
    };
    require_zero(cursor.u16()?, "broker_welcome_trailing_reserved")?;
    cursor.finish()?;
    Ok(welcome)
}

pub fn encode_broker_v1_request_frame(
    transaction: TransactionId,
    request: &BrokerV1Request,
) -> Result<Vec<u8>, IpcCodecError> {
    require_transaction(transaction)?;
    let mut payload = Vec::new();
    let kind = match request {
        BrokerV1Request::SurfaceAdmitted { .. } => 1,
        BrokerV1Request::CandidateReduced { .. } => 2,
        BrokerV1Request::AttentionChanged { .. } => 3,
        BrokerV1Request::SurfaceRemoved { .. } => 4,
        BrokerV1Request::SetDisclosure { .. } => 5,
    };
    push_u16(&mut payload, kind);
    push_u16(&mut payload, 0);
    push_u64(&mut payload, request.connection_epoch());
    match request {
        BrokerV1Request::SurfaceAdmitted {
            surface, profile, ..
        } => {
            encode_surface_id(*surface, &mut payload);
            push_u16(&mut payload, encode_namespace_profile(*profile));
        }
        BrokerV1Request::CandidateReduced { candidate, .. } => {
            encode_surface_id(candidate.surface, &mut payload);
            push_u16(&mut payload, encode_disclosure(candidate.disclosure));
            encode_optional_text(
                &mut payload,
                "broker_candidate_label",
                candidate.label.as_ref().map(|label| label.text.as_str()),
                MAX_CHROME_LABEL_LEN,
            )?;
            push_u8(
                &mut payload,
                u8::from(candidate.label.as_ref().is_some_and(|label| label.redacted)),
            );
            push_u64(&mut payload, candidate.generation);
        }
        BrokerV1Request::AttentionChanged {
            surface, attention, ..
        } => {
            encode_surface_id(*surface, &mut payload);
            push_u16(&mut payload, encode_attention(*attention));
        }
        BrokerV1Request::SurfaceRemoved { surface, .. } => {
            encode_surface_id(*surface, &mut payload);
        }
        BrokerV1Request::SetDisclosure {
            surface,
            disclosure,
            ..
        } => {
            encode_surface_id(*surface, &mut payload);
            push_u16(&mut payload, encode_disclosure(*disclosure));
        }
    }
    encode_frame(IpcMessageKind::BrokerV1Request, transaction, &payload)
}

pub fn decode_broker_v1_request_frame(
    frame: &[u8],
) -> Result<(TransactionId, BrokerV1Request), IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    require_kind(header.message_kind, IpcMessageKind::BrokerV1Request)?;
    require_transaction(header.transaction)?;
    let mut cursor = Cursor::new(payload);
    let kind = cursor.u16()?;
    require_zero(cursor.u16()?, "broker_request_reserved")?;
    let connection_epoch = cursor.u64()?;
    let request = match kind {
        1 => BrokerV1Request::SurfaceAdmitted {
            connection_epoch,
            surface: decode_surface_id(&mut cursor)?,
            profile: decode_namespace_profile(cursor.u16()?)?,
        },
        2 => {
            let surface = decode_surface_id(&mut cursor)?;
            let disclosure = decode_disclosure(cursor.u16()?)?;
            let label =
                decode_optional_text(&mut cursor, "broker_candidate_label", MAX_CHROME_LABEL_LEN)?;
            let redacted = decode_bool(&mut cursor, "broker_candidate_redacted")?;
            if label.is_none() && redacted {
                return Err(IpcCodecError::InvalidBool {
                    field: "broker_candidate_redacted_without_label",
                    value: 1,
                });
            }
            let generation = cursor.u64()?;
            BrokerV1Request::CandidateReduced {
                connection_epoch,
                candidate: ReducedMetadataCandidate {
                    surface,
                    label: label.map(|text| DisplayLabel { text, redacted }),
                    disclosure,
                    generation,
                },
            }
        }
        3 => BrokerV1Request::AttentionChanged {
            connection_epoch,
            surface: decode_surface_id(&mut cursor)?,
            attention: decode_attention(cursor.u16()?)?,
        },
        4 => BrokerV1Request::SurfaceRemoved {
            connection_epoch,
            surface: decode_surface_id(&mut cursor)?,
        },
        5 => BrokerV1Request::SetDisclosure {
            connection_epoch,
            surface: decode_surface_id(&mut cursor)?,
            disclosure: decode_disclosure(cursor.u16()?)?,
        },
        other => {
            return Err(IpcCodecError::InvalidEnum {
                field: "broker_request_kind",
                value: u32::from(other),
            });
        }
    };
    cursor.finish()?;
    Ok((header.transaction, request))
}

pub fn encode_broker_v1_response_frame(
    transaction: TransactionId,
    response: &BrokerV1Response,
) -> Result<Vec<u8>, IpcCodecError> {
    require_transaction(transaction)?;
    let mut payload = Vec::new();
    let kind = match response {
        BrokerV1Response::PublishRule { .. } => 1,
        BrokerV1Response::EmitDescriptor { .. } => 2,
        BrokerV1Response::RetireSurface { .. } => 3,
        BrokerV1Response::Rejected { .. } => 4,
        BrokerV1Response::NoChange { .. } => 5,
    };
    push_u16(&mut payload, kind);
    push_u16(&mut payload, 0);
    push_u64(&mut payload, response.connection_epoch());
    match response {
        BrokerV1Response::PublishRule { rule, .. } => encode_rule(rule, &mut payload),
        BrokerV1Response::EmitDescriptor {
            descriptor, action, ..
        } => {
            if action.target_generation != descriptor.generation {
                return Err(IpcCodecError::InvalidRecord(
                    "broker_toplevel_action_target",
                ));
            }
            encode_descriptor(descriptor, &mut payload)?;
            encode_action_grant(*action, &mut payload)?;
        }
        BrokerV1Response::RetireSurface { surface, .. } => {
            encode_surface_id(*surface, &mut payload);
        }
        BrokerV1Response::Rejected { rejection, .. } => {
            push_u16(&mut payload, encode_rejection(*rejection));
        }
        BrokerV1Response::NoChange { .. } => {}
    }
    encode_frame(IpcMessageKind::BrokerV1Response, transaction, &payload)
}

pub fn decode_broker_v1_response_frame(
    frame: &[u8],
) -> Result<(TransactionId, BrokerV1Response), IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    require_kind(header.message_kind, IpcMessageKind::BrokerV1Response)?;
    require_transaction(header.transaction)?;
    let mut cursor = Cursor::new(payload);
    let kind = cursor.u16()?;
    require_zero(cursor.u16()?, "broker_response_reserved")?;
    let connection_epoch = cursor.u64()?;
    let response = match kind {
        1 => BrokerV1Response::PublishRule {
            connection_epoch,
            rule: decode_rule(&mut cursor)?,
        },
        2 => {
            let descriptor = decode_descriptor(&mut cursor)?;
            let action = decode_action_grant(&mut cursor)?;
            if action.target_generation != descriptor.generation {
                return Err(IpcCodecError::InvalidRecord(
                    "broker_toplevel_action_target",
                ));
            }
            BrokerV1Response::EmitDescriptor {
                connection_epoch,
                descriptor,
                action,
            }
        }
        3 => BrokerV1Response::RetireSurface {
            connection_epoch,
            surface: decode_surface_id(&mut cursor)?,
        },
        4 => BrokerV1Response::Rejected {
            connection_epoch,
            rejection: decode_rejection(cursor.u16()?)?,
        },
        5 => BrokerV1Response::NoChange { connection_epoch },
        other => {
            return Err(IpcCodecError::InvalidEnum {
                field: "broker_response_kind",
                value: u32::from(other),
            });
        }
    };
    cursor.finish()?;
    Ok((header.transaction, response))
}

fn encode_rule(rule: &MetadataDisclosureRule, out: &mut Vec<u8>) {
    encode_surface_id(rule.surface, out);
    push_u16(out, encode_disclosure(rule.disclosure));
    push_u16(out, encode_trust(rule.trust_level));
    encode_optional_icon(rule.icon, out);
    push_u64(out, rule.generation);
}

fn decode_rule(cursor: &mut Cursor<'_>) -> Result<MetadataDisclosureRule, IpcCodecError> {
    Ok(MetadataDisclosureRule {
        surface: decode_surface_id(cursor)?,
        disclosure: decode_disclosure(cursor.u16()?)?,
        trust_level: decode_trust(cursor.u16()?)?,
        icon: decode_optional_icon(cursor)?,
        generation: cursor.u64()?,
    })
}

fn encode_descriptor(
    descriptor: &SanitizedChromeMetadata,
    out: &mut Vec<u8>,
) -> Result<(), IpcCodecError> {
    encode_surface_id(descriptor.surface, out);
    encode_optional_text(
        out,
        "broker_descriptor_label",
        descriptor.label.as_deref(),
        MAX_CHROME_LABEL_LEN,
    )?;
    push_u8(out, u8::from(descriptor.label_redacted));
    encode_optional_icon(descriptor.icon, out);
    push_u16(out, encode_trust(descriptor.trust_level));
    push_u16(out, encode_attention(descriptor.attention));
    push_u64(out, descriptor.generation);
    Ok(())
}

fn decode_descriptor(cursor: &mut Cursor<'_>) -> Result<SanitizedChromeMetadata, IpcCodecError> {
    let surface = decode_surface_id(cursor)?;
    let label = decode_optional_text(cursor, "broker_descriptor_label", MAX_CHROME_LABEL_LEN)?;
    let label_redacted = decode_bool(cursor, "broker_descriptor_redacted")?;
    if label.is_none() && label_redacted {
        return Err(IpcCodecError::InvalidBool {
            field: "broker_descriptor_redacted_without_label",
            value: 1,
        });
    }
    Ok(SanitizedChromeMetadata {
        surface,
        label,
        label_redacted,
        icon: decode_optional_icon(cursor)?,
        trust_level: decode_trust(cursor.u16()?)?,
        attention: decode_attention(cursor.u16()?)?,
        generation: cursor.u64()?,
    })
}

fn encode_action_grant(
    grant: BrokerToplevelActionGrant,
    out: &mut Vec<u8>,
) -> Result<(), IpcCodecError> {
    validate_action_grant(grant)?;
    push_u64(out, grant.token);
    push_u64(out, grant.revocation_epoch);
    push_u64(out, grant.target_generation);
    Ok(())
}

fn decode_action_grant(
    cursor: &mut Cursor<'_>,
) -> Result<BrokerToplevelActionGrant, IpcCodecError> {
    let grant = BrokerToplevelActionGrant {
        token: cursor.u64()?,
        revocation_epoch: cursor.u64()?,
        target_generation: cursor.u64()?,
    };
    validate_action_grant(grant)?;
    Ok(grant)
}

fn validate_action_grant(grant: BrokerToplevelActionGrant) -> Result<(), IpcCodecError> {
    if grant.token == 0 || grant.revocation_epoch == 0 || grant.target_generation == 0 {
        Err(IpcCodecError::InvalidRecord("broker_toplevel_action_grant"))
    } else {
        Ok(())
    }
}

fn encode_optional_icon(icon: Option<IconTokenId>, out: &mut Vec<u8>) {
    match icon {
        Some(icon) => {
            push_u8(out, 1);
            push_u64(out, icon.raw());
        }
        None => push_u8(out, 0),
    }
}

fn decode_optional_icon(cursor: &mut Cursor<'_>) -> Result<Option<IconTokenId>, IpcCodecError> {
    match cursor.u8()? {
        0 => Ok(None),
        1 => Ok(Some(IconTokenId::from_raw(cursor.u64()?))),
        value => Err(IpcCodecError::InvalidBool {
            field: "broker_icon_present",
            value,
        }),
    }
}

fn decode_bool(cursor: &mut Cursor<'_>, field: &'static str) -> Result<bool, IpcCodecError> {
    match cursor.u8()? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(IpcCodecError::InvalidBool { field, value }),
    }
}

fn encode_namespace_profile(profile: NamespaceProfile) -> u16 {
    match profile {
        NamespaceProfile::ClassicShared => 1,
        NamespaceProfile::Confined => 2,
    }
}

fn decode_namespace_profile(value: u16) -> Result<NamespaceProfile, IpcCodecError> {
    match value {
        1 => Ok(NamespaceProfile::ClassicShared),
        2 => Ok(NamespaceProfile::Confined),
        other => invalid_enum("broker_namespace_profile", other),
    }
}

fn encode_disclosure(disclosure: MetadataDisclosure) -> u16 {
    match disclosure {
        MetadataDisclosure::None => 1,
        MetadataDisclosure::ClassOnly => 2,
        MetadataDisclosure::Full => 3,
    }
}

fn decode_disclosure(value: u16) -> Result<MetadataDisclosure, IpcCodecError> {
    match value {
        1 => Ok(MetadataDisclosure::None),
        2 => Ok(MetadataDisclosure::ClassOnly),
        3 => Ok(MetadataDisclosure::Full),
        other => invalid_enum("broker_metadata_disclosure", other),
    }
}

fn encode_trust(trust: TrustLevel) -> u16 {
    match trust {
        TrustLevel::Unknown => 1,
        TrustLevel::Trusted => 2,
        TrustLevel::Untrusted => 3,
        TrustLevel::Isolated => 4,
    }
}

fn decode_trust(value: u16) -> Result<TrustLevel, IpcCodecError> {
    match value {
        1 => Ok(TrustLevel::Unknown),
        2 => Ok(TrustLevel::Trusted),
        3 => Ok(TrustLevel::Untrusted),
        4 => Ok(TrustLevel::Isolated),
        other => invalid_enum("broker_trust_level", other),
    }
}

fn encode_attention(attention: AttentionState) -> u16 {
    match attention {
        AttentionState::None => 1,
        AttentionState::Notice => 2,
        AttentionState::Critical => 3,
    }
}

fn decode_attention(value: u16) -> Result<AttentionState, IpcCodecError> {
    match value {
        1 => Ok(AttentionState::None),
        2 => Ok(AttentionState::Notice),
        3 => Ok(AttentionState::Critical),
        other => invalid_enum("broker_attention", other),
    }
}

fn encode_rejection(rejection: BrokerV1Rejection) -> u16 {
    match rejection {
        BrokerV1Rejection::UnknownSurface => 1,
        BrokerV1Rejection::StaleGeneration => 2,
        BrokerV1Rejection::CapacityExhausted => 3,
        BrokerV1Rejection::DisclosureExceeded => 4,
        BrokerV1Rejection::InvalidConnectionEpoch => 5,
    }
}

fn decode_rejection(value: u16) -> Result<BrokerV1Rejection, IpcCodecError> {
    match value {
        1 => Ok(BrokerV1Rejection::UnknownSurface),
        2 => Ok(BrokerV1Rejection::StaleGeneration),
        3 => Ok(BrokerV1Rejection::CapacityExhausted),
        4 => Ok(BrokerV1Rejection::DisclosureExceeded),
        5 => Ok(BrokerV1Rejection::InvalidConnectionEpoch),
        other => invalid_enum("broker_rejection", other),
    }
}

fn require_kind(actual: IpcMessageKind, expected: IpcMessageKind) -> Result<(), IpcCodecError> {
    if actual == expected {
        Ok(())
    } else {
        Err(IpcCodecError::InvalidEnum {
            field: "message_kind",
            value: actual as u32,
        })
    }
}

fn require_transaction(transaction: TransactionId) -> Result<(), IpcCodecError> {
    if transaction.is_valid() {
        Ok(())
    } else {
        Err(IpcCodecError::InvalidTransaction(transaction.raw()))
    }
}

fn require_handshake_transaction(transaction: TransactionId) -> Result<(), IpcCodecError> {
    if transaction.is_valid() {
        Err(IpcCodecError::InvalidTransaction(transaction.raw()))
    } else {
        Ok(())
    }
}

fn require_zero(value: u16, field: &'static str) -> Result<(), IpcCodecError> {
    if value == 0 {
        Ok(())
    } else {
        Err(IpcCodecError::InvalidEnum {
            field,
            value: u32::from(value),
        })
    }
}

fn invalid_enum<T>(field: &'static str, value: u16) -> Result<T, IpcCodecError> {
    Err(IpcCodecError::InvalidEnum {
        field,
        value: u32::from(value),
    })
}
