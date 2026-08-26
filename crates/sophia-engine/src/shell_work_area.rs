//! Engine-side admission for shell work-area reservations.
//!
//! A `sophia_shell_v1` candidate may claim one exclusive edge zone on its
//! output. This module owns the Engine half of that claim: validating it
//! against the realized topology, converting it into the same root-relative
//! band the work-area reducer already consumes for X-side struts, and holding
//! the presented claim across the lifecycle the coordination model fixes
//! (`validation/tla/ShellWorkAreaCoordination.tla`).
//!
//! The lifecycle is deliberately asymmetric. A claim is *prepared* when its
//! candidate is admitted, *presented* only when the coherent bundle commits,
//! and *retained* across shell disconnect: no action here clears a presented
//! reservation on connection loss, because silently growing the work area
//! while no shell can re-present is exactly the half-new desktop the model
//! forbids. Only a later exact bundle — a fresh candidate that reserves
//! differently or not at all — changes the presented claim.

use sophia_protocol::{
    AxisSpan, OutputEdge, OutputId, OutputReservation, Rect, ShellV1ReservationEdge,
    ShellV1WorkAreaReservation,
};

/// Why a candidate's reservation was not admitted.
///
/// Every refusal is named where it is decided: an absent capability and a
/// refused claim look identical to the shell otherwise, and telling those
/// apart is the whole reason to look.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellReservationRefusal {
    /// The candidate names an output the realized topology does not have.
    UnknownOutput,
    /// The candidate's connection epoch is not the current one; a claim from
    /// a dead epoch fails closed rather than resurrecting.
    StaleEpoch,
    /// The candidate generation does not advance past both the prepared and
    /// the presented generation.
    StaleGeneration,
    /// The claimed thickness leaves no work area on its output. A bar taller
    /// than its display is a defective claim with no sensible clamp target.
    ExhaustsOutput,
}

impl ShellReservationRefusal {
    /// The stable reason token trace lines carry.
    pub const fn reason(self) -> &'static str {
        match self {
            Self::UnknownOutput => "unknown_output",
            Self::StaleEpoch => "stale_epoch",
            Self::StaleGeneration => "stale_generation",
            Self::ExhaustsOutput => "exhausts_output",
        }
    }
}

/// One admitted claim: the identity that must match at bundle commit and the
/// band the reducer consumes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedShellReservation {
    pub connection_epoch: u64,
    pub candidate_generation: u64,
    pub output: OutputId,
    pub band: OutputReservation,
}

/// The claim a bundle carries between admission and commit. `None` means the
/// candidate reserves nothing, which at commit withdraws any presented claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedShellReservation {
    pub connection_epoch: u64,
    pub candidate_generation: u64,
    pub reservation: Option<AdmittedShellReservation>,
}

/// Engine-owned state for the shell's exclusive work-area claim.
#[derive(Debug, Default)]
pub struct ShellWorkAreaCoordinator {
    prepared: Option<PreparedShellReservation>,
    presented: Option<AdmittedShellReservation>,
}

