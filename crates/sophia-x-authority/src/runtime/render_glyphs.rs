/// One glyph: its metrics and its coverage or colour bytes, already unpacked
/// into the tight `width * height` layout the compositor samples.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct XRenderGlyph {
    pub info: crate::XRenderGlyphInfo,
    /// Premultiplied `[b, g, r, a]` per pixel. A coverage-only set stores its
    /// coverage in the alpha byte, which is what makes one sampling path
    /// serve both a mask set and a colour set.
    pub pixels: Vec<[u8; 4]>,
}

/// A glyph set's shared contents.
///
/// `ReferenceGlyphSet` gives an existing set a second identifier, and the
/// protocol is explicit that the two names share storage rather than copying
/// it -- a glyph added through one is visible through the other. Refcounting
/// the store is what makes that true.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct XRenderGlyphStore {
    pub references: u32,
    pub format: crate::XRenderPictFormatKind,
    pub glyphs: BTreeMap<u32, XRenderGlyph>,
}

/// Why a glyph request was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum XRenderGlyphError {
    UnknownGlyphSet,
    UnknownGlyph,
    IdInUse,
    /// A format id that names no format, or one a glyph set may not take.
    UnsupportedFormat,
    /// The glyph table and the image bytes disagree about how much data the
    /// request carries.
    MalformedGlyphData,
}

impl XAuthorityRuntime {
    /// The scanline stride RENDER pads glyph image rows to, per format.
    fn render_glyph_stride(format: crate::XRenderPictFormatKind, width: u16) -> Option<usize> {
        let width = usize::from(width);
        match format {
            crate::XRenderPictFormatKind::A1 => width.checked_add(31).map(|w| (w / 32) * 4),
            crate::XRenderPictFormatKind::A8 => width.checked_add(3).map(|w| w & !3),
            crate::XRenderPictFormatKind::Argb32 => width.checked_mul(4),
            // A glyph set is a mask or a colour set; RGB24 has no alpha to
            // carry coverage and is refused at creation.
            crate::XRenderPictFormatKind::Rgb24 => None,
        }
    }

    pub(crate) fn render_create_glyph_set(
        &mut self,
        namespace: NamespaceId,
        glyphset: crate::XResourceId,
        format_id: u32,
        generation: u64,
    ) -> Result<(), XRenderGlyphError> {
        if self.resource_id_in_use(glyphset) {
            return Err(XRenderGlyphError::IdInUse);
        }
        let format = crate::XRenderPictFormatKind::from_format_id(format_id)
            .ok_or(XRenderGlyphError::UnsupportedFormat)?;
        // ARGB32 is the subpixel path: a set whose glyphs carry colour, which
        // a component-alpha mask then renders. RGB24 cannot carry coverage.
        if matches!(format, crate::XRenderPictFormatKind::Rgb24) {
            return Err(XRenderGlyphError::UnsupportedFormat);
        }
        self.resources
            .insert(glyphset, XResourceKind::GlyphSet, namespace, generation)
            .map_err(|_| XRenderGlyphError::IdInUse)?;
        let store = self.next_glyph_store;
        self.next_glyph_store = self.next_glyph_store.saturating_add(1);
        self.render_glyph_stores.insert(
            store,
            XRenderGlyphStore {
                references: 1,
                format,
                glyphs: BTreeMap::new(),
            },
        );
        self.render_glyphsets.insert(glyphset, store);
        Ok(())
    }

    pub(crate) fn render_reference_glyph_set(
        &mut self,
        namespace: NamespaceId,
        glyphset: crate::XResourceId,
        existing: crate::XResourceId,
        generation: u64,
    ) -> Result<(), XRenderGlyphError> {
        if self.resource_id_in_use(glyphset) {
            return Err(XRenderGlyphError::IdInUse);
        }
        let store = self.render_glyph_store_id(namespace, existing)?;
        self.resources
            .insert(glyphset, XResourceKind::GlyphSet, namespace, generation)
            .map_err(|_| XRenderGlyphError::IdInUse)?;
        if let Some(entry) = self.render_glyph_stores.get_mut(&store) {
            entry.references = entry.references.saturating_add(1);
        }
        self.render_glyphsets.insert(glyphset, store);
        Ok(())
    }

    fn render_glyph_store_id(
        &self,
        namespace: NamespaceId,
        glyphset: crate::XResourceId,
    ) -> Result<u64, XRenderGlyphError> {
        self.resources
            .lookup(namespace, glyphset, XResourceKind::GlyphSet)
            .map_err(|_| XRenderGlyphError::UnknownGlyphSet)?;
        self.render_glyphsets
            .get(&glyphset)
            .copied()
            .ok_or(XRenderGlyphError::UnknownGlyphSet)
    }

