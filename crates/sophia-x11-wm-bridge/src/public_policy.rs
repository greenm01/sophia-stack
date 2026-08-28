use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use sophia_protocol::{
    LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, OutputId, PolicyActionRegistration,
    PolicyProjectionOutcome, PolicyProjectionProposal, PolicyProjectionRequest, PolicyRequestCause,
    PolicySceneSnapshot, PolicySurfaceKind, Rect, SessionApplicationId, SurfaceId,
    SurfacePlacementPreference, TransactionId, WM_DEFAULT_WORKSPACES, WmActionActivation,
    WmActionId, WmChromePolicy, WmCommand, WmFocusRequest, WmRelayoutWorkspace, WmRequestKind,
    WmRequestPacket, WmResponsePacket, WmSessionAction, WorkspaceId,
};
use sophia_wm_demo::PolicyV1Client;

use crate::{
    LegacyWmLaunchSpec, LegacyWmProfile, LegacyX11WmBridgeRuntime, WmWorkspaceState,
    XMONAD_ACTION_APPLICATION_1, XMONAD_ACTION_CLOSE, XMONAD_ACTION_DECREASE_MASTER_COUNT,
    XMONAD_ACTION_EXPAND, XMONAD_ACTION_FOCUS_MASTER, XMONAD_ACTION_FOCUS_NEXT,
    XMONAD_ACTION_FOCUS_PREVIOUS, XMONAD_ACTION_INCREASE_MASTER_COUNT,
    XMONAD_ACTION_MOVE_WORKSPACE_BASE, XMONAD_ACTION_NEXT_LAYOUT, XMONAD_ACTION_RESET_LAYOUT,
    XMONAD_ACTION_SHRINK, XMONAD_ACTION_SINK, XMONAD_ACTION_SWAP_DOWN, XMONAD_ACTION_SWAP_MASTER,
    XMONAD_ACTION_SWAP_UP, XMONAD_ACTION_TOGGLE_FLOATING, XMONAD_ACTION_VIEW_WORKSPACE_BASE,
    adapt_legacy_policy_plan,
};

const PUBLIC_POLICY_TIMEOUT: Duration = Duration::from_secs(8);
const POLICY_TIMEOUT_MSEC: u32 = 2_000;

/// The exact public action catalog consumed by the checked-in xmonad desktop
/// profile. Session operations remain Engine-owned and are named only by their
/// profile-local slot.
pub fn xmonad_public_policy_actions() -> Vec<PolicyActionRegistration> {
    let mut actions = vec![
        action(XMONAD_ACTION_FOCUS_NEXT, "focus-next"),
        action(XMONAD_ACTION_FOCUS_PREVIOUS, "focus-previous"),
        action(XMONAD_ACTION_NEXT_LAYOUT, "next-layout"),
        action(XMONAD_ACTION_TOGGLE_FLOATING, "toggle-floating"),
        action(XMONAD_ACTION_RESET_LAYOUT, "reset-layout"),
        action(XMONAD_ACTION_FOCUS_MASTER, "focus-master"),
        action(XMONAD_ACTION_SWAP_MASTER, "swap-master"),
        action(XMONAD_ACTION_SWAP_DOWN, "swap-down"),
        action(XMONAD_ACTION_SWAP_UP, "swap-up"),
        action(XMONAD_ACTION_SHRINK, "shrink"),
        action(XMONAD_ACTION_EXPAND, "expand"),
        action(XMONAD_ACTION_SINK, "sink"),
        action(XMONAD_ACTION_INCREASE_MASTER_COUNT, "increase-master-count"),
        action(XMONAD_ACTION_DECREASE_MASTER_COUNT, "decrease-master-count"),
    ];
    for slot in 1..=9_u64 {
        actions.push(action(
            XMONAD_ACTION_VIEW_WORKSPACE_BASE + slot,
            format!("view-workspace {slot}"),
        ));
        actions.push(action(
            XMONAD_ACTION_MOVE_WORKSPACE_BASE + slot,
            format!("move-to-workspace {slot}"),
        ));
    }
    actions.push(session_action(
        XMONAD_ACTION_APPLICATION_1,
        "spawn-terminal",
        1,
    ));
    actions.push(session_action(XMONAD_ACTION_CLOSE, "close-window", 3));
    actions
}

