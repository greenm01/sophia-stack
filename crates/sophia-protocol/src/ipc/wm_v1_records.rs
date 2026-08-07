use core::mem::size_of;

use crate::{
    LayoutNodeCapabilities, OutputId, PolicyOutputProjection, PolicyOutputSnapshot,
    PolicyProjectionOutcome, PolicyProjectionProposal, PolicySceneSnapshot, PolicySurfacePlacement,
    PolicySurfaceSnapshot, PolicyTransform, Rect, Size, SurfaceConstraints, SurfaceId,
    TransactionId, WmActionId, WmBindingRegistration, WmModifierMask,
};

use super::{
    IpcCodecError, PROJECTION_OUTPUT_RECORD_KIND, PROJECTION_PLACEMENT_RECORD_KIND,
    SNAPSHOT_BINDING_RECORD_KIND, SNAPSHOT_OUTPUT_RECORD_KIND, SNAPSHOT_SURFACE_RECORD_KIND,
    SOPHIA_WM_OUTCOME_COMMITTED, SOPHIA_WM_OUTCOME_DISCONNECTED,
    SOPHIA_WM_OUTCOME_REJECTED_INVALID, SOPHIA_WM_OUTCOME_REJECTED_STALE,
    SOPHIA_WM_OUTCOME_TIMED_OUT, WmV1ProjectionBegin, WmV1ProjectionChunk, WmV1ProjectionEnd,
    WmV1ProjectionOutcome, WmV1ProjectionOutputRecord, WmV1ProjectionPlacementRecord,
    WmV1ProjectionRequest, WmV1SnapshotBegin, WmV1SnapshotBindingRecord, WmV1SnapshotChunk,
    WmV1SnapshotEnd, WmV1SnapshotOutputRecord, WmV1SnapshotSurfaceRecord,
    decode_wm_v1_projection_output_records, decode_wm_v1_projection_placement_records,
    decode_wm_v1_snapshot_binding_records, decode_wm_v1_snapshot_output_records,
    decode_wm_v1_snapshot_surface_records, encode_wm_v1_projection_output_records,
    encode_wm_v1_projection_placement_records, encode_wm_v1_snapshot_binding_records,
    encode_wm_v1_snapshot_output_records, encode_wm_v1_snapshot_surface_records,
};

const OUTPUT_ID_WIRE_SIZE: usize = size_of::<u64>();

