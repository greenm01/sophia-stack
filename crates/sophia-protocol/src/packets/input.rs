use crate::geometry::{Point, Transform};
use crate::ids::{ApplicationRouteLeaseId, DeviceId, SeatId, SurfaceId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationRouteLeaseIdentity {
    pub id: ApplicationRouteLeaseId,
    pub seat: SeatId,
    pub frontend_sequence: u64,
    pub control_epoch: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputEventPacket {
    pub serial: u64,
    pub seat: SeatId,
    pub device: DeviceId,
    pub time_msec: u64,
    pub kind: InputEventKind,
    pub global_position: Option<Point>,
    pub target_surface: Option<SurfaceId>,
    pub local_position: Option<Point>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputEventKind {
    PointerMotion,
    PointerButton {
        button: u32,
        pressed: bool,
    },
    PointerAxis {
        horizontal_v120: i32,
        vertical_v120: i32,
    },
    Key {
        keycode: u32,
        pressed: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputRoute {
    pub input_serial: u64,
    pub target_surface: Option<SurfaceId>,
    pub global_position: Point,
    pub local_position: Option<Point>,
    pub transform: Transform,
    pub outcome: InputRouteOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputRouteOutcome {
    Routed,
    NoTarget,
    StaleTarget,
    Denied,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoutedInputRequest {
    pub serial: u64,
    pub seat: SeatId,
    pub device: DeviceId,
    pub time_msec: u64,
    pub target_surface: SurfaceId,
    pub global_position: Point,
    pub local_position: Point,
    pub kind: InputEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedInputDecision {
    pub serial: u64,
    pub target_surface: SurfaceId,
    pub outcome: RoutedInputOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutedInputOutcome {
    Accepted,
    RejectedStaleTarget,
    RejectedDeniedNamespace,
    RejectedActiveGrab,
    RejectedFocusPolicy,
    RejectedUnsupportedEvent,
}