    pub(crate) fn render_free_glyph_set(
        &mut self,
        namespace: NamespaceId,
        glyphset: crate::XResourceId,
    ) -> Result<(), XRenderGlyphError> {
        let store = self.render_glyph_store_id(namespace, glyphset)?;
        self.resources.remove(glyphset);
        self.render_glyphsets.remove(&glyphset);
        // The contents outlive this name for as long as another name holds
        // them.
        let drop_store = match self.render_glyph_stores.get_mut(&store) {
            Some(entry) => {
                entry.references = entry.references.saturating_sub(1);
                entry.references == 0
            }
            None => false,
        };
        if drop_store {
            self.render_glyph_stores.remove(&store);
        }
        Ok(())
    }

    pub(crate) fn render_add_glyphs(
        &mut self,
        namespace: NamespaceId,
        glyphset: crate::XResourceId,
        ids: &[u32],
        glyphs: &[crate::XRenderGlyphInfo],
        data: &[u8],
    ) -> Result<(), XRenderGlyphError> {
        let store_id = self.render_glyph_store_id(namespace, glyphset)?;
        let format = self
            .render_glyph_stores
            .get(&store_id)
            .ok_or(XRenderGlyphError::UnknownGlyphSet)?
            .format;
        if ids.len() != glyphs.len() {
            return Err(XRenderGlyphError::MalformedGlyphData);
        }
        // Unpack every glyph before storing any, so a request whose byte
        // count disagrees with its glyph table leaves the set untouched
        // rather than half-updated.
        let mut unpacked = Vec::with_capacity(ids.len());
        let mut offset = 0usize;
        for (id, info) in ids.iter().zip(glyphs) {
            let stride =
                Self::render_glyph_stride(format, info.width).ok_or(XRenderGlyphError::UnsupportedFormat)?;
            let height = usize::from(info.height);
            let len = stride
                .checked_mul(height)
                .ok_or(XRenderGlyphError::MalformedGlyphData)?;
            let end = offset
                .checked_add(len)
                .ok_or(XRenderGlyphError::MalformedGlyphData)?;
            let image = data
                .get(offset..end)
                .ok_or(XRenderGlyphError::MalformedGlyphData)?;
            unpacked.push((*id, XRenderGlyph {
                info: *info,
                pixels: Self::render_unpack_glyph(format, *info, stride, image),
            }));
            offset = end;
        }
        let store = self
            .render_glyph_stores
            .get_mut(&store_id)
            .ok_or(XRenderGlyphError::UnknownGlyphSet)?;
        for (id, glyph) in unpacked {
            store.glyphs.insert(id, glyph);
        }
        Ok(())
    }

    /// Expand one glyph's padded scanlines into tight premultiplied pixels.
    fn render_unpack_glyph(
        format: crate::XRenderPictFormatKind,
        info: crate::XRenderGlyphInfo,
        stride: usize,
        image: &[u8],
    ) -> Vec<[u8; 4]> {
        let width = usize::from(info.width);
        let height = usize::from(info.height);
        let mut pixels = Vec::with_capacity(width.saturating_mul(height));
        for y in 0..height {
            let row = y.saturating_mul(stride);
            for x in 0..width {
                let pixel = match format {
                    crate::XRenderPictFormatKind::A8 => {
                        let coverage = image.get(row + x).copied().unwrap_or(0);
                        [0, 0, 0, coverage]
                    }
                    crate::XRenderPictFormatKind::A1 => {
                        // Bit order within a byte is least-significant first,
                        // matching the server's image byte order.
                        let byte = image.get(row + x / 8).copied().unwrap_or(0);
                        let set = byte & (1 << (x % 8)) != 0;
                        [0, 0, 0, if set { 0xff } else { 0 }]
                    }
                    crate::XRenderPictFormatKind::Argb32 => {
                        let offset = row + x * 4;
                        image
                            .get(offset..offset + 4)
                            .and_then(|slice| slice.try_into().ok())
                            .unwrap_or([0; 4])
                    }
                    crate::XRenderPictFormatKind::Rgb24 => [0; 4],
                };
                pixels.push(pixel);
            }
        }
        pixels
    }

    pub(crate) fn render_free_glyphs(
        &mut self,
        namespace: NamespaceId,
        glyphset: crate::XResourceId,
        ids: &[u32],
    ) -> Result<(), XRenderGlyphError> {
        let store_id = self.render_glyph_store_id(namespace, glyphset)?;
        let store = self
            .render_glyph_stores
            .get_mut(&store_id)
            .ok_or(XRenderGlyphError::UnknownGlyphSet)?;
        for id in ids {
            store.glyphs.remove(id);
        }
        Ok(())
    }
}

