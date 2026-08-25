#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct XDrawableImageDescriptor {
    pub size: Size,
    pub depth: u8,
    pub visual: u32,
    /// Pixmaps have no root-relative visibility requirement.
    pub root_position: Option<(i32, i32)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum XDrawableImageError {
    Access(XAuthorityRuntimeError),
    BadMatch,
    AllocationFailed,
}

impl XAuthorityRuntime {
    pub(crate) fn drawable_image_descriptor(
        &self,
        namespace: NamespaceId,
        drawable: crate::XResourceId,
    ) -> Result<XDrawableImageDescriptor, XDrawableImageError> {
        if drawable.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT) {
            let size = self
                .output_topology
                .root_size()
                .map_err(|_| XDrawableImageError::BadMatch)?;
            return Ok(XDrawableImageDescriptor {
                size,
                depth: 24,
                visual: crate::X_SETUP_DEFAULT_VISUAL,
                root_position: Some((0, 0)),
            });
        }
        self.validate_drawable_access(namespace, drawable)
            .map_err(XDrawableImageError::Access)?;
        if let Ok(geometry) = self.window_geometry(namespace, drawable) {
            if self
                .window_map_state(namespace, drawable)
                .map_err(XDrawableImageError::Access)?
                != crate::XMapState::Viewable
            {
                return Err(XDrawableImageError::BadMatch);
            }
            let (depth, visual, _) = self.window_visual(drawable);
            return Ok(XDrawableImageDescriptor {
                size: Size {
                    width: geometry.width,
                    height: geometry.height,
                },
                depth,
                visual,
                root_position: Some(
                    self.window_absolute_position(namespace, drawable)
                        .map_err(XDrawableImageError::Access)?,
                ),
            });
        }
        let (size, depth) = self
            .pixmap_geometry(namespace, drawable)
            .map_err(XDrawableImageError::Access)?;
        Ok(XDrawableImageDescriptor {
            size,
            depth,
            visual: crate::X_ATOM_NONE,
            root_position: None,
        })
    }

    pub(crate) fn validate_drawable_image_region(
        &self,
        descriptor: XDrawableImageDescriptor,
        region: Rect,
    ) -> Result<(), XDrawableImageError> {
        if region.x < 0 || region.y < 0 || region.width < 0 || region.height < 0 {
            return Err(XDrawableImageError::BadMatch);
        }
        let right = region
            .x
            .checked_add(region.width)
            .ok_or(XDrawableImageError::BadMatch)?;
        let bottom = region
            .y
            .checked_add(region.height)
            .ok_or(XDrawableImageError::BadMatch)?;
        if right > descriptor.size.width || bottom > descriptor.size.height {
            return Err(XDrawableImageError::BadMatch);
        }
        if let Some((root_x, root_y)) = descriptor.root_position {
            let root_size = self
                .output_topology
                .root_size()
                .map_err(|_| XDrawableImageError::BadMatch)?;
            let root_right = root_x
                .checked_add(right)
                .ok_or(XDrawableImageError::BadMatch)?;
            let root_bottom = root_y
                .checked_add(bottom)
                .ok_or(XDrawableImageError::BadMatch)?;
            if root_x.checked_add(region.x).is_none_or(|x| x < 0)
                || root_y.checked_add(region.y).is_none_or(|y| y < 0)
                || root_right > root_size.width
                || root_bottom > root_size.height
            {
                return Err(XDrawableImageError::BadMatch);
            }
        }
        Ok(())
    }

    /// What a drawable is, for the requests that accept more than one kind.
    ///
    /// States facts and leaves admission to the caller, because the callers
    /// disagree: core drawing must refuse a drawable with no server storage,
    /// while a request that only names one may accept it.
    pub fn drawable_facts(
        &self,
        namespace: NamespaceId,
        drawable: crate::XResourceId,
    ) -> Result<XDrawableFacts, XAuthorityRuntimeError> {
        if drawable.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT) {
            let root = self
                .output_topology()
                .root_size()
                .map_err(|_| XAuthorityRuntimeError::UnknownResource)?;
            return Ok(XDrawableFacts {
                kind: XDrawableKind::Root,
                geometry: Rect {
                    x: 0,
                    y: 0,
                    width: root.width,
                    height: root.height,
                },
                depth: 24,
            });
        }
        // The window error is the one a miss reports, so an unknown id keeps the
        // exact identity it had before this resolver existed.
        let window_error = match self.window_geometry(namespace, drawable) {
            Ok(geometry) => {
                return Ok(XDrawableFacts {
                    kind: XDrawableKind::Window,
                    geometry,
                    depth: self.window_visual(drawable).0,
                });
            }
            Err(error) => error,
        };
        if let Ok((size, depth)) = self.pixmap_geometry(namespace, drawable) {
            return Ok(XDrawableFacts {
                kind: XDrawableKind::Pixmap,
                geometry: Rect {
                    x: 0,
                    y: 0,
                    width: size.width,
                    height: size.height,
                },
                depth,
            });
        }
        // A pbuffer is its own X drawable: the client names the same id when it
        // asks for geometry, so this is the request that decides whether the
        // drawable it just created exists at all. Its depth comes from its
        // configuration, since there is no window to read one from.
        if let Ok((size, fbconfig)) = self.glx_pbuffer(namespace, drawable)
            && let Some(config) = crate::x_glx_fb_config(fbconfig)
        {
            return Ok(XDrawableFacts {
                kind: XDrawableKind::GlxPbuffer,
                geometry: Rect {
                    x: 0,
                    y: 0,
                    width: size.width,
                    height: size.height,
                },
                depth: config.depth(),
            });
        }
        Err(window_error)
    }

    pub fn validate_drawable_access(
        &self,
        namespace: NamespaceId,
        drawable: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        if drawable.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT) {
            return Ok(());
        }
        if !namespace.is_valid() {
            return Err(XAuthorityRuntimeError::InvalidNamespace);
        }
        let record = self
            .resources
            .get(drawable)
            .ok_or(XAuthorityRuntimeError::UnknownResource)?;
        if !matches!(record.kind, XResourceKind::Window | XResourceKind::Pixmap) {
            return Err(XAuthorityRuntimeError::WrongResourceKind);
        }
        if record.owner_namespace != namespace {
            return Err(XAuthorityRuntimeError::CrossNamespaceDenied);
        }
        Ok(())
    }

    pub fn create_graphics_context(
        &mut self,
        namespace: NamespaceId,
        gc: crate::XResourceId,
        drawable: crate::XResourceId,
        values: XGraphicsContextValues,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.validate_drawable_access(namespace, drawable)?;
        let depth = self.drawable_depth(namespace, drawable)?;
        let font_face = values
            .font
            .map(|font| self.font_face(namespace, font))
            .transpose()?
            .unwrap_or_default();
        self.graphics_contexts
            .create(namespace, gc, drawable, depth, values, font_face)
            .map_err(XAuthorityRuntimeError::from)?;
        Ok(())
    }

    pub fn graphics_context_values(
        &self,
        namespace: NamespaceId,
        gc: crate::XResourceId,
    ) -> Result<XGraphicsContextValues, XAuthorityRuntimeError> {
        self.graphics_contexts
            .get(namespace, gc)
            .map(|record| record.values.clone())
            .map_err(Into::into)
    }

    pub(crate) fn graphics_context_depth_and_values(
        &self,
        namespace: NamespaceId,
        gc: crate::XResourceId,
    ) -> Result<(u8, XGraphicsContextValues), XAuthorityRuntimeError> {
        self.graphics_contexts
            .get(namespace, gc)
            .map(|record| (record.depth, record.values.clone()))
            .map_err(Into::into)
    }

    pub(crate) fn graphics_context_depth_values_and_font(
        &self,
        namespace: NamespaceId,
        gc: crate::XResourceId,
    ) -> Result<(u8, XGraphicsContextValues, XFontFace), XAuthorityRuntimeError> {
        self.graphics_contexts
            .get(namespace, gc)
            .map(|record| (record.depth, record.values.clone(), record.font_face))
            .map_err(Into::into)
    }

    pub(crate) fn fontable_face(
        &self,
        namespace: NamespaceId,
        fontable: crate::XResourceId,
    ) -> Result<XFontFace, XAuthorityRuntimeError> {
        match self.font_face(namespace, fontable) {
            Ok(face) => Ok(face),
            Err(
                XAuthorityRuntimeError::UnknownResource | XAuthorityRuntimeError::WrongResourceKind,
            ) => self
                .graphics_contexts
                .get(namespace, fontable)
                .map(|record| record.font_face)
                .map_err(Into::into),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn drawable_depth(
        &self,
        namespace: NamespaceId,
        drawable: crate::XResourceId,
    ) -> Result<u8, XAuthorityRuntimeError> {
        self.validate_drawable_access(namespace, drawable)?;
        if drawable.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT) {
            return Ok(24);
        }
        if let Ok(depth) = self.pixmap_depth(namespace, drawable) {
            return Ok(depth);
        }
        Ok(self.window_visual(drawable).0)
    }

    pub fn change_graphics_context(
        &mut self,
        namespace: NamespaceId,
        gc: crate::XResourceId,
        mask: u32,
        values: XGraphicsContextValues,
    ) -> Result<(), XAuthorityRuntimeError> {
        let font_face = if mask & (1 << 14) != 0 {
            Some(self.font_face(
                namespace,
                values.font.ok_or(XAuthorityRuntimeError::InvalidResource)?,
            )?)
        } else {
            None
        };
        self.graphics_contexts
            .change(namespace, gc, mask, values, font_face)
            .map_err(Into::into)
    }

    pub fn set_graphics_context_clip_rectangles(
        &mut self,
        namespace: NamespaceId,
        gc: crate::XResourceId,
        rectangles: Vec<Rect>,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.graphics_contexts
            .set_clip_rectangles(namespace, gc, rectangles)
            .map_err(Into::into)
    }

    pub fn free_graphics_context(
        &mut self,
        namespace: NamespaceId,
        gc: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.graphics_contexts
            .remove(namespace, gc)
            .map_err(Into::into)
    }

    pub fn window_background_pixel(
        &self,
        namespace: NamespaceId,
        window: crate::XResourceId,
    ) -> Result<u32, XAuthorityRuntimeError> {
        self.validate_window_access(namespace, window)?;
        Ok(self
            .window_background_pixels
            .get(&window)
            .copied()
            .unwrap_or(0))
    }

    pub fn set_window_background_pixel(
        &mut self,
        namespace: NamespaceId,
        window: crate::XResourceId,
        pixel: u32,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.validate_window_access(namespace, window)?;
        self.window_background_pixels.insert(window, pixel);
        Ok(())
    }

    pub fn apply_core_draw(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        damage: Region,
    ) -> XAuthorityResponsePacket {
        self.apply_core_draw_with_gc(
            transaction,
            namespace,
            window,
            damage,
            &XGraphicsContextValues::default(),
        )
    }

    pub fn apply_core_draw_with_gc(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        damage: Region,
        gc: &XGraphicsContextValues,
    ) -> XAuthorityResponsePacket {
        let Some(record) = self.windows.get(window) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::UnknownResource,
            );
        };
        let Some(buffer) = self.software_buffers.paint_damage(
            window,
            Size {
                width: record.geometry.width,
                height: record.geometry.height,
            },
            &damage.rects,
            gc,
        ) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let handle = buffer.handle();
        self.pending_raster_command = Some(XAuthorityRasterCommand::Paint {
            rects: damage.rects.clone(),
            gc: gc.clone(),
        });
        self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            window,
            handle,
            damage,
            record.generation,
            250,
        ))
    }

    pub fn apply_line_draw(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        points: &[XPoint],
        gc: &XGraphicsContextValues,
    ) -> XAuthorityResponsePacket {
        let Some(record) = self.windows.get(window) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::UnknownResource,
            );
        };
        let Some(update) = self.software_buffers.draw_lines(
            window,
            Size {
                width: record.geometry.width,
                height: record.geometry.height,
            },
            points,
            gc,
        ) else {
            return XAuthorityResponsePacket::accepted(transaction);
        };
        let damage = Region::single(Rect {
            x: points
                .iter()
                .map(|point| i32::from(point.x))
                .min()
                .unwrap_or(0),
            y: points
                .iter()
                .map(|point| i32::from(point.y))
                .min()
                .unwrap_or(0),
            width: points
                .iter()
                .map(|point| i32::from(point.x))
                .max()
                .unwrap_or(0)
                .saturating_sub(
                    points
                        .iter()
                        .map(|point| i32::from(point.x))
                        .min()
                        .unwrap_or(0),
                )
                .saturating_add(i32::from(gc.line_width.max(1))),
            height: points
                .iter()
                .map(|point| i32::from(point.y))
                .max()
                .unwrap_or(0)
                .saturating_sub(
                    points
                        .iter()
                        .map(|point| i32::from(point.y))
                        .min()
                        .unwrap_or(0),
                )
                .saturating_add(i32::from(gc.line_width.max(1))),
        });
        let handle = update.handle();
        self.pending_raster_command = Some(XAuthorityRasterCommand::Lines {
            points: points
                .iter()
                .map(|point| XRasterPoint {
                    x: i32::from(point.x),
                    y: i32::from(point.y),
                })
                .collect(),
            gc: gc.clone(),
        });
        self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            window,
            handle,
            damage,
            record.generation,
            250,
        ))
    }

    pub fn apply_rectangle_draw(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        rectangles: &[Rect],
        gc: &XGraphicsContextValues,
    ) -> XAuthorityResponsePacket {
        let Some(record) = self.windows.get(window) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::UnknownResource,
            );
        };
        let Some((update, damage)) = self.software_buffers.draw_rectangles(
            window,
            Size {
                width: record.geometry.width,
                height: record.geometry.height,
            },
            rectangles,
            gc,
        ) else {
            return XAuthorityResponsePacket::accepted(transaction);
        };
        let handle = update.handle();
        self.pending_raster_command = Some(XAuthorityRasterCommand::Rectangles {
            rectangles: rectangles.to_vec(),
            gc: gc.clone(),
        });
        self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            window,
            handle,
            Region::single(damage),
            record.generation,
            250,
        ))
    }

    pub(crate) fn apply_text_draw(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        drawable: crate::XResourceId,
        draws: &[XTextDraw<'_>],
        gc: &XGraphicsContextValues,
    ) -> XAuthorityResponsePacket {
        if let Err(error) = self.validate_drawable_access(namespace, drawable) {
            return XAuthorityResponsePacket::rejected(transaction, error);
        }
        if draws.iter().all(|draw| draw.text.is_empty()) {
            return XAuthorityResponsePacket::accepted(transaction);
        }
        let (size, window_generation) = if let Ok(size) = self.pixmap_size(namespace, drawable) {
            (size, None)
        } else if let Some(record) = self.windows.get(drawable) {
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
        let mut damage = Region::empty();
        for draw in draws {
            if draw.text.is_empty() {
                continue;
            }
            damage.push(Rect {
                x: draw.x,
                y: draw.baseline.saturating_sub(draw.font.ascent()),
                width: i32::try_from(draw.text.len())
                    .unwrap_or(i32::MAX)
                    .saturating_mul(draw.font.width()),
                height: draw.font.ascent().saturating_add(draw.font.descent()),
            });
        }
        let Some(buffer) = self.software_buffers.draw_text(drawable, size, draws, gc) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let Some(generation) = window_generation else {
            return XAuthorityResponsePacket::accepted(transaction);
        };
        let handle = buffer.handle();
        self.pending_raster_command = Some(XAuthorityRasterCommand::Text {
            draws: draws
                .iter()
                .map(|draw| XOwnedTextDraw {
                    x: draw.x,
                    baseline: draw.baseline,
                    text: draw.text.to_vec(),
                    image: draw.image,
                    font: draw.font,
                })
                .collect(),
            gc: gc.clone(),
        });
        self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            drawable,
            handle,
            damage,
            generation,
            250,
        ))
    }

    pub fn apply_clear(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        damage: Region,
    ) -> XAuthorityResponsePacket {
        self.apply_clear_with_pixel(transaction, namespace, window, damage, 0)
    }

    pub fn apply_clear_with_pixel(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        damage: Region,
        pixel: u32,
    ) -> XAuthorityResponsePacket {
        let Some(record) = self.windows.get(window) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::UnknownResource,
            );
        };
        let Some(rect) = damage.rects.first().copied() else {
            return XAuthorityResponsePacket::accepted(transaction);
        };
        let Some(buffer) = self.software_buffers.clear(
            window,
            Size {
                width: record.geometry.width,
                height: record.geometry.height,
            },
            rect,
            pixel,
        ) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let handle = buffer.handle();
        self.pending_raster_command = Some(XAuthorityRasterCommand::Clear { rect, pixel });
        self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            window,
            handle,
            damage,
            record.generation,
            250,
        ))
    }

    fn finish_drawing_update(&mut self, mut update: XDrawingUpdate) -> XAuthorityResponsePacket {
        let transaction_id = update.transaction;
        let source_window = update.target_window;
        let semantic_command = self.pending_raster_command.take();
        let mut cpu_buffer_updates = Vec::new();
        if matches!(
            update.buffer,
            sophia_protocol::BufferSource::CpuBuffer { .. }
        ) && update.kind != crate::XDrawingUpdateKind::PresentPixmap
        {
            let (presentation_window, offset_x, offset_y) =
                match self.windows.presentation_root_and_offset(source_window) {
                    Ok(presentation) => presentation,
                    Err(error) => {
                        return XAuthorityResponsePacket::rejected(transaction_id, error.into());
                    }
                };
            let Some(presentation_record) = self.windows.get(presentation_window) else {
                return XAuthorityResponsePacket::rejected(
                    transaction_id,
                    XAuthorityRuntimeError::UnknownResource,
                );
            };
            let presentation_size = Size {
                width: presentation_record.geometry.width,
                height: presentation_record.geometry.height,
            };
            update.previous_committed_generation = presentation_record.generation;
            let Some(presentation_update) = self.software_buffers.present_window_damage(
                presentation_window,
                presentation_size,
                source_window,
                offset_x,
                offset_y,
                &update.damage.rects,
            ) else {
                return XAuthorityResponsePacket::rejected(
                    transaction_id,
                    XAuthorityRuntimeError::InvalidResource,
                );
            };
            update.target_window = presentation_window;
            update.buffer = sophia_protocol::BufferSource::CpuBuffer {
                handle: presentation_update.handle(),
            };
            // The authority composed this raster itself, at the presentation
            // buffer's size, so it spans exactly what it fills.
            update.presentation_extent = Some(presentation_update.size());
            update.raster_extent = Some(presentation_update.size());
            update.damage = Region {
                rects: update
                    .damage
                    .rects
                    .iter()
                    .map(|rect| Rect {
                        x: rect.x.saturating_add(offset_x),
                        y: rect.y.saturating_add(offset_y),
                        width: rect.width,
                        height: rect.height,
                    })
                    .collect(),
            };
            cpu_buffer_updates.push(presentation_update.clone());
            if let Some(command) = semantic_command {
                cpu_buffer_updates.extend(self.raster_store.record(
                    presentation_window,
                    presentation_update.size(),
                    command.translated(offset_x, offset_y),
                ));
            } else {
                self.raster_store.invalidate_unjournaled_presentation(
                    presentation_window,
                    presentation_update.size(),
                );
            }
        }
        let window = update.target_window;
        let previous_generation = update.previous_committed_generation;
        let mut transaction = match surface_transaction_from_drawing_update(&self.windows, update) {
            Ok(transaction) => transaction,
            Err(error) => {
                return XAuthorityResponsePacket::rejected(transaction_id, error.into());
            }
        };
        if let Err(error) = self.windows.advance_generation(window, previous_generation) {
            return XAuthorityResponsePacket::rejected(transaction_id, error.into());
        }
        if let Some(canonical) = self.software_buffers.presentation_snapshot(window) {
            transaction.content = self.raster_store.content_set(window, canonical);
        }
        self.last_cpu_buffer_updates.extend(cpu_buffer_updates);
        let mut response = XAuthorityResponsePacket::accepted(transaction_id);
        response.transactions.push(transaction);
        response
    }

    /// Applies one protocol-neutral Engine raster requirement to the
    /// presentation surface currently owning that `SurfaceId`. Late or stale
    /// requirements fail closed before allocating or publishing pixels.
    pub fn apply_surface_raster_requirements(
        &mut self,
        transaction: TransactionId,
        requirements: &sophia_protocol::SurfaceRasterRequirements,
    ) -> Result<crate::XSurfaceRasterOutcome, XAuthorityRuntimeError> {
        use crate::XSurfaceRasterOutcome;
        requirements
            .validate()
            .map_err(|_| XAuthorityRuntimeError::InvalidResource)?;
        if !transaction.is_valid() {
            return Err(XAuthorityRuntimeError::InvalidResource);
        }
        let record = self
            .windows
            .presentation_for_surface(requirements.surface)
            .cloned()
            .ok_or(XAuthorityRuntimeError::UnknownResource)?;
        // A requirement is advisory demand, not a contract pinned to the
        // generation Engine had committed when it asked. Engine builds
        // requirements from its committed scene and commits authority
        // transactions as an ordered chain, so under a drawing client it names
        // a generation this authority already passed. Answering from current
        // state is correct because this call publishes a complete replacement
        // transaction rather than amending committed content, and the response
        // travels the same ordered egress as ordinary draws, so it commits
        // once Engine's chain reaches the generation it is anchored at.
        //
        // The authority running *behind* the request is the genuine error: it
        // names content that was never produced.
        if record.generation < requirements.committed_content_generation {
            return Ok(XSurfaceRasterOutcome::SampledFallback {
                cause: crate::XRasterFallbackCause::StaleContentGeneration,
                observed_content_generation: record.generation,
            });
        }
        // A surface whose pixels arrived through a renderer or pixmap Present
        // has no canonical CPU drawable, so there is nothing to replay from.
        // That is an ordinary content state, not a runtime failure: reporting
        // it as an error here would propagate out of the connection loop and
        // take the whole X server down over one surface's demand.
        let Some(canonical) = self.software_buffers.presentation_snapshot(record.id).cloned()
        else {
            return Ok(XSurfaceRasterOutcome::SampledFallback {
                cause: crate::XRasterFallbackCause::NoCanonicalRaster,
                observed_content_generation: record.generation,
            });
        };
        if canonical.size != requirements.logical_extent {
            return Ok(XSurfaceRasterOutcome::SampledFallback {
                cause: crate::XRasterFallbackCause::LogicalExtentMismatch,
                observed_content_generation: record.generation,
            });
        }
        // The store's refusals — a changed extent, a projected size or stride
        // overflow, a backing bound — are all states this surface can
        // legitimately be in, so they answer the demand rather than fail the
        // runtime.
        let satisfied = match self
            .raster_store
            .satisfy(record.id, requirements, canonical.bytes.len())
        {
            Ok(outcome) => outcome,
            Err(_) => {
                return Ok(XSurfaceRasterOutcome::SampledFallback {
                    cause: crate::XRasterFallbackCause::LogicalExtentMismatch,
                    observed_content_generation: record.generation,
                });
            }
        };
        let updates = match satisfied {
            crate::XRasterSatisfyOutcome::Satisfied(updates) => updates,
            crate::XRasterSatisfyOutcome::Fallback(cause) => {
                return Ok(XSurfaceRasterOutcome::SampledFallback {
                    cause,
                    observed_content_generation: record.generation,
                });
            }
        };
        let content = self.raster_store.content_set(record.id, &canonical);
        let all_satisfied = requirements.classes.iter().all(|class| {
            content.variants().iter().any(|variant| {
                variant.density_millis == class.density_millis
                    && variant.transform == class.transform
                    && variant.fidelity
                        == sophia_protocol::SurfaceContentFidelity::AuthorityRaster
            })
        });
        if !all_satisfied {
            // Guards content-set variant truncation: the store accepted the
            // requirement, but publication could not carry every class.
            return Ok(XSurfaceRasterOutcome::SampledFallback {
                cause: crate::XRasterFallbackCause::BackingCapacity,
                observed_content_generation: record.generation,
            });
        }
        let surface_transaction = sophia_protocol::SurfaceTransaction {
            transaction,
            authority: sophia_protocol::AuthorityKind::SophiaX,
            surface: record.surface,
            namespace: Some(record.namespace),
            target_geometry: record.geometry,
            content,
            // Engine asked for this raster at this extent and the store
            // produced it, so what it fills is what it spans.
            presentation_extent: requirements.logical_extent,
            damage: Region::single(Rect {
                x: 0,
                y: 0,
                width: requirements.logical_extent.width,
                height: requirements.logical_extent.height,
            }),
            readiness: sophia_protocol::SurfaceTransactionReadiness::Ready,
            timeout_msec: 250,
            previous_committed_generation: record.generation,
        };
        self.windows
            .advance_generation(record.id, record.generation)
            .map_err(XAuthorityRuntimeError::from)?;
        Ok(XSurfaceRasterOutcome::Satisfied(Box::new(
            crate::XAuthorityRasterRequirementResponse {
                identity: sophia_protocol::SurfaceRasterResponseIdentity {
                    transaction,
                    surface: record.surface,
                    // The generation this content was actually produced from,
                    // which may lead the one requested. Reporting the request
                    // back would misdescribe the pixels.
                    source_content_generation: record.generation,
                    requirement_generation: requirements.requirement_generation,
                },
                transaction: surface_transaction,
                cpu_buffer_updates: updates,
            },
        )))
    }
}
