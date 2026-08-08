use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use sophia_protocol::{
    IpcCodecError, POLICY_MAX_OUTPUTS, PolicyOutputProjection, PolicyProjectionOutcome,
    PolicyProjectionProposal, PolicyProjectionRequest, PolicySceneSnapshot, PolicySurfacePlacement,
    PolicyTransform, Rect, SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN,
    SOPHIA_WM_CAPABILITY_ACTIONS, SOPHIA_WM_CAPABILITY_BINDINGS, SOPHIA_WM_CAPABILITY_MULTI_OUTPUT,
    SOPHIA_WM_MAX_BINDINGS, SOPHIA_WM_MAX_OUTPUTS, SOPHIA_WM_MAX_SURFACES, Size, TransactionId,
    WmV1ClientHello, WmV1DecodedSnapshot, WmV1SnapshotTransfer,
    decode_wm_v1_policy_projection_outcome, decode_wm_v1_policy_projection_request,
    decode_wm_v1_policy_snapshot, decode_wm_v1_projection_outcome_frame,
    decode_wm_v1_projection_request_frame, decode_wm_v1_server_welcome_frame,
    decode_wm_v1_snapshot_begin_frame, decode_wm_v1_snapshot_chunk_frame,
    decode_wm_v1_snapshot_end_frame, encode_wm_v1_client_hello_frame,
    encode_wm_v1_policy_projection, encode_wm_v1_projection_begin_frame,
    encode_wm_v1_projection_chunk_frame, encode_wm_v1_projection_end_frame,
};

#[derive(Debug)]
pub enum PolicyV1ClientError {
    Io(std::io::Error),
    Codec(IpcCodecError),
    UnsupportedRevision(u16),
    InvalidWelcome,
    MissingCapability,
    ConnectionEpochMismatch,
    SceneGenerationMismatch,
    InvalidScene,
    TransactionExhausted,
}

impl core::fmt::Display for PolicyV1ClientError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PolicyV1ClientError {}

impl From<std::io::Error> for PolicyV1ClientError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<IpcCodecError> for PolicyV1ClientError {
    fn from(error: IpcCodecError) -> Self {
        Self::Codec(error)
    }
}

/// Dormant public-v1 reference client. The installed session remains on API v7
/// until its immutable promotion gate permits a production migration.
pub struct PolicyV1Client {
    stream: UnixStream,
    connection_epoch: u64,
    capabilities: u64,
    max_outputs: usize,
    max_surfaces: usize,
    max_bindings: usize,
    next_transaction: u64,
}

impl PolicyV1Client {
    pub fn connect(path: impl AsRef<Path>, timeout: Duration) -> Result<Self, PolicyV1ClientError> {
        let mut stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        let hello = WmV1ClientHello {
            minimum_revision: 1,
            maximum_revision: 1,
            capabilities: SOPHIA_WM_CAPABILITY_BINDINGS
                | SOPHIA_WM_CAPABILITY_ACTIONS
                | SOPHIA_WM_CAPABILITY_MULTI_OUTPUT,
        };
        stream.write_all(&encode_wm_v1_client_hello_frame(&hello)?)?;
        stream.flush()?;
        let welcome = decode_wm_v1_server_welcome_frame(&read_frame(&mut stream)?)?;
        if welcome.selected_revision != 1 {
            return Err(PolicyV1ClientError::UnsupportedRevision(
                welcome.selected_revision,
            ));
        }
        if welcome.connection_epoch == 0 {
            return Err(PolicyV1ClientError::ConnectionEpochMismatch);
        }
        let max_outputs = usize::from(welcome.max_outputs);
        let max_surfaces = welcome.max_surfaces as usize;
        let max_bindings = usize::from(welcome.max_bindings);
        if max_outputs == 0
            || max_outputs > SOPHIA_WM_MAX_OUTPUTS
            || max_surfaces == 0
            || max_surfaces > SOPHIA_WM_MAX_SURFACES
            || max_bindings > SOPHIA_WM_MAX_BINDINGS
            || welcome.max_chunk_bytes == 0
            || welcome.max_chunk_bytes as usize > SOPHIA_IPC_MAX_PAYLOAD_LEN
        {
            return Err(PolicyV1ClientError::InvalidWelcome);
        }
        Ok(Self {
            stream,
            connection_epoch: welcome.connection_epoch,
            capabilities: welcome.capabilities,
            max_outputs,
            max_surfaces,
            max_bindings,
            next_transaction: 1,
        })
    }

