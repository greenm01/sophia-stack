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
        X_RENDER_CREATE_GLYPH_SET_MINOR_OPCODE => {
            require_exact_len(X_RENDER_MAJOR_OPCODE, 12, bytes.len())?;
            let glyphset = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(glyphset)?;
            Ok(XWireRequest::RenderCreateGlyphSet {
                glyphset: XResourceId::new(u64::from(glyphset), 1),
                format: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_RENDER_REFERENCE_GLYPH_SET_MINOR_OPCODE => {
            require_exact_len(X_RENDER_MAJOR_OPCODE, 12, bytes.len())?;
            let glyphset = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(glyphset)?;
            Ok(XWireRequest::RenderReferenceGlyphSet {
                glyphset: XResourceId::new(u64::from(glyphset), 1),
                existing: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
            })
        }
        X_RENDER_FREE_GLYPH_SET_MINOR_OPCODE => {
            require_exact_len(X_RENDER_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::RenderFreeGlyphSet {
                glyphset: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        X_RENDER_ADD_GLYPHS_MINOR_OPCODE => {
            require_len(X_RENDER_MAJOR_OPCODE, 12, bytes.len())?;
            let count = context.byte_order.u32(&bytes[8..12]);
            // The count is a CARD32 and the glyph table that follows is
            // bounded by the request itself; check the two agree before
            // allocating anything sized by the client's number.
            let count = usize::try_from(count).map_err(|_| XWireParseError::InvalidLength {
                opcode: X_RENDER_MAJOR_OPCODE,
                expected_at_least: 12,
                actual: bytes.len(),
            })?;
            let table_len = count.checked_mul(12).ok_or(XWireParseError::InvalidLength {
                opcode: X_RENDER_MAJOR_OPCODE,
                expected_at_least: 12,
                actual: bytes.len(),
            })?;
            let ids_len = count.checked_mul(4).ok_or(XWireParseError::InvalidLength {
                opcode: X_RENDER_MAJOR_OPCODE,
                expected_at_least: 12,
                actual: bytes.len(),
            })?;
            let header_len = 12usize
                .checked_add(ids_len)
                .and_then(|len| len.checked_add(table_len))
                .ok_or(XWireParseError::InvalidLength {
                    opcode: X_RENDER_MAJOR_OPCODE,
                    expected_at_least: 12,
                    actual: bytes.len(),
                })?;
            require_len(X_RENDER_MAJOR_OPCODE, header_len, bytes.len())?;
            let ids = bytes[12..12 + ids_len]
                .chunks_exact(4)
                .map(|id| context.byte_order.u32(id))
                .collect::<Vec<_>>();
            let mut glyphs = Vec::with_capacity(count);
            for entry in bytes[12 + ids_len..header_len].chunks_exact(12) {
                glyphs.push(XRenderGlyphInfo {
                    width: context.byte_order.u16(&entry[0..2]),
                    height: context.byte_order.u16(&entry[2..4]),
                    x: context.byte_order.i16(&entry[4..6]),
                    y: context.byte_order.i16(&entry[6..8]),
                    off_x: context.byte_order.i16(&entry[8..10]),
                    off_y: context.byte_order.i16(&entry[10..12]),
                });
            }
            Ok(XWireRequest::RenderAddGlyphs {
                glyphset: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                ids,
                glyphs,
                data: bytes[header_len..].to_vec(),
            })
        }
        X_RENDER_FREE_GLYPHS_MINOR_OPCODE => {
            require_len(X_RENDER_MAJOR_OPCODE, 8, bytes.len())?;
            if !(bytes.len() - 8).is_multiple_of(4) {
                return Err(XWireParseError::InvalidLength {
                    opcode: X_RENDER_MAJOR_OPCODE,
                    expected_at_least: 8,
                    actual: bytes.len(),
                });
            }
            Ok(XWireRequest::RenderFreeGlyphs {
                glyphset: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                ids: bytes[8..]
                    .chunks_exact(4)
                    .map(|id| context.byte_order.u32(id))
                    .collect(),
            })
        }
        minor @ (X_RENDER_COMPOSITE_GLYPHS_8_MINOR_OPCODE
        | X_RENDER_COMPOSITE_GLYPHS_16_MINOR_OPCODE
        | X_RENDER_COMPOSITE_GLYPHS_32_MINOR_OPCODE) => {
            require_len(X_RENDER_MAJOR_OPCODE, 28, bytes.len())?;
            let id_width = match minor {
                X_RENDER_COMPOSITE_GLYPHS_8_MINOR_OPCODE => 1,
                X_RENDER_COMPOSITE_GLYPHS_16_MINOR_OPCODE => 2,
                _ => 4,
            };
            Ok(XWireRequest::RenderCompositeGlyphs {
                op: bytes[4],
                source: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
                destination: XResourceId::new(
                    u64::from(context.byte_order.u32(&bytes[12..16])),
                    1,
                ),
                mask_format: context.byte_order.u32(&bytes[16..20]),
                glyphset: XResourceId::new(u64::from(context.byte_order.u32(&bytes[20..24])), 1),
                source_x: context.byte_order.i16(&bytes[24..26]),
                source_y: context.byte_order.i16(&bytes[26..28]),
                elements: decode_render_glyph_elements(
                    context.byte_order,
                    &bytes[28..],
                    id_width,
                ),
                minor_opcode: minor,
            })
        }
        X_RENDER_CREATE_CURSOR_MINOR_OPCODE => {
            require_exact_len(X_RENDER_MAJOR_OPCODE, 16, bytes.len())?;
            let cursor = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(cursor)?;
            Ok(XWireRequest::RenderCreateCursor {
                cursor: XResourceId::new(u64::from(cursor), 1),
                source: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
                hotspot_x: context.byte_order.u16(&bytes[12..14]),
                hotspot_y: context.byte_order.u16(&bytes[14..16]),
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

/// One glyph's placement metrics, as `AddGlyphs` sends them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XRenderGlyphInfo {
    pub width: u16,
    pub height: u16,
    /// The origin's offset inside the bitmap, positive downward and rightward
    /// from the top-left, which is how RENDER carries a glyph's bearing.
    pub x: i16,
    pub y: i16,
    pub off_x: i16,
    pub off_y: i16,
}

/// One run of glyphs drawn at an offset, from a possibly different set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XRenderGlyphElement {
    /// A glyph set switch, when the element carried one.
    pub glyphset: Option<XResourceId>,
    pub delta_x: i16,
    pub delta_y: i16,
    pub glyphs: Vec<u32>,
}

/// Walk the glyph element list.
///
/// The list is self-describing rather than counted: a leading byte of 255
/// marks a glyph-set switch carrying a four-byte id, anything else is a run
/// of that many glyph identifiers preceded by two deltas. A malformed tail
/// stops the walk instead of failing the request, because the elements
/// already parsed are well-formed and a client that truncated its own list
/// gets what it asked to draw.
fn decode_render_glyph_elements(
    byte_order: XByteOrder,
    mut bytes: &[u8],
    id_width: usize,
) -> Vec<XRenderGlyphElement> {
    let mut elements = Vec::new();
    let mut pending_glyphset = None;
    while bytes.len() >= 8 {
        let count = bytes[0];
        if count == 255 {
            pending_glyphset = Some(XResourceId::new(
                u64::from(byte_order.u32(&bytes[4..8])),
                1,
            ));
            bytes = &bytes[8..];
            continue;
        }
        let count = usize::from(count);
        let glyph_bytes = count.saturating_mul(id_width);
        // Runs are padded to a four-byte boundary.
        let padded = glyph_bytes.next_multiple_of(4);
        if bytes.len() < 8 + padded {
            break;
        }
        let glyphs = bytes[8..8 + glyph_bytes]
            .chunks_exact(id_width)
            .map(|id| match id_width {
                1 => u32::from(id[0]),
                2 => u32::from(byte_order.u16(id)),
                _ => byte_order.u32(id),
            })
            .collect();
        elements.push(XRenderGlyphElement {
            glyphset: pending_glyphset.take(),
            delta_x: byte_order.i16(&bytes[4..6]),
            delta_y: byte_order.i16(&bytes[6..8]),
            glyphs,
        });
        bytes = &bytes[8 + padded..];
    }
    elements
}