fn action(id: u64, name: impl Into<String>) -> PolicyActionRegistration {
    PolicyActionRegistration {
        action: WmActionId::from_raw(id),
        name: name.into(),
        session_operation_slot: None,
    }
}

fn session_action(id: u64, name: &str, slot: u16) -> PolicyActionRegistration {
    PolicyActionRegistration {
        action: WmActionId::from_raw(id),
        name: name.to_owned(),
        session_operation_slot: Some(slot),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XmonadProjectionDecision {
    Commit,
    RebuildPrivateAdapter,
    Fatal,
}

/// A recoverable rejection invalidates xmonad's speculative private model but
/// not the public policy connection. Rebuilding the private adapter from the
/// fresh committed snapshot preserves the stateful-peer discard rule without
/// spending the session supervisor's crash budget on an ordinary scene race.
pub const fn xmonad_projection_decision(
    outcome: PolicyProjectionOutcome,
) -> XmonadProjectionDecision {
    match outcome {
        PolicyProjectionOutcome::Committed => XmonadProjectionDecision::Commit,
        PolicyProjectionOutcome::RejectedStale | PolicyProjectionOutcome::TimedOut => {
            XmonadProjectionDecision::RebuildPrivateAdapter
        }
        PolicyProjectionOutcome::RejectedInvalid | PolicyProjectionOutcome::Disconnected => {
            XmonadProjectionDecision::Fatal
        }
    }
}

/// Runs xmonad as a blind compatibility policy peer on the public revision-3
/// socket. Recoverable noncommitted proposals rebuild the private xmonad model;
/// invalid or disconnected outcomes end the supervised policy process.
pub fn run_public_xmonad_policy(
    socket: impl AsRef<Path>,
    launch: LegacyWmLaunchSpec,
) -> Result<(), Box<dyn std::error::Error>> {
    run_public_xmonad_policy_cycles(socket, launch, None)
}

/// Bounded form used only by the compatibility corpus. Production passes no
/// cycle limit and remains connected for the supervised process lifetime.
pub fn run_public_xmonad_policy_cycles(
    socket: impl AsRef<Path>,
    launch: LegacyWmLaunchSpec,
    max_cycles: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    if max_cycles == Some(0) {
        return Err("public xmonad cycle limit must be nonzero".into());
    }
    let mut client = PolicyV1Client::connect(socket, PUBLIC_POLICY_TIMEOUT)?;
    client.activate_profile_and_configure_with(
        xmonad_public_policy_actions(),
        WmChromePolicy::default(),
    )?;
    let mut snapshot = client.receive_snapshot()?;
    let mut adapter = PublicXmonadPolicyAdapter::start(launch.clone(), &snapshot.scene)?;
    let mut completed_cycles = 0;
    let mut adapter_rebuilds = 0_u64;

    loop {
        let request = client.receive_projection_request()?;
        let transaction = client.new_transaction()?;
        let pending = adapter.plan(&snapshot.scene, &request, transaction)?;
        client.send_projection(&pending.proposal)?;
        let outcome = client.receive_projection_outcome(&pending.proposal)?;
        match xmonad_projection_decision(outcome) {
            XmonadProjectionDecision::Commit => {}
            XmonadProjectionDecision::RebuildPrivateAdapter => {
                drop(adapter);
                snapshot = client.receive_snapshot()?;
                adapter = PublicXmonadPolicyAdapter::start(launch.clone(), &snapshot.scene)?;
                adapter_rebuilds = adapter_rebuilds.saturating_add(1);
                println!(
                    "sophia_xmonad_policy schema=1 status=adapter_rebuilt outcome={outcome:?} rebuilds={adapter_rebuilds}"
                );
                continue;
            }
            XmonadProjectionDecision::Fatal => {
                return Err(
                    format!("public xmonad projection was not committed: {outcome:?}").into(),
                );
            }
        }
        let operation = pending.session_operation;
        adapter.commit(pending);
        if let Some((token, target)) = operation {
            let outcome = client.request_session_operation(token, target)?;
            if outcome.outcome != PolicyProjectionOutcome::Committed {
                return Err(format!(
                    "public xmonad session operation was not committed: {:?}",
                    outcome.outcome
                )
                .into());
            }
        }
        completed_cycles += 1;
        if max_cycles == Some(completed_cycles) {
            return Ok(());
        }
        snapshot = client.receive_snapshot()?;
    }
}

pub struct PublicXmonadPolicyAdapter {
    runtime: LegacyX11WmBridgeRuntime,
    workspace_state: WmWorkspaceState,
    known_surfaces: BTreeSet<SurfaceId>,
}

pub struct PendingPublicXmonadProjection {
    pub proposal: PolicyProjectionProposal,
    candidate: WmWorkspaceState,
    session_operation: Option<(u64, Option<SurfaceId>)>,
}

impl PublicXmonadPolicyAdapter {
    pub fn start(
        launch: LegacyWmLaunchSpec,
        scene: &PolicySceneSnapshot,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let initial_root = scene_root(scene)?;
        let workspace_state = WmWorkspaceState::new(
            scene
                .outputs
                .iter()
                .map(|output| (output.output, output.work_area)),
            WM_DEFAULT_WORKSPACES,
        )?;
        let runtime = LegacyX11WmBridgeRuntime::start_with_root(
            launch.with_profile(LegacyWmProfile::Xmonad),
            initial_root,
        )?;
        let mut adapter = Self {
            runtime,
            workspace_state,
            known_surfaces: BTreeSet::new(),
        };
        adapter.synchronize(scene)?;
        Ok(adapter)
    }

    pub fn plan(
        &mut self,
        scene: &PolicySceneSnapshot,
        request: &PolicyProjectionRequest,
        transaction: TransactionId,
    ) -> Result<PendingPublicXmonadProjection, Box<dyn std::error::Error>> {
        if scene.generation != request.scene_generation || request.affected_outputs.is_empty() {
            return Err("public xmonad request does not match its scene".into());
        }
        self.synchronize(scene)?;
        let session_actions = scene_session_actions(scene);
        self.runtime
            .configure_session(self.workspace_state.descriptor(session_actions.clone()))?;

        let response = self.translate_request(scene, request, transaction)?;
        let plan = self
            .workspace_state
            .plan_response(&response, &session_actions)?;
        let proposal = adapt_legacy_policy_plan(request, scene, &plan)?;
        let session_operation = plan
            .session_action
            .and_then(|(action, target)| operation_slot(action).map(|slot| (slot, target)))
            .and_then(|(slot, target)| {
                scene
                    .session_operations
                    .iter()
                    .find(|operation| operation.slot == slot)
                    .map(|operation| (operation.token, target))
            });
        if plan.session_action.is_some() && session_operation.is_none() {
            return Err("xmonad requested an unavailable public session operation".into());
        }
        Ok(PendingPublicXmonadProjection {
            proposal,
            candidate: plan.candidate,
            session_operation,
        })
    }

    pub fn commit(&mut self, pending: PendingPublicXmonadProjection) {
        self.workspace_state = pending.candidate;
    }

    fn synchronize(
        &mut self,
        scene: &PolicySceneSnapshot,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.workspace_state.replace_outputs(
            scene
                .outputs
                .iter()
                .map(|output| (output.output, output.work_area)),
        )?;
        let live = scene
            .surfaces
            .iter()
            .map(|surface| surface.surface)
            .collect::<BTreeSet<_>>();
        for surface in self
            .known_surfaces
            .difference(&live)
            .copied()
            .collect::<Vec<_>>()
        {
            self.workspace_state.remove_surface(surface);
        }
        let active_workspace = self
            .workspace_state
            .output(scene.active_output)
            .ok_or("public xmonad scene has an unknown active output")?
            .workspace;
        for surface in &scene.surfaces {
            let workspace = match surface.current_output {
                Some(output) => {
                    self.workspace_state
                        .output(output)
                        .ok_or("public xmonad surface names an unknown output")?
                        .workspace
                }
                None if self.known_surfaces.contains(&surface.surface) => continue,
                None => active_workspace,
            };
            self.workspace_state
                .register_surface(surface.surface, workspace)?;
        }
        self.known_surfaces = live;
        Ok(())
    }

    fn translate_request(
        &mut self,
        scene: &PolicySceneSnapshot,
        request: &PolicyProjectionRequest,
        transaction: TransactionId,
    ) -> Result<WmResponsePacket, Box<dyn std::error::Error>> {
        match request.cause {
            PolicyRequestCause::Action { action, .. } => {
                let output = request.affected_outputs[0];
                let packet = WmRequestPacket {
                    transaction,
                    kind: WmRequestKind::ActionActivated(WmActionActivation {
                        action,
                        output,
                        workspace: self.output_workspace(output)?,
                        focused_surface: scene_output(scene, output)?.focus,
                        nodes: self.output_nodes(scene, output)?,
                    }),
                };
                Ok(self.runtime.handle_request(&packet)?)
            }
            PolicyRequestCause::Focus { target } => {
                let output = request.affected_outputs[0];
                let packet = WmRequestPacket {
                    transaction,
                    kind: WmRequestKind::FocusRequested(WmFocusRequest {
                        surface: target,
                        output,
                        workspace: self.output_workspace(output)?,
                    }),
                };
                Ok(self.runtime.handle_request(&packet)?)
            }
            PolicyRequestCause::SceneChanged => {
                let mut commands = Vec::new();
                let mut timeout_msec = POLICY_TIMEOUT_MSEC;
                let mut focus = None;
                for output in &request.affected_outputs {
                    let snapshot = scene_output(scene, *output)?;
                    let packet = WmRequestPacket {
                        transaction,
                        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                            output: *output,
                            workspace: self.output_workspace(*output)?,
                            bounds: snapshot.work_area,
                            nodes: self.output_nodes(scene, *output)?,
                        }),
                    };
                    let response = self.runtime.handle_request(&packet)?;
                    timeout_msec = timeout_msec.max(response.timeout_msec);
                    for command in response.commands {
                        if let WmCommand::FocusSurface(surface) = command {
                            if *output == scene.active_output {
                                focus = Some(surface);
                            }
                        } else {
                            commands.push(command);
                        }
                    }
                }
                if let Some(surface) = focus {
                    commands.push(WmCommand::FocusSurface(surface));
                }
                Ok(WmResponsePacket {
                    transaction,
                    commands,
                    timeout_msec,
                })
            }
            PolicyRequestCause::Interaction { .. } => {
                Err("xmonad public profile does not register pointer interactions".into())
            }
        }
    }

    fn output_workspace(
        &self,
        output: OutputId,
    ) -> Result<WorkspaceId, Box<dyn std::error::Error>> {
        self.workspace_state
            .output(output)
            .map(|state| state.workspace)
            .ok_or_else(|| "public xmonad request names an unknown output".into())
    }

    fn output_nodes(
        &self,
        scene: &PolicySceneSnapshot,
        output: OutputId,
    ) -> Result<Vec<LayoutNodeSnapshot>, Box<dyn std::error::Error>> {
        let workspace = self.output_workspace(output)?;
        scene
            .surfaces
            .iter()
            .filter(|surface| {
                self.workspace_state
                    .surface_visible_on_output(surface.surface, output)
                    .unwrap_or(false)
            })
            .map(|surface| {
                Ok(LayoutNodeSnapshot {
                    surface: surface.surface,
                    workspace,
                    kind: match surface.kind {
                        PolicySurfaceKind::Toplevel => LayoutNodeKind::Toplevel,
                        PolicySurfaceKind::Dialog => LayoutNodeKind::Dialog,
                        PolicySurfaceKind::Utility => LayoutNodeKind::Utility,
                        PolicySurfaceKind::Popup => LayoutNodeKind::Popup,
                        PolicySurfaceKind::Unknown => LayoutNodeKind::Unknown,
                    },
                    placement_preference: if self.workspace_state.surface_floating(surface.surface)
                    {
                        SurfacePlacementPreference::Floating
                    } else {
                        SurfacePlacementPreference::Default
                    },
                    transient_owner: surface.transient_owner,
                    capabilities: surface.capabilities,
                    state: LayoutNodeState {
                        focused: scene_output(scene, output)?.focus == Some(surface.surface),
                        urgent: false,
                        fullscreen: surface.current_state.fullscreen,
                        floating: self.workspace_state.surface_floating(surface.surface),
                        visible: true,
                    },
                    constraints: surface.constraints,
                    geometry: surface.geometry,
                    generation: surface.generation,
                })
            })
            .collect()
    }
}