    pub fn receive_snapshot(&mut self) -> Result<WmV1DecodedSnapshot, PolicyV1ClientError> {
        let (transaction, begin) =
            decode_wm_v1_snapshot_begin_frame(&read_frame(&mut self.stream)?)?;
        if begin.connection_epoch != self.connection_epoch {
            return Err(PolicyV1ClientError::ConnectionEpochMismatch);
        }
        let mut chunks = Vec::with_capacity(usize::from(begin.chunk_count));
        for _ in 0..begin.chunk_count {
            let (chunk_transaction, chunk) =
                decode_wm_v1_snapshot_chunk_frame(&read_frame(&mut self.stream)?)?;
            if chunk_transaction != transaction {
                return Err(PolicyV1ClientError::ConnectionEpochMismatch);
            }
            chunks.push(chunk);
        }
        let (end_transaction, end) =
            decode_wm_v1_snapshot_end_frame(&read_frame(&mut self.stream)?)?;
        if end_transaction != transaction {
            return Err(PolicyV1ClientError::ConnectionEpochMismatch);
        }
        let snapshot = decode_wm_v1_policy_snapshot(&WmV1SnapshotTransfer {
            transaction,
            begin,
            chunks,
            end,
        })?;
        if snapshot.scene.outputs.len() > self.max_outputs
            || snapshot.scene.surfaces.len() > self.max_surfaces
            || snapshot.bindings.len() > self.max_bindings
        {
            return Err(PolicyV1ClientError::InvalidScene);
        }
        validate_policy_scene(&snapshot.scene)?;
        Ok(snapshot)
    }

    pub fn receive_projection_request(
        &mut self,
    ) -> Result<PolicyProjectionRequest, PolicyV1ClientError> {
        let (_, request) = decode_wm_v1_projection_request_frame(&read_frame(&mut self.stream)?)?;
        let request = decode_wm_v1_policy_projection_request(&request)?;
        if request.connection_epoch != self.connection_epoch {
            return Err(PolicyV1ClientError::ConnectionEpochMismatch);
        }
        Ok(request)
    }

    pub fn send_projection(
        &mut self,
        proposal: &PolicyProjectionProposal,
    ) -> Result<(), PolicyV1ClientError> {
        if proposal.connection_epoch != self.connection_epoch {
            return Err(PolicyV1ClientError::ConnectionEpochMismatch);
        }
        let transfer = encode_wm_v1_policy_projection(proposal)?;
        self.stream.write_all(&encode_wm_v1_projection_begin_frame(
            transfer.transaction,
            &transfer.begin,
        )?)?;
        for chunk in &transfer.chunks {
            self.stream.write_all(&encode_wm_v1_projection_chunk_frame(
                transfer.transaction,
                chunk,
            )?)?;
        }
        self.stream.write_all(&encode_wm_v1_projection_end_frame(
            transfer.transaction,
            &transfer.end,
        )?)?;
        self.stream.flush()?;
        Ok(())
    }

    pub fn receive_projection_outcome(
        &mut self,
        proposal: &PolicyProjectionProposal,
    ) -> Result<PolicyProjectionOutcome, PolicyV1ClientError> {
        let (transaction, outcome) =
            decode_wm_v1_projection_outcome_frame(&read_frame(&mut self.stream)?)?;
        if transaction != proposal.transaction
            || outcome.connection_epoch != self.connection_epoch
            || outcome.request_id != proposal.request_id
        {
            return Err(PolicyV1ClientError::ConnectionEpochMismatch);
        }
        Ok(decode_wm_v1_policy_projection_outcome(&outcome)?)
    }

    pub fn tile_once(
        &mut self,
        snapshot: &PolicySceneSnapshot,
        request: &PolicyProjectionRequest,
    ) -> Result<PolicyProjectionProposal, PolicyV1ClientError> {
        if request.affected_outputs.len() > 1
            && self.capabilities & SOPHIA_WM_CAPABILITY_MULTI_OUTPUT == 0
        {
            return Err(PolicyV1ClientError::MissingCapability);
        }
        let transaction = self.next_transaction;
        self.next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or(PolicyV1ClientError::TransactionExhausted)?;
        tile_policy_scene(TransactionId::from_raw(transaction), snapshot, request)
    }

    pub const fn connection_epoch(&self) -> u64 {
        self.connection_epoch
    }
}

