use sophia_protocol::{
    AxisSpan, BufferSource, ClientAdmissionContext, ClientAdmissionId, ClientAuthProvenance,
    ClientAuthenticationMethod, DeviceId, InputEventKind, LayoutNodeKind, NamespaceCapabilities,
    NamespaceContext, NamespaceId, NamespacePortalCapability, NamespaceProfile, OutputEdge,
    OutputId, OutputReservation, OutputTopologyEntry, OutputTopologySnapshot, Point,
    PortalBrokerRequestPacket, PortalDecision, PortalGrant, PortalGrantState, PortalRequest,
    PortalTransfer, PortalTransferKind, Rect, Region, RoutedInputRequest, SeatId, Size,
    SurfaceConstraints, SurfaceId, SurfacePlacementPreference, SurfacePresentationRole,
    TransactionId,
};
use sophia_x_authority::*;
include!("x11_wire/transport_events.rs");
include!("x11_wire/core_decode.rs");
include!("x11_wire/graphics_decode.rs");
include!("x11_wire/core_dispatch.rs");
include!("x11_wire/extensions_dispatch.rs");
include!("x11_wire/rendering_dispatch.rs");
include!("x11_wire/properties_dispatch.rs");
include!("x11_wire/output_and_draw.rs");
include!("x11_wire/image_readback.rs");
include!("x11_wire/resources_frontend.rs");
include!("x11_wire/admission_frontend.rs");
include!("x11_wire/clipboard_frontend.rs");
include!("x11_wire/socket_observation.rs");
include!("x11_wire/map_hierarchy.rs");
include!("x11_wire/output_reservation_socket.rs");
include!("x11_wire/routed_service.rs");
include!("x11_wire/focus_routing.rs");
include!("x11_wire/support_requests.rs");
include!("x11_wire/support_extensions.rs");

#[test]
fn present_feedback_phases_accept_copy_and_flip_order_once() {
    let mut copy = XPresentFeedbackPhases::default();
    assert!(copy.observe_idle());
    assert!(!copy.finished());
    assert!(!copy.observe_idle());
    assert!(copy.observe_complete());
    assert!(copy.finished());
    assert!(!copy.observe_complete());

    let mut flip = XPresentFeedbackPhases::default();
    assert!(flip.observe_complete());
    assert!(!flip.finished());
    assert!(flip.observe_idle());
    assert!(flip.finished());
}
