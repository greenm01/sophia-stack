use crate::{
    SessionApplicationId, SurfaceId, SurfacePlacement, SurfaceSizeRequest, TransactionId,
    WmActionActivation, WmActionId, WmBindingRegistration, WmCapabilities, WmChromeStyle,
    WmCommand, WmFocusRequest, WmHello, WmManageSurface, WmModifierMask, WmOutputWorkspace,
    WmPolicyAck, WmPolicyAckOutcome, WmPolicyUpdate, WmRelayoutWorkspace, WmRequestKind,
    WmRequestPacket, WmResponsePacket, WmRgb8, WmSessionAction, WmSessionDescriptor,
};

use super::cursor::{Cursor, push_i32, push_u16, push_u32, push_u64};
use super::frame::{decode_frame, encode_frame};
use super::primitives::{
    check_count, decode_count, decode_layout_node, decode_option_rect, decode_output_id,
    decode_rect, decode_size, decode_surface_id, decode_transform, decode_workspace_id,
    encode_layout_node, encode_option_rect, encode_output_id, encode_rect, encode_size,
    encode_surface_id, encode_transform, encode_workspace_id,
};
use super::types::{IpcCodecError, IpcMessageKind};

pub fn encode_wm_request_frame(request: &WmRequestPacket) -> Result<Vec<u8>, IpcCodecError> {
    let mut payload = Vec::new();
    encode_wm_request_payload(request, &mut payload)?;
    encode_frame(IpcMessageKind::WmRequest, request.transaction, &payload)
}

pub fn decode_wm_request_frame(frame: &[u8]) -> Result<WmRequestPacket, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    if header.message_kind != IpcMessageKind::WmRequest {
        return Err(IpcCodecError::InvalidEnum {
            field: "message_kind",
            value: header.message_kind as u32,
        });
    }
    let mut cursor = Cursor::new(payload);
    let packet = decode_wm_request_payload(header.transaction, &mut cursor)?;
    cursor.finish()?;
    Ok(packet)
}

pub fn encode_wm_response_frame(response: &WmResponsePacket) -> Result<Vec<u8>, IpcCodecError> {
    let mut payload = Vec::new();
    encode_wm_response_payload(response, &mut payload)?;
    encode_frame(IpcMessageKind::WmResponse, response.transaction, &payload)
}

pub fn decode_wm_response_frame(frame: &[u8]) -> Result<WmResponsePacket, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    if header.message_kind != IpcMessageKind::WmResponse {
        return Err(IpcCodecError::InvalidEnum {
            field: "message_kind",
            value: header.message_kind as u32,
        });
    }
    let mut cursor = Cursor::new(payload);
    let packet = decode_wm_response_payload(header.transaction, &mut cursor)?;
    cursor.finish()?;
    Ok(packet)
}
pub fn encode_wm_hello_frame(hello: &WmHello) -> Result<Vec<u8>, IpcCodecError> {
    check_count(hello.bindings.len())?;
    let mut payload = Vec::new();
    push_u16(&mut payload, hello.api_version);
    push_u64(&mut payload, hello.capabilities.bits);
    push_u64(&mut payload, hello.policy_generation);
    push_u32(&mut payload, hello.bindings.len() as u32);
    for binding in &hello.bindings {
        push_u64(&mut payload, binding.action.raw());
        push_u32(&mut payload, binding.keycode);
        push_u32(&mut payload, binding.modifiers.bits);
    }
    encode_chrome(hello.chrome, &mut payload);
    encode_frame(IpcMessageKind::WmHello, TransactionId::INVALID, &payload)
}