/// Places each affected output's currently assigned surfaces in equal columns.
pub fn tile_policy_scene(
    transaction: TransactionId,
    scene: &PolicySceneSnapshot,
    request: &PolicyProjectionRequest,
) -> Result<PolicyProjectionProposal, PolicyV1ClientError> {
    if scene.generation != request.scene_generation {
        return Err(PolicyV1ClientError::SceneGenerationMismatch);
    }
    validate_policy_scene(scene)?;
    if !transaction.is_valid()
        || request.affected_outputs.is_empty()
        || request.affected_outputs.len() > POLICY_MAX_OUTPUTS
    {
        return Err(PolicyV1ClientError::InvalidScene);
    }
    let mut outputs = Vec::with_capacity(request.affected_outputs.len());
    for output_id in &request.affected_outputs {
        let output = scene
            .outputs
            .iter()
            .find(|output| output.output == *output_id)
            .ok_or(PolicyV1ClientError::InvalidScene)?;
        let surfaces = scene
            .surfaces
            .iter()
            .filter(|surface| surface.current_output == Some(*output_id))
            .collect::<Vec<_>>();
        let mut placements = Vec::with_capacity(surfaces.len());
        let mut focus = output.focus;
        for (index, surface) in surfaces.iter().enumerate() {
            let geometry = column_geometry(output.bounds, index, surfaces.len())?;
            let requested_size = clamp_size(
                Size {
                    width: geometry.width,
                    height: geometry.height,
                },
                surface.constraints.min_size,
                surface.constraints.max_size,
            );
            if focus.is_none() && surface.capabilities.focusable {
                focus = Some(surface.surface);
            }
            placements.push(PolicySurfacePlacement {
                surface: surface.surface,
                surface_generation: surface.generation,
                geometry,
                requested_size: Some(requested_size),
                crop: None,
                transform: PolicyTransform::Identity,
                presentation: surface.current_state,
            });
        }
        outputs.push(PolicyOutputProjection {
            output: *output_id,
            placements,
            focus,
        });
    }
    Ok(PolicyProjectionProposal {
        transaction,
        connection_epoch: request.connection_epoch,
        request_id: request.request_id,
        base_generation: request.scene_generation,
        outputs,
        indicators: Vec::new(),
        output_statuses: Vec::new(),
    })
}

fn column_geometry(bounds: Rect, index: usize, count: usize) -> Result<Rect, PolicyV1ClientError> {
    if bounds.is_empty() || count == 0 {
        return Err(PolicyV1ClientError::InvalidScene);
    }
    let count = i32::try_from(count).map_err(|_| PolicyV1ClientError::InvalidScene)?;
    let index = i32::try_from(index).map_err(|_| PolicyV1ClientError::InvalidScene)?;
    let width = bounds.width / count;
    if width <= 0 {
        return Err(PolicyV1ClientError::InvalidScene);
    }
    let x = bounds.x + width * index;
    Ok(Rect {
        x,
        y: bounds.y,
        width: if index + 1 == count {
            bounds.x + bounds.width - x
        } else {
            width
        },
        height: bounds.height,
    })
}

fn validate_policy_scene(scene: &PolicySceneSnapshot) -> Result<(), PolicyV1ClientError> {
    if scene.generation == 0
        || scene.outputs.is_empty()
        || scene.outputs.len() > SOPHIA_WM_MAX_OUTPUTS
        || scene.surfaces.len() > SOPHIA_WM_MAX_SURFACES
    {
        return Err(PolicyV1ClientError::InvalidScene);
    }
    let mut outputs = BTreeSet::new();
    for output in &scene.outputs {
        if !output.output.is_valid()
            || output.generation == 0
            || output.bounds.is_empty()
            || !outputs.insert(output.output)
        {
            return Err(PolicyV1ClientError::InvalidScene);
        }
    }
    let mut surfaces = BTreeSet::new();
    for surface in &scene.surfaces {
        if !surface.surface.is_valid()
            || surface.generation == 0
            || surface.geometry.is_empty()
            || !surfaces.insert(surface.surface)
            || surface
                .current_output
                .is_some_and(|output| !outputs.contains(&output))
            || surface
                .constraints
                .min_size
                .is_some_and(|size| size.width <= 0 || size.height <= 0)
            || surface
                .constraints
                .max_size
                .is_some_and(|size| size.width <= 0 || size.height <= 0)
            || matches!(
                (surface.constraints.min_size, surface.constraints.max_size),
                (Some(minimum), Some(maximum))
                    if minimum.width > maximum.width || minimum.height > maximum.height
            )
        {
            return Err(PolicyV1ClientError::InvalidScene);
        }
    }
    if scene.surfaces.iter().any(|surface| {
        surface
            .transient_owner
            .is_some_and(|owner| owner == surface.surface || !surfaces.contains(&owner))
    }) {
        return Err(PolicyV1ClientError::InvalidScene);
    }
    for output in &scene.outputs {
        if output.focus.is_some_and(|focus| {
            !scene.surfaces.iter().any(|surface| {
                surface.surface == focus
                    && surface.current_output == Some(output.output)
                    && surface.capabilities.focusable
            })
        }) {
            return Err(PolicyV1ClientError::InvalidScene);
        }
    }
    Ok(())
}

fn clamp_size(size: Size, minimum: Option<Size>, maximum: Option<Size>) -> Size {
    let mut size = size;
    if let Some(minimum) = minimum {
        size.width = size.width.max(minimum.width);
        size.height = size.height.max(minimum.height);
    }
    if let Some(maximum) = maximum {
        size.width = size.width.min(maximum.width);
        size.height = size.height.min(maximum.height);
    }
    size
}

fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>, PolicyV1ClientError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    stream.read_exact(&mut header)?;
    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed header contains payload length"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(PolicyV1ClientError::Codec(IpcCodecError::PayloadTooLarge(
            payload_len,
        )));
    }
    let mut frame = header.to_vec();
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream.read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])?;
    Ok(frame)
}
