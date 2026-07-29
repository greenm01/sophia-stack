use std::collections::BTreeMap;

use sophia_protocol::{Rect, Size};

use crate::LiveCpuBufferSource;

pub const LIVE_CPU_PATCH_BATCH_MAX_RECTS: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCpuBufferPatch {
    pub handle: u64,
    pub size: Size,
    pub stride: u32,
    pub format: u32,
    pub generation: u64,
    pub rect: Rect,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCpuBufferPatchRegion {
    pub rect: Rect,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCpuBufferPatchBatch {
    pub handle: u64,
    pub size: Size,
    pub stride: u32,
    pub format: u32,
    pub generation: u64,
    pub patches: Vec<LiveCpuBufferPatchRegion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveCpuBufferUpdate {
    Replace(LiveCpuBufferSource),
    Patch(LiveCpuBufferPatch),
    PatchBatch(LiveCpuBufferPatchBatch),
}

impl LiveCpuBufferUpdate {
    pub const fn handle(&self) -> u64 {
        match self {
            Self::Replace(buffer) => buffer.handle,
            Self::Patch(patch) => patch.handle,
            Self::PatchBatch(batch) => batch.handle,
        }
    }

    pub const fn generation(&self) -> u64 {
        match self {
            Self::Replace(buffer) => buffer.generation,
            Self::Patch(patch) => patch.generation,
            Self::PatchBatch(batch) => batch.generation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveCpuBufferRegistryError {
    InvalidBufferMetadata,
    MissingPatchBase,
    PatchMetadataMismatch,
    InvalidPatchBounds,
    InvalidPatchBytes,
    PatchBatchCapacityExceeded,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveCpuBufferRegistry {
    buffers: BTreeMap<u64, LiveCpuBufferSource>,
}

impl LiveCpuBufferRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(
        &mut self,
        update: LiveCpuBufferUpdate,
    ) -> Result<bool, LiveCpuBufferRegistryError> {
        if self
            .buffers
            .get(&update.handle())
            .is_some_and(|current| update.generation() < current.generation)
        {
            return Ok(false);
        }
        match update {
            LiveCpuBufferUpdate::Replace(buffer) => {
                if !valid_buffer(&buffer) {
                    return Err(LiveCpuBufferRegistryError::InvalidBufferMetadata);
                }
                self.buffers.insert(buffer.handle, buffer);
            }
            LiveCpuBufferUpdate::Patch(patch) => self.apply_patch(patch)?,
            LiveCpuBufferUpdate::PatchBatch(batch) => self.apply_patch_batch(batch)?,
        }
        Ok(true)
    }

    pub fn get(&self, handle: u64) -> Option<&LiveCpuBufferSource> {
        self.buffers.get(&handle)
    }

    pub fn retain_handles(&mut self, mut retain: impl FnMut(u64) -> bool) {
        self.buffers.retain(|handle, _| retain(*handle));
    }

    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    pub fn total_bytes(&self) -> usize {
        self.buffers.values().fold(0usize, |total, buffer| {
            total.saturating_add(buffer.bytes.len())
        })
    }

    pub fn contains(&self, handle: u64) -> bool {
        self.buffers.contains_key(&handle)
    }

    pub fn checksum(&self) -> u64 {
        self.buffers
            .values()
            .fold(0xcbf2_9ce4_8422_2325u64, |hash, buffer| {
                buffer.bytes.iter().fold(hash, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
                })
            })
    }

    fn apply_patch(&mut self, patch: LiveCpuBufferPatch) -> Result<(), LiveCpuBufferRegistryError> {
        let buffer = self
            .buffers
            .get_mut(&patch.handle)
            .ok_or(LiveCpuBufferRegistryError::MissingPatchBase)?;
        if buffer.size != patch.size
            || buffer.stride != patch.stride
            || buffer.format != patch.format
            || patch.generation < buffer.generation
        {
            return Err(LiveCpuBufferRegistryError::PatchMetadataMismatch);
        }
        let region = LiveCpuBufferPatchRegion {
            rect: patch.rect,
            bytes: patch.bytes,
        };
        apply_patch_region(buffer, &region)?;
        buffer.generation = patch.generation;
        Ok(())
    }

    fn apply_patch_batch(
        &mut self,
        batch: LiveCpuBufferPatchBatch,
    ) -> Result<(), LiveCpuBufferRegistryError> {
        if batch.patches.len() > LIVE_CPU_PATCH_BATCH_MAX_RECTS {
            return Err(LiveCpuBufferRegistryError::PatchBatchCapacityExceeded);
        }
        let buffer = self
            .buffers
            .get_mut(&batch.handle)
            .ok_or(LiveCpuBufferRegistryError::MissingPatchBase)?;
        if buffer.size != batch.size
            || buffer.stride != batch.stride
            || buffer.format != batch.format
            || batch.generation < buffer.generation
        {
            return Err(LiveCpuBufferRegistryError::PatchMetadataMismatch);
        }
        for patch in &batch.patches {
            validate_patch_region(buffer, patch)?;
        }
        for patch in &batch.patches {
            apply_patch_region(buffer, patch)?;
        }
        buffer.generation = batch.generation;
        Ok(())
    }
}

fn validate_patch_region(
    buffer: &LiveCpuBufferSource,
    patch: &LiveCpuBufferPatchRegion,
) -> Result<(), LiveCpuBufferRegistryError> {
    let x = usize::try_from(patch.rect.x)
        .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
    let y = usize::try_from(patch.rect.y)
        .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
    let width = usize::try_from(patch.rect.width)
        .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
    let height = usize::try_from(patch.rect.height)
        .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
    let buffer_width = usize::try_from(buffer.size.width)
        .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
    let buffer_height = usize::try_from(buffer.size.height)
        .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
    let expected = width
        .checked_mul(4)
        .and_then(|row| row.checked_mul(height))
        .ok_or(LiveCpuBufferRegistryError::InvalidPatchBytes)?;
    if width == 0
        || height == 0
        || x.checked_add(width)
            .is_none_or(|right| right > buffer_width)
        || y.checked_add(height)
            .is_none_or(|bottom| bottom > buffer_height)
        || patch.bytes.len() != expected
    {
        return Err(if patch.bytes.len() != expected {
            LiveCpuBufferRegistryError::InvalidPatchBytes
        } else {
            LiveCpuBufferRegistryError::InvalidPatchBounds
        });
    }
    Ok(())
}

fn apply_patch_region(
    buffer: &mut LiveCpuBufferSource,
    patch: &LiveCpuBufferPatchRegion,
) -> Result<(), LiveCpuBufferRegistryError> {
    validate_patch_region(buffer, patch)?;
    let x = usize::try_from(patch.rect.x)
        .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
    let y = usize::try_from(patch.rect.y)
        .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
    let width = usize::try_from(patch.rect.width)
        .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
    let height = usize::try_from(patch.rect.height)
        .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
    let stride = usize::try_from(buffer.stride)
        .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
    let row_bytes = width
        .checked_mul(4)
        .ok_or(LiveCpuBufferRegistryError::InvalidPatchBounds)?;
    for row in 0..height {
        let source = row
            .checked_mul(row_bytes)
            .ok_or(LiveCpuBufferRegistryError::InvalidPatchBounds)?;
        let target = y
            .checked_add(row)
            .and_then(|row| row.checked_mul(stride))
            .and_then(|offset| offset.checked_add(x.saturating_mul(4)))
            .ok_or(LiveCpuBufferRegistryError::InvalidPatchBounds)?;
        let target_end = target
            .checked_add(row_bytes)
            .ok_or(LiveCpuBufferRegistryError::InvalidPatchBounds)?;
        buffer
            .bytes
            .get_mut(target..target_end)
            .ok_or(LiveCpuBufferRegistryError::InvalidPatchBounds)?
            .copy_from_slice(&patch.bytes[source..source + row_bytes]);
    }
    Ok(())
}

fn valid_buffer(buffer: &LiveCpuBufferSource) -> bool {
    let Ok(width) = usize::try_from(buffer.size.width) else {
        return false;
    };
    let Ok(height) = usize::try_from(buffer.size.height) else {
        return false;
    };
    let Ok(stride) = usize::try_from(buffer.stride) else {
        return false;
    };
    let Some(row_bytes) = width.checked_mul(4) else {
        return false;
    };
    let Some(byte_len) = stride.checked_mul(height) else {
        return false;
    };
    width > 0
        && height > 0
        && stride >= row_bytes
        && byte_len <= 64 * 1024 * 1024
        && buffer.bytes.len() == byte_len
}
