use std::fmt;

use sophia_backend_live::{
    LibdrmNativeAtomicTopologyHead, LibdrmNativeOutputCapability, LibdrmNativeOutputTiming,
    LibdrmNativePrimaryPlaneResourceDevice, LiveProductionNativeScanout,
    discover_native_primary_plane_property_handles, resolve_native_connector_mode,
};
use sophia_protocol::OutputId;

use crate::desktop_output_topology::NativeOutputActivationPlan;

/// Why one output could not contribute a head.
///
/// Each arm is a different fact about the host, and the resolver keeps them apart
/// because they send an operator to different places: a missing route is a
/// selection problem, missing properties are a driver problem, and an unknown
/// timing is a configuration problem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOutputHeadUnavailable {
    /// The output has no connector and CRTC route on this host.
    MissingSelection,
    /// Connector, CRTC, or plane properties an atomic modeset needs are absent.
    MissingProperties,
    /// The connector does not advertise the requested timing.
    UnknownTiming,
    /// The mode exists, but the kernel would not create a blob for it.
    ModeBlobRefused,
}

/// One head plus the mode blob it names.
///
/// The blob is returned rather than kept by the composer because its lifetime
/// belongs to the resolution that requested it, not to the hardware.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeOutputComposedHead<H> {
    pub head: H,
    pub mode_blob: u64,
}

/// The hardware side of resolving a plan into heads.
///
/// The resolver decides which outputs participate and at which timing; this
/// composes the KMS objects that express those decisions. The split is what keeps
/// the decisions testable without a DRM device, and it keeps DRM handle types out
/// of the resolver entirely — `Head` is opaque here.
pub trait NativeOutputTopologyHardware {
    /// A composed head. Real hardware yields `LibdrmNativeAtomicTopologyHead`.
    type Head;

    /// Composes one head for `output` at `timing`, creating the mode blob it
    /// needs.
    fn compose_head(
        &self,
        output: OutputId,
        timing: LibdrmNativeOutputTiming,
    ) -> Result<NativeOutputComposedHead<Self::Head>, NativeOutputHeadUnavailable>;

    /// Releases a blob this hardware created for `output`. Called for every blob
    /// the resolver took, including on the failure path.
    ///
    /// The output is named because a blob belongs to the card that created it, and
    /// a session can span more than one card. Releasing against the wrong device
    /// would leak the blob and disturb an unrelated one.
    fn release_mode_blob(&self, output: OutputId, blob: u64);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOutputHeadResolveError {
    /// The plan named an output the capability set does not describe.
    MissingCapability(u64),
    /// One output could not be composed, so the whole topology fails closed.
    Unavailable {
        output: u64,
        cause: NativeOutputHeadUnavailable,
    },
    /// Every target in the plan is disabled, so there is no topology to validate.
    /// A backstop: reconciliation refuses an all-disabled candidate before a plan
    /// exists, so nothing in production reaches this today.
    NoEnabledOutputs,
}

impl fmt::Display for NativeOutputHeadResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCapability(output) => {
                write!(formatter, "native output {output} has no DRM capability")
            }
            Self::Unavailable { output, cause } => {
                let reason = match cause {
                    NativeOutputHeadUnavailable::MissingSelection => "has no connector route",
                    NativeOutputHeadUnavailable::MissingProperties => {
                        "is missing atomic modeset properties"
                    }
                    NativeOutputHeadUnavailable::UnknownTiming => {
                        "does not advertise the requested timing"
                    }
                    NativeOutputHeadUnavailable::ModeBlobRefused => "was refused a mode blob",
                };
                write!(formatter, "native output {output} {reason}")
            }
            Self::NoEnabledOutputs => {
                formatter.write_str("native activation plan enables no output")
            }
        }
    }
}

impl std::error::Error for NativeOutputHeadResolveError {}

/// Heads resolved from one plan, owning the mode blobs they name.
///
/// The blobs are kernel resources created for this resolution and belong to it.
/// Tying them together means a caller cannot submit heads whose blobs have already
/// been released, and cannot leak them by taking an early return.
#[derive(Debug)]
pub struct NativeOutputTopologyHeads<'a, H>
where
    H: NativeOutputTopologyHardware,
{
    hardware: &'a H,
    heads: Vec<H::Head>,
    blobs: Vec<(OutputId, u64)>,
}

impl<H> NativeOutputTopologyHeads<'_, H>
where
    H: NativeOutputTopologyHardware,
{
    pub fn heads(&self) -> &[H::Head] {
        &self.heads
    }

    pub fn len(&self) -> usize {
        self.heads.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heads.is_empty()
    }
}

impl<H> Drop for NativeOutputTopologyHeads<'_, H>
where
    H: NativeOutputTopologyHardware,
{
    fn drop(&mut self) {
        for (output, blob) in self.blobs.drain(..) {
            self.hardware.release_mode_blob(output, blob);
        }
    }
}

