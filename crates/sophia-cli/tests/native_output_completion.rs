use sophia_cli::native_output_completion::{
    NativeOutputChecksumError, validate_native_output_checksums,
};
use sophia_protocol::OutputId;

fn output(raw: u64) -> OutputId {
    OutputId::from_raw(raw)
}

#[test]
fn mirrored_heads_share_one_logical_checksum() {
    assert_eq!(
        validate_native_output_checksums([(output(1), 44), (output(1), 44)]),
        Ok(())
    );
}

#[test]
fn a_mirror_group_rejects_divergent_logical_checksums() {
    assert_eq!(
        validate_native_output_checksums([(output(1), 44), (output(1), 45)]),
        Err(NativeOutputChecksumError::MirrorMismatch {
            output: output(1),
            expected: 44,
            actual: 45,
        })
    );
}

#[test]
fn extended_logical_outputs_remain_distinguishable() {
    assert_eq!(
        validate_native_output_checksums([(output(1), 44), (output(2), 45)]),
        Ok(())
    );
    assert_eq!(
        validate_native_output_checksums([(output(1), 44), (output(2), 44)]),
        Err(NativeOutputChecksumError::LogicalOutputCollision {
            first: output(1),
            second: output(2),
            checksum: 44,
        })
    );
}
