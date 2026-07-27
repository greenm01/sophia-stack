use super::*;
use crate::commands::live_session::{
    LiveWmLayoutFingerprint, PersistentLiveLayout, live_layout_node, planning_state_for_response,
};
use sophia_engine::WmWorkspaceState;

fn test_layer(surface: SurfaceId, geometry: Rect) -> LayerSnapshot {
    LayerSnapshot {
        surface,
        authority_local_id: None,
        namespace: None,
        stack_rank: 0,
        geometry,
        source: BufferSource::None,
        damage: Region::single(geometry),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 1,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }
}

#[test]
fn wm_layout_fingerprint_tracks_planned_surface_lifetimes_only() {
    let managed = SurfaceId::new(1, 1);
    let concurrent = SurfaceId::new(2, 1);
    let output = sophia_protocol::OutputId::from_raw(1);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    };
    let mut state = WmWorkspaceState::new([(output, geometry)], 1).unwrap();
    let workspace = sophia_protocol::WorkspaceId::from_raw(1);
    state.register_surface(managed, workspace).unwrap();
    let mut layout = PersistentLiveLayout::default();
    layout.layers.insert(managed, test_layer(managed, geometry));
    let fingerprint = LiveWmLayoutFingerprint::capture(&layout, &state);

    layout.layers.insert(
        concurrent,
        test_layer(concurrent, Rect { x: 640, ..geometry }),
    );
    assert!(fingerprint.still_matches(&layout));

    layout.layers.get_mut(&managed).unwrap().geometry.x = 1;
    assert!(fingerprint.still_matches(&layout));

    layout.layers.remove(&managed);
    assert!(!fingerprint.still_matches(&layout));
}

#[test]
fn queued_manage_response_rebases_on_the_latest_committed_state() {
    let first = SurfaceId::new(1, 1);
    let queued = SurfaceId::new(2, 1);
    let output = sophia_protocol::OutputId::from_raw(1);
    let workspace = sophia_protocol::WorkspaceId::from_raw(1);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    };
    let mut current = WmWorkspaceState::new([(output, bounds)], 2).unwrap();
    current.register_surface(first, workspace).unwrap();
    let request = sophia_protocol::WmRequestPacket {
        transaction: sophia_protocol::TransactionId::from_raw(2),
        kind: sophia_protocol::WmRequestKind::ManageSurface(sophia_protocol::WmManageSurface {
            node: live_layout_node(
                &test_layer(queued, bounds),
                workspace,
                &sophia_engine::LayoutEpochCoordinator::default(),
            ),
            output,
            workspace,
            bounds,
        }),
    };

    let rebased = planning_state_for_response(&current, &request).unwrap();
    assert_eq!(rebased.surface_workspace(first), Some(workspace));
    assert_eq!(rebased.surface_workspace(queued), Some(workspace));
}
