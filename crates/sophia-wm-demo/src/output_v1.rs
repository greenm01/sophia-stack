use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use sophia_protocol::{
    IpcCodecError, OutputAuthoritySnapshot, OutputGroupMember, OutputHeadDescriptor,
    OutputHeadMapping, OutputHeadTargetProposal, OutputLogicalGroupProposal,
    OutputLogicalGroupState, OutputTopologyCandidate, OutputTopologyCandidateError,
    OutputTopologyIntent, OutputTransform, OutputV1ClientHello, OutputV1Outcome,
    OutputV1OutcomeKind, OutputV1Proposal, OutputVrrPolicy, Rect, SOPHIA_IPC_HEADER_LEN,
    SOPHIA_IPC_MAX_PAYLOAD_LEN, SOPHIA_OUTPUT_CAPABILITY_CONFIGURE,
    SOPHIA_OUTPUT_CAPABILITY_OBSERVE, SOPHIA_OUTPUT_INTERFACE_REVISION, TransactionId,
    decode_output_v1_outcome_frame, decode_output_v1_server_welcome_frame,
    decode_output_v1_snapshot_frame, encode_output_v1_client_hello_frame,
    encode_output_v1_proposal_frame,
};

#[derive(Debug)]
pub enum OutputV1ClientError {
    Io(std::io::Error),
    Codec(IpcCodecError),
    Candidate(OutputTopologyCandidateError),
    UnsupportedRevision(u16),
    MissingCapability,
    InvalidWelcome,
    ConnectionEpochMismatch,
    TransactionMismatch,
    NonCommittedOutcome(OutputV1OutcomeKind),
    InvalidProofTopology(&'static str),
    TransactionExhausted,
}

impl core::fmt::Display for OutputV1ClientError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for OutputV1ClientError {}

impl From<std::io::Error> for OutputV1ClientError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<IpcCodecError> for OutputV1ClientError {
    fn from(error: IpcCodecError) -> Self {
        Self::Codec(error)
    }
}

impl From<OutputTopologyCandidateError> for OutputV1ClientError {
    fn from(error: OutputTopologyCandidateError) -> Self {
        Self::Candidate(error)
    }
}

/// Reference client for the exclusive physical-output role.
///
/// Physical head labels remain private to this role. The policy connection
/// still receives only logical outputs and opaque surface identities.
pub struct OutputV1Client {
    stream: UnixStream,
    connection_epoch: u64,
    max_heads: usize,
    max_groups: usize,
    max_modes_per_head: usize,
    max_heads_per_group: usize,
    next_transaction: u64,
}

impl OutputV1Client {
    pub fn connect(path: impl AsRef<Path>, timeout: Duration) -> Result<Self, OutputV1ClientError> {
        let mut stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        stream.write_all(&encode_output_v1_client_hello_frame(OutputV1ClientHello {
            minimum_revision: SOPHIA_OUTPUT_INTERFACE_REVISION,
            maximum_revision: SOPHIA_OUTPUT_INTERFACE_REVISION,
            capabilities: SOPHIA_OUTPUT_CAPABILITY_OBSERVE | SOPHIA_OUTPUT_CAPABILITY_CONFIGURE,
        })?)?;
        stream.flush()?;
        let welcome = decode_output_v1_server_welcome_frame(&read_frame(&mut stream)?)?;
        if welcome.selected_revision != SOPHIA_OUTPUT_INTERFACE_REVISION {
            return Err(OutputV1ClientError::UnsupportedRevision(
                welcome.selected_revision,
            ));
        }
        let required = SOPHIA_OUTPUT_CAPABILITY_OBSERVE | SOPHIA_OUTPUT_CAPABILITY_CONFIGURE;
        if welcome.capabilities & required != required {
            return Err(OutputV1ClientError::MissingCapability);
        }
        if welcome.connection_epoch == 0
            || welcome.max_heads < 3
            || welcome.max_groups < 2
            || welcome.max_modes_per_head == 0
            || welcome.max_heads_per_group < 2
        {
            return Err(OutputV1ClientError::InvalidWelcome);
        }
        Ok(Self {
            stream,
            connection_epoch: welcome.connection_epoch,
            max_heads: usize::from(welcome.max_heads),
            max_groups: usize::from(welcome.max_groups),
            max_modes_per_head: usize::from(welcome.max_modes_per_head),
            max_heads_per_group: usize::from(welcome.max_heads_per_group),
            next_transaction: 1,
        })
    }

