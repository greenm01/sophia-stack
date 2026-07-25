use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveSeatEvent {
    Enable,
    Disable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveSeatState {
    Active,
    ReleasePending,
    Suspended,
    AcquirePending,
    Failed,
}

impl LiveSeatState {
    pub const fn observe(self, event: LiveSeatEvent) -> Self {
        match (self, event) {
            (Self::Active, LiveSeatEvent::Disable) => Self::ReleasePending,
            (Self::Suspended, LiveSeatEvent::Enable) => Self::AcquirePending,
            (Self::Active, LiveSeatEvent::Enable)
            | (Self::Suspended, LiveSeatEvent::Disable)
            | (Self::ReleasePending, LiveSeatEvent::Disable)
            | (Self::AcquirePending, LiveSeatEvent::Enable) => self,
            _ => Self::Failed,
        }
    }

    pub const fn released(self) -> Self {
        if matches!(self, Self::ReleasePending) {
            Self::Suspended
        } else {
            Self::Failed
        }
    }

    pub const fn acquired(self) -> Self {
        if matches!(self, Self::AcquirePending) {
            Self::Active
        } else {
            Self::Failed
        }
    }
}

pub struct LiveSeatController {
    seat: libseat::Seat,
    events: Arc<Mutex<VecDeque<LiveSeatEvent>>>,
}

impl LiveSeatController {
    pub fn open() -> Result<Self, String> {
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let callback_events = Arc::clone(&events);
        let seat = libseat::Seat::open(move |_seat, event| {
            let event = match event {
                libseat::SeatEvent::Enable => LiveSeatEvent::Enable,
                libseat::SeatEvent::Disable => LiveSeatEvent::Disable,
            };
            if let Ok(mut events) = callback_events.lock() {
                events.push_back(event);
            }
        })
        .map_err(|error| format!("libseat open failed: {error}"))?;
        Ok(Self { seat, events })
    }

    pub fn name(&mut self) -> String {
        self.seat.name().to_owned()
    }

    pub fn dispatch(&mut self) -> Result<Option<LiveSeatEvent>, String> {
        self.seat
            .dispatch(0)
            .map_err(|error| format!("libseat dispatch failed: {error}"))?;
        self.events
            .lock()
            .map_err(|_| "libseat event queue was poisoned".to_owned())
            .map(|mut events| events.pop_front())
    }

    pub fn switch_session(&mut self, terminal: u8) -> Result<(), String> {
        self.seat
            .switch_session(i32::from(terminal))
            .map_err(|error| format!("libseat switch to VT{terminal} failed: {error}"))
    }

    pub fn acknowledge_disable(&mut self) -> Result<(), String> {
        self.seat
            .disable()
            .map_err(|error| format!("libseat disable acknowledgement failed: {error}"))
    }
}