impl XAuthorityRuntime {
    /// Draw glyph runs onto a destination picture.
    ///
    /// Each glyph's coverage attenuates the source colour and the result is
    /// composited at the pen position, which advances by the glyph's own
    /// offsets. Elements may switch glyph sets mid-run, and the pen carries
    /// across them -- that is how a client draws one line of text out of two
    /// fonts.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_apply_composite_glyphs(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        op: u8,
        source: crate::XResourceId,
        destination: crate::XResourceId,
        glyphset: crate::XResourceId,
        source_origin: (i16, i16),
        elements: &[crate::XRenderGlyphElement],
    ) -> Result<XAuthorityResponsePacket, XRenderCompositeGlyphsError> {
        let source_record = self
            .render_picture_record(namespace, source)
            .map_err(XRenderCompositeGlyphsError::Picture)?;
        let destination_record = self
            .render_picture_record(namespace, destination)
            .map_err(XRenderCompositeGlyphsError::Picture)?;
        let mut store_id = self
            .render_glyph_store_id(namespace, glyphset)
            .map_err(XRenderCompositeGlyphsError::Glyph)?;
        let Some((size, window_generation)) =
            self.render_target_geometry(namespace, &destination_record)
        else {
            return Err(XRenderCompositeGlyphsError::Picture(
                XRenderPictureError::Drawable,
            ));
        };

        // Resolve every glyph before drawing any, so a run naming a glyph the
        // set does not hold refuses without having drawn its prefix.
        let mut placements = Vec::new();
        let mut pen = (0i32, 0i32);
        for element in elements {
            if let Some(switch) = element.glyphset {
                store_id = self
                    .render_glyph_store_id(namespace, switch)
                    .map_err(XRenderCompositeGlyphsError::Glyph)?;
            }
            pen.0 = pen.0.saturating_add(i32::from(element.delta_x));
            pen.1 = pen.1.saturating_add(i32::from(element.delta_y));
            let store = self
                .render_glyph_stores
                .get(&store_id)
                .ok_or(XRenderCompositeGlyphsError::Glyph(
                    XRenderGlyphError::UnknownGlyphSet,
                ))?;
            for id in &element.glyphs {
                let glyph = store.glyphs.get(id).ok_or(
                    XRenderCompositeGlyphsError::Glyph(XRenderGlyphError::UnknownGlyph),
                )?;
                // The glyph's origin sits inside its bitmap, so the bitmap's
                // top-left is the pen less that bearing.
                placements.push((
                    pen.0.saturating_sub(i32::from(glyph.info.x)),
                    pen.1.saturating_sub(i32::from(glyph.info.y)),
                    glyph.clone(),
                ));
                pen.0 = pen.0.saturating_add(i32::from(glyph.info.off_x));
                pen.1 = pen.1.saturating_add(i32::from(glyph.info.off_y));
            }
        }
        if placements.is_empty() {
            return Ok(XAuthorityResponsePacket::accepted(transaction));
        }

        let source_plane = self.software_buffers.render_sample_plane(
            source_record.drawable,
            source_record.format,
            source_record.repeat,
        );
        let clip = Self::render_translated_clip(&destination_record);
        let mut damage = Region::empty();
        let mut handle = None;
        for (x, y, glyph) in placements {
            let rect = Rect {
                x,
                y,
                width: i32::from(glyph.info.width),
                height: i32::from(glyph.info.height),
            };
            if rect.width <= 0 || rect.height <= 0 {
                continue;
            }
            let mask = crate::XRenderSamplePlane::from_glyph(
                &glyph.pixels,
                usize::from(glyph.info.width),
                usize::from(glyph.info.height),
            );
            let Some(result) = self.software_buffers.render_composite(
                destination_record.drawable,
                size,
                op,
                &source_plane,
                Some(&mask),
                // A colour glyph set carries its own channels, which is the
                // subpixel path; a coverage set has none and multiplies the
                // source uniformly.
                matches!(
                    self.render_glyph_stores
                        .get(&store_id)
                        .map(|store| store.format),
                    Some(crate::XRenderPictFormatKind::Argb32)
                ),
                (
                    i32::from(source_origin.0).saturating_add(x),
                    i32::from(source_origin.1).saturating_add(y),
                ),
                (0, 0),
                rect,
                &clip,
                destination_record.format,
            ) else {
                return Ok(XAuthorityResponsePacket::rejected(
                    transaction,
                    XAuthorityRuntimeError::InvalidResource,
                ));
            };
            handle = Some(result.handle());
            damage.push(rect);
        }
        let (Some(handle), Some(generation)) = (handle, window_generation) else {
            return Ok(XAuthorityResponsePacket::accepted(transaction));
        };
        self.pending_raster_command = Some(XAuthorityRasterCommand::Unsupported(
            XRasterUnsupportedKind::RenderOperation,
        ));
        Ok(self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            destination_record.drawable,
            handle,
            damage,
            generation,
            250,
        )))
    }
}

