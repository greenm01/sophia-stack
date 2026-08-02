use sophia_protocol::{
    OutputId, TransactionId, WmActionActivation, WmActionId, WmFocusRequest, WmManageSurface,
    WmPointerGestureCompleted, WmPointerGestureMode, WmPointerPosition, WmRelayoutWorkspace,
    WmRequestKind, WmRequestPacket, WorkspaceId,
};

use super::{
    codec::{
        arg_values, encode_node, encode_rect, parse_node, parse_surface_token, process_usage,
        required_node, required_rect, required_surface, required_u64,
    },
    error::WmProcessError,
};

pub fn request_to_process_args(request: &WmRequestPacket) -> Vec<String> {
    match &request.kind {
        WmRequestKind::ManageSurface(manage) => {
            let mut args = vec![
                "manage".to_owned(),
                format!("--transaction={}", request.transaction.raw()),
                format!("--output={}", manage.output.raw()),
                format!("--workspace={}", manage.workspace.raw()),
                format!("--bounds={}", encode_rect(manage.bounds)),
                format!("--node={}", encode_node(&manage.node)),
            ];
            args.shrink_to_fit();
            args
        }
        WmRequestKind::RelayoutWorkspace(relayout) => {
            let mut args = vec![
                "relayout".to_owned(),
                format!("--transaction={}", request.transaction.raw()),
                format!("--output={}", relayout.output.raw()),
                format!("--workspace={}", relayout.workspace.raw()),
                format!("--bounds={}", encode_rect(relayout.bounds)),
            ];
            args.extend(
                relayout
                    .nodes
                    .iter()
                    .map(|node| format!("--node={}", encode_node(node))),
            );
            args
        }
        WmRequestKind::SurfaceRemoved { surface, workspace } => vec![
            "remove".to_owned(),
            format!("--transaction={}", request.transaction.raw()),
            format!("--workspace={}", workspace.raw()),
            format!("--surface={}:{}", surface.index(), surface.generation()),
        ],
        WmRequestKind::ActionActivated(activation) => {
            let mut args = vec![
                "action".to_owned(),
                format!("--transaction={}", request.transaction.raw()),
                format!("--action={}", activation.action.raw()),
                format!("--output={}", activation.output.raw()),
                format!("--workspace={}", activation.workspace.raw()),
            ];
            if let Some(surface) = activation.focused_surface {
                args.push(format!(
                    "--focus={}:{}",
                    surface.index(),
                    surface.generation()
                ));
            }
            args.extend(
                activation
                    .nodes
                    .iter()
                    .map(|node| format!("--node={}", encode_node(node))),
            );
            args
        }
        WmRequestKind::FocusRequested(focus) => vec![
            "focus".to_owned(),
            format!("--transaction={}", request.transaction.raw()),
            format!("--output={}", focus.output.raw()),
            format!("--workspace={}", focus.workspace.raw()),
            format!(
                "--surface={}:{}",
                focus.surface.index(),
                focus.surface.generation()
            ),
        ],
        WmRequestKind::PointerGestureCompleted(gesture) => vec![
            "pointer".to_owned(),
            format!("--transaction={}", request.transaction.raw()),
            format!("--output={}", gesture.output.raw()),
            format!("--workspace={}", gesture.workspace.raw()),
            format!(
                "--surface={}:{}",
                gesture.surface.index(),
                gesture.surface.generation()
            ),
            format!(
                "--mode={}",
                match gesture.mode {
                    WmPointerGestureMode::Move => "move",
                    WmPointerGestureMode::Resize => "resize",
                }
            ),
            format!("--start={},{}", gesture.start.x, gesture.start.y),
            format!("--end={},{}", gesture.end.x, gesture.end.y),
        ],
    }
}

pub fn parse_process_request(args: &[String]) -> Result<WmRequestPacket, WmProcessError> {
    let Some(kind) = args.first().map(String::as_str) else {
        return Err(WmProcessError::new(process_usage()));
    };
    let transaction = TransactionId::from_raw(required_u64(args, "--transaction")?);
    let workspace = WorkspaceId::from_raw(required_u64(args, "--workspace")?);

    match kind {
        "manage" => {
            let output = OutputId::from_raw(required_u64(args, "--output")?);
            let bounds = required_rect(args, "--bounds")?;
            let node = required_node(args, "--node", workspace)?;
            Ok(WmRequestPacket {
                transaction,
                kind: WmRequestKind::ManageSurface(WmManageSurface {
                    node,
                    output,
                    workspace,
                    bounds,
                }),
            })
        }
        "relayout" => {
            let output = OutputId::from_raw(required_u64(args, "--output")?);
            let bounds = required_rect(args, "--bounds")?;
            let nodes = arg_values(args, "--node")
                .into_iter()
                .map(|value| parse_node(value, workspace))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(WmRequestPacket {
                transaction,
                kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                    output,
                    workspace,
                    bounds,
                    nodes,
                }),
            })
        }
        "remove" => {
            let surface = required_surface(args, "--surface")?;
            Ok(WmRequestPacket {
                transaction,
                kind: WmRequestKind::SurfaceRemoved { surface, workspace },
            })
        }
        "action" => {
            let output = OutputId::from_raw(required_u64(args, "--output")?);
            let action = WmActionId::from_raw(required_u64(args, "--action")?);
            let focused_surface = arg_values(args, "--focus")
                .first()
                .map(|value| parse_surface_token(value))
                .transpose()?;
            let nodes = arg_values(args, "--node")
                .into_iter()
                .map(|value| parse_node(value, workspace))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(WmRequestPacket {
                transaction,
                kind: WmRequestKind::ActionActivated(WmActionActivation {
                    action,
                    output,
                    workspace,
                    focused_surface,
                    nodes,
                }),
            })
        }
        "focus" => {
            let output = OutputId::from_raw(required_u64(args, "--output")?);
            let surface = required_surface(args, "--surface")?;
            Ok(WmRequestPacket {
                transaction,
                kind: WmRequestKind::FocusRequested(WmFocusRequest {
                    surface,
                    output,
                    workspace,
                }),
            })
        }
        "pointer" => {
            let output = OutputId::from_raw(required_u64(args, "--output")?);
            let surface = required_surface(args, "--surface")?;
            let parse_position = |key| -> Result<WmPointerPosition, WmProcessError> {
                let value = super::codec::required_value(args, key)?;
                let fields = value.split(',').collect::<Vec<_>>();
                if fields.len() != 2 {
                    return Err(WmProcessError::new(format!("invalid {key}: {value}")));
                }
                Ok(WmPointerPosition {
                    x: fields[0]
                        .parse()
                        .map_err(|_| WmProcessError::new(format!("invalid {key}: {value}")))?,
                    y: fields[1]
                        .parse()
                        .map_err(|_| WmProcessError::new(format!("invalid {key}: {value}")))?,
                })
            };
            let mode = match super::codec::required_value(args, "--mode")? {
                "move" => WmPointerGestureMode::Move,
                "resize" => WmPointerGestureMode::Resize,
                value => return Err(WmProcessError::new(format!("invalid --mode: {value}"))),
            };
            Ok(WmRequestPacket {
                transaction,
                kind: WmRequestKind::PointerGestureCompleted(WmPointerGestureCompleted {
                    surface,
                    output,
                    workspace,
                    mode,
                    start: parse_position("--start")?,
                    end: parse_position("--end")?,
                }),
            })
        }
        _ => Err(WmProcessError::new(process_usage())),
    }
}
