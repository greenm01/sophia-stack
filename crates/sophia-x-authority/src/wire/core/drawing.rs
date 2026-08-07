fn decode_put_image(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_PUT_IMAGE, X_PUT_IMAGE_REQ_LEN, bytes.len())?;
    validate_wire_image_format(bytes[1])?;
    let data_len = bytes.len() - X_PUT_IMAGE_REQ_LEN;
    if data_len > X_PUT_IMAGE_MAX_DATA_BYTES {
        return Err(XWireParseError::PropertyValueTooLarge {
            len: data_len,
            max: X_PUT_IMAGE_MAX_DATA_BYTES,
        });
    }

    Ok(XWireRequest::PutImage {
        format: bytes[1],
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        width: context.byte_order.u16(&bytes[12..14]),
        height: context.byte_order.u16(&bytes[14..16]),
        dst_x: context.byte_order.i16(&bytes[16..18]),
        dst_y: context.byte_order.i16(&bytes[18..20]),
        left_pad: bytes[20],
        depth: bytes[21],
        data: bytes[X_PUT_IMAGE_REQ_LEN..].to_vec(),
    })
}

fn decode_get_image(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GET_IMAGE, X_GET_IMAGE_REQ_LEN, bytes.len())?;
    validate_wire_get_image_format(bytes[1])?;
    Ok(XWireRequest::GetImage {
        format: bytes[1],
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        x: context.byte_order.i16(&bytes[8..10]),
        y: context.byte_order.i16(&bytes[10..12]),
        width: context.byte_order.u16(&bytes[12..14]),
        height: context.byte_order.u16(&bytes[14..16]),
        plane_mask: context.byte_order.u32(&bytes[16..20]),
    })
}

fn decode_poly_text8(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_POLY_TEXT8, X_POLY_TEXT8_REQ_LEN, bytes.len())?;
    let item_bytes = &bytes[X_POLY_TEXT8_REQ_LEN..];
    if item_bytes.len() > X_POLY_TEXT8_MAX_BYTES {
        return Err(XWireParseError::PropertyValueTooLarge {
            len: item_bytes.len(),
            max: X_POLY_TEXT8_MAX_BYTES,
        });
    }

    let mut offset = 0usize;
    let mut text = Vec::new();
    while offset < item_bytes.len() {
        let len = item_bytes[offset];
        offset += 1;
        if len == 0 && item_bytes[offset..].iter().all(|byte| *byte == 0) {
            break;
        }
        if len == u8::MAX {
            if item_bytes.len().saturating_sub(offset) < 4 {
                return Err(XWireParseError::InvalidLength {
                    opcode: X_POLY_TEXT8,
                    expected_at_least: X_POLY_TEXT8_REQ_LEN + offset + 4,
                    actual: bytes.len(),
                });
            }
            offset += 4;
            continue;
        }

        let remaining = item_bytes.len().saturating_sub(offset);
        let glyph_len = usize::from(len);
        if remaining > glyph_len {
            offset += 1;
            text.extend_from_slice(&item_bytes[offset..offset + glyph_len]);
            offset += glyph_len;
            continue;
        }
        if remaining == glyph_len && glyph_len > 0 {
            offset += 1;
            text.extend_from_slice(&item_bytes[offset..offset + glyph_len - 1]);
            offset += glyph_len - 1;
            continue;
        }
        let item_len = 1usize + glyph_len;
        if remaining < item_len {
            return Err(XWireParseError::InvalidLength {
                opcode: X_POLY_TEXT8,
                expected_at_least: X_POLY_TEXT8_REQ_LEN + offset + item_len,
                actual: bytes.len(),
            });
        }
    }

    Ok(XWireRequest::PolyText8 {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        x: context.byte_order.i16(&bytes[12..14]),
        y: context.byte_order.i16(&bytes[14..16]),
        text,
    })
}

fn decode_image_text8(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_IMAGE_TEXT8, X_IMAGE_TEXT8_REQ_LEN, bytes.len())?;
    let text_len = usize::from(bytes[1]);
    if text_len > X_IMAGE_TEXT8_MAX_BYTES {
        return Err(XWireParseError::PropertyValueTooLarge {
            len: text_len,
            max: X_IMAGE_TEXT8_MAX_BYTES,
        });
    }
    let expected_len = X_IMAGE_TEXT8_REQ_LEN + padded_len(text_len);
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_IMAGE_TEXT8,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }

    Ok(XWireRequest::ImageText8 {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        x: context.byte_order.i16(&bytes[12..14]),
        y: context.byte_order.i16(&bytes[14..16]),
        text: bytes[X_IMAGE_TEXT8_REQ_LEN..X_IMAGE_TEXT8_REQ_LEN + text_len].to_vec(),
    })
}

fn decode_copy_area(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_COPY_AREA, X_COPY_AREA_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::CopyArea {
        source: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        destination: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[12..16])), 1),
        src_x: context.byte_order.i16(&bytes[16..18]),
        src_y: context.byte_order.i16(&bytes[18..20]),
        dst_x: context.byte_order.i16(&bytes[20..22]),
        dst_y: context.byte_order.i16(&bytes[22..24]),
        width: context.byte_order.u16(&bytes[24..26]),
        height: context.byte_order.u16(&bytes[26..28]),
    })
}