fn scene_output(
    scene: &PolicySceneSnapshot,
    output: OutputId,
) -> Result<&sophia_protocol::PolicyOutputSnapshot, Box<dyn std::error::Error>> {
    scene
        .outputs
        .iter()
        .find(|snapshot| snapshot.output == output)
        .ok_or_else(|| "public xmonad scene is missing an affected output".into())
}

fn scene_root(scene: &PolicySceneSnapshot) -> Result<Rect, Box<dyn std::error::Error>> {
    scene
        .outputs
        .iter()
        .map(|output| output.work_area)
        .reduce(union_rect)
        .ok_or_else(|| "public xmonad scene has no outputs".into())
}

fn union_rect(left: Rect, right: Rect) -> Rect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let right_edge = left
        .x
        .saturating_add(left.width)
        .max(right.x.saturating_add(right.width));
    let bottom_edge = left
        .y
        .saturating_add(left.height)
        .max(right.y.saturating_add(right.height));
    Rect {
        x,
        y,
        width: right_edge.saturating_sub(x),
        height: bottom_edge.saturating_sub(y),
    }
}

fn scene_session_actions(scene: &PolicySceneSnapshot) -> Vec<WmSessionAction> {
    let slots = scene
        .session_operations
        .iter()
        .map(|operation| operation.slot)
        .collect::<BTreeSet<_>>();
    let mut actions = Vec::new();
    if slots.contains(&1) {
        actions.push(WmSessionAction::LaunchApplication {
            application: SessionApplicationId::from_raw(1),
        });
    }
    if slots.contains(&2) {
        actions.push(WmSessionAction::LaunchApplication {
            application: SessionApplicationId::from_raw(2),
        });
    }
    if slots.contains(&3) {
        actions.push(WmSessionAction::CloseFocused);
    }
    if slots.contains(&4) {
        actions.push(WmSessionAction::Logout);
    }
    actions
}