pub fn decode_wm_hello_frame(frame: &[u8]) -> Result<WmHello, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    expect_message_kind(header.message_kind, IpcMessageKind::WmHello)?;
    let mut cursor = Cursor::new(payload);
    let api_version = cursor.u16()?;
    let capabilities = WmCapabilities {
        bits: cursor.u64()?,
    };
    let policy_generation = cursor.u64()?;
    let count = decode_count(&mut cursor)?;
    let mut bindings = Vec::with_capacity(count);
    for _ in 0..count {
        bindings.push(WmBindingRegistration {
            action: WmActionId::from_raw(cursor.u64()?),
            keycode: cursor.u32()?,
            modifiers: WmModifierMask {
                bits: cursor.u32()?,
            },
        });
    }
    let chrome = decode_chrome(&mut cursor)?;
    cursor.finish()?;
    Ok(WmHello {
        api_version,
        capabilities,
        policy_generation,
        bindings,
        chrome,
    })
}

pub fn encode_wm_policy_update_frame(update: &WmPolicyUpdate) -> Result<Vec<u8>, IpcCodecError> {
    check_count(update.bindings.len())?;
    let mut payload = Vec::new();
    push_u16(&mut payload, update.api_version);
    push_u64(&mut payload, update.generation);
    push_u32(&mut payload, update.bindings.len() as u32);
    for binding in &update.bindings {
        push_u64(&mut payload, binding.action.raw());
        push_u32(&mut payload, binding.keycode);
        push_u32(&mut payload, binding.modifiers.bits);
    }
    encode_chrome(update.chrome, &mut payload);
    encode_frame(
        IpcMessageKind::WmPolicyUpdate,
        TransactionId::INVALID,
        &payload,
    )
}

pub fn decode_wm_policy_update_frame(frame: &[u8]) -> Result<WmPolicyUpdate, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    expect_message_kind(header.message_kind, IpcMessageKind::WmPolicyUpdate)?;
    let mut cursor = Cursor::new(payload);
    let api_version = cursor.u16()?;
    let generation = cursor.u64()?;
    let count = decode_count(&mut cursor)?;
    let mut bindings = Vec::with_capacity(count);
    for _ in 0..count {
        bindings.push(WmBindingRegistration {
            action: WmActionId::from_raw(cursor.u64()?),
            keycode: cursor.u32()?,
            modifiers: WmModifierMask {
                bits: cursor.u32()?,
            },
        });
    }
    let chrome = decode_chrome(&mut cursor)?;
    cursor.finish()?;
    Ok(WmPolicyUpdate {
        api_version,
        generation,
        bindings,
        chrome,
    })
}

pub fn encode_wm_policy_ack_frame(ack: WmPolicyAck) -> Result<Vec<u8>, IpcCodecError> {
    let mut payload = Vec::new();
    push_u64(&mut payload, ack.generation);
    push_u16(
        &mut payload,
        match ack.outcome {
            WmPolicyAckOutcome::Applied => 1,
            WmPolicyAckOutcome::RejectedStale => 2,
            WmPolicyAckOutcome::RejectedInvalid => 3,
        },
    );
    encode_frame(
        IpcMessageKind::WmPolicyAck,
        TransactionId::INVALID,
        &payload,
    )
}

pub fn decode_wm_policy_ack_frame(frame: &[u8]) -> Result<WmPolicyAck, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    expect_message_kind(header.message_kind, IpcMessageKind::WmPolicyAck)?;
    let mut cursor = Cursor::new(payload);
    let generation = cursor.u64()?;
    let outcome = match cursor.u16()? {
        1 => WmPolicyAckOutcome::Applied,
        2 => WmPolicyAckOutcome::RejectedStale,
        3 => WmPolicyAckOutcome::RejectedInvalid,
        other => {
            return Err(IpcCodecError::InvalidEnum {
                field: "wm_policy_ack",
                value: u32::from(other),
            });
        }
    };
    cursor.finish()?;
    Ok(WmPolicyAck {
        generation,
        outcome,
    })
}

fn encode_chrome(chrome: WmChromeStyle, payload: &mut Vec<u8>) {
    push_u16(payload, u16::from(chrome.enabled));
    push_u32(payload, chrome.thickness);
    payload.extend_from_slice(&[chrome.color.red, chrome.color.green, chrome.color.blue]);
}

