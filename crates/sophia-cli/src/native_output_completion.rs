use std::collections::BTreeMap;

use sophia_protocol::OutputId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOutputChecksumError {
    MirrorMismatch {
        output: OutputId,
        expected: u64,
        actual: u64,
    },
    LogicalOutputCollision {
        first: OutputId,
        second: OutputId,
        checksum: u64,
    },
}

/// Validates the checksum identity carried by physical scanout heads.
///
/// Heads behind one mirrored `OutputId` carry one logical scene and must agree.
/// Separate logical outputs must remain distinguishable, even when each output
/// happens to own only one physical head.
pub fn validate_native_output_checksums(
    heads: impl IntoIterator<Item = (OutputId, u64)>,
) -> Result<(), NativeOutputChecksumError> {
    let mut output_checksums = BTreeMap::<OutputId, u64>::new();
    let mut checksum_outputs = BTreeMap::<u64, OutputId>::new();

    for (output, checksum) in heads {
        if let Some(expected) = output_checksums.get(&output).copied() {
            if expected != checksum {
                return Err(NativeOutputChecksumError::MirrorMismatch {
                    output,
                    expected,
                    actual: checksum,
                });
            }
            continue;
        }
        if let Some(first) = checksum_outputs.get(&checksum).copied() {
            return Err(NativeOutputChecksumError::LogicalOutputCollision {
                first,
                second: output,
                checksum,
            });
        }
        output_checksums.insert(output, checksum);
        checksum_outputs.insert(checksum, output);
    }
    Ok(())
}
