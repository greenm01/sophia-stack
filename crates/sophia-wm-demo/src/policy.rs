use sophia_protocol::{
    LayoutNodeSnapshot, LayoutTransaction, Rect, Size, SurfacePlacement, SurfaceSizeRequest,
    TransactionId, Transform, WmCommand, WmRequestKind, WmRequestPacket, WmResponsePacket,
    WmSessionAction, WorkspaceId,
};

pub fn empty_transaction(transaction: TransactionId) -> LayoutTransaction {
    LayoutTransaction {
        transaction,
        requested_sizes: Vec::new(),
        focus: None,
        render_positions: Vec::new(),
        timeout_msec: 300,
    }
}

pub fn tile_workspace(
    transaction: TransactionId,
    workspace: WorkspaceId,
    bounds: Rect,
    nodes: &[LayoutNodeSnapshot],
) -> LayoutTransaction {
    tile_workspace_with_timeout(transaction, workspace, bounds, nodes, 300)
}

fn tile_workspace_with_timeout(
    transaction: TransactionId,
    workspace: WorkspaceId,
    bounds: Rect,
    nodes: &[LayoutNodeSnapshot],
    timeout_msec: u32,
) -> LayoutTransaction {
    let visible_nodes = nodes
        .iter()
        .filter(|node| node.workspace == workspace && node.state.visible)
        .collect::<Vec<_>>();

    if visible_nodes.is_empty() || bounds.is_empty() {
        let mut transaction = empty_transaction(transaction);
        transaction.timeout_msec = timeout_msec;
        return transaction;
    }

    let width = bounds.width / i32::try_from(visible_nodes.len()).expect("visible node overflow");
    let mut render_positions = Vec::with_capacity(visible_nodes.len());
    let mut requested_sizes = Vec::with_capacity(visible_nodes.len());
    let mut focus = None;

    for (index, node) in visible_nodes.iter().enumerate() {
        let index = i32::try_from(index).expect("visible node index overflow");
        let is_last =
            usize::try_from(index + 1).expect("visible node index overflow") == visible_nodes.len();
        let x = bounds.x + width * index;
        let tile_width = if is_last {
            bounds.x + bounds.width - x
        } else {
            width
        };
        let geometry = Rect {
            x,
            y: bounds.y,
            width: tile_width.max(1),
            height: bounds.height,
        };
        let requested = clamp_size(
            Size {
                width: geometry.width,
                height: geometry.height,
            },
            node.constraints.min_size,
            node.constraints.max_size,
        );

        if focus.is_none() && node.capabilities.focusable {
            focus = Some(node.surface);
        }

        requested_sizes.push(SurfaceSizeRequest {
            surface: node.surface,
            size: requested,
        });
        render_positions.push(SurfacePlacement {
            surface: node.surface,
            geometry,
            z_index: index,
            crop: None,
            transform: Transform::IDENTITY,
        });
    }

    LayoutTransaction {
        transaction,
        requested_sizes,
        focus,
        render_positions,
        timeout_msec,
    }
}

pub fn place_workspace_at_natural_size(
    transaction: TransactionId,
    workspace: WorkspaceId,
    bounds: Rect,
    nodes: &[LayoutNodeSnapshot],
    timeout_msec: u32,
) -> LayoutTransaction {
    let visible_nodes = nodes
        .iter()
        .filter(|node| node.workspace == workspace && node.state.visible)
        .collect::<Vec<_>>();
    if visible_nodes.is_empty() || bounds.is_empty() {
        let mut transaction = empty_transaction(transaction);
        transaction.timeout_msec = timeout_msec;
        return transaction;
    }

    let mut render_positions = Vec::with_capacity(visible_nodes.len());
    let mut focus = None;
    for (index, node) in visible_nodes.iter().enumerate() {
        let natural = clamp_size(
            Size {
                width: node.geometry.width,
                height: node.geometry.height,
            },
            node.constraints.min_size,
            node.constraints.max_size,
        );
        let size = Size {
            width: natural.width.min(bounds.width).max(1),
            height: natural.height.min(bounds.height).max(1),
        };
        let geometry = Rect {
            x: bounds.x + bounds.width.saturating_sub(size.width) / 2,
            y: bounds.y + bounds.height.saturating_sub(size.height) / 2,
            width: size.width,
            height: size.height,
        };
        if focus.is_none() && node.capabilities.focusable {
            focus = Some(node.surface);
        }
        render_positions.push(SurfacePlacement {
            surface: node.surface,
            geometry,
            z_index: i32::try_from(index).unwrap_or(i32::MAX),
            crop: None,
            transform: Transform::IDENTITY,
        });
    }

    LayoutTransaction {
        transaction,
        requested_sizes: Vec::new(),
        focus,
        render_positions,
        timeout_msec,
    }
}

