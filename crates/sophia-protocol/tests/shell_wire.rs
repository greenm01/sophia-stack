use sophia_protocol::*;

const VALID_CORPUS: &str = include_str!("../../../protocol/golden/sophia-shell-v1.frames");
const MALFORMED_CORPUS: &str =
    include_str!("../../../protocol/golden/sophia-shell-v1-malformed.frames");

#[test]
fn rust_codec_matches_every_shell_golden_frame() {
    for line in corpus_lines(VALID_CORPUS) {
        let mut fields = line.split('|');
        let name = fields.next().unwrap();
        let transaction = TransactionId::from_raw(fields.next().unwrap().parse().unwrap());
        let frame = decode_hex(fields.next().unwrap());
        assert!(fields.next().is_none(), "invalid corpus row: {line}");
        assert_eq!(roundtrip(name, transaction, &frame), frame, "{name}");
    }
}

#[test]
fn rust_codec_rejects_every_shell_malformed_frame() {
    for line in corpus_lines(MALFORMED_CORPUS) {
        let mut fields = line.split('|');
        let case = fields.next().unwrap();
        let decoder = fields.next().unwrap();
        let expected = fields.next().unwrap();
        let frame = decode_hex(fields.next().unwrap());
        assert!(fields.next().is_none(), "invalid corpus row: {line}");
        let error = match decoder {
            "client_hello" => decode_shell_v1_client_hello_frame(&frame).unwrap_err(),
            "server_welcome" => decode_shell_v1_server_welcome_frame(&frame).unwrap_err(),
            "descriptor_snapshot" => decode_shell_v1_descriptor_snapshot_frame(&frame).unwrap_err(),
            "candidate" => decode_shell_v1_candidate_frame(&frame).unwrap_err(),
            "activation" => decode_shell_v1_activation_frame(&frame).unwrap_err(),
            other => panic!("unknown decoder `{other}`"),
        };
        assert_eq!(
            error_name(&error),
            expected,
            "wrong error for {case}: {error:?}"
        );
    }
}

fn roundtrip(name: &str, transaction: TransactionId, frame: &[u8]) -> Vec<u8> {
    match name {
        "client_hello" => {
            encode_shell_v1_client_hello_frame(decode_shell_v1_client_hello_frame(frame).unwrap())
                .unwrap()
        }
        "server_welcome" => encode_shell_v1_server_welcome_frame(
            decode_shell_v1_server_welcome_frame(frame).unwrap(),
        )
        .unwrap(),
        "descriptor_snapshot" | "descriptor_snapshot_unlabeled" => {
            let (actual, message) = decode_shell_v1_descriptor_snapshot_frame(frame).unwrap();
            assert_eq!(actual, transaction);
            encode_shell_v1_descriptor_snapshot_frame(actual, &message).unwrap()
        }
        "candidate" | "candidate_reserved" | "candidate_hidden" => {
            let (actual, message) = decode_shell_v1_candidate_frame(frame).unwrap();
            assert_eq!(actual, transaction);
            encode_shell_v1_candidate_frame(actual, &message).unwrap()
        }
        "candidate_outcome" => {
            let (actual, message) = decode_shell_v1_candidate_outcome_frame(frame).unwrap();
            assert_eq!(actual, transaction);
            encode_shell_v1_candidate_outcome_frame(actual, message).unwrap()
        }
        "activation" => {
            let (actual, message) = decode_shell_v1_activation_frame(frame).unwrap();
            assert_eq!(actual, transaction);
            encode_shell_v1_activation_frame(actual, message).unwrap()
        }
        "activation_ack" => {
            let (actual, message) = decode_shell_v1_activation_ack_frame(frame).unwrap();
            assert_eq!(actual, transaction);
            encode_shell_v1_activation_ack_frame(actual, message).unwrap()
        }
        other => panic!("unknown message `{other}`"),
    }
}

fn corpus_lines(corpus: &str) -> impl Iterator<Item = &str> {
    corpus
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

fn decode_hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn error_name(error: &IpcCodecError) -> &'static str {
    match error {
        IpcCodecError::Truncated => "truncated",
        IpcCodecError::BadMagic => "bad_magic",
        IpcCodecError::UnsupportedVersion(_) => "unsupported_frame_version",
        IpcCodecError::UnknownMessageKind(_) => "wrong_message_kind",
        IpcCodecError::PayloadTooLarge(_) => "payload_too_large",
        IpcCodecError::ReservedNonZero(_) => "reserved_nonzero",
        IpcCodecError::TrailingBytes(_) => "trailing_bytes",
        IpcCodecError::InvalidTransaction(_) => "invalid_transaction",
        IpcCodecError::InvalidEnum { .. } => "invalid_enum",
        IpcCodecError::InvalidRecord(_) => "invalid_record",
        other => panic!("malformed corpus lacks an error name for {other:?}"),
    }
}
