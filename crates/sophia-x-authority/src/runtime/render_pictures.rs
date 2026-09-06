/// One RENDER picture: a format-aware view over a drawable.
///
/// The record carries what compositing needs and nothing the drawable
/// already knows. `format` decides how the 32-bit store slots behind the
/// drawable are read and written; the clip list is kept in destination
/// coordinates exactly as the client sent it, translated at use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XRenderPictureRecord {
    pub drawable: crate::XResourceId,
    pub drawable_is_window: bool,
    pub format: crate::XRenderPictFormatKind,
    pub repeat: bool,
    pub clip_rects: Vec<Rect>,
    pub clip_x_origin: i16,
    pub clip_y_origin: i16,
    pub component_alpha: bool,
}

/// Why a RENDER picture request was refused, kept fine-grained because the
/// extension has error codes of its own and a client's fallback logic keys on
/// which one it receives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum XRenderPictureError {
    /// The named drawable does not exist or belongs to another namespace.
    Drawable,
    /// The chosen picture id is already a live resource.
    IdInUse,
    /// The format id names no format this server offers.
    UnknownFormat,
    /// The format's depth is not the drawable's depth.
    DepthMismatch,
    /// A value outside what the protocol defines for its attribute.
    InvalidValue,
    /// An attribute this server declines by name (alpha maps, pixmap clip
    /// masks) rather than by silently ignoring it.
    RefusedAttribute,
    /// The named picture does not exist or belongs to another namespace.
    UnknownPicture,
}

impl XAuthorityRuntime {
    /// Apply a value set to a record, shared between create and change.
    fn render_apply_picture_values(
        record: &mut XRenderPictureRecord,
        values: &crate::XRenderPictureValueSet,
    ) -> Result<(), XRenderPictureError> {
        if values.invalid_mask {
            return Err(XRenderPictureError::InvalidValue);
        }
        if values.refused_attribute {
            return Err(XRenderPictureError::RefusedAttribute);
        }
        if let Some(repeat) = values.repeat {
            // Pad and Reflect entered at 0.10, above what is advertised, so
            // for this server they are values the protocol does not define.
            record.repeat = match repeat {
                0 => false,
                1 => true,
                _ => return Err(XRenderPictureError::InvalidValue),
            };
        }
        if let Some(origin) = values.clip_x_origin {
            record.clip_x_origin = origin;
        }
        if let Some(origin) = values.clip_y_origin {
            record.clip_y_origin = origin;
        }
        if let Some(component_alpha) = values.component_alpha {
            record.component_alpha = match component_alpha {
                0 => false,
                1 => true,
                _ => return Err(XRenderPictureError::InvalidValue),
            };
        }
        Ok(())
    }

    pub(crate) fn render_create_picture(
        &mut self,
        namespace: NamespaceId,
        picture: crate::XResourceId,
        drawable: crate::XResourceId,
        format_id: u32,
        values: &crate::XRenderPictureValueSet,
        generation: u64,
    ) -> Result<(), XRenderPictureError> {
        if self
            .validate_drawable_access(namespace, drawable)
            .is_err()
        {
            return Err(XRenderPictureError::Drawable);
        }
        if self.resource_id_in_use(picture) {
            return Err(XRenderPictureError::IdInUse);
        }
        let format = crate::XRenderPictFormatKind::from_format_id(format_id)
            .ok_or(XRenderPictureError::UnknownFormat)?;
        // A format is a view over the drawable's slots, so its depth must be
        // the drawable's depth: binding A8 over a depth-24 window would read
        // color bytes as coverage.
        let (depth, drawable_is_window) = if let Ok(depth) = self.pixmap_depth(namespace, drawable)
        {
            (depth, false)
        } else {
            (self.window_visual(drawable).0, true)
        };
        if format.depth() != depth {
            return Err(XRenderPictureError::DepthMismatch);
        }
        let mut record = XRenderPictureRecord {
            drawable,
            drawable_is_window,
            format,
            repeat: false,
            clip_rects: Vec::new(),
            clip_x_origin: 0,
            clip_y_origin: 0,
            component_alpha: false,
        };
        Self::render_apply_picture_values(&mut record, values)?;
        self.resources
            .insert(picture, XResourceKind::Picture, namespace, generation)
            .map_err(|_| XRenderPictureError::IdInUse)?;
        self.render_pictures.insert(picture, record);
        Ok(())
    }

