use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};
use std::time::{Duration, Instant};

use sophia_protocol::{SurfaceId, TransactionId};
use sophia_x_authority::{
    XAuthorityClientControlAck, XAuthorityClientControlCommand, XAuthorityControlKind,
    XAuthorityControlOutcome, XServerFrontendClientId, XServerFrontendControlRouter,
    XServerFrontendRouteError,
};

pub const SESSION_CONTROL_CAPACITY: usize = 32;
pub const SESSION_CONTROL_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionControlKey {
    pub client: XServerFrontendClientId,
    pub kind: XAuthorityControlKind,
    pub transaction: TransactionId,
    pub surface: SurfaceId,
}

impl SessionControlKey {
    pub fn from_command(command: XAuthorityClientControlCommand) -> Self {
        Self {
            client: command.client,
            kind: command.command.kind(),
            transaction: command.command.transaction(),
            surface: command.command.surface(),
        }
    }

    fn from_ack(ack: XAuthorityClientControlAck) -> Self {
        Self {
            client: ack.client,
            kind: ack.acknowledgement.kind,
            transaction: ack.acknowledgement.transaction,
            surface: ack.acknowledgement.surface,
        }
    }

    fn is_focus(self) -> bool {
        matches!(
            self.kind,
            XAuthorityControlKind::FocusSurface | XAuthorityControlKind::ClearFocus
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionControlFailure {
    Capacity,
    Duplicate,
    Rejected(XAuthorityControlOutcome),
    TimedOut,
    UnexpectedAcknowledgement,
    Disconnected,
}

impl SessionControlFailure {
    /// Returns true when an acknowledged command lost its target during the
    /// bounded Engine-to-frontend handoff.
    pub const fn is_stale_target(self) -> bool {
        matches!(self, Self::Rejected(XAuthorityControlOutcome::ClientGone))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionControlCompletion {
    pub key: SessionControlKey,
    pub failure: Option<SessionControlFailure>,
    pub queue_dwell: Duration,
    pub acknowledgement_latency: Duration,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SessionControlMetrics {
    pub enqueued: usize,
    pub dispatched: usize,
    pub delivered: usize,
    pub rejected: usize,
    pub timed_out: usize,
    pub unexpected: usize,
    pub peak_depth: usize,
    pub max_queue_dwell: Duration,
    pub max_acknowledgement_latency: Duration,
}

#[derive(Clone, Copy, Debug)]
struct PendingControl {
    command: XAuthorityClientControlCommand,
    key: SessionControlKey,
    queued_at: Instant,
    dispatched_at: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct SessionControlQueue {
    pending: VecDeque<PendingControl>,
    metrics: SessionControlMetrics,
}

pub trait SessionControlSender {
    fn try_send_control(
        &self,
        command: XAuthorityClientControlCommand,
    ) -> Result<(), TrySendError<XAuthorityClientControlCommand>>;
}

impl SessionControlSender for SyncSender<XAuthorityClientControlCommand> {
    fn try_send_control(
        &self,
        command: XAuthorityClientControlCommand,
    ) -> Result<(), TrySendError<XAuthorityClientControlCommand>> {
        self.try_send(command)
    }
}

impl SessionControlSender for XServerFrontendControlRouter {
    fn try_send_control(
        &self,
        command: XAuthorityClientControlCommand,
    ) -> Result<(), TrySendError<XAuthorityClientControlCommand>> {
        match self.route_control(command) {
            Ok(()) => Ok(()),
            Err(XServerFrontendRouteError::ClientQueueFull { .. }) => {
                Err(TrySendError::Full(command))
            }
            Err(_) => Err(TrySendError::Disconnected(command)),
        }
    }
}

impl SessionControlQueue {
    pub fn enqueue(
        &mut self,
        command: XAuthorityClientControlCommand,
        now: Instant,
    ) -> Result<SessionControlKey, SessionControlFailure> {
        let key = SessionControlKey::from_command(command);
        if self.pending.iter().any(|pending| pending.key == key) {
            return Err(SessionControlFailure::Duplicate);
        }
        if self.pending.len() >= SESSION_CONTROL_CAPACITY {
            return Err(SessionControlFailure::Capacity);
        }
        self.pending.push_back(PendingControl {
            command,
            key,
            queued_at: now,
            dispatched_at: None,
        });
        self.metrics.enqueued += 1;
        self.metrics.peak_depth = self.metrics.peak_depth.max(self.pending.len());
        Ok(key)
    }

    pub fn service(
        &mut self,
        sender: &impl SessionControlSender,
        receiver: &Receiver<XAuthorityClientControlAck>,
        now: Instant,
        completions: &mut Vec<SessionControlCompletion>,
    ) -> Result<(), SessionControlFailure> {
        self.service_when(sender, receiver, now, completions, true)
    }

    pub fn service_when(
        &mut self,
        sender: &impl SessionControlSender,
        receiver: &Receiver<XAuthorityClientControlAck>,
        now: Instant,
        completions: &mut Vec<SessionControlCompletion>,
        dispatch_ready: bool,
    ) -> Result<(), SessionControlFailure> {
        self.receive_acknowledgements(receiver, now, completions)?;
        self.expire(now, completions);
        if dispatch_ready {
            self.dispatch(sender, now)?;
        }
        Ok(())
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn has_non_configure_pending(&self) -> bool {
        self.pending
            .iter()
            .any(|pending| pending.key.kind != XAuthorityControlKind::ConfigureSurface)
    }

    pub fn metrics(&self) -> SessionControlMetrics {
        self.metrics
    }

    fn receive_acknowledgements(
        &mut self,
        receiver: &Receiver<XAuthorityClientControlAck>,
        now: Instant,
        completions: &mut Vec<SessionControlCompletion>,
    ) -> Result<(), SessionControlFailure> {
        loop {
            let acknowledgement = match receiver.try_recv() {
                Ok(acknowledgement) => acknowledgement,
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => return Err(SessionControlFailure::Disconnected),
            };
            let key = SessionControlKey::from_ack(acknowledgement);
            let Some(index) = self
                .pending
                .iter()
                .position(|pending| pending.key == key && pending.dispatched_at.is_some())
            else {
                self.metrics.unexpected += 1;
                return Err(SessionControlFailure::UnexpectedAcknowledgement);
            };
            let pending = self.pending.remove(index).expect("located pending control");
            let dispatched_at = pending.dispatched_at.expect("matched dispatched control");
            let queue_dwell = dispatched_at.duration_since(pending.queued_at);
            let acknowledgement_latency = now.duration_since(dispatched_at);
            self.metrics.max_queue_dwell = self.metrics.max_queue_dwell.max(queue_dwell);
            self.metrics.max_acknowledgement_latency = self
                .metrics
                .max_acknowledgement_latency
                .max(acknowledgement_latency);
            let failure =
                if acknowledgement.acknowledgement.outcome == XAuthorityControlOutcome::Delivered {
                    self.metrics.delivered += 1;
                    None
                } else {
                    self.metrics.rejected += 1;
                    Some(SessionControlFailure::Rejected(
                        acknowledgement.acknowledgement.outcome,
                    ))
                };
            completions.push(SessionControlCompletion {
                key,
                failure,
                queue_dwell,
                acknowledgement_latency,
            });
        }
    }

    fn expire(&mut self, now: Instant, completions: &mut Vec<SessionControlCompletion>) {
        let mut index = 0;
        while index < self.pending.len() {
            if now.duration_since(self.pending[index].queued_at) < SESSION_CONTROL_TIMEOUT {
                index += 1;
                continue;
            }
            let pending = self.pending.remove(index).expect("pending index exists");
            self.metrics.timed_out += 1;
            completions.push(SessionControlCompletion {
                key: pending.key,
                failure: Some(SessionControlFailure::TimedOut),
                queue_dwell: pending
                    .dispatched_at
                    .unwrap_or(now)
                    .duration_since(pending.queued_at),
                acknowledgement_latency: pending
                    .dispatched_at
                    .map_or(Duration::ZERO, |sent| now.duration_since(sent)),
            });
        }
    }

    fn dispatch(
        &mut self,
        sender: &impl SessionControlSender,
        now: Instant,
    ) -> Result<(), SessionControlFailure> {
        let focus_in_flight = self
            .pending
            .iter()
            .any(|pending| pending.key.is_focus() && pending.dispatched_at.is_some());
        let mut dispatched_focus = focus_in_flight;
        for pending in &mut self.pending {
            if pending.dispatched_at.is_some() || (pending.key.is_focus() && dispatched_focus) {
                continue;
            }
            match sender.try_send_control(pending.command) {
                Ok(()) => {
                    pending.dispatched_at = Some(now);
                    self.metrics.dispatched += 1;
                    dispatched_focus |= pending.key.is_focus();
                }
                Err(TrySendError::Full(_)) => break,
                Err(TrySendError::Disconnected(_)) => {
                    return Err(SessionControlFailure::Disconnected);
                }
            }
        }
        Ok(())
    }
}