pub fn handle_wm_request(request: WmRequestPacket) -> WmResponsePacket {
    match request.kind {
        WmRequestKind::ManageSurface(manage) => {
            let transaction = tile_workspace(
                request.transaction,
                manage.workspace,
                manage.bounds,
                &[manage.node],
            );
            response_from_layout_transaction(transaction, Some(manage.workspace))
        }
        WmRequestKind::RelayoutWorkspace(relayout) => {
            let transaction = tile_workspace(
                request.transaction,
                relayout.workspace,
                relayout.bounds,
                &relayout.nodes,
            );
            response_from_layout_transaction(transaction, None)
        }
        WmRequestKind::SurfaceRemoved { .. } => WmResponsePacket {
            transaction: request.transaction,
            commands: Vec::new(),
            timeout_msec: 300,
        },
        WmRequestKind::ActionActivated(activation) => {
            let commands = match activation.action.raw() {
                1 => {
                    let focus = activation
                        .focused_surface
                        .and_then(|focused| {
                            activation
                                .nodes
                                .iter()
                                .position(|node| node.surface == focused)
                        })
                        .map_or(0, |index| (index + 1) % activation.nodes.len().max(1));
                    activation
                        .nodes
                        .get(focus)
                        .filter(|node| node.capabilities.focusable)
                        .map(|node| vec![WmCommand::FocusSurface(node.surface)])
                        .unwrap_or_default()
                }
                2 => vec![WmCommand::ActivateWorkspace {
                    output: activation.output,
                    workspace: WorkspaceId::from_raw(2),
                }],
                3 => vec![WmCommand::RequestSessionAction {
                    action: WmSessionAction::LaunchApplication {
                        application: sophia_protocol::SessionApplicationId::from_raw(1),
                    },
                    target: None,
                }],
                _ => Vec::new(),
            };
            WmResponsePacket {
                transaction: request.transaction,
                commands,
                timeout_msec: 300,
            }
        }
        WmRequestKind::FocusRequested(focus) => WmResponsePacket {
            transaction: request.transaction,
            commands: vec![WmCommand::FocusSurface(focus.surface)],
            timeout_msec: 300,
        },
    }
}

pub fn handle_wm_request_with_config(
    request: WmRequestPacket,
    config: &sophia_config::WmConfigSnapshot,
) -> WmResponsePacket {
    match request.kind {
        WmRequestKind::ManageSurface(manage) => {
            let transaction = match config.layout {
                sophia_config::WmLayoutKind::Columns => tile_workspace_with_timeout(
                    request.transaction,
                    manage.workspace,
                    manage.bounds,
                    &[manage.node],
                    config.timeout_msec,
                ),
                sophia_config::WmLayoutKind::Natural => place_workspace_at_natural_size(
                    request.transaction,
                    manage.workspace,
                    manage.bounds,
                    &[manage.node],
                    config.timeout_msec,
                ),
            };
            response_from_layout_transaction(transaction, Some(manage.workspace))
        }
        WmRequestKind::RelayoutWorkspace(relayout) => {
            let transaction = match config.layout {
                sophia_config::WmLayoutKind::Columns => tile_workspace_with_timeout(
                    request.transaction,
                    relayout.workspace,
                    relayout.bounds,
                    &relayout.nodes,
                    config.timeout_msec,
                ),
                sophia_config::WmLayoutKind::Natural => place_workspace_at_natural_size(
                    request.transaction,
                    relayout.workspace,
                    relayout.bounds,
                    &relayout.nodes,
                    config.timeout_msec,
                ),
            };
            response_from_layout_transaction(transaction, None)
        }
        WmRequestKind::SurfaceRemoved { .. } => WmResponsePacket {
            transaction: request.transaction,
            commands: Vec::new(),
            timeout_msec: config.timeout_msec,
        },
        WmRequestKind::ActionActivated(activation) => {
            let commands = config
                .actions
                .iter()
                .find(|action| action.id == activation.action.raw())
                .map(|action| commands_for_behavior(action.behavior, &activation))
                .unwrap_or_default();
            WmResponsePacket {
                transaction: request.transaction,
                commands,
                timeout_msec: config.timeout_msec,
            }
        }
        WmRequestKind::FocusRequested(focus) => WmResponsePacket {
            transaction: request.transaction,
            commands: vec![WmCommand::FocusSurface(focus.surface)],
            timeout_msec: config.timeout_msec,
        },
    }
}

