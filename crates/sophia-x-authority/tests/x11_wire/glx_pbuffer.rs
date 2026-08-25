// GLX offscreen drawables: the pbuffer a GL client creates before it has a window.

/// Builds a `CreatePbuffer` request. `num_attribs` counts pairs, not words.
fn glx_create_pbuffer_request(
    byte_order: XByteOrder,
    fbconfig: u32,
    pbuffer: u32,
    attributes: &[(u32, u32)],
) -> Vec<u8> {
    let mut out = vec![X_GLX_MAJOR_OPCODE, X_GLX_CREATE_PBUFFER_MINOR_OPCODE];
    let words = 5 + attributes.len() * 2;
    push_u16(&mut out, byte_order, u16::try_from(words).unwrap());
    push_u32(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, fbconfig);
    push_u32(&mut out, byte_order, pbuffer);
    push_u32(&mut out, byte_order, u32::try_from(attributes.len()).unwrap());
    for (name, value) in attributes {
        push_u32(&mut out, byte_order, *name);
        push_u32(&mut out, byte_order, *value);
    }
    out
}

fn glx_destroy_pbuffer_request(byte_order: XByteOrder, pbuffer: u32) -> Vec<u8> {
    let mut out = vec![X_GLX_MAJOR_OPCODE, X_GLX_DESTROY_PBUFFER_MINOR_OPCODE];
    push_u16(&mut out, byte_order, 2);
    push_u32(&mut out, byte_order, pbuffer);
    out
}

/// The exact request Helium's GL layer sends to bootstrap a display.
#[test]
fn glx_decoder_accepts_the_initialization_pbuffer_in_both_byte_orders() {
    let namespace = NamespaceId::from_raw(71);
    for byte_order in [XByteOrder::LittleEndian, XByteOrder::BigEndian] {
        let request = glx_create_pbuffer_request(
            byte_order,
            1,
            0x220301,
            &[
                (X_GLX_PBUFFER_WIDTH_ATTRIBUTE, 1),
                (X_GLX_PBUFFER_HEIGHT_ATTRIBUTE, 1),
            ],
        );
        assert_eq!(
            decode_x11_core_request(context(namespace, 1, byte_order), &request).unwrap(),
            XWireRequest::GlxCreatePbuffer {
                screen: 0,
                fbconfig: 1,
                pbuffer: XResourceId::new(0x220301, 1),
                width: 1,
                height: 1,
                largest: false,
            }
        );
    }
}

/// Height is the lower attribute number, so a swap decodes as a transposed
/// surface rather than failing.
#[test]
fn glx_pbuffer_attributes_are_read_by_name_not_position() {
    let namespace = NamespaceId::from_raw(72);
    let request = glx_create_pbuffer_request(
        XByteOrder::LittleEndian,
        2,
        0x220302,
        &[
            (X_GLX_PBUFFER_HEIGHT_ATTRIBUTE, 48),
            (X_GLX_PBUFFER_WIDTH_ATTRIBUTE, 64),
        ],
    );
    assert_eq!(
        decode_x11_core_request(context(namespace, 1, XByteOrder::LittleEndian), &request).unwrap(),
        XWireRequest::GlxCreatePbuffer {
            screen: 0,
            fbconfig: 2,
            pbuffer: XResourceId::new(0x220302, 1),
            width: 64,
            height: 48,
            largest: false,
        }
    );
}

/// A client may ask for more than Sophia implements; only the list's length is
/// its business.
#[test]
fn glx_pbuffer_ignores_attributes_it_does_not_implement() {
    let namespace = NamespaceId::from_raw(73);
    let request = glx_create_pbuffer_request(
        XByteOrder::LittleEndian,
        1,
        0x220303,
        &[
            (0x801B, 1),
            (X_GLX_PBUFFER_WIDTH_ATTRIBUTE, 16),
            (0x9999, 7),
            (X_GLX_PBUFFER_HEIGHT_ATTRIBUTE, 16),
            (X_GLX_LARGEST_PBUFFER_ATTRIBUTE, 1),
        ],
    );
    assert_eq!(
        decode_x11_core_request(context(namespace, 1, XByteOrder::LittleEndian), &request).unwrap(),
        XWireRequest::GlxCreatePbuffer {
            screen: 0,
            fbconfig: 1,
            pbuffer: XResourceId::new(0x220303, 1),
            width: 16,
            height: 16,
            largest: true,
        }
    );

    // A pair count that disagrees with the bytes that arrived is malformed. The
    // request's own length stays honest, so this is the arm's check firing and
    // not the outer one.
    let mut mismatched = request.clone();
    let claimed = u32::try_from(request[20..].len() / 8 + 1).unwrap();
    mismatched[16..20].copy_from_slice(&claimed.to_le_bytes());
    assert!(matches!(
        decode_x11_core_request(context(namespace, 1, XByteOrder::LittleEndian), &mismatched),
        Err(XWireParseError::InvalidLength { .. })
    ));
}

