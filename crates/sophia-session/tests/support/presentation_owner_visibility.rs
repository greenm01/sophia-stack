use super::*;
use sophia_protocol::{LayoutNodeKind, SurfacePlacementPreference, SurfacePresentationRole};

fn observe(
    layout: &mut PersistentLiveLayout,
    surface: SurfaceId,
    role: SurfacePresentationRole,
    owner: Option<SurfaceId>,
    mapped: bool,
) {
    let mut batch = crate::live_session::wm_update_coordinator_batch(TransactionId::from_raw(70));
    batch.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            surface,
            role,
            kind: LayoutNodeKind::Toplevel,
            placement_preference: SurfacePlacementPreference::Default,
            owner,
            stack_rank: 0,
            mapped,
            geometry: Rect {
                x: 0,
                y: 0,
                width: 240,
                height: 112,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    );
    layout.observe_authority_batch(&batch);
}

#[test]
fn panel_popups_inherit_mapping_without_requesting_wm_placement() {
    let panel = SurfaceId::new(70, 1);
    let popup = SurfaceId::new(71, 1);
    let nested = SurfaceId::new(72, 1);
    let mut layout = PersistentLiveLayout::default();
    for (surface, owner) in [(panel, None), (popup, Some(panel)), (nested, Some(popup))] {
        observe(
            &mut layout,
            surface,
            SurfacePresentationRole::ClientPositioned,
            owner,
            true,
        );
    }
    let visible = |layout: &PersistentLiveLayout, surface| {
        layout
            .client_positioned_visible::<()>(surface, |_| panic!("panel has no WM placement"))
            .unwrap()
    };
    assert!(visible(&layout, nested));
    observe(
        &mut layout,
        panel,
        SurfacePresentationRole::ClientPositioned,
        None,
        false,
    );
    assert!(!visible(&layout, nested));
    observe(
        &mut layout,
        panel,
        SurfacePresentationRole::ClientPositioned,
        None,
        true,
    );
    assert!(visible(&layout, popup));

    let mut removal = crate::live_session::wm_update_coordinator_batch(TransactionId::from_raw(71));
    removal.removed_surfaces.push(panel);
    layout.observe_authority_batch(&removal);
    observe(
        &mut layout,
        SurfaceId::new(70, 2),
        SurfacePresentationRole::ClientPositioned,
        None,
        true,
    );
    assert!(!visible(&layout, popup));
}

#[test]
fn nested_popup_visibility_reaches_managed_owner_and_propagates_errors() {
    let managed = SurfaceId::new(80, 1);
    let popup = SurfaceId::new(81, 1);
    let nested = SurfaceId::new(82, 1);
    let mut layout = PersistentLiveLayout::default();
    observe(
        &mut layout,
        managed,
        SurfacePresentationRole::PolicyManaged,
        None,
        true,
    );
    observe(
        &mut layout,
        popup,
        SurfacePresentationRole::ClientPositioned,
        Some(managed),
        true,
    );
    observe(
        &mut layout,
        nested,
        SurfacePresentationRole::ClientPositioned,
        Some(popup),
        true,
    );
    for visible in [false, true] {
        assert_eq!(
            layout.client_positioned_visible::<()>(nested, |owner| {
                assert_eq!(owner, managed);
                Ok(visible)
            }),
            Ok(visible)
        );
    }
    assert_eq!(
        layout.client_positioned_visible(nested, |_| Err("policy unavailable")),
        Err("policy unavailable")
    );
}

#[test]
fn cyclic_popup_ownership_is_not_visible() {
    let first = SurfaceId::new(90, 1);
    let second = SurfaceId::new(91, 1);
    let mut layout = PersistentLiveLayout::default();
    for (surface, owner) in [(first, second), (second, first)] {
        observe(
            &mut layout,
            surface,
            SurfacePresentationRole::ClientPositioned,
            Some(owner),
            true,
        );
    }
    assert!(
        !layout
            .client_positioned_visible::<()>(first, |_| panic!("cycle has no managed ancestor"))
            .unwrap()
    );
}