/// A glyph composite can fail on either resource, and the protocol has a
/// different error code for each.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum XRenderCompositeGlyphsError {
    Picture(XRenderPictureError),
    Glyph(XRenderGlyphError),
}

/// A cursor image a client supplied through RENDER.
///
/// The bytes are premultiplied little-endian `[b, g, r, a]`, which is the
/// engine's `CursorAsset` contract exactly -- RENDER pictures are already
/// premultiplied, unlike core `CreateCursor`'s source and mask bitmaps.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XRenderCursorImage {
    pub width: u16,
    pub height: u16,
    pub hotspot_x: u16,
    pub hotspot_y: u16,
    pub premultiplied_bgra: Vec<u8>,
}

/// Why a RENDER cursor was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum XRenderCursorError {
    Picture(XRenderPictureError),
    IdInUse,
    /// The source picture is not a premultiplied 32-bit format, so it has no
    /// alpha for the cursor's shape.
    NotArgb32,
    /// The hotspot lies outside the image.
    HotspotOutsideImage,
    /// Larger than the engine will accept as a cursor.
    TooLarge,
}

/// The largest cursor edge the engine's `CursorAsset` accepts. Kept here as a
/// named constant rather than reached for across the crate boundary, and
/// checked at ingest so a stored image is always one the engine could take.
const X_RENDER_MAX_CURSOR_EDGE: u16 = 128;

impl XAuthorityRuntime {
    pub(crate) fn render_create_cursor(
        &mut self,
        namespace: NamespaceId,
        cursor: crate::XResourceId,
        source: crate::XResourceId,
        hotspot_x: u16,
        hotspot_y: u16,
        generation: u64,
    ) -> Result<(), XRenderCursorError> {
        let record = self
            .render_picture_record(namespace, source)
            .map_err(XRenderCursorError::Picture)?;
        if !matches!(record.format, crate::XRenderPictFormatKind::Argb32) {
            return Err(XRenderCursorError::NotArgb32);
        }
        if self.resource_id_in_use(cursor) {
            return Err(XRenderCursorError::IdInUse);
        }
        let size = self
            .render_target_geometry(namespace, &record)
            .map(|(size, _)| size)
            .ok_or(XRenderCursorError::Picture(XRenderPictureError::Drawable))?;
        let width = u16::try_from(size.width).map_err(|_| XRenderCursorError::TooLarge)?;
        let height = u16::try_from(size.height).map_err(|_| XRenderCursorError::TooLarge)?;
        if width > X_RENDER_MAX_CURSOR_EDGE || height > X_RENDER_MAX_CURSOR_EDGE {
            return Err(XRenderCursorError::TooLarge);
        }
        if hotspot_x >= width || hotspot_y >= height {
            return Err(XRenderCursorError::HotspotOutsideImage);
        }
        let region = Rect {
            x: 0,
            y: 0,
            width: size.width,
            height: size.height,
        };
        let mut pixels = self
            .software_buffers
            .image_region(record.drawable, region)
            .unwrap_or_else(|| vec![0; usize::from(width) * usize::from(height) * 4]);
        // The engine validates premultiplication and rejects a pixel whose
        // colour exceeds its alpha. A client can send one -- nothing on the
        // wire enforces the invariant -- so clamp at ingest rather than
        // storing an image the engine would later refuse.
        for pixel in pixels.chunks_exact_mut(4) {
            let alpha = pixel[3];
            for channel in &mut pixel[0..3] {
                *channel = (*channel).min(alpha);
            }
        }
        self.resources
            .insert(cursor, XResourceKind::Cursor, namespace, generation)
            .map_err(|_| XRenderCursorError::IdInUse)?;
        self.render_cursor_images.insert(
            cursor,
            XRenderCursorImage {
                width,
                height,
                hotspot_x,
                hotspot_y,
                premultiplied_bgra: pixels,
            },
        );
        Ok(())
    }

    /// The stored image for a cursor, if a client supplied one.
    ///
    /// Public because the session's cursor plumbing will read it from outside
    /// this crate once client-visible cursors land; today its only consumer
    /// is the proof that the image is stored and released.
    pub fn render_cursor_image(
        &self,
        cursor: crate::XResourceId,
    ) -> Option<&XRenderCursorImage> {
        self.render_cursor_images.get(&cursor)
    }
}
