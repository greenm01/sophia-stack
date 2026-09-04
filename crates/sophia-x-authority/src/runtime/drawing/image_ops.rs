// Pixel-transfer drawing operations: area copies, client image upload, and
// image readback.
//
// These share one concern the vector and text operations do not: they move
// opaque pixels rather than semantic primitives, so each decides separately
// whether the semantic journal can replay the result.

impl XAuthorityRuntime {
    pub fn apply_copy_area(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        source: crate::XResourceId,
        destination: crate::XResourceId,
        damage: Region,
    ) -> XAuthorityResponsePacket {
        if let Err(error) = self.validate_drawable_access(namespace, source) {
            return XAuthorityResponsePacket::rejected(transaction, error);
        }
        if self.validate_pixmap_access(namespace, destination).is_ok() {
            return XAuthorityResponsePacket::accepted(transaction);
        }
        self.apply_core_draw(transaction, namespace, destination, damage)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_copy_area_with_gc(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        source: crate::XResourceId,
        destination: crate::XResourceId,
        src_x: i16,
        src_y: i16,
        dst_x: i16,
        dst_y: i16,
        width: u16,
        height: u16,
        gc: &XGraphicsContextValues,
    ) -> XAuthorityResponsePacket {
        if let Err(error) = self.validate_drawable_access(namespace, source) {
            return XAuthorityResponsePacket::rejected(transaction, error);
        }
        if let Err(error) = self.validate_drawable_access(namespace, destination) {
            return XAuthorityResponsePacket::rejected(transaction, error);
        }
        if self.drawable_depth(namespace, source) != self.drawable_depth(namespace, destination) {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidSurface,
            );
        }
        if width == 0 || height == 0 {
            return XAuthorityResponsePacket::accepted(transaction);
        }
        let (destination_size, window_generation) =
            if let Ok(size) = self.pixmap_size(namespace, destination) {
                (size, None)
            } else if let Some(record) = self.windows.get(destination) {
                (
                    Size {
                        width: record.geometry.width,
                        height: record.geometry.height,
                    },
                    Some(record.generation),
                )
            } else {
                return XAuthorityResponsePacket::rejected(
                    transaction,
                    XAuthorityRuntimeError::UnknownResource,
                );
            };
        let Some((update, damage)) = self.software_buffers.copy_area(
            source,
            destination,
            destination_size,
            Rect {
                x: i32::from(src_x),
                y: i32::from(src_y),
                width: i32::from(width),
                height: i32::from(height),
            },
            dst_x,
            dst_y,
            gc,
        ) else {
            // Missing or entirely clipped source storage leaves the destination
            // unchanged. Exposure delivery is handled separately from pixels.
            return XAuthorityResponsePacket::accepted(transaction);
        };
        let Some(generation) = window_generation else {
            return XAuthorityResponsePacket::accepted(transaction);
        };
        let handle = update.handle();
        self.pending_raster_command = Some(if source == destination {
            XAuthorityRasterCommand::CopyArea {
                source: Rect {
                    x: i32::from(src_x),
                    y: i32::from(src_y),
                    width: i32::from(width),
                    height: i32::from(height),
                },
                destination_x: i32::from(dst_x),
                destination_y: i32::from(dst_y),
                gc: gc.clone(),
            }
        } else {
            // Cross-drawable replay needs an explicit source-generation
            // dependency, which the journal does not yet carry.
            XAuthorityRasterCommand::Unsupported(XRasterUnsupportedKind::CrossDrawableCopy)
        });
        self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            destination,
            handle,
            Region::single(damage),
            generation,
            250,
        ))
    }

    /// Applies one validated image in tight, little-endian 32-bit pixels.
    ///
    /// Wire dispatch owns decoding before this call. `semantics` carries the
    /// GC operation and original format facts the journal needs to decide whether
    /// the upload is replayable. `None` means the caller cannot vouch for the
    /// format, so a window destination poisons its journal with a named cause
    /// rather than retaining pixels that might not reproduce the drawable.
    pub fn apply_put_image(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        drawable: crate::XResourceId,
        damage: Region,
        data: Option<&[u8]>,
        semantics: Option<&XPutImageSemantics>,
    ) -> XAuthorityResponsePacket {
        if let Err(error) = self.validate_drawable_access(namespace, drawable) {
            return XAuthorityResponsePacket::rejected(transaction, error);
        }
        let Some(data) = data else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let Some(rect) = damage.rects.first().copied() else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let Ok(size) = self.drawable_facts(namespace, drawable).map(|facts| Size {
            width: facts.geometry.width,
            height: facts.geometry.height,
        }) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        if rect.width == 0
            || rect.height == 0
            || rect.x >= size.width
            || rect.y >= size.height
            || rect.x.saturating_add(rect.width) <= 0
            || rect.y.saturating_add(rect.height) <= 0
        {
            return XAuthorityResponsePacket::accepted(transaction);
        }
        if let Ok(size) = self.pixmap_size(namespace, drawable) {
            let wrote_image = self
                .software_buffers
                .put_image(drawable, size, rect, data, semantics);
            if wrote_image.is_none() {
                return XAuthorityResponsePacket::rejected(
                    transaction,
                    XAuthorityRuntimeError::InvalidResource,
                );
            }
            crate::image::trace_image_pixels("upload", transaction, drawable, rect, data);
            return XAuthorityResponsePacket::accepted(transaction);
        }
        let Some(record) = self.windows.get(drawable) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::UnknownResource,
            );
        };
        let size = Size {
            width: record.geometry.width,
            height: record.geometry.height,
        };
        let Some(buffer) = self
            .software_buffers
            .put_image(drawable, size, rect, data, semantics)
        else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let handle = buffer.handle();
        crate::image::trace_image_pixels("upload", transaction, drawable, rect, data);
        // Classification needs the destination rectangle the canonical writer
        // consumed, so it reads the same first damage rect.
        self.pending_raster_command = Some(match semantics {
            Some(semantics) => XAuthorityRasterCommand::from_put_image(semantics, rect, data),
                _ => XAuthorityRasterCommand::Unsupported(XRasterUnsupportedKind::PutImage),
        });
        self.finish_drawing_update(XDrawingUpdate::shm_put_image(
            transaction,
            namespace,
            drawable,
            handle,
            damage,
            record.generation,
            250,
        ))
    }

    pub fn drawable_image_region(
        &self,
        namespace: NamespaceId,
        drawable: crate::XResourceId,
        region: Rect,
    ) -> Result<Vec<u8>, XAuthorityRuntimeError> {
        self.validate_drawable_access(namespace, drawable)?;
        self.software_buffers
            .image_region(drawable, region)
            .ok_or(XAuthorityRuntimeError::InvalidResource)
    }

    pub(crate) fn read_drawable_image_region(
        &self,
        drawable: crate::XResourceId,
        descriptor: XDrawableImageDescriptor,
        region: Rect,
    ) -> Result<Vec<u8>, XDrawableImageError> {
        self.validate_drawable_image_region(descriptor, region)?;
        self.software_buffers
            .image_region(drawable, region)
            .ok_or(XDrawableImageError::AllocationFailed)
    }
}