/// Resolves an activation plan into the heads that express it.
///
/// Only enabled targets become heads. A disabled output is expressed by having no
/// head rather than by an inactive one, which matches how disablement reaches
/// policy: by omission from the complete snapshot.
///
/// This resolves the requested topology only. Rollback heads are deliberately not
/// produced here. Restoring a previous topology needs that topology's own mode
/// blobs, still alive at the moment rollback runs, which is after an apply has
/// already failed; creating them alongside every candidate would spend kernel
/// resources on every plan for a path most plans never take.
///
/// Failure is all-or-nothing, and a partial resolution releases every blob it
/// created on the way out. A topology missing one head is a different desktop, not
/// a degraded version of the requested one.
pub fn resolve_native_output_topology_heads<'a, H>(
    plan: &NativeOutputActivationPlan,
    capabilities: &[LibdrmNativeOutputCapability],
    hardware: &'a H,
) -> Result<NativeOutputTopologyHeads<'a, H>, NativeOutputHeadResolveError>
where
    H: NativeOutputTopologyHardware,
{
    // Held as the owner from the first blob onward, so every early return below
    // runs its Drop and releases what has been created so far.
    let mut resolved = NativeOutputTopologyHeads {
        hardware,
        heads: Vec::new(),
        blobs: Vec::new(),
    };

    for target in plan.targets() {
        let requested = target.requested();
        if !requested.enabled {
            continue;
        }
        let output = target.output();
        let raw = output.raw();

        if !capabilities
            .iter()
            .any(|capability| capability.output() == output)
        {
            return Err(NativeOutputHeadResolveError::MissingCapability(raw));
        }

        let timing = LibdrmNativeOutputTiming::new(
            requested.mode.width,
            requested.mode.height,
            requested.mode.refresh_millihz,
        );
        match hardware.compose_head(output, timing) {
            Ok(composed) => {
                resolved.blobs.push((output, composed.mode_blob));
                resolved.heads.push(composed.head);
            }
            Err(cause) => {
                return Err(NativeOutputHeadResolveError::Unavailable { output: raw, cause });
            }
        }
    }

    if resolved.heads.is_empty() {
        return Err(NativeOutputHeadResolveError::NoEnabledOutputs);
    }
    Ok(resolved)
}

/// Composes heads from the live scanout's own KMS objects.
///
/// Every output the session drives already has a selection naming its connector,
/// CRTC, and plane, and the card that owns them. This reads those rather than
/// re-deriving an assignment, so a validated topology names exactly the objects the
/// session runs on.
pub struct LiveNativeOutputTopologyHardware<'a> {
    scanout: &'a LiveProductionNativeScanout,
}

impl<'a> LiveNativeOutputTopologyHardware<'a> {
    pub const fn new(scanout: &'a LiveProductionNativeScanout) -> Self {
        Self { scanout }
    }
}

impl NativeOutputTopologyHardware for LiveNativeOutputTopologyHardware<'_> {
    type Head = LibdrmNativeAtomicTopologyHead;

    fn compose_head(
        &self,
        output: OutputId,
        timing: LibdrmNativeOutputTiming,
    ) -> Result<NativeOutputComposedHead<Self::Head>, NativeOutputHeadUnavailable> {
        let Some(index) = self.scanout.output_index(output) else {
            return Err(NativeOutputHeadUnavailable::MissingSelection);
        };
        let selection = self.scanout.selection(index);
        let card = self.scanout.card(index);

        let discovery = discover_native_primary_plane_property_handles(
            card,
            selection.connector_handle(),
            selection.crtc_handle(),
            selection.plane_handle(),
        );
        let Some(properties) = discovery.properties else {
            return Err(NativeOutputHeadUnavailable::MissingProperties);
        };

        // A timing this connector never advertised is a configuration error, and it
        // fails here rather than as an opaque kernel refusal later.
        let Ok(Some(mode)) =
            resolve_native_connector_mode(card, selection.connector_handle(), timing)
        else {
            return Err(NativeOutputHeadUnavailable::UnknownTiming);
        };
        let Ok(mode_blob) = card.create_mode_blob(mode) else {
            return Err(NativeOutputHeadUnavailable::ModeBlobRefused);
        };
        if mode_blob == 0 {
            return Err(NativeOutputHeadUnavailable::ModeBlobRefused);
        }

        Ok(NativeOutputComposedHead {
            head: LibdrmNativeAtomicTopologyHead::from_selection(selection, mode_blob, properties),
            mode_blob,
        })
    }

    fn release_mode_blob(&self, output: OutputId, blob: u64) {
        // Release against the card that created it. Failing to release is worth a
        // line but not worth failing a validation that already completed.
        let Some(index) = self.scanout.output_index(output) else {
            return;
        };
        if let Err(error) = self.scanout.card(index).destroy_mode_blob(blob) {
            tracing::warn!(
                schema = 1,
                blob,
                %error,
                "sophia_native_output_mode_blob release failed"
            );
        }
    }
}
