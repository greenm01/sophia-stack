use sophia_protocol::*;

fn corpus(text: &str) -> impl Iterator<Item = (&str, Vec<u8>)> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let (name, hex) = line.split_once(' ').unwrap();
            (
                name,
                hex.as_bytes()
                    .chunks_exact(2)
                    .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                    .collect(),
            )
        })
}

#[test]
fn schema_samples_roundtrip_through_independent_rust_codec() {
    for (name, bytes) in corpus(include_str!(
        "../../../protocol/golden/sophia-control-v1.frames"
    )) {
        let (id, message) =
            decode_control_frame(&bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(encode_control_frame(id, &message).unwrap(), bytes, "{name}");
    }
}

#[test]
fn schema_malformed_samples_are_rejected() {
    for (name, bytes) in corpus(include_str!(
        "../../../protocol/golden/sophia-control-v1-malformed.frames"
    )) {
        assert!(decode_control_frame(&bytes).is_err(), "{name}");
    }
}

#[test]
fn complete_catalog_fits_and_preserves_sorted_exact_names() {
    let mut commands = (0..256)
        .map(|n| ControlCommand {
            owner: ControlOwner::Policy,
            name: format!("step {n:03}"),
        })
        .collect::<Vec<_>>();
    for name in ["reload-profile", "restart-wm"] {
        commands.push(ControlCommand {
            owner: ControlOwner::Session,
            name: name.into(),
        });
    }
    let message = ControlMessage::Catalog(ControlCatalog {
        generation: 7,
        commands,
    });
    let bytes = encode_control_frame(1, &message).unwrap();
    assert_eq!(bytes.len(), 24 + 35100);
    assert_eq!(decode_control_frame(&bytes).unwrap().1, message);
    let ControlMessage::Catalog(mut catalog) = message else {
        unreachable!()
    };
    catalog.commands.swap(0, 1);
    assert!(encode_control_frame(1, &ControlMessage::Catalog(catalog)).is_err());
}

#[test]
fn strict_strings_and_outcome_semantics() {
    for name in ["", " leading", "trailing ", "x;exec", "☃", "x\0"] {
        assert!(validate_control_name(name).is_err());
    }
    for name in ["resize-width -0.1", "focus-workspace 1", "a.b_C-9"] {
        validate_control_name(name).unwrap();
    }
    for code in 1..=10 {
        let message = ControlMessage::Outcome {
            generation: 4,
            outcome: ControlOutcome::from_wire(code).unwrap(),
            detail: "réglage".into(),
        };
        let bytes = encode_control_frame(3, &message).unwrap();
        assert_eq!(decode_control_frame(&bytes).unwrap().1, message);
    }
    for detail in ["x\0", "\u{85}", "\u{1b}"] {
        assert!(
            encode_control_frame(
                1,
                &ControlMessage::Outcome {
                    generation: 1,
                    outcome: ControlOutcome::Rejected,
                    detail: detail.into()
                }
            )
            .is_err()
        );
    }
}

#[test]
fn malformed_catalog_and_invoke_payloads_fail_closed() {
    let invoke = ControlMessage::Invoke {
        generation: 7,
        command: ControlCommand {
            owner: ControlOwner::Policy,
            name: "focus-next".into(),
        },
    };
    let frame = encode_control_frame(2, &invoke).unwrap();
    for (offset, value) in [(32, 3), (34, 129), (36, 255), (46, 1)] {
        let mut malformed = frame.clone();
        malformed[offset] = value;
        assert!(decode_control_frame(&malformed).is_err(), "offset {offset}");
    }
    for end in 0..frame.len() {
        assert!(decode_control_frame(&frame[..end]).is_err());
    }
}