fn decode_poly_segment(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_POLY_SEGMENT, X_POLY_SEGMENT_REQ_LEN, bytes.len())?;
    let segment_bytes = &bytes[X_POLY_SEGMENT_REQ_LEN..];
    if !segment_bytes.len().is_multiple_of(8) {
        return Err(XWireParseError::InvalidLength {
            opcode: X_POLY_SEGMENT,
            expected_at_least: X_POLY_SEGMENT_REQ_LEN + ((segment_bytes.len() + 7) & !7),
            actual: bytes.len(),
        });
    }
    let mut damage = Vec::with_capacity(segment_bytes.len() / 8);
    for segment in segment_bytes.chunks_exact(8) {
        let x1 = i32::from(context.byte_order.i16(&segment[0..2]));
        let y1 = i32::from(context.byte_order.i16(&segment[2..4]));
        let x2 = i32::from(context.byte_order.i16(&segment[4..6]));
        let y2 = i32::from(context.byte_order.i16(&segment[6..8]));
        let x = x1.min(x2);
        let y = y1.min(y2);
        damage.push(Rect {
            x,
            y,
            width: x1.max(x2).saturating_sub(x).saturating_add(1),
            height: y1.max(y2).saturating_sub(y).saturating_add(1),
        });
    }
    Ok(XWireRequest::PolySegment {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        damage,
    })
}

fn decode_poly_line(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_POLY_LINE, X_POLY_LINE_REQ_LEN, bytes.len())?;
    let point_bytes = &bytes[X_POLY_LINE_REQ_LEN..];
    if !point_bytes.len().is_multiple_of(4) {
        return Err(XWireParseError::InvalidLength {
            opcode: X_POLY_LINE,
            expected_at_least: X_POLY_LINE_REQ_LEN + ((point_bytes.len() + 3) & !3),
            actual: bytes.len(),
        });
    }

    let mut points = Vec::with_capacity(point_bytes.len() / 4);
    let mut previous = XPoint { x: 0, y: 0 };
    for point in point_bytes.chunks_exact(4) {
        let mut decoded = XPoint {
            x: context.byte_order.i16(&point[0..2]),
            y: context.byte_order.i16(&point[2..4]),
        };
        if bytes[1] == 1 && !points.is_empty() {
            decoded.x = previous.x.saturating_add(decoded.x);
            decoded.y = previous.y.saturating_add(decoded.y);
        }
        previous = decoded;
        points.push(decoded);
    }
    Ok(XWireRequest::PolyLine {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        points,
    })
}

fn decode_fill_poly(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_FILL_POLY, X_FILL_POLY_REQ_LEN, bytes.len())?;
    let point_bytes = &bytes[X_FILL_POLY_REQ_LEN..];
    if !point_bytes.len().is_multiple_of(4) {
        return Err(XWireParseError::InvalidLength {
            opcode: X_FILL_POLY,
            expected_at_least: X_FILL_POLY_REQ_LEN + ((point_bytes.len() + 3) & !3),
            actual: bytes.len(),
        });
    }

    Ok(XWireRequest::FillPoly {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        damage: point_damage_bounds(context, point_bytes),
    })
}

fn point_damage_bounds(context: XWireClientContext, point_bytes: &[u8]) -> Option<Rect> {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for point in point_bytes.chunks_exact(4) {
        let x = i32::from(context.byte_order.i16(&point[0..2]));
        let y = i32::from(context.byte_order.i16(&point[2..4]));
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    if min_x == i32::MAX {
        None
    } else {
        Some(Rect {
            x: min_x,
            y: min_y,
            width: max_x.saturating_sub(min_x).saturating_add(1),
            height: max_y.saturating_sub(min_y).saturating_add(1),
        })
    }
}

fn decode_poly_fill_rectangle(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(
        X_POLY_FILL_RECTANGLE,
        X_POLY_FILL_RECTANGLE_REQ_LEN,
        bytes.len(),
    )?;
    let rectangle_bytes = &bytes[X_POLY_FILL_RECTANGLE_REQ_LEN..];
    if !rectangle_bytes.len().is_multiple_of(8) {
        return Err(XWireParseError::InvalidLength {
            opcode: X_POLY_FILL_RECTANGLE,
            expected_at_least: X_POLY_FILL_RECTANGLE_REQ_LEN + ((rectangle_bytes.len() + 7) & !7),
            actual: bytes.len(),
        });
    }
    let mut rectangles = Vec::with_capacity(rectangle_bytes.len() / 8);
    for rectangle in rectangle_bytes.chunks_exact(8) {
        rectangles.push(Rect {
            x: i32::from(context.byte_order.i16(&rectangle[0..2])),
            y: i32::from(context.byte_order.i16(&rectangle[2..4])),
            width: i32::from(context.byte_order.u16(&rectangle[4..6])),
            height: i32::from(context.byte_order.u16(&rectangle[6..8])),
        });
    }
    Ok(XWireRequest::PolyFillRectangle {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        rectangles,
    })
}

fn decode_poly_fill_arc(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_POLY_FILL_ARC, X_POLY_FILL_ARC_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::PolyFillArc {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        damage: arc_damage_bounds(context, X_POLY_FILL_ARC, X_POLY_FILL_ARC_REQ_LEN, bytes)?,
    })
}

fn arc_damage_bounds(
    context: XWireClientContext,
    opcode: u8,
    header_len: usize,
    bytes: &[u8],
) -> Result<Vec<Rect>, XWireParseError> {
    let arc_bytes = &bytes[header_len..];
    if !arc_bytes.len().is_multiple_of(12) {
        return Err(XWireParseError::InvalidLength {
            opcode,
            expected_at_least: header_len + arc_bytes.len().div_ceil(12) * 12,
            actual: bytes.len(),
        });
    }

    let mut damage = Vec::with_capacity(arc_bytes.len() / 12);
    for arc in arc_bytes.chunks_exact(12) {
        damage.push(Rect {
            x: i32::from(context.byte_order.i16(&arc[0..2])),
            y: i32::from(context.byte_order.i16(&arc[2..4])),
            width: i32::from(context.byte_order.u16(&arc[4..6])),
            height: i32::from(context.byte_order.u16(&arc[6..8])),
        });
    }
    Ok(damage)
}
