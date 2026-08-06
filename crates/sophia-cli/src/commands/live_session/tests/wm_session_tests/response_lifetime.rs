#[test]
fn stale_wm_response_reconciles_removed_state_before_restart() {
    let removed = SurfaceId::new(11, 1);
    let retained = SurfaceId::new(12, 1);
    let pending_manage = SurfaceId::new(13, 1);
    let output = sophia_protocol::OutputId::from_raw(1);
    let workspace = sophia_protocol::WorkspaceId::from_raw(1);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    };
    let mut state = WmWorkspaceState::new([(output, geometry)], 1).unwrap();
    state.register_surface(removed, workspace).unwrap();
    state.register_surface(retained, workspace).unwrap();
    let mut layout = PersistentLiveLayout::default();
    layout.layers.insert(removed, test_layer(removed, geometry));
    layout
        .layers
        .insert(retained, test_layer(retained, Rect { x: 640, ..geometry }));
    let fingerprint = LiveWmLayoutFingerprint::capture(&layout, &state);

    layout.layers.remove(&removed);

    assert_eq!(
        fingerprint.reconcile_response_lifetime(&layout, &mut state),
        LiveWmResponseLifetime::RestartAndReseed {
            removed_registered_surfaces: 1,
        }
    );
    assert_eq!(state.surface_workspace(removed), None);
    assert_eq!(state.surface_workspace(retained), Some(workspace));

    layout.layers.insert(
        pending_manage,
        test_layer(
            pending_manage,
            Rect {
                x: 1280,
                ..geometry
            },
        ),
    );
    let mut planning_state = state.clone();
    planning_state
        .register_surface(pending_manage, workspace)
        .unwrap();
    let pending_fingerprint = LiveWmLayoutFingerprint::capture(&layout, &planning_state);
    layout.layers.remove(&pending_manage);
    assert_eq!(
        pending_fingerprint.reconcile_response_lifetime(&layout, &mut state),
        LiveWmResponseLifetime::RestartAndReseed {
            removed_registered_surfaces: 0,
        }
    );
    assert_eq!(state.surface_workspace(pending_manage), None);
    assert_eq!(
        LiveWmLayoutFingerprint::capture(&layout, &state)
            .reconcile_response_lifetime(&layout, &mut state),
        LiveWmResponseLifetime::Current
    );
}
