use sophia_protocol::{
    BufferSource, ClientAdmissionContext, ClientAdmissionId, ClientAuthProvenance,
    ClientAuthenticationMethod, DeviceId, InputEventKind, NamespaceCapabilities, NamespaceContext,
    NamespaceId, NamespacePortalCapability, NamespaceProfile, OutputId, OutputTopologyEntry,
    OutputTopologySnapshot, Point, PortalBrokerRequestPacket, PortalDecision, PortalGrant,
    PortalGrantState, PortalRequest, PortalTransfer, PortalTransferId, PortalTransferKind, Rect,
    Region, RoutedInputRequest, SeatId, Size, SurfaceConstraints, SurfaceId, TransactionId,
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
include!("x11_wire/resources_frontend.rs");
include!("x11_wire/admission_frontend.rs");
include!("x11_wire/clipboard_frontend.rs");
include!("x11_wire/socket_observation.rs");
include!("x11_wire/routed_service.rs");
include!("x11_wire/support_requests.rs");
include!("x11_wire/support_extensions.rs");