    pub fn receive_snapshot(
        &mut self,
    ) -> Result<(TransactionId, OutputAuthoritySnapshot), OutputV1ClientError> {
        let (transaction, message) =
            decode_output_v1_snapshot_frame(&read_frame(&mut self.stream)?)?;
        if message.connection_epoch != self.connection_epoch {
            return Err(OutputV1ClientError::ConnectionEpochMismatch);
        }
        if message.snapshot.heads.len() > self.max_heads
            || message.snapshot.groups.len() > self.max_groups
            || message
                .snapshot
                .heads
                .iter()
                .any(|head| head.modes.len() > self.max_modes_per_head)
            || message
                .snapshot
                .groups
                .iter()
                .any(|group| group.members.len() > self.max_heads_per_group)
        {
            return Err(OutputV1ClientError::InvalidWelcome);
        }
        message.snapshot.validate()?;
        Ok((transaction, message.snapshot))
    }

    pub fn submit(
        &mut self,
        candidate: OutputTopologyCandidate,
        snapshot: &OutputAuthoritySnapshot,
    ) -> Result<OutputV1Outcome, OutputV1ClientError> {
        candidate.validate_against(snapshot)?;
        let transaction = TransactionId::from_raw(self.next_transaction);
        self.next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or(OutputV1ClientError::TransactionExhausted)?;
        let frame = encode_output_v1_proposal_frame(
            transaction,
            &OutputV1Proposal {
                connection_epoch: self.connection_epoch,
                candidate,
            },
        )?;
        self.stream.write_all(&frame)?;
        self.stream.flush()?;
        // A snapshot is an unsolicited update and may arrive at any moment,
        // including between a proposal and the outcome answering it. Reading
        // the next frame as an outcome regardless once turned a published
        // topology into a decode failure and took the session down with it, so
        // updates are consumed while waiting rather than tripped over.
        let (outcome_transaction, outcome) = loop {
            let frame = read_frame(&mut self.stream)?;
            match decode_output_v1_snapshot_frame(&frame) {
                Ok(_) => continue,
                Err(_) => break decode_output_v1_outcome_frame(&frame)?,
            }
        };
        if outcome_transaction != transaction {
            return Err(OutputV1ClientError::TransactionMismatch);
        }
        if outcome.connection_epoch != self.connection_epoch {
            return Err(OutputV1ClientError::ConnectionEpochMismatch);
        }
        Ok(outcome)
    }

    pub const fn connection_epoch(&self) -> u64 {
        self.connection_epoch
    }
}

/// Which head of a mirror group keeps its own pixels.
///
/// A mirror group has one logical size and its members are placed into it, so
/// exactly one member can be exact and the rest are resampled to reach it.
/// Choosing that member is the whole of what macOS calls "optimize for
/// <display>", and it is a property of the group rather than a mode change:
/// every head keeps its own mode either way.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MirrorOptimizedHead {
    /// The larger panel stays pixel-exact and the member resamples down.
    #[default]
    Primary,
    /// The member stays pixel-exact and the primary resamples up.
    Member,
}

