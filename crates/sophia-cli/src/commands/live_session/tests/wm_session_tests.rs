use super::*;
use crate::commands::live_session::{
    LiveWmLayoutFingerprint, PersistentLiveLayout, live_layout_node, live_layout_node_from_facts,
    planning_state_for_response,
};
use sophia_engine::WmWorkspaceState;
use sophia_protocol::{SurfaceConstraints, TransactionId, WorkspaceId};

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
                sophia_engine::SurfaceChromeStyle::default(),
            )
            .unwrap(),
            output,
            workspace,
            bounds,
        }),
    };

    let rebased = planning_state_for_response(&current, &request).unwrap();
    assert_eq!(rebased.surface_workspace(first), Some(workspace));
    assert_eq!(rebased.surface_workspace(queued), Some(workspace));
}

#[test]
fn recovery_content_extent_crosses_wm_boundary_as_outer_allocation() {
    let surface = SurfaceId::new(3, 1);
    let workspace = sophia_protocol::WorkspaceId::from_raw(1);
    let content = Size {
        width: 500,
        height: 500,
    };
    let mut epochs = sophia_engine::LayoutEpochCoordinator::default();
    epochs.record_committed(surface, content);
    epochs
        .begin_recovery(
            [(
                surface,
                Size {
                    width: 1276,
                    height: 1422,
                },
            )],
            [surface],
        )
        .unwrap();
    let style = sophia_engine::SurfaceChromeStyle {
        frame: sophia_engine::SurfaceFrameStyle {
            width: 2,
            ..sophia_engine::SurfaceFrameStyle::default()
        },
        ..sophia_engine::SurfaceChromeStyle::default()
    };

    let node = live_layout_node(
        &test_layer(
            surface,
            Rect {
                x: 2,
                y: 2,
                width: content.width,
                height: content.height,
            },
        ),
        workspace,
        &epochs,
        style,
    )
    .unwrap();

    let outer = Size {
        width: 504,
        height: 504,
    };
    assert_eq!(node.constraints.min_size, Some(outer));
    assert_eq!(node.constraints.max_size, Some(outer));
    assert_eq!(
        node.geometry,
        Rect {
            x: 0,
            y: 0,
            width: outer.width,
            height: outer.height,
        }
    );
}

#[test]
fn presentation_request_produces_a_wm_node_before_pixels_exist() {
    let surface = SurfaceId::new(4, 1);
    let geometry = Rect {
        x: 80,
        y: 60,
        width: 500,
        height: 500,
    };
    let intent = sophia_protocol::SurfacePresentationIntent {
        surface,
        kind: sophia_protocol::SurfacePresentationIntentKind::Request,
        role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
        geometry,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        generation: 1,
    };
    let mut batch =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(10));
    batch.client = Some(sophia_x_authority::XServerFrontendClientId::from_raw(1));
    batch.presentation_intents.push(intent);
    let mut layout = PersistentLiveLayout::default();

    let observation = layout.observe_authority_batch(&batch);

    assert_eq!(observation.new_surfaces, vec![surface]);
    assert_eq!(layout.next_unmanaged_surface(), Some(surface));
    assert!(layout.layers.is_empty());
    assert_eq!(layout.planning_layers()[0].source, BufferSource::None);
    let chrome = sophia_engine::SurfaceChromeStyle::default();
    let node = live_layout_node_from_facts(
        layout.layout_facts(surface).unwrap(),
        WorkspaceId::from_raw(1),
        &layout.layout_epochs,
        chrome,
    )
    .unwrap();
    assert_eq!(node.surface, surface);
    assert_eq!(
        node.geometry,
        sophia_engine::outer_surface_geometry(geometry, chrome).unwrap()
    );
}
