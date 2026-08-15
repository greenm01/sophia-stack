use sophia_cli::native_output_completion::{
    NativeOutputContentEvidence, NativeOutputContentEvidenceError,
    validate_native_output_content_evidence,
};
use sophia_protocol::OutputId;

fn evidence(
    output: u64,
    scene_generation: u64,
    logical_content_checksum: u64,
    head_pixel_checksum: Option<u64>,
) -> NativeOutputContentEvidence {
    NativeOutputContentEvidence {
        output: OutputId::from_raw(output),
        scene_generation,
        logical_content_checksum,
        head_pixel_checksum,
    }
}

#[test]
fn mirrored_heads_share_logical_identity_while_native_pixels_differ() {
    assert_eq!(
        validate_native_output_content_evidence([
            evidence(1, 7, 44, Some(100)),
            evidence(1, 7, 44, Some(200)),
        ]),
        Ok(())
    );
}

#[test]
fn a_mirror_group_rejects_divergent_scene_generations() {
    assert_eq!(
        validate_native_output_content_evidence([
            evidence(1, 7, 44, None),
            evidence(1, 8, 44, None),
        ]),
        Err(NativeOutputContentEvidenceError::MirrorGenerationMismatch {
            output: OutputId::from_raw(1),
            expected: 7,
            actual: 8,
        })
    );
}

#[test]
fn a_mirror_group_rejects_divergent_logical_content() {
    assert_eq!(
        validate_native_output_content_evidence([
            evidence(1, 7, 44, None),
            evidence(1, 7, 45, None),
        ]),
        Err(
            NativeOutputContentEvidenceError::MirrorLogicalContentMismatch {
                output: OutputId::from_raw(1),
                expected: 44,
                actual: 45,
            }
        )
    );
}

#[test]
fn unrelated_outputs_may_show_identical_content_and_pixels() {
    assert_eq!(
        validate_native_output_content_evidence([
            evidence(1, 7, 44, Some(100)),
            evidence(2, 7, 44, Some(100)),
        ]),
        Ok(())
    );
}