/// The differential test: a created pbuffer answers core `GetGeometry`.
///
/// A GL client names the same id for the drawable it just created, and that
/// request is what decides whether the drawable exists at all. Before pbuffers
/// existed it returned `BadWindow`, which is the error a physical session failed
/// on six times.
#[test]
fn a_created_pbuffer_answers_core_get_geometry() {
    let namespace = NamespaceId::from_raw(74);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let pbuffer = 0x220304;

    let create = decode_x11_core_request(
        context(namespace, 1, XByteOrder::LittleEndian),
        &glx_create_pbuffer_request(
            XByteOrder::LittleEndian,
            2,
            pbuffer,
            &[
                (X_GLX_PBUFFER_WIDTH_ATTRIBUTE, 64),
                (X_GLX_PBUFFER_HEIGHT_ATTRIBUTE, 48),
            ],
        ),
    )
    .unwrap();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, X_GLX_MAJOR_OPCODE),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(result.outputs.is_empty(), "creating a pbuffer must not error");

    let geometry = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 14),
        XWireRequest::GetGeometry {
            drawable: XResourceId::new(u64::from(pbuffer), 1),
        },
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(matches!(
        geometry.outputs.as_slice(),
        [XClientOutput::Reply(XClientReply::GetGeometry {
            depth: 32,
            geometry: Rect { width: 64, height: 48, .. },
            border_width: 0,
            ..
        })],
    ));

    // The fix must not degenerate into answering for anything: an id that was
    // never created still reports the error it always did, naming itself.
    let unknown = 0x220999;
    let missing = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 14),
        XWireRequest::GetGeometry {
            drawable: XResourceId::new(u64::from(unknown), 1),
        },
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(matches!(
        missing.outputs.as_slice(),
        [XClientOutput::Error(XClientError {
            code: XErrorCode::BadWindow,
            resource_id,
            ..
        })] if *resource_id == unknown
    ));

    // Destroying it takes the answer away again.
    let destroy = decode_x11_core_request(
        context(namespace, 5, XByteOrder::LittleEndian),
        &glx_destroy_pbuffer_request(XByteOrder::LittleEndian, pbuffer),
    )
    .unwrap();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 6, XByteOrder::LittleEndian, X_GLX_MAJOR_OPCODE),
        destroy,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(result.outputs.is_empty());
    let after = dispatch_x11_wire_request(
        dispatch_context(namespace, 7, XByteOrder::LittleEndian, 14),
        XWireRequest::GetGeometry {
            drawable: XResourceId::new(u64::from(pbuffer), 1),
        },
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(matches!(
        after.outputs.as_slice(),
        [XClientOutput::Error(XClientError {
            code: XErrorCode::BadWindow,
            ..
        })]
    ));
}