fn operation_slot(action: WmSessionAction) -> Option<u16> {
    match action {
        WmSessionAction::LaunchApplication { application } if application.raw() == 1 => Some(1),
        WmSessionAction::LaunchApplication { application } if application.raw() == 2 => Some(2),
        WmSessionAction::CloseFocused => Some(3),
        WmSessionAction::Logout => Some(4),
        WmSessionAction::LaunchApplication { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_catalog_is_unique_and_matches_the_xmonad_profile() {
        let actions = xmonad_public_policy_actions();
        assert_eq!(actions.len(), 34);
        assert_eq!(
            actions
                .iter()
                .map(|action| action.action)
                .collect::<BTreeSet<_>>()
                .len(),
            actions.len()
        );
        assert_eq!(
            actions
                .iter()
                .filter_map(|action| action.session_operation_slot)
                .collect::<Vec<_>>(),
            vec![1, 3]
        );
    }

    #[test]
    fn root_union_preserves_negative_and_extended_coordinates() {
        assert_eq!(
            union_rect(
                Rect {
                    x: -800,
                    y: 0,
                    width: 800,
                    height: 600,
                },
                Rect {
                    x: 0,
                    y: -100,
                    width: 1920,
                    height: 1080,
                },
            ),
            Rect {
                x: -800,
                y: -100,
                width: 2720,
                height: 1080,
            }
        );
    }
}