impl ShellWorkAreaCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Admits one candidate's claim against the realized topology.
    ///
    /// `outputs` are the realized output bounds; `root` is their bounding
    /// rectangle, the space the reducer's bands live in. A candidate with no
    /// reservation is admitted as an explicit withdrawal so the commit path
    /// has one shape for both directions.
    pub fn admit(
        &mut self,
        current_connection_epoch: u64,
        candidate_epoch: u64,
        candidate_generation: u64,
        output: OutputId,
        reservation: Option<ShellV1WorkAreaReservation>,
        root: Rect,
        outputs: &[(OutputId, Rect)],
    ) -> Result<PreparedShellReservation, ShellReservationRefusal> {
        if candidate_epoch != current_connection_epoch {
            return Err(ShellReservationRefusal::StaleEpoch);
        }
        let presented_generation = self
            .presented
            .filter(|presented| presented.connection_epoch == current_connection_epoch)
            .map_or(0, |presented| presented.candidate_generation);
        let prepared_generation = self
            .prepared
            .filter(|prepared| prepared.connection_epoch == current_connection_epoch)
            .map_or(0, |prepared| prepared.candidate_generation);
        if candidate_generation <= presented_generation.max(prepared_generation) {
            return Err(ShellReservationRefusal::StaleGeneration);
        }
        let admitted = match reservation {
            None => None,
            Some(reservation) => {
                let bounds = outputs
                    .iter()
                    .find(|(candidate, _)| *candidate == output)
                    .map(|(_, bounds)| *bounds)
                    .ok_or(ShellReservationRefusal::UnknownOutput)?;
                let band = shell_reservation_band(reservation, root, bounds)
                    .ok_or(ShellReservationRefusal::ExhaustsOutput)?;
                Some(AdmittedShellReservation {
                    connection_epoch: candidate_epoch,
                    candidate_generation,
                    output,
                    band,
                })
            }
        };
        let prepared = PreparedShellReservation {
            connection_epoch: candidate_epoch,
            candidate_generation,
            reservation: admitted,
        };
        self.prepared = Some(prepared);
        Ok(prepared)
    }

    /// Commits the prepared claim once its bundle has presented.
    ///
    /// The identity must be the exact prepared one; anything else means the
    /// bundle being committed is not the bundle that was admitted, and the
    /// presented claim is preserved unchanged.
    pub fn commit(&mut self, connection_epoch: u64, candidate_generation: u64) -> bool {
        let Some(prepared) = self.prepared else {
            return false;
        };
        if prepared.connection_epoch != connection_epoch
            || prepared.candidate_generation != candidate_generation
        {
            return false;
        }
        self.prepared = None;
        self.presented = prepared.reservation;
        true
    }

    /// Rejects the in-flight claim, preserving the presented one.
    pub fn reject_prepared(&mut self) {
        self.prepared = None;
    }

    /// Connection loss burns only the in-flight claim. The presented claim is
    /// retained with the inert pixels: the work area may not change without a
    /// coherent bundle to change it.
    pub fn on_disconnect(&mut self) {
        self.prepared = None;
    }

    /// The presented claim, if any.
    pub fn presented(&self) -> Option<AdmittedShellReservation> {
        self.presented
    }

    /// The bands the work-area reduction must subtract.
    pub fn active_bands(&self) -> Vec<OutputReservation> {
        self.presented
            .map(|presented| presented.band)
            .into_iter()
            .collect()
    }
}

/// Converts an edge claim on one output into the root-relative band the
/// reducer consumes, exactly as an X-side strut on that output would express
/// it: depth measured from the root's edge through the output to the claimed
/// thickness, span covering the output's extent on the perpendicular axis.
///
/// Returns `None` when the thickness meets or exceeds the output's own extent
/// on the claimed axis, which would reduce that output's work area to nothing.
pub fn shell_reservation_band(
    reservation: ShellV1WorkAreaReservation,
    root: Rect,
    output: Rect,
) -> Option<OutputReservation> {
    if root.is_empty() || output.is_empty() {
        return None;
    }
    let thickness = i32::from(reservation.thickness_px);
    let (edge, depth, span) = match reservation.edge {
        ShellV1ReservationEdge::Top => {
            if thickness >= output.height {
                return None;
            }
            (
                OutputEdge::Top,
                output.y.checked_sub(root.y)?.checked_add(thickness)?,
                AxisSpan {
                    start: output.x,
                    end: output.x.checked_add(output.width)?,
                },
            )
        }
        ShellV1ReservationEdge::Bottom => {
            if thickness >= output.height {
                return None;
            }
            let root_bottom = root.y.checked_add(root.height)?;
            let output_bottom = output.y.checked_add(output.height)?;
            (
                OutputEdge::Bottom,
                root_bottom
                    .checked_sub(output_bottom)?
                    .checked_add(thickness)?,
                AxisSpan {
                    start: output.x,
                    end: output.x.checked_add(output.width)?,
                },
            )
        }
        ShellV1ReservationEdge::Left => {
            if thickness >= output.width {
                return None;
            }
            (
                OutputEdge::Left,
                output.x.checked_sub(root.x)?.checked_add(thickness)?,
                AxisSpan {
                    start: output.y,
                    end: output.y.checked_add(output.height)?,
                },
            )
        }
        ShellV1ReservationEdge::Right => {
            if thickness >= output.width {
                return None;
            }
            let root_right = root.x.checked_add(root.width)?;
            let output_right = output.x.checked_add(output.width)?;
            (
                OutputEdge::Right,
                root_right
                    .checked_sub(output_right)?
                    .checked_add(thickness)?,
                AxisSpan {
                    start: output.y,
                    end: output.y.checked_add(output.height)?,
                },
            )
        }
    };
    let band = OutputReservation { edge, depth, span };
    band.is_valid().then_some(band)
}
