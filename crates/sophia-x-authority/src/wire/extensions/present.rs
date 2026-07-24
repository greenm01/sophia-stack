fn decode_present(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    match bytes[1] {
        X_PRESENT_QUERY_VERSION_MINOR_OPCODE => decode_extension_query_version(
            context,
            bytes,
            X_PRESENT_MAJOR_OPCODE,
            X_PRESENT_QUERY_VERSION_MINOR_OPCODE,
            |major_version, minor_version| XWireRequest::PresentQueryVersion {
                major_version,
                minor_version,
            },
        ),
        X_PRESENT_PIXMAP_MINOR_OPCODE => {
            require_len(X_PRESENT_MAJOR_OPCODE, 72, bytes.len())?;
            if !(bytes.len() - 72).is_multiple_of(8) {
                return Err(XWireParseError::InvalidLength {
                    opcode: X_PRESENT_MAJOR_OPCODE,
                    expected_at_least: 72,
                    actual: bytes.len(),
                });
            }
            let raw_resource = |offset: usize| context.byte_order.u32(&bytes[offset..offset + 4]);
            let resource = |offset: usize| XResourceId::new(u64::from(raw_resource(offset)), 1);
            let optional_resource = |offset: usize| {
                let raw = raw_resource(offset);
                (raw != 0).then(|| XResourceId::new(u64::from(raw), 1))
            };
            let notifies = bytes[72..]
                .chunks_exact(8)
                .map(|notify| {
                    (
                        XResourceId::new(u64::from(context.byte_order.u32(&notify[..4])), 1),
                        context.byte_order.u32(&notify[4..]),
                    )
                })
                .collect();
            Ok(XWireRequest::PresentPixmap {
                transaction: context.transaction,
                window: resource(4),
                pixmap: resource(8),
                serial: raw_resource(12),
                valid_region: raw_resource(16),
                update_region: raw_resource(20),
                x_offset: context.byte_order.i16(&bytes[24..26]),
                y_offset: context.byte_order.i16(&bytes[26..28]),
                target_crtc: raw_resource(28),
                wait_fence: optional_resource(32),
                idle_fence: optional_resource(36),
                options: raw_resource(40),
                target_msc: context.byte_order.u64(&bytes[48..56]),
                divisor: context.byte_order.u64(&bytes[56..64]),
                remainder: context.byte_order.u64(&bytes[64..72]),
                notifies,
            })
        }
        X_PRESENT_SELECT_INPUT_MINOR_OPCODE => {
            require_exact_len(X_PRESENT_MAJOR_OPCODE, 16, bytes.len())?;
            let event_id = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(event_id)?;
            Ok(XWireRequest::PresentSelectInput {
                event_id: XResourceId::new(u64::from(event_id), 1),
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
                event_mask: context.byte_order.u32(&bytes[12..16]),
            })
        }
        X_PRESENT_QUERY_CAPABILITIES_MINOR_OPCODE => {
            require_exact_len(X_PRESENT_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::PresentQueryCapabilities {
                target: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        _ => Err(XWireParseError::UnknownOpcode(bytes[1])),
    }
}

