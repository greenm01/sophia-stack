fn decode_xfixes(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    match bytes[1] {
        X_XFIXES_QUERY_VERSION_MINOR_OPCODE => decode_extension_query_version(
            context,
            bytes,
            X_XFIXES_MAJOR_OPCODE,
            X_XFIXES_QUERY_VERSION_MINOR_OPCODE,
            |major_version, minor_version| XWireRequest::XfixesQueryVersion {
                major_version,
                minor_version,
            },
        ),
        X_XFIXES_SELECT_SELECTION_INPUT_MINOR_OPCODE => {
            require_exact_len(X_XFIXES_MAJOR_OPCODE, 16, bytes.len())?;
            Ok(XWireRequest::XfixesSelectSelectionInput {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                selection: context.byte_order.u32(&bytes[8..12]),
                event_mask: context.byte_order.u32(&bytes[12..16]),
            })
        }
        X_XFIXES_CREATE_REGION_MINOR_OPCODE => {
            require_len(X_XFIXES_MAJOR_OPCODE, 8, bytes.len())?;
            if (bytes.len() - 8) % 8 != 0 {
                return Err(XWireParseError::InvalidLength {
                    opcode: X_XFIXES_MAJOR_OPCODE,
                    expected_at_least: 8,
                    actual: bytes.len(),
                });
            }
            let region = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(region)?;
            let rectangles = bytes[8..]
                .chunks_exact(8)
                .map(|rectangle| Rect {
                    x: i32::from(context.byte_order.i16(&rectangle[0..2])),
                    y: i32::from(context.byte_order.i16(&rectangle[2..4])),
                    width: i32::from(context.byte_order.u16(&rectangle[4..6])),
                    height: i32::from(context.byte_order.u16(&rectangle[6..8])),
                })
                .collect();
            Ok(XWireRequest::XfixesCreateRegion {
                region: XResourceId::new(u64::from(region), 1),
                rectangles,
            })
        }
        X_XFIXES_SET_REGION_MINOR_OPCODE => {
            require_len(X_XFIXES_MAJOR_OPCODE, 8, bytes.len())?;
            if (bytes.len() - 8) % 8 != 0 {
                return Err(XWireParseError::InvalidLength {
                    opcode: X_XFIXES_MAJOR_OPCODE,
                    expected_at_least: 8,
                    actual: bytes.len(),
                });
            }
            let rectangles = bytes[8..]
                .chunks_exact(8)
                .map(|rectangle| Rect {
                    x: i32::from(context.byte_order.i16(&rectangle[0..2])),
                    y: i32::from(context.byte_order.i16(&rectangle[2..4])),
                    width: i32::from(context.byte_order.u16(&rectangle[4..6])),
                    height: i32::from(context.byte_order.u16(&rectangle[6..8])),
                })
                .collect();
            Ok(XWireRequest::XfixesSetRegion {
                region: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                rectangles,
            })
        }
        X_XFIXES_DESTROY_REGION_MINOR_OPCODE => {
            require_exact_len(X_XFIXES_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::XfixesDestroyRegion {
                region: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        _ => Err(XWireParseError::UnknownOpcode(bytes[1])),
    }
}