    pub(crate) fn render_change_picture(
        &mut self,
        namespace: NamespaceId,
        picture: crate::XResourceId,
        values: &crate::XRenderPictureValueSet,
    ) -> Result<(), XRenderPictureError> {
        self.resources
            .lookup(namespace, picture, XResourceKind::Picture)
            .map_err(|_| XRenderPictureError::UnknownPicture)?;
        let mut record = self
            .render_pictures
            .get(&picture)
            .cloned()
            .ok_or(XRenderPictureError::UnknownPicture)?;
        Self::render_apply_picture_values(&mut record, values)?;
        self.render_pictures.insert(picture, record);
        Ok(())
    }

    pub(crate) fn render_set_picture_clip_rectangles(
        &mut self,
        namespace: NamespaceId,
        picture: crate::XResourceId,
        clip_x_origin: i16,
        clip_y_origin: i16,
        rectangles: Vec<Rect>,
    ) -> Result<(), XRenderPictureError> {
        self.resources
            .lookup(namespace, picture, XResourceKind::Picture)
            .map_err(|_| XRenderPictureError::UnknownPicture)?;
        let record = self
            .render_pictures
            .get_mut(&picture)
            .ok_or(XRenderPictureError::UnknownPicture)?;
        record.clip_x_origin = clip_x_origin;
        record.clip_y_origin = clip_y_origin;
        record.clip_rects = rectangles;
        Ok(())
    }

    pub(crate) fn render_free_picture(
        &mut self,
        namespace: NamespaceId,
        picture: crate::XResourceId,
    ) -> Result<(), XRenderPictureError> {
        self.resources
            .lookup(namespace, picture, XResourceKind::Picture)
            .map_err(|_| XRenderPictureError::UnknownPicture)?;
        self.resources.remove(picture);
        self.render_pictures.remove(&picture);
        Ok(())
    }

    /// Drop every picture bound to a drawable that is going away. The spec
    /// ties a picture's life to its drawable, and a picture left behind would
    /// hold a view over slots the store has already released.
    pub(crate) fn render_drop_pictures_of_drawable(&mut self, drawable: crate::XResourceId) {
        let dead: Vec<crate::XResourceId> = self
            .render_pictures
            .iter()
            .filter(|(_, record)| record.drawable == drawable)
            .map(|(id, _)| *id)
            .collect();
        for picture in dead {
            self.resources.remove(picture);
            self.render_pictures.remove(&picture);
        }
    }

    /// The picture's clip list translated into destination coordinates,
    /// ready for the store's per-pixel check. Empty means unclipped.
    fn render_translated_clip(record: &XRenderPictureRecord) -> Vec<Rect> {
        record
            .clip_rects
            .iter()
            .map(|rect| Rect {
                x: rect.x.saturating_add(i32::from(record.clip_x_origin)),
                y: rect.y.saturating_add(i32::from(record.clip_y_origin)),
                width: rect.width,
                height: rect.height,
            })
            .collect()
    }

    /// The target size and, for a window, its generation -- the same split
    /// `apply_text_draw` makes, because a pixmap mutation ends in the store
    /// while a window mutation must reach the engine.
    fn render_target_geometry(
        &self,
        namespace: NamespaceId,
        record: &XRenderPictureRecord,
    ) -> Option<(Size, Option<u64>)> {
        if record.drawable_is_window {
            let window = self.windows.get(record.drawable)?;
            Some((
                Size {
                    width: window.geometry.width,
                    height: window.geometry.height,
                },
                Some(window.generation),
            ))
        } else {
            let size = self.pixmap_size(namespace, record.drawable).ok()?;
            Some((size, None))
        }
    }