pub const POLICY_SURFACE_CAPABILITY_MOVABLE: u16 = 1 << 0;
pub const POLICY_SURFACE_CAPABILITY_RESIZABLE: u16 = 1 << 1;
pub const POLICY_SURFACE_CAPABILITY_FOCUSABLE: u16 = 1 << 2;
pub const POLICY_SURFACE_CAPABILITY_CLOSABLE: u16 = 1 << 3;
pub const POLICY_SURFACE_CAPABILITY_FULLSCREENABLE: u16 = 1 << 4;
const POLICY_SURFACE_CAPABILITY_SUPPORTED: u16 = POLICY_SURFACE_CAPABILITY_MOVABLE
    | POLICY_SURFACE_CAPABILITY_RESIZABLE
    | POLICY_SURFACE_CAPABILITY_FOCUSABLE
    | POLICY_SURFACE_CAPABILITY_CLOSABLE
    | POLICY_SURFACE_CAPABILITY_FULLSCREENABLE;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmV1SnapshotTransfer {
    pub transaction: TransactionId,
    pub begin: WmV1SnapshotBegin,
    pub chunks: Vec<WmV1SnapshotChunk>,
    pub end: WmV1SnapshotEnd,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmV1DecodedSnapshot {
    pub scene: PolicySceneSnapshot,
    pub bindings: Vec<WmBindingRegistration>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmV1ProjectionTransfer {
    pub transaction: TransactionId,
    pub begin: WmV1ProjectionBegin,
    pub chunks: Vec<WmV1ProjectionChunk>,
    pub end: WmV1ProjectionEnd,
}

pub fn encode_wm_v1_policy_projection_request(
    request: &crate::PolicyProjectionRequest,
) -> Result<WmV1ProjectionRequest, IpcCodecError> {
    if request.connection_epoch == 0 || request.request_id == 0 || request.scene_generation == 0 {
        return Err(invalid("projection_request_identity", 0));
    }
    if request.affected_outputs.is_empty()
        || request.affected_outputs.len() > crate::POLICY_MAX_OUTPUTS
    {
        return Err(IpcCodecError::CountTooLarge {
            count: request.affected_outputs.len(),
            max: crate::POLICY_MAX_OUTPUTS,
        });
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut affected_outputs =
        Vec::with_capacity(request.affected_outputs.len() * OUTPUT_ID_WIRE_SIZE);
    for output in &request.affected_outputs {
        if !output.is_valid() || !seen.insert(*output) {
            return Err(invalid("affected_output", output.raw() as u32));
        }
        affected_outputs.extend_from_slice(&output.raw().to_le_bytes());
    }
    Ok(WmV1ProjectionRequest {
        connection_epoch: request.connection_epoch,
        request_id: request.request_id,
        scene_generation: request.scene_generation,
        affected_output_count: request.affected_outputs.len() as u16,
        affected_outputs,
    })
}

pub fn decode_wm_v1_policy_projection_request(
    request: &WmV1ProjectionRequest,
) -> Result<crate::PolicyProjectionRequest, IpcCodecError> {
    let count = usize::from(request.affected_output_count);
    if request.connection_epoch == 0 || request.request_id == 0 || request.scene_generation == 0 {
        return Err(invalid("projection_request_identity", 0));
    }
    if count == 0 || count > crate::POLICY_MAX_OUTPUTS {
        return Err(IpcCodecError::CountTooLarge {
            count,
            max: crate::POLICY_MAX_OUTPUTS,
        });
    }
    if request.affected_outputs.len() != count * OUTPUT_ID_WIRE_SIZE {
        return Err(invalid(
            "affected_output_bytes",
            request.affected_outputs.len() as u32,
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut affected_outputs = Vec::with_capacity(count);
    for bytes in request.affected_outputs.chunks_exact(OUTPUT_ID_WIRE_SIZE) {
        let output = OutputId::from_raw(u64::from_le_bytes(
            bytes.try_into().expect("fixed output-id chunk"),
        ));
        if !output.is_valid() || !seen.insert(output) {
            return Err(invalid("affected_output", output.raw() as u32));
        }
        affected_outputs.push(output);
    }
    Ok(crate::PolicyProjectionRequest {
        connection_epoch: request.connection_epoch,
        request_id: request.request_id,
        scene_generation: request.scene_generation,
        affected_outputs,
    })
}

pub fn encode_wm_v1_policy_projection_outcome(
    connection_epoch: u64,
    request_id: u64,
    scene_generation: u64,
    outcome: PolicyProjectionOutcome,
) -> Result<WmV1ProjectionOutcome, IpcCodecError> {
    if connection_epoch == 0 || request_id == 0 || scene_generation == 0 {
        return Err(invalid("projection_outcome_identity", 0));
    }
    Ok(WmV1ProjectionOutcome {
        connection_epoch,
        request_id,
        scene_generation,
        outcome: match outcome {
            PolicyProjectionOutcome::Committed => SOPHIA_WM_OUTCOME_COMMITTED,
            PolicyProjectionOutcome::RejectedStale => SOPHIA_WM_OUTCOME_REJECTED_STALE,
            PolicyProjectionOutcome::RejectedInvalid => SOPHIA_WM_OUTCOME_REJECTED_INVALID,
            PolicyProjectionOutcome::TimedOut => SOPHIA_WM_OUTCOME_TIMED_OUT,
            PolicyProjectionOutcome::Disconnected => SOPHIA_WM_OUTCOME_DISCONNECTED,
        },
    })
}

pub fn decode_wm_v1_policy_projection_outcome(
    outcome: &WmV1ProjectionOutcome,
) -> Result<PolicyProjectionOutcome, IpcCodecError> {
    if outcome.connection_epoch == 0 || outcome.request_id == 0 || outcome.scene_generation == 0 {
        return Err(invalid("projection_outcome_identity", 0));
    }
    match outcome.outcome {
        SOPHIA_WM_OUTCOME_COMMITTED => Ok(PolicyProjectionOutcome::Committed),
        SOPHIA_WM_OUTCOME_REJECTED_STALE => Ok(PolicyProjectionOutcome::RejectedStale),
        SOPHIA_WM_OUTCOME_REJECTED_INVALID => Ok(PolicyProjectionOutcome::RejectedInvalid),
        SOPHIA_WM_OUTCOME_TIMED_OUT => Ok(PolicyProjectionOutcome::TimedOut),
        SOPHIA_WM_OUTCOME_DISCONNECTED => Ok(PolicyProjectionOutcome::Disconnected),
        other => Err(invalid("projection_outcome", u32::from(other))),
    }
}

pub fn encode_wm_v1_policy_snapshot(
    transaction: TransactionId,
    connection_epoch: u64,
    scene: &PolicySceneSnapshot,
    bindings: &[WmBindingRegistration],
) -> Result<WmV1SnapshotTransfer, IpcCodecError> {
    if !transaction.is_valid() {
        return Err(IpcCodecError::InvalidTransaction(0));
    }
    let outputs = scene
        .outputs
        .iter()
        .map(|output| {
            let (focus_index, focus_generation) = output
                .focus
                .map(|surface| (surface.index(), surface.generation()))
                .unwrap_or((0, 0));
            WmV1SnapshotOutputRecord {
                output: output.output.raw(),
                generation: output.generation,
                focus_index,
                focus_generation,
                x: output.bounds.x,
                y: output.bounds.y,
                width: output.bounds.width,
                height: output.bounds.height,
            }
        })
        .collect::<Vec<_>>();
    let surfaces = scene
        .surfaces
        .iter()
        .map(encode_surface_record)
        .collect::<Vec<_>>();
    let bindings = bindings
        .iter()
        .map(|binding| WmV1SnapshotBindingRecord {
            action: binding.action.raw(),
            keycode: binding.keycode,
            modifier_bits: binding.modifiers.bits,
        })
        .collect::<Vec<_>>();
    let mut chunks = Vec::new();
    push_snapshot_chunk(
        &mut chunks,
        connection_epoch,
        SNAPSHOT_OUTPUT_RECORD_KIND,
        outputs.len(),
        encode_wm_v1_snapshot_output_records(&outputs)?,
    )?;
    push_snapshot_chunk(
        &mut chunks,
        connection_epoch,
        SNAPSHOT_SURFACE_RECORD_KIND,
        surfaces.len(),
        encode_wm_v1_snapshot_surface_records(&surfaces)?,
    )?;
    push_snapshot_chunk(
        &mut chunks,
        connection_epoch,
        SNAPSHOT_BINDING_RECORD_KIND,
        bindings.len(),
        encode_wm_v1_snapshot_binding_records(&bindings)?,
    )?;
    let chunk_count = u16::try_from(chunks.len()).map_err(|_| IpcCodecError::CountTooLarge {
        count: chunks.len(),
        max: u16::MAX as usize,
    })?;
    let begin = WmV1SnapshotBegin {
        connection_epoch,
        scene_generation: scene.generation,
        chunk_count,
        output_count: u16::try_from(outputs.len()).map_err(|_| IpcCodecError::CountTooLarge {
            count: outputs.len(),
            max: u16::MAX as usize,
        })?,
        surface_count: surfaces.len() as u32,
        binding_count: bindings.len() as u16,
    };
    let end = WmV1SnapshotEnd {
        connection_epoch,
        scene_generation: scene.generation,
        chunk_count,
    };
    Ok(WmV1SnapshotTransfer {
        transaction,
        begin,
        chunks,
        end,
    })
}

pub fn decode_wm_v1_policy_snapshot(
    transfer: &WmV1SnapshotTransfer,
) -> Result<WmV1DecodedSnapshot, IpcCodecError> {
    if !transfer.transaction.is_valid() {
        return Err(IpcCodecError::InvalidTransaction(0));
    }
    if transfer.begin.connection_epoch != transfer.end.connection_epoch
        || transfer.begin.scene_generation != transfer.end.scene_generation
        || transfer.begin.chunk_count != transfer.end.chunk_count
        || usize::from(transfer.begin.chunk_count) != transfer.chunks.len()
    {
        return Err(invalid("snapshot_transfer", 0));
    }
    let mut outputs = Vec::new();
    let mut surfaces = Vec::new();
    let mut bindings = Vec::new();
    for (ordinal, chunk) in transfer.chunks.iter().enumerate() {
        if chunk.connection_epoch != transfer.begin.connection_epoch
            || usize::from(chunk.ordinal) != ordinal
        {
            return Err(invalid("snapshot_chunk_identity", chunk.ordinal as u32));
        }
        match chunk.record_kind {
            SNAPSHOT_OUTPUT_RECORD_KIND => outputs.extend(decode_wm_v1_snapshot_output_records(
                &chunk.data,
                chunk.item_count,
            )?),
            SNAPSHOT_SURFACE_RECORD_KIND => surfaces.extend(decode_wm_v1_snapshot_surface_records(
                &chunk.data,
                chunk.item_count,
            )?),
            SNAPSHOT_BINDING_RECORD_KIND => bindings.extend(decode_wm_v1_snapshot_binding_records(
                &chunk.data,
                chunk.item_count,
            )?),
            other => return Err(invalid("snapshot_record_kind", u32::from(other))),
        }
    }
    require_count(outputs.len(), transfer.begin.output_count as usize)?;
    require_count(surfaces.len(), transfer.begin.surface_count as usize)?;
    require_count(bindings.len(), transfer.begin.binding_count as usize)?;
    Ok(WmV1DecodedSnapshot {
        scene: PolicySceneSnapshot {
            generation: transfer.begin.scene_generation,
            outputs: outputs
                .into_iter()
                .map(|record| {
                    Ok(PolicyOutputSnapshot {
                        output: OutputId::from_raw(record.output),
                        generation: record.generation,
                        focus: decode_optional_surface(
                            record.focus_index,
                            record.focus_generation,
                            "output_focus",
                        )?,
                        bounds: Rect {
                            x: record.x,
                            y: record.y,
                            width: record.width,
                            height: record.height,
                        },
                    })
                })
                .collect::<Result<Vec<_>, IpcCodecError>>()?,
            surfaces: surfaces
                .into_iter()
                .map(decode_surface_record)
                .collect::<Result<Vec<_>, _>>()?,
        },
        bindings: bindings
            .into_iter()
            .map(|record| WmBindingRegistration {
                action: WmActionId::from_raw(record.action),
                keycode: record.keycode,
                modifiers: WmModifierMask {
                    bits: record.modifier_bits,
                },
            })
            .collect(),
    })
}

pub fn encode_wm_v1_policy_projection(
    proposal: &PolicyProjectionProposal,
) -> Result<WmV1ProjectionTransfer, IpcCodecError> {
    if !proposal.transaction.is_valid() {
        return Err(IpcCodecError::InvalidTransaction(0));
    }
    let outputs = proposal
        .outputs
        .iter()
        .map(|output| {
            let (focus_index, focus_generation) = output
                .focus
                .map(|surface| (surface.index(), surface.generation()))
                .unwrap_or((0, 0));
            Ok(WmV1ProjectionOutputRecord {
                output: output.output.raw(),
                placement_count: u32::try_from(output.placements.len()).map_err(|_| {
                    IpcCodecError::CountTooLarge {
                        count: output.placements.len(),
                        max: u32::MAX as usize,
                    }
                })?,
                focus_index,
                focus_generation,
            })
        })
        .collect::<Result<Vec<_>, IpcCodecError>>()?;
    let placements = proposal
        .outputs
        .iter()
        .flat_map(|output| output.placements.iter())
        .map(encode_placement_record)
        .collect::<Vec<_>>();
    let mut chunks = Vec::new();
    push_projection_chunk(
        &mut chunks,
        proposal.connection_epoch,
        PROJECTION_OUTPUT_RECORD_KIND,
        outputs.len(),
        encode_wm_v1_projection_output_records(&outputs)?,
    )?;
    push_projection_chunk(
        &mut chunks,
        proposal.connection_epoch,
        PROJECTION_PLACEMENT_RECORD_KIND,
        placements.len(),
        encode_wm_v1_projection_placement_records(&placements)?,
    )?;
    let chunk_count = chunks.len() as u16;
    let begin = WmV1ProjectionBegin {
        connection_epoch: proposal.connection_epoch,
        request_id: proposal.request_id,
        base_generation: proposal.base_generation,
        chunk_count,
        output_count: outputs.len() as u16,
        placement_count: placements.len() as u32,
    };
    let end = WmV1ProjectionEnd {
        connection_epoch: proposal.connection_epoch,
        request_id: proposal.request_id,
        base_generation: proposal.base_generation,
        chunk_count,
    };
    Ok(WmV1ProjectionTransfer {
        transaction: proposal.transaction,
        begin,
        chunks,
        end,
    })
}

pub fn decode_wm_v1_policy_projection(
    transfer: &WmV1ProjectionTransfer,
) -> Result<PolicyProjectionProposal, IpcCodecError> {
    if !transfer.transaction.is_valid() {
        return Err(IpcCodecError::InvalidTransaction(0));
    }
    if transfer.begin.connection_epoch != transfer.end.connection_epoch
        || transfer.begin.request_id != transfer.end.request_id
        || transfer.begin.base_generation != transfer.end.base_generation
        || transfer.begin.chunk_count != transfer.end.chunk_count
        || usize::from(transfer.begin.chunk_count) != transfer.chunks.len()
    {
        return Err(invalid("projection_transfer", 0));
    }
    let mut outputs = Vec::new();
    let mut placements = Vec::new();
    for (ordinal, chunk) in transfer.chunks.iter().enumerate() {
        if chunk.connection_epoch != transfer.begin.connection_epoch
            || usize::from(chunk.ordinal) != ordinal
        {
            return Err(invalid("projection_chunk_identity", chunk.ordinal as u32));
        }
        match chunk.record_kind {
            PROJECTION_OUTPUT_RECORD_KIND => outputs.extend(
                decode_wm_v1_projection_output_records(&chunk.data, chunk.item_count)?,
            ),
            PROJECTION_PLACEMENT_RECORD_KIND => placements.extend(
                decode_wm_v1_projection_placement_records(&chunk.data, chunk.item_count)?,
            ),
            other => return Err(invalid("projection_record_kind", u32::from(other))),
        }
    }
    require_count(outputs.len(), transfer.begin.output_count as usize)?;
    require_count(placements.len(), transfer.begin.placement_count as usize)?;
    let mut placement_cursor = placements.into_iter();
    let mut projected_outputs = Vec::with_capacity(outputs.len());
    for output in outputs {
        let focus = decode_optional_surface(output.focus_index, output.focus_generation, "focus")?;
        let mut projected = Vec::with_capacity(output.placement_count as usize);
        for _ in 0..output.placement_count {
            let record = placement_cursor
                .next()
                .ok_or_else(|| invalid("placement_count", output.placement_count))?;
            projected.push(decode_placement_record(record)?);
        }
        projected_outputs.push(PolicyOutputProjection {
            output: OutputId::from_raw(output.output),
            placements: projected,
            focus,
        });
    }
    if placement_cursor.next().is_some() {
        return Err(invalid("placement_count", transfer.begin.placement_count));
    }
    Ok(PolicyProjectionProposal {
        transaction: transfer.transaction,
        connection_epoch: transfer.begin.connection_epoch,
        request_id: transfer.begin.request_id,
        base_generation: transfer.begin.base_generation,
        outputs: projected_outputs,
    })
}

fn push_snapshot_chunk(
    chunks: &mut Vec<WmV1SnapshotChunk>,
    connection_epoch: u64,
    record_kind: u16,
    count: usize,
    data: Vec<u8>,
) -> Result<(), IpcCodecError> {
    if count == 0 {
        return Ok(());
    }
    chunks.push(WmV1SnapshotChunk {
        connection_epoch,
        ordinal: chunks.len() as u16,
        record_kind,
        item_count: u32::try_from(count).map_err(|_| IpcCodecError::CountTooLarge {
            count,
            max: u32::MAX as usize,
        })?,
        data,
    });
    Ok(())
}

fn push_projection_chunk(
    chunks: &mut Vec<WmV1ProjectionChunk>,
    connection_epoch: u64,
    record_kind: u16,
    count: usize,
    data: Vec<u8>,
) -> Result<(), IpcCodecError> {
    if count == 0 {
        return Ok(());
    }
    chunks.push(WmV1ProjectionChunk {
        connection_epoch,
        ordinal: chunks.len() as u16,
        record_kind,
        item_count: u32::try_from(count).map_err(|_| IpcCodecError::CountTooLarge {
            count,
            max: u32::MAX as usize,
        })?,
        data,
    });
    Ok(())
}

fn encode_surface_record(surface: &PolicySurfaceSnapshot) -> WmV1SnapshotSurfaceRecord {
    let mut capability_bits = 0;
    capability_bits |= u16::from(surface.capabilities.movable) * POLICY_SURFACE_CAPABILITY_MOVABLE;
    capability_bits |=
        u16::from(surface.capabilities.resizable) * POLICY_SURFACE_CAPABILITY_RESIZABLE;
    capability_bits |=
        u16::from(surface.capabilities.focusable) * POLICY_SURFACE_CAPABILITY_FOCUSABLE;
    capability_bits |=
        u16::from(surface.capabilities.closable) * POLICY_SURFACE_CAPABILITY_CLOSABLE;
    capability_bits |=
        u16::from(surface.capabilities.fullscreenable) * POLICY_SURFACE_CAPABILITY_FULLSCREENABLE;
    let (transient_index, transient_generation) = surface
        .transient_owner
        .map(|owner| (owner.index(), owner.generation()))
        .unwrap_or((0, 0));
    let (min_width, min_height) = encode_optional_size(surface.constraints.min_size);
    let (max_width, max_height) = encode_optional_size(surface.constraints.max_size);
    WmV1SnapshotSurfaceRecord {
        surface_index: surface.surface.index(),
        surface_generation: surface.surface.generation(),
        state_generation: surface.generation,
        current_output: surface.current_output.map_or(0, OutputId::raw),
        capability_bits,
        transient_index,
        transient_generation,
        x: surface.geometry.x,
        y: surface.geometry.y,
        width: surface.geometry.width,
        height: surface.geometry.height,
        min_width,
        min_height,
        max_width,
        max_height,
    }
}

fn decode_surface_record(
    record: WmV1SnapshotSurfaceRecord,
) -> Result<PolicySurfaceSnapshot, IpcCodecError> {
    if record.capability_bits & !POLICY_SURFACE_CAPABILITY_SUPPORTED != 0 {
        return Err(invalid(
            "surface_capabilities",
            u32::from(record.capability_bits),
        ));
    }
    Ok(PolicySurfaceSnapshot {
        surface: SurfaceId::new(record.surface_index, record.surface_generation),
        generation: record.state_generation,
        current_output: (record.current_output != 0)
            .then(|| OutputId::from_raw(record.current_output)),
        capabilities: LayoutNodeCapabilities {
            movable: record.capability_bits & POLICY_SURFACE_CAPABILITY_MOVABLE != 0,
            resizable: record.capability_bits & POLICY_SURFACE_CAPABILITY_RESIZABLE != 0,
            focusable: record.capability_bits & POLICY_SURFACE_CAPABILITY_FOCUSABLE != 0,
            closable: record.capability_bits & POLICY_SURFACE_CAPABILITY_CLOSABLE != 0,
            fullscreenable: record.capability_bits & POLICY_SURFACE_CAPABILITY_FULLSCREENABLE != 0,
        },
        constraints: SurfaceConstraints {
            min_size: decode_optional_size(record.min_width, record.min_height, "min_size")?,
            max_size: decode_optional_size(record.max_width, record.max_height, "max_size")?,
        },
        transient_owner: decode_optional_surface(
            record.transient_index,
            record.transient_generation,
            "transient_owner",
        )?,
        geometry: Rect {
            x: record.x,
            y: record.y,
            width: record.width,
            height: record.height,
        },
    })
}

fn encode_placement_record(placement: &PolicySurfacePlacement) -> WmV1ProjectionPlacementRecord {
    let (requested_width, requested_height) = encode_optional_size(placement.requested_size);
    let crop = placement.crop.unwrap_or_default();
    WmV1ProjectionPlacementRecord {
        surface_index: placement.surface.index(),
        surface_generation: placement.surface.generation(),
        state_generation: placement.surface_generation,
        x: placement.geometry.x,
        y: placement.geometry.y,
        width: placement.geometry.width,
        height: placement.geometry.height,
        requested_width,
        requested_height,
        crop_x: crop.x,
        crop_y: crop.y,
        crop_width: crop.width,
        crop_height: crop.height,
        transform: placement.transform as u16,
    }
}

fn decode_placement_record(
    record: WmV1ProjectionPlacementRecord,
) -> Result<PolicySurfacePlacement, IpcCodecError> {
    let crop = if record.crop_width == 0 && record.crop_height == 0 {
        if record.crop_x != 0 || record.crop_y != 0 {
            return Err(invalid("crop", 0));
        }
        None
    } else if record.crop_width > 0 && record.crop_height > 0 {
        Some(Rect {
            x: record.crop_x,
            y: record.crop_y,
            width: record.crop_width,
            height: record.crop_height,
        })
    } else {
        return Err(invalid("crop", 0));
    };
    Ok(PolicySurfacePlacement {
        surface: SurfaceId::new(record.surface_index, record.surface_generation),
        surface_generation: record.state_generation,
        geometry: Rect {
            x: record.x,
            y: record.y,
            width: record.width,
            height: record.height,
        },
        requested_size: decode_optional_size(
            record.requested_width,
            record.requested_height,
            "requested_size",
        )?,
        crop,
        transform: match record.transform {
            1 => PolicyTransform::Identity,
            other => return Err(invalid("policy_transform", u32::from(other))),
        },
    })
}

fn encode_optional_size(size: Option<Size>) -> (i32, i32) {
    size.map(|size| (size.width, size.height)).unwrap_or((0, 0))
}

fn decode_optional_size(
    width: i32,
    height: i32,
    field: &'static str,
) -> Result<Option<Size>, IpcCodecError> {
    if width == 0 && height == 0 {
        Ok(None)
    } else if width > 0 && height > 0 {
        Ok(Some(Size { width, height }))
    } else {
        Err(invalid(field, 0))
    }
}

fn decode_optional_surface(
    index: u32,
    generation: u32,
    field: &'static str,
) -> Result<Option<SurfaceId>, IpcCodecError> {
    if index == 0 && generation == 0 {
        Ok(None)
    } else if generation != 0 {
        Ok(Some(SurfaceId::new(index, generation)))
    } else {
        Err(invalid(field, index))
    }
}

fn require_count(actual: usize, expected: usize) -> Result<(), IpcCodecError> {
    if actual == expected {
        Ok(())
    } else {
        Err(invalid("record_count", actual as u32))
    }
}

fn invalid(field: &'static str, value: u32) -> IpcCodecError {
    IpcCodecError::InvalidEnum { field, value }
}
