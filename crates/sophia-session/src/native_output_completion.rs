use std::collections::BTreeMap;

use sophia_protocol::OutputId;

/// Reduced logical and optional physical evidence for one native head.
///
/// Scene generation and logical-content checksum describe the shared cohort.
/// A native pixel checksum, when a renderer can provide one, describes only
/// that head's final pixels and is never a mirror join key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeOutputContentEvidence {
    pub output: OutputId,
    pub scene_generation: u64,
    pub logical_content_checksum: u64,
    pub head_pixel_checksum: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOutputContentEvidenceError {
    MirrorGenerationMismatch {
        output: OutputId,
        expected: u64,
        actual: u64,
    },
    MirrorLogicalContentMismatch {
        output: OutputId,
        expected: u64,
        actual: u64,
    },
}

/// Renders a head's optional native pixel checksum for an evidence record.
///
/// No renderer supplies one yet, so today this always reads `unavailable`. It
/// is a function rather than a literal in the format string because the day one
/// does, a hardcoded `unavailable` would keep reporting absence while the value
/// sat in the struct, and the verifiers already accept the number.
pub fn head_pixel_checksum_field(head_pixel_checksum: Option<u64>) -> String {
    head_pixel_checksum.map_or_else(|| "unavailable".to_owned(), |value| value.to_string())
}

/// Validates logical cohort identity without conflating it with head pixels.
///
/// Heads behind one `OutputId` must report one scene generation and logical
/// content identity. Independent outputs may legitimately show identical
/// content, and head-local native pixel checksums may legitimately differ.
pub fn validate_native_output_content_evidence(
    heads: impl IntoIterator<Item = NativeOutputContentEvidence>,
) -> Result<(), NativeOutputContentEvidenceError> {
    let mut outputs = BTreeMap::<OutputId, (u64, u64)>::new();

    for head in heads {
        let Some((expected_generation, expected_checksum)) = outputs.get(&head.output).copied()
        else {
            outputs.insert(
                head.output,
                (head.scene_generation, head.logical_content_checksum),
            );
            continue;
        };
        if expected_generation != head.scene_generation {
            return Err(NativeOutputContentEvidenceError::MirrorGenerationMismatch {
                output: head.output,
                expected: expected_generation,
                actual: head.scene_generation,
            });
        }
        if expected_checksum != head.logical_content_checksum {
            return Err(
                NativeOutputContentEvidenceError::MirrorLogicalContentMismatch {
                    output: head.output,
                    expected: expected_checksum,
                    actual: head.logical_content_checksum,
                },
            );
        }
    }
    Ok(())
}