    /// Look a picture up and return its record, without borrowing the map.
    fn render_picture_record(
        &self,
        namespace: NamespaceId,
        picture: crate::XResourceId,
    ) -> Result<XRenderPictureRecord, XRenderPictureError> {
        self.resources
            .lookup(namespace, picture, XResourceKind::Picture)
            .map_err(|_| XRenderPictureError::UnknownPicture)?;
        self.render_pictures
            .get(&picture)
            .cloned()
            .ok_or(XRenderPictureError::UnknownPicture)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_apply_composite(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        op: u8,
        source: crate::XResourceId,
        mask: Option<crate::XResourceId>,
        destination: crate::XResourceId,
        source_origin: (i16, i16),
        mask_origin: (i16, i16),
        destination_origin: (i16, i16),
        width: u16,
        height: u16,
    ) -> Result<XAuthorityResponsePacket, XRenderPictureError> {
        let source_record = self.render_picture_record(namespace, source)?;
        let destination_record = self.render_picture_record(namespace, destination)?;
        let mask_record = match mask {
            Some(mask) => Some(self.render_picture_record(namespace, mask)?),
            None => None,
        };
        if width == 0 || height == 0 {
            return Ok(XAuthorityResponsePacket::accepted(transaction));
        }
        let Some((size, window_generation)) =
            self.render_target_geometry(namespace, &destination_record)
        else {
            return Err(XRenderPictureError::Drawable);
        };
        // Sampled before the destination is touched, so a picture composited
        // onto itself reads its original pixels throughout.
        let source_plane = self.software_buffers.render_sample_plane(
            source_record.drawable,
            source_record.format,
            source_record.repeat,
        );
        let mask_plane = mask_record.as_ref().map(|record| {
            self.software_buffers.render_sample_plane(
                record.drawable,
                record.format,
                record.repeat,
            )
        });
        let component_alpha = mask_record
            .as_ref()
            .is_some_and(|record| record.component_alpha);
        let rect = Rect {
            x: i32::from(destination_origin.0),
            y: i32::from(destination_origin.1),
            width: i32::from(width),
            height: i32::from(height),
        };
        let clip = Self::render_translated_clip(&destination_record);
        let Some(result) = self.software_buffers.render_composite(
            destination_record.drawable,
            size,
            op,
            &source_plane,
            mask_plane.as_ref(),
            component_alpha,
            (i32::from(source_origin.0), i32::from(source_origin.1)),
            (i32::from(mask_origin.0), i32::from(mask_origin.1)),
            rect,
            &clip,
            destination_record.format,
        ) else {
            return Ok(XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            ));
        };
        let Some(generation) = window_generation else {
            return Ok(XAuthorityResponsePacket::accepted(transaction));
        };
        self.pending_raster_command = Some(XAuthorityRasterCommand::Unsupported(
            XRasterUnsupportedKind::RenderOperation,
        ));
        Ok(self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            destination_record.drawable,
            result.handle(),
            Region::single(rect),
            generation,
            250,
        )))
    }

    pub(crate) fn render_apply_fill_rectangles(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        op: u8,
        picture: crate::XResourceId,
        color: [u16; 4],
        rectangles: &[Rect],
    ) -> Result<XAuthorityResponsePacket, XRenderPictureError> {
        self.resources
            .lookup(namespace, picture, XResourceKind::Picture)
            .map_err(|_| XRenderPictureError::UnknownPicture)?;
        let record = self
            .render_pictures
            .get(&picture)
            .cloned()
            .ok_or(XRenderPictureError::UnknownPicture)?;
        if rectangles.is_empty() {
            return Ok(XAuthorityResponsePacket::accepted(transaction));
        }
        let Some((size, window_generation)) = self.render_target_geometry(namespace, &record)
        else {
            return Err(XRenderPictureError::Drawable);
        };
        // Wire colors are premultiplied, per the protocol; the store works in
        // premultiplied bytes, so the conversion is a narrowing.
        let color = [
            (color[2] >> 8) as u8,
            (color[1] >> 8) as u8,
            (color[0] >> 8) as u8,
            (color[3] >> 8) as u8,
        ];
        let clip = Self::render_translated_clip(&record);
        let Some(result) = self.software_buffers.render_fill(
            record.drawable,
            size,
            op,
            color,
            rectangles,
            &clip,
            record.format,
        ) else {
            return Ok(XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            ));
        };
        let Some(generation) = window_generation else {
            return Ok(XAuthorityResponsePacket::accepted(transaction));
        };
        let mut damage = Region::empty();
        for rectangle in rectangles {
            damage.push(*rectangle);
        }
        // RENDER results have no journal representation yet, so the surface's
        // density variants fall back to scaling the 1x raster.
        self.pending_raster_command = Some(XAuthorityRasterCommand::Unsupported(
            XRasterUnsupportedKind::RenderOperation,
        ));
        Ok(self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            record.drawable,
            result.handle(),
            damage,
            generation,
            250,
        )))
    }
}