/// Builds a complete three-head candidate: two heads mirror one logical output,
/// and the third extends it to the right.
///
/// The proof shape is deliberately exact. Extra connected heads are rejected
/// instead of being disabled as a side effect, and all three heads keep their
/// current modes. The extended head is always exact; within the mirror group,
/// the optimized head is exact and the other member fits itself to the group's
/// logical size, which is that optimized head's own mode.
pub fn mixed_mirror_extended_candidate(
    snapshot: &OutputAuthoritySnapshot,
    mirror_primary_label: &str,
    mirror_member_label: &str,
    extended_label: &str,
    optimized: MirrorOptimizedHead,
) -> Result<OutputTopologyCandidate, OutputV1ClientError> {
    snapshot.validate()?;
    if mirror_primary_label == mirror_member_label
        || mirror_primary_label == extended_label
        || mirror_member_label == extended_label
    {
        return Err(OutputV1ClientError::InvalidProofTopology(
            "proof labels are not distinct",
        ));
    }
    let connected = snapshot
        .heads
        .iter()
        .filter(|head| head.connected)
        .collect::<Vec<_>>();
    if connected.len() != 3 {
        return Err(OutputV1ClientError::InvalidProofTopology(
            "proof requires exactly three connected heads",
        ));
    }
    let primary = head_by_label(&connected, mirror_primary_label)?;
    let member = head_by_label(&connected, mirror_member_label)?;
    let extended = head_by_label(&connected, extended_label)?;
    for head in [primary, member, extended] {
        if !head.enabled || head.current_mode.is_none() {
            return Err(OutputV1ClientError::InvalidProofTopology(
                "proof head is not enabled with a current mode",
            ));
        }
        if !head.transforms.contains(OutputTransform::Normal) {
            return Err(OutputV1ClientError::InvalidProofTopology(
                "proof head does not support the normal transform",
            ));
        }
    }

    let primary_group = group_for_head(snapshot, primary)?;
    let member_group = group_for_head(snapshot, member)?;
    let extended_group = group_for_head(snapshot, extended)?;
    if primary_group.output == extended_group.output {
        return Err(OutputV1ClientError::InvalidProofTopology(
            "mirror primary and extended head already share one logical output",
        ));
    }
    // A pre-existing mirror is acceptable, but an unrelated shared identity is
    // not: consuming it would silently remove another logical placement.
    if member_group.output != primary_group.output && member_group.output == extended_group.output {
        return Err(OutputV1ClientError::InvalidProofTopology(
            "mirror member currently belongs to the extended output",
        ));
    }

    // The group is sized by the head it is optimized for, so that head places
    // exactly and the other reaches the same logical size by resampling.
    let optimized_size = current_mode_pixel_size(match optimized {
        MirrorOptimizedHead::Primary => primary,
        MirrorOptimizedHead::Member => member,
    })?;
    let mirror_logical = Rect {
        x: primary_group.logical.x,
        y: primary_group.logical.y,
        width: optimized_size.width,
        height: optimized_size.height,
    };
    let extended_x = mirror_logical.x.checked_add(mirror_logical.width).ok_or(
        OutputV1ClientError::InvalidProofTopology("extended placement overflows root coordinates"),
    )?;
    let (primary_mapping, member_mapping) = match optimized {
        MirrorOptimizedHead::Primary => (OutputHeadMapping::Exact, OutputHeadMapping::Fit),
        MirrorOptimizedHead::Member => (OutputHeadMapping::Fit, OutputHeadMapping::Exact),
    };
    let targets = [primary, member, extended]
        .into_iter()
        .map(|head| OutputHeadTargetProposal {
            head: head.head,
            head_generation: head.generation,
            mode: head
                .current_mode
                .expect("enabled proof head has a current mode"),
            transform: OutputTransform::Normal,
            vrr: OutputVrrPolicy::Disabled,
        })
        .collect::<Vec<_>>();
    let candidate = OutputTopologyCandidate {
        base_topology_epoch: snapshot.topology_epoch,
        intent: OutputTopologyIntent::Apply,
        primary_group_index: 0,
        heads: targets,
        groups: vec![
            OutputLogicalGroupProposal {
                output: primary_group.output,
                logical: mirror_logical,
                members: vec![
                    OutputGroupMember {
                        head: primary.head,
                        mapping: primary_mapping,
                    },
                    OutputGroupMember {
                        head: member.head,
                        mapping: member_mapping,
                    },
                ],
            },
            OutputLogicalGroupProposal {
                output: extended_group.output,
                logical: Rect {
                    x: extended_x,
                    y: mirror_logical.y,
                    width: extended_group.logical.width,
                    height: extended_group.logical.height,
                },
                members: vec![OutputGroupMember {
                    head: extended.head,
                    mapping: OutputHeadMapping::Exact,
                }],
            },
        ],
    };
    candidate.validate_against(snapshot)?;
    Ok(candidate)
}

