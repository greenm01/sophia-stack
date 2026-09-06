fn decode_render(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    match bytes[1] {
        X_RENDER_QUERY_VERSION_MINOR_OPCODE => {
            require_exact_len(
                X_RENDER_MAJOR_OPCODE,
                X_RENDER_QUERY_VERSION_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RenderQueryVersion {
                major: context.byte_order.u32(&bytes[4..8]),
                minor: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_RENDER_QUERY_PICT_FORMATS_MINOR_OPCODE => {
            require_exact_len(
                X_RENDER_MAJOR_OPCODE,
                X_RENDER_QUERY_PICT_FORMATS_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RenderQueryPictFormats)
        }
        X_RENDER_CREATE_PICTURE_MINOR_OPCODE => {
            require_len(X_RENDER_MAJOR_OPCODE, 20, bytes.len())?;
            let picture = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(picture)?;
            let mask = context.byte_order.u32(&bytes[16..20]);
            let values = decode_render_picture_values(context.byte_order, mask, &bytes[20..])?;
            Ok(XWireRequest::RenderCreatePicture {
                picture: XResourceId::new(u64::from(picture), 1),
                drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
                format: context.byte_order.u32(&bytes[12..16]),
                values,
            })
        }
        X_RENDER_CHANGE_PICTURE_MINOR_OPCODE => {
            require_len(X_RENDER_MAJOR_OPCODE, 12, bytes.len())?;
            let mask = context.byte_order.u32(&bytes[8..12]);
            let values = decode_render_picture_values(context.byte_order, mask, &bytes[12..])?;
            Ok(XWireRequest::RenderChangePicture {
                picture: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                values,
            })
        }
        X_RENDER_SET_PICTURE_CLIP_RECTANGLES_MINOR_OPCODE => {
            require_len(X_RENDER_MAJOR_OPCODE, 12, bytes.len())?;
            if !(bytes.len() - 12).is_multiple_of(8) {
                return Err(XWireParseError::InvalidLength {
                    opcode: X_RENDER_MAJOR_OPCODE,
                    expected_at_least: 12,
                    actual: bytes.len(),
                });
            }
            Ok(XWireRequest::RenderSetPictureClipRectangles {
                picture: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                clip_x_origin: context.byte_order.i16(&bytes[8..10]),
                clip_y_origin: context.byte_order.i16(&bytes[10..12]),
                rectangles: decode_render_rectangles(context.byte_order, &bytes[12..]),
            })
        }
        X_RENDER_FREE_PICTURE_MINOR_OPCODE => {
            require_exact_len(X_RENDER_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::RenderFreePicture {
                picture: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        X_RENDER_FILL_RECTANGLES_MINOR_OPCODE => {
            require_len(X_RENDER_MAJOR_OPCODE, 20, bytes.len())?;
            if !(bytes.len() - 20).is_multiple_of(8) {
                return Err(XWireParseError::InvalidLength {
                    opcode: X_RENDER_MAJOR_OPCODE,
                    expected_at_least: 20,
                    actual: bytes.len(),
                });
            }
            Ok(XWireRequest::RenderFillRectangles {
                op: bytes[4],
                picture: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
                color: [
                    context.byte_order.u16(&bytes[12..14]),
                    context.byte_order.u16(&bytes[14..16]),
                    context.byte_order.u16(&bytes[16..18]),
                    context.byte_order.u16(&bytes[18..20]),
                ],
                rectangles: decode_render_rectangles(context.byte_order, &bytes[20..]),
            })
        }
        X_RENDER_COMPOSITE_MINOR_OPCODE => {
            require_exact_len(X_RENDER_MAJOR_OPCODE, 36, bytes.len())?;
            let mask = context.byte_order.u32(&bytes[12..16]);
            Ok(XWireRequest::RenderComposite {
                op: bytes[4],
                source: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
                // Mask None is the common case: a plain blit sends zero here.
                mask: (mask != 0).then(|| XResourceId::new(u64::from(mask), 1)),
                destination: XResourceId::new(
                    u64::from(context.byte_order.u32(&bytes[16..20])),
                    1,
                ),
                source_x: context.byte_order.i16(&bytes[20..22]),
                source_y: context.byte_order.i16(&bytes[22..24]),
                mask_x: context.byte_order.i16(&bytes[24..26]),
                mask_y: context.byte_order.i16(&bytes[26..28]),
                destination_x: context.byte_order.i16(&bytes[28..30]),
                destination_y: context.byte_order.i16(&bytes[30..32]),
                width: context.byte_order.u16(&bytes[32..34]),
                height: context.byte_order.u16(&bytes[34..36]),
            })
        }
        // Decoded so the refusal can name the request. RENDER has thirty-six
        // minors and this server implements a subset; a parse rejection would
        // tell a client only that the extension exists, not which request it
        // was denied.
        minor_opcode => Ok(XWireRequest::RenderUnimplemented { minor_opcode }),
    }
}

/// The picture attributes this server acts on, plus what it refuses.
///
/// The full CP mask carries thirteen attributes; the ones with no effect
/// here -- subwindow mode, poly edge and mode, dither, graphics exposures,
/// alpha-map origins -- decode and are dropped, because none can matter to a
/// server that composites on CPU buffers with no subwindows or exposure
/// events in this path. Alpha maps and pixmap clip masks are refused rather
/// than dropped: dropping one silently changes what the client drew.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XRenderPictureValueSet {
    pub repeat: Option<u32>,
    pub clip_x_origin: Option<i16>,
    pub clip_y_origin: Option<i16>,
    pub component_alpha: Option<u32>,
    /// The mask named CPAlphaMap or CPClipMask with a value other than None.
    pub refused_attribute: bool,
    /// The mask carried bits no protocol version defines.
    pub invalid_mask: bool,
}

fn decode_render_picture_values(
    byte_order: XByteOrder,
    mask: u32,
    values: &[u8],
) -> Result<XRenderPictureValueSet, XWireParseError> {
    let expected = usize::try_from(mask.count_ones()).unwrap_or(0) * 4;
    if values.len() != expected {
        return Err(XWireParseError::InvalidLength {
            opcode: X_RENDER_MAJOR_OPCODE,
            expected_at_least: expected,
            actual: values.len(),
        });
    }
    let mut set = XRenderPictureValueSet {
        invalid_mask: mask & !0x1fff != 0,
        ..Default::default()
    };
    let mut offset = 0;
    for bit in 0..32 {
        if mask & (1 << bit) == 0 {
            continue;
        }
        let value = byte_order.u32(&values[offset..offset + 4]);
        offset += 4;
        match bit {
            0 => set.repeat = Some(value),
            // CPAlphaMap: None means no alpha map, which is the one value
            // this server implements.
            1 | 6 => {
                if value != 0 {
                    set.refused_attribute = true;
                }
            }
            4 => set.clip_x_origin = Some(value as u16 as i16),
            5 => set.clip_y_origin = Some(value as u16 as i16),
            12 => set.component_alpha = Some(value),
            // Origins for the refused alpha map, and the attributes with no
            // effect here.
            _ => {}
        }
    }
    Ok(set)
}

fn decode_render_rectangles(byte_order: XByteOrder, bytes: &[u8]) -> Vec<Rect> {
    bytes
        .chunks_exact(8)
        .map(|rectangle| Rect {
            x: i32::from(byte_order.i16(&rectangle[0..2])),
            y: i32::from(byte_order.i16(&rectangle[2..4])),
            width: i32::from(byte_order.u16(&rectangle[4..6])),
            height: i32::from(byte_order.u16(&rectangle[6..8])),
        })
        .collect()
}
