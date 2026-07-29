use sophia_protocol::{Rect, Size};
use sophia_renderer_live::{
    LiveCpuBufferPatch, LiveCpuBufferPatchBatch, LiveCpuBufferPatchRegion, LiveCpuBufferRegistry,
    LiveCpuBufferRegistryError, LiveCpuBufferSource, LiveCpuBufferUpdate,
};

fn buffer(handle: u64, generation: u64) -> LiveCpuBufferSource {
    LiveCpuBufferSource {
        handle,
        size: Size {
            width: 2,
            height: 2,
        },
        stride: 8,
        format: u32::from_le_bytes(*b"XR24"),
        generation,
        bytes: vec![0; 16],
    }
}

#[test]
fn replacement_and_patch_preserve_generation_order() {
    let mut registry = LiveCpuBufferRegistry::new();
    assert!(
        registry
            .apply(LiveCpuBufferUpdate::Replace(buffer(7, 2)))
            .unwrap()
    );
    assert!(
        registry
            .apply(LiveCpuBufferUpdate::Patch(LiveCpuBufferPatch {
                handle: 7,
                size: Size {
                    width: 2,
                    height: 2
                },
                stride: 8,
                format: u32::from_le_bytes(*b"XR24"),
                generation: 3,
                rect: Rect {
                    x: 1,
                    y: 0,
                    width: 1,
                    height: 2
                },
                bytes: vec![1, 2, 3, 4, 5, 6, 7, 8],
            }))
            .unwrap()
    );
    let stored = registry.get(7).unwrap();
    assert_eq!(stored.generation, 3);
    assert_eq!(&stored.bytes[4..8], &[1, 2, 3, 4]);
    assert_eq!(&stored.bytes[12..16], &[5, 6, 7, 8]);

    assert!(
        !registry
            .apply(LiveCpuBufferUpdate::Replace(buffer(7, 1)))
            .unwrap()
    );
    assert_eq!(registry.get(7).unwrap().generation, 3);
}

#[test]
fn malformed_replacement_fails_closed() {
    let mut registry = LiveCpuBufferRegistry::new();
    let mut malformed = buffer(4, 1);
    malformed.bytes.pop();
    assert_eq!(
        registry.apply(LiveCpuBufferUpdate::Replace(malformed)),
        Err(LiveCpuBufferRegistryError::InvalidBufferMetadata)
    );
    assert!(registry.get(4).is_none());
}

#[test]
fn malformed_patch_fails_closed_without_mutating_base() {
    let mut registry = LiveCpuBufferRegistry::new();
    registry
        .apply(LiveCpuBufferUpdate::Replace(buffer(9, 1)))
        .unwrap();
    let before = registry.get(9).unwrap().clone();
    let error = registry
        .apply(LiveCpuBufferUpdate::Patch(LiveCpuBufferPatch {
            handle: 9,
            size: Size {
                width: 2,
                height: 2,
            },
            stride: 8,
            format: u32::from_le_bytes(*b"XR24"),
            generation: 2,
            rect: Rect {
                x: 1,
                y: 1,
                width: 2,
                height: 1,
            },
            bytes: vec![1; 8],
        }))
        .unwrap_err();
    assert_eq!(error, LiveCpuBufferRegistryError::InvalidPatchBounds);
    assert_eq!(registry.get(9), Some(&before));
}

#[test]
fn patch_batch_applies_atomically_at_one_generation() {
    let mut registry = LiveCpuBufferRegistry::new();
    registry
        .apply(LiveCpuBufferUpdate::Replace(buffer(11, 1)))
        .unwrap();
    let update = LiveCpuBufferUpdate::PatchBatch(LiveCpuBufferPatchBatch {
        handle: 11,
        size: Size {
            width: 2,
            height: 2,
        },
        stride: 8,
        format: u32::from_le_bytes(*b"XR24"),
        generation: 2,
        patches: vec![
            LiveCpuBufferPatchRegion {
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                bytes: vec![1, 2, 3, 4],
            },
            LiveCpuBufferPatchRegion {
                rect: Rect {
                    x: 1,
                    y: 1,
                    width: 1,
                    height: 1,
                },
                bytes: vec![5, 6, 7, 8],
            },
        ],
    });
    assert!(registry.apply(update).unwrap());
    let stored = registry.get(11).unwrap();
    assert_eq!(stored.generation, 2);
    assert_eq!(&stored.bytes[0..4], &[1, 2, 3, 4]);
    assert_eq!(&stored.bytes[12..16], &[5, 6, 7, 8]);
}

#[test]
fn malformed_patch_batch_does_not_apply_valid_prefix() {
    let mut registry = LiveCpuBufferRegistry::new();
    registry
        .apply(LiveCpuBufferUpdate::Replace(buffer(12, 1)))
        .unwrap();
    let before = registry.get(12).unwrap().clone();
    let error = registry
        .apply(LiveCpuBufferUpdate::PatchBatch(LiveCpuBufferPatchBatch {
            handle: 12,
            size: before.size,
            stride: before.stride,
            format: before.format,
            generation: 2,
            patches: vec![
                LiveCpuBufferPatchRegion {
                    rect: Rect {
                        x: 0,
                        y: 0,
                        width: 1,
                        height: 1,
                    },
                    bytes: vec![9; 4],
                },
                LiveCpuBufferPatchRegion {
                    rect: Rect {
                        x: 2,
                        y: 0,
                        width: 1,
                        height: 1,
                    },
                    bytes: vec![7; 4],
                },
            ],
        }))
        .unwrap_err();
    assert_eq!(error, LiveCpuBufferRegistryError::InvalidPatchBounds);
    assert_eq!(registry.get(12), Some(&before));
}

#[test]
fn retention_drops_unreferenced_renderer_resources() {
    let mut registry = LiveCpuBufferRegistry::new();
    registry
        .apply(LiveCpuBufferUpdate::Replace(buffer(1, 1)))
        .unwrap();
    registry
        .apply(LiveCpuBufferUpdate::Replace(buffer(2, 1)))
        .unwrap();
    registry.retain_handles(|handle| handle == 2);
    assert!(registry.get(1).is_none());
    assert!(registry.get(2).is_some());
}