fn commands_for_behavior(
    behavior: sophia_config::WmActionBehavior,
    activation: &sophia_protocol::WmActionActivation,
) -> Vec<WmCommand> {
    match behavior {
        sophia_config::WmActionBehavior::FocusNext => focus_relative_command(activation, 1),
        sophia_config::WmActionBehavior::FocusPrevious => {
            focus_relative_command(activation, activation.nodes.len().saturating_sub(1))
        }
        sophia_config::WmActionBehavior::NextLayout => Vec::new(),
        sophia_config::WmActionBehavior::ActivateWorkspace { workspace } => {
            vec![WmCommand::ActivateWorkspace {
                output: activation.output,
                workspace: WorkspaceId::from_raw(workspace),
            }]
        }
        sophia_config::WmActionBehavior::LaunchApplication { application } => {
            vec![WmCommand::RequestSessionAction {
                action: WmSessionAction::LaunchApplication {
                    application: sophia_protocol::SessionApplicationId::from_raw(application),
                },
                target: None,
            }]
        }
        sophia_config::WmActionBehavior::CloseFocused => {
            vec![WmCommand::RequestSessionAction {
                action: WmSessionAction::CloseFocused,
                target: activation.focused_surface,
            }]
        }
        sophia_config::WmActionBehavior::Logout => {
            vec![WmCommand::RequestSessionAction {
                action: WmSessionAction::Logout,
                target: None,
            }]
        }
    }
}

fn focus_relative_command(
    activation: &sophia_protocol::WmActionActivation,
    offset: usize,
) -> Vec<WmCommand> {
    let focus = activation
        .focused_surface
        .and_then(|focused| {
            activation
                .nodes
                .iter()
                .position(|node| node.surface == focused)
        })
        .map_or(0, |index| (index + offset) % activation.nodes.len().max(1));
    activation
        .nodes
        .get(focus)
        .filter(|node| node.capabilities.focusable)
        .map(|node| vec![WmCommand::FocusSurface(node.surface)])
        .unwrap_or_default()
}

pub fn response_from_layout_transaction(
    transaction: LayoutTransaction,
    assigned_workspace: Option<WorkspaceId>,
) -> WmResponsePacket {
    let mut commands = Vec::new();

    if let Some(workspace) = assigned_workspace {
        for placement in &transaction.render_positions {
            commands.push(WmCommand::AssignWorkspace {
                surface: placement.surface,
                workspace,
            });
        }
    }

    commands.extend(
        transaction
            .requested_sizes
            .iter()
            .copied()
            .map(WmCommand::ConfigureSurface),
    );

    if let Some(focus) = transaction.focus {
        commands.push(WmCommand::FocusSurface(focus));
    }

    commands.extend(
        transaction
            .render_positions
            .iter()
            .copied()
            .map(WmCommand::RenderSurface),
    );

    WmResponsePacket {
        transaction: transaction.transaction,
        commands,
        timeout_msec: transaction.timeout_msec,
    }
}
fn clamp_size(size: Size, min_size: Option<Size>, max_size: Option<Size>) -> Size {
    let mut width = size.width;
    let mut height = size.height;

    if let Some(min_size) = min_size {
        width = width.max(min_size.width);
        height = height.max(min_size.height);
    }

    if let Some(max_size) = max_size {
        width = width.min(max_size.width);
        height = height.min(max_size.height);
    }

    Size { width, height }
}