fn decode_chrome(cursor: &mut Cursor<'_>) -> Result<WmChromeStyle, IpcCodecError> {
    Ok(WmChromeStyle {
        enabled: match cursor.u16()? {
            0 => false,
            1 => true,
            other => {
                return Err(IpcCodecError::InvalidBool {
                    field: "wm_chrome_enabled",
                    value: u8::try_from(other).unwrap_or(u8::MAX),
                });
            }
        },
        thickness: cursor.u32()?,
        color: WmRgb8 {
            red: cursor.u8()?,
            green: cursor.u8()?,
            blue: cursor.u8()?,
        },
    })
}

pub fn encode_wm_session_descriptor_frame(
    descriptor: &WmSessionDescriptor,
) -> Result<Vec<u8>, IpcCodecError> {
    check_count(descriptor.workspaces.len())?;
    check_count(descriptor.active_workspaces.len())?;
    check_count(descriptor.session_actions.len())?;
    let mut payload = Vec::new();
    push_u16(&mut payload, descriptor.api_version);
    push_u32(&mut payload, descriptor.workspaces.len() as u32);
    for workspace in &descriptor.workspaces {
        encode_workspace_id(*workspace, &mut payload);
    }
    push_u32(&mut payload, descriptor.active_workspaces.len() as u32);
    for active in &descriptor.active_workspaces {
        encode_output_id(active.output, &mut payload);
        encode_workspace_id(active.workspace, &mut payload);
    }
    push_u32(&mut payload, descriptor.session_actions.len() as u32);
    for action in &descriptor.session_actions {
        encode_session_action(*action, &mut payload);
    }
    encode_frame(
        IpcMessageKind::WmSessionDescriptor,
        TransactionId::INVALID,
        &payload,
    )
}

pub fn decode_wm_session_descriptor_frame(
    frame: &[u8],
) -> Result<WmSessionDescriptor, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    expect_message_kind(header.message_kind, IpcMessageKind::WmSessionDescriptor)?;
    let mut cursor = Cursor::new(payload);
    let api_version = cursor.u16()?;
    let workspace_count = decode_count(&mut cursor)?;
    let mut workspaces = Vec::with_capacity(workspace_count);
    for _ in 0..workspace_count {
        workspaces.push(decode_workspace_id(&mut cursor)?);
    }
    let active_count = decode_count(&mut cursor)?;
    let mut active_workspaces = Vec::with_capacity(active_count);
    for _ in 0..active_count {
        active_workspaces.push(WmOutputWorkspace {
            output: decode_output_id(&mut cursor)?,
            workspace: decode_workspace_id(&mut cursor)?,
        });
    }
    let action_count = decode_count(&mut cursor)?;
    let mut session_actions = Vec::with_capacity(action_count);
    for _ in 0..action_count {
        session_actions.push(decode_session_action(&mut cursor)?);
    }
    cursor.finish()?;
    Ok(WmSessionDescriptor {
        api_version,
        workspaces,
        active_workspaces,
        session_actions,
    })
}

fn encode_session_action(action: WmSessionAction, out: &mut Vec<u8>) {
    match action {
        WmSessionAction::LaunchApplication { application } => {
            push_u16(out, 1);
            push_u64(out, application.raw());
        }
        WmSessionAction::CloseFocused => push_u16(out, 2),
        WmSessionAction::Logout => push_u16(out, 3),
    }
}

fn decode_session_action(cursor: &mut Cursor<'_>) -> Result<WmSessionAction, IpcCodecError> {
    match cursor.u16()? {
        1 => {
            let application = SessionApplicationId::from_raw(cursor.u64()?);
            if !application.is_valid() {
                return Err(IpcCodecError::InvalidEnum {
                    field: "session_application",
                    value: 0,
                });
            }
            Ok(WmSessionAction::LaunchApplication { application })
        }
        2 => Ok(WmSessionAction::CloseFocused),
        3 => Ok(WmSessionAction::Logout),
        other => Err(IpcCodecError::InvalidEnum {
            field: "wm_session_action",
            value: u32::from(other),
        }),
    }
}