/// The advertised maximum and the refusal threshold are one number.
#[test]
fn a_pbuffer_beyond_the_advertised_maximum_is_refused_or_clamped() {
    let namespace = NamespaceId::from_raw(75);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    // Read the bound out of the encoded catalog, so editing one without the
    // other fails here.
    let configs = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, X_GLX_MAJOR_OPCODE),
        XWireRequest::GlxGetFbConfigs { screen: 0 },
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian)
    .remove(0);
    let attributes = read_u32(XByteOrder::LittleEndian, &configs[12..16]) as usize;
    let advertised = (0..attributes)
        .map(|index| {
            let at = 32 + index * 8;
            (
                read_u32(XByteOrder::LittleEndian, &configs[at..at + 4]),
                read_u32(XByteOrder::LittleEndian, &configs[at + 4..at + 8]),
            )
        })
        .find(|(name, _)| *name == 0x8016)
        .expect("the catalog advertises a maximum pbuffer width")
        .1;
    assert!(
        advertised > 0,
        "a drawable type we implement needs a stated bound"
    );

    let oversized = |largest: bool, id: u32| {
        decode_x11_core_request(
            context(namespace, 2, XByteOrder::LittleEndian),
            &glx_create_pbuffer_request(
                XByteOrder::LittleEndian,
                1,
                id,
                &[
                    (X_GLX_PBUFFER_WIDTH_ATTRIBUTE, advertised + 1),
                    (X_GLX_PBUFFER_HEIGHT_ATTRIBUTE, 1),
                    (X_GLX_LARGEST_PBUFFER_ATTRIBUTE, u32::from(largest)),
                ],
            ),
        )
        .unwrap()
    };

    let refused = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, X_GLX_MAJOR_OPCODE),
        oversized(false, 0x220305),
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(matches!(
        refused.outputs.as_slice(),
        [XClientOutput::Error(XClientError {
            code: XErrorCode::BadAlloc,
            ..
        })]
    ));

    // Asking for the largest available clamps rather than failing.
    let clamped = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, X_GLX_MAJOR_OPCODE),
        oversized(true, 0x220306),
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(clamped.outputs.is_empty());
    let geometry = dispatch_x11_wire_request(
        dispatch_context(namespace, 5, XByteOrder::LittleEndian, 14),
        XWireRequest::GetGeometry {
            drawable: XResourceId::new(0x220306, 1),
        },
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(matches!(
        geometry.outputs.as_slice(),
        [XClientOutput::Reply(XClientReply::GetGeometry {
            geometry: Rect { width, .. },
            ..
        })] if u32::try_from(*width).unwrap() == advertised
    ));
}

/// The drawable types Sophia advertises are the ones it implements.
///
/// The catalog used to promise pixmap drawables with none of the four requests
/// that make one behind it -- the same advertise-then-refuse that cost this tree
/// a physical run twice. Withdrawing the bit is what makes the promise true, and
/// this keeps the two from parting again.
#[test]
fn glx_advertises_only_the_drawable_types_it_implements() {
    let namespace = NamespaceId::from_raw(77);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let configs = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, X_GLX_MAJOR_OPCODE),
        XWireRequest::GlxGetFbConfigs { screen: 0 },
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian)
    .remove(0);
    let attributes = read_u32(XByteOrder::LittleEndian, &configs[12..16]) as usize;
    let drawable_type = (0..attributes)
        .map(|index| {
            let at = 32 + index * 8;
            (
                read_u32(XByteOrder::LittleEndian, &configs[at..at + 4]),
                read_u32(XByteOrder::LittleEndian, &configs[at + 4..at + 8]),
            )
        })
        .find(|(name, _)| *name == 0x8010)
        .expect("the catalog states which drawable types it supports")
        .1;

    // Window and pbuffer, both implemented. Pixmap is not promised.
    assert_eq!(drawable_type & 0x1, 0x1, "window drawables are implemented");
    assert_eq!(drawable_type & 0x4, 0x4, "pbuffer drawables are implemented");
    assert_eq!(
        drawable_type & 0x2,
        0,
        "GLX pixmaps are not implemented, so they must not be advertised"
    );

    // And the requests behind the withdrawn bit really are absent, so the bit
    // cannot be restored without them.
    for minor in [13u8, 15, 22, 23] {
        let request = vec![X_GLX_MAJOR_OPCODE, minor, 1, 0];
        assert!(
            matches!(
                decode_x11_core_request(
                    context(namespace, 1, XByteOrder::LittleEndian),
                    &request
                ),
                Err(XWireParseError::UnknownOpcode(_))
            ),
            "GLX minor {minor} is advertised nowhere and implemented nowhere"
        );
    }
}

/// A pbuffer has no storage, so core drawing must keep refusing it. This pins
/// the deliberate narrowness of `validate_drawable_access`.
#[test]
fn core_drawing_still_refuses_an_offscreen_drawable() {
    let namespace = NamespaceId::from_raw(76);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let pbuffer = 0x220307;

    let create = decode_x11_core_request(
        context(namespace, 1, XByteOrder::LittleEndian),
        &glx_create_pbuffer_request(
            XByteOrder::LittleEndian,
            1,
            pbuffer,
            &[
                (X_GLX_PBUFFER_WIDTH_ATTRIBUTE, 8),
                (X_GLX_PBUFFER_HEIGHT_ATTRIBUTE, 8),
            ],
        ),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, X_GLX_MAJOR_OPCODE),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(
        runtime
            .validate_drawable_access(namespace, XResourceId::new(u64::from(pbuffer), 1))
            .is_err(),
        "a drawable with no pixels is not a drawing target"
    );
}