/// Whether the snapshot already shows the topology this proof asks for.
///
/// A supervised policy is restarted, and a restart lands after the topology it
/// applied is already live. Re-submitting the candidate it built the first time
/// names a base epoch the compositor has moved past, which is refused as stale
/// -- correctly, and fatally for a proof that reads any non-commit as failure.
/// So the proof asks first whether the desk already looks the way it wants,
/// which is the only question that survives being asked twice.
pub fn mixed_mirror_extended_topology_is_applied(
    snapshot: &OutputAuthoritySnapshot,
    mirror_primary_label: &str,
    mirror_member_label: &str,
    extended_label: &str,
    optimized: MirrorOptimizedHead,
) -> bool {
    let connected = snapshot
        .heads
        .iter()
        .filter(|head| head.connected)
        .collect::<Vec<_>>();
    let (Ok(primary), Ok(member), Ok(extended)) = (
        head_by_label(&connected, mirror_primary_label),
        head_by_label(&connected, mirror_member_label),
        head_by_label(&connected, extended_label),
    ) else {
        return false;
    };
    let (Ok(primary_group), Ok(member_group), Ok(extended_group)) = (
        group_for_head(snapshot, primary),
        group_for_head(snapshot, member),
        group_for_head(snapshot, extended),
    ) else {
        return false;
    };
    if primary_group.output != member_group.output
        || primary_group.output == extended_group.output
        || extended_group.members.len() != 1
    {
        return false;
    }
    let (primary_mapping, member_mapping) = match optimized {
        MirrorOptimizedHead::Primary => (OutputHeadMapping::Exact, OutputHeadMapping::Fit),
        MirrorOptimizedHead::Member => (OutputHeadMapping::Fit, OutputHeadMapping::Exact),
    };
    let mapping_of = |head: &OutputHeadDescriptor| {
        primary_group
            .members
            .iter()
            .find(|candidate| candidate.head == head.head)
            .map(|candidate| candidate.mapping)
    };
    mapping_of(primary) == Some(primary_mapping) && mapping_of(member) == Some(member_mapping)
}

/// The pixel size of the mode this head is currently running.
fn current_mode_pixel_size(
    head: &OutputHeadDescriptor,
) -> Result<sophia_protocol::Size, OutputV1ClientError> {
    let mode = head
        .current_mode
        .ok_or(OutputV1ClientError::InvalidProofTopology(
            "proof head is not enabled with a current mode",
        ))?;
    head.modes
        .iter()
        .find(|descriptor| descriptor.mode == mode)
        .map(|descriptor| descriptor.pixel_size)
        .ok_or(OutputV1ClientError::InvalidProofTopology(
            "proof head reports a current mode it does not advertise",
        ))
}

fn head_by_label<'a>(
    heads: &[&'a OutputHeadDescriptor],
    label: &str,
) -> Result<&'a OutputHeadDescriptor, OutputV1ClientError> {
    let mut matches = heads.iter().copied().filter(|head| head.label == label);
    let head = matches
        .next()
        .ok_or(OutputV1ClientError::InvalidProofTopology(
            "proof label is absent",
        ))?;
    if matches.next().is_some() {
        return Err(OutputV1ClientError::InvalidProofTopology(
            "proof label is ambiguous",
        ));
    }
    Ok(head)
}

fn group_for_head<'a>(
    snapshot: &'a OutputAuthoritySnapshot,
    head: &OutputHeadDescriptor,
) -> Result<&'a OutputLogicalGroupState, OutputV1ClientError> {
    snapshot
        .groups
        .iter()
        .find(|group| group.members.iter().any(|member| member.head == head.head))
        .ok_or(OutputV1ClientError::InvalidProofTopology(
            "proof head has no logical output",
        ))
}

fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>, OutputV1ClientError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    stream.read_exact(&mut header)?;
    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed frame payload range is present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(OutputV1ClientError::Codec(IpcCodecError::PayloadTooLarge(
            payload_len,
        )));
    }
    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream.read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])?;
    Ok(frame)
}