fn encode_option_surface(surface: Option<SurfaceId>, out: &mut Vec<u8>) {
    match surface {
        Some(surface) => {
            push_u16(out, 1);
            encode_surface_id(surface, out);
        }
        None => push_u16(out, 0),
    }
}

fn decode_option_surface(cursor: &mut Cursor<'_>) -> Result<Option<SurfaceId>, IpcCodecError> {
    match cursor.u16()? {
        0 => Ok(None),
        1 => Ok(Some(decode_surface_id(cursor)?)),
        other => Err(IpcCodecError::InvalidBool {
            field: "optional_surface",
            value: u8::try_from(other).unwrap_or(u8::MAX),
        }),
    }
}

fn expect_message_kind(
    actual: IpcMessageKind,
    expected: IpcMessageKind,
) -> Result<(), IpcCodecError> {
    if actual == expected {
        Ok(())
    } else {
        Err(IpcCodecError::InvalidEnum {
            field: "message_kind",
            value: actual as u32,
        })
    }
}

fn encode_wm_request_payload(
    request: &WmRequestPacket,
    out: &mut Vec<u8>,
) -> Result<(), IpcCodecError> {
    match &request.kind {
        WmRequestKind::ManageSurface(manage) => {
            push_u16(out, 1);
            encode_output_id(manage.output, out);
            encode_workspace_id(manage.workspace, out);
            encode_rect(manage.bounds, out);
            encode_layout_node(&manage.node, out);
        }
        WmRequestKind::RelayoutWorkspace(relayout) => {
            check_count(relayout.nodes.len())?;
            push_u16(out, 2);
            encode_output_id(relayout.output, out);
            encode_workspace_id(relayout.workspace, out);
            encode_rect(relayout.bounds, out);
            push_u32(out, relayout.nodes.len() as u32);
            for node in &relayout.nodes {
                encode_layout_node(node, out);
            }
        }
        WmRequestKind::SurfaceRemoved { surface, workspace } => {
            push_u16(out, 3);
            encode_surface_id(*surface, out);
            encode_workspace_id(*workspace, out);
        }
        WmRequestKind::ActionActivated(activation) => {
            check_count(activation.nodes.len())?;
            push_u16(out, 4);
            push_u64(out, activation.action.raw());
            encode_output_id(activation.output, out);
            encode_workspace_id(activation.workspace, out);
            encode_option_surface(activation.focused_surface, out);
            push_u32(out, activation.nodes.len() as u32);
            for node in &activation.nodes {
                encode_layout_node(node, out);
            }
        }
        WmRequestKind::FocusRequested(focus) => {
            push_u16(out, 5);
            encode_surface_id(focus.surface, out);
            encode_output_id(focus.output, out);
            encode_workspace_id(focus.workspace, out);
        }
    }
    Ok(())
}

fn decode_wm_request_payload(
    transaction: TransactionId,
    cursor: &mut Cursor<'_>,
) -> Result<WmRequestPacket, IpcCodecError> {
    let kind = match cursor.u16()? {
        1 => WmRequestKind::ManageSurface(WmManageSurface {
            output: decode_output_id(cursor)?,
            workspace: decode_workspace_id(cursor)?,
            bounds: decode_rect(cursor)?,
            node: decode_layout_node(cursor)?,
        }),
        2 => {
            let output = decode_output_id(cursor)?;
            let workspace = decode_workspace_id(cursor)?;
            let bounds = decode_rect(cursor)?;
            let count = decode_count(cursor)?;
            let mut nodes = Vec::with_capacity(count);
            for _ in 0..count {
                nodes.push(decode_layout_node(cursor)?);
            }
            WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                output,
                workspace,
                bounds,
                nodes,
            })
        }
        3 => WmRequestKind::SurfaceRemoved {
            surface: decode_surface_id(cursor)?,
            workspace: decode_workspace_id(cursor)?,
        },
        4 => {
            let action = WmActionId::from_raw(cursor.u64()?);
            let output = decode_output_id(cursor)?;
            let workspace = decode_workspace_id(cursor)?;
            let focused_surface = decode_option_surface(cursor)?;
            let count = decode_count(cursor)?;
            let mut nodes = Vec::with_capacity(count);
            for _ in 0..count {
                nodes.push(decode_layout_node(cursor)?);
            }
            WmRequestKind::ActionActivated(WmActionActivation {
                action,
                output,
                workspace,
                focused_surface,
                nodes,
            })
        }
        5 => WmRequestKind::FocusRequested(WmFocusRequest {
            surface: decode_surface_id(cursor)?,
            output: decode_output_id(cursor)?,
            workspace: decode_workspace_id(cursor)?,
        }),
        other => {
            return Err(IpcCodecError::InvalidEnum {
                field: "wm_request_kind",
                value: u32::from(other),
            });
        }
    };

    Ok(WmRequestPacket { transaction, kind })
}

