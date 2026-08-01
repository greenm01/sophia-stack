use super::*;
use crate::commands::live_session::PersistentLiveLayout;
use sophia_protocol::{SurfaceConstraints, TransactionId};

#[test]
fn client_positioned_role_transition_withdraws_surface_from_wm_policy() {
    let surface = SurfaceId::new(44, 1);
    let geometry = Rect {
        x: 10,
        y: 20,
        width: 640,
        height: 480,
    };
    let mut request =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(40));
    request
        .presentation_intents
        .push(sophia_protocol::SurfacePresentationIntent {
            surface,
            kind: sophia_protocol::SurfacePresentationIntentKind::Request,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        });
    let mut layout = PersistentLiveLayout::default();
    layout.observe_authority_batch(&request);
    assert_eq!(layout.next_unmanaged_surface(), Some(surface));

    let mut reparent =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(41));
    reparent.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            surface,
            role: sophia_protocol::SurfacePresentationRole::ClientPositioned,
            mapped: true,
            geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 2,
        },
    );

    let observation = layout.observe_authority_batch(&reparent);

    assert_eq!(observation.withdrawn_surfaces, vec![surface]);
    assert_eq!(layout.next_unmanaged_surface(), None);
    assert!(!layout.is_policy_managed(surface));
}