fn encode_wm_response_payload(
    response: &WmResponsePacket,
    out: &mut Vec<u8>,
) -> Result<(), IpcCodecError> {
    check_count(response.commands.len())?;
    push_u32(out, response.timeout_msec);
    push_u32(out, response.commands.len() as u32);
    for command in &response.commands {
        match command {
            WmCommand::ConfigureSurface(request) => {
                push_u16(out, 1);
                encode_surface_id(request.surface, out);
                encode_size(request.size, out);
            }
            WmCommand::FocusSurface(surface) => {
                push_u16(out, 2);
                encode_surface_id(*surface, out);
            }
            WmCommand::AssignWorkspace { surface, workspace } => {
                push_u16(out, 3);
                encode_surface_id(*surface, out);
                encode_workspace_id(*workspace, out);
            }
            WmCommand::RenderSurface(placement) => {
                push_u16(out, 4);
                encode_surface_id(placement.surface, out);
                encode_rect(placement.geometry, out);
                push_i32(out, placement.z_index);
                encode_option_rect(placement.crop, out);
                encode_transform(placement.transform, out);
            }
            WmCommand::ActivateWorkspace { output, workspace } => {
                push_u16(out, 5);
                encode_output_id(*output, out);
                encode_workspace_id(*workspace, out);
            }
            WmCommand::RequestSessionAction { action, target } => {
                push_u16(out, 6);
                encode_session_action(*action, out);
                encode_option_surface(*target, out);
            }
        }
    }
    Ok(())
}

fn decode_wm_response_payload(
    transaction: TransactionId,
    cursor: &mut Cursor<'_>,
) -> Result<WmResponsePacket, IpcCodecError> {
    let timeout_msec = cursor.u32()?;
    let count = decode_count(cursor)?;
    let mut commands = Vec::with_capacity(count);
    for _ in 0..count {
        let command = match cursor.u16()? {
            1 => WmCommand::ConfigureSurface(SurfaceSizeRequest {
                surface: decode_surface_id(cursor)?,
                size: decode_size(cursor)?,
            }),
            2 => WmCommand::FocusSurface(decode_surface_id(cursor)?),
            3 => WmCommand::AssignWorkspace {
                surface: decode_surface_id(cursor)?,
                workspace: decode_workspace_id(cursor)?,
            },
            4 => WmCommand::RenderSurface(SurfacePlacement {
                surface: decode_surface_id(cursor)?,
                geometry: decode_rect(cursor)?,
                z_index: cursor.i32()?,
                crop: decode_option_rect(cursor)?,
                transform: decode_transform(cursor)?,
            }),
            5 => WmCommand::ActivateWorkspace {
                output: decode_output_id(cursor)?,
                workspace: decode_workspace_id(cursor)?,
            },
            6 => WmCommand::RequestSessionAction {
                action: decode_session_action(cursor)?,
                target: decode_option_surface(cursor)?,
            },
            other => {
                return Err(IpcCodecError::InvalidEnum {
                    field: "wm_command",
                    value: u32::from(other),
                });
            }
        };
        commands.push(command);
    }

    Ok(WmResponsePacket {
        transaction,
        commands,
        timeout_msec,
    })
}
