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
         if let Some(font) = values.font {
             self.validate_font_access(namespace, font)?;
         }
         self.graphics_contexts
             .create(namespace, gc, drawable, values)
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
 
     pub fn change_graphics_context(
         &mut self,
         namespace: NamespaceId,
         gc: crate::XResourceId,
         mask: u32,
         values: XGraphicsContextValues,
     ) -> Result<(), XAuthorityRuntimeError> {
         if mask & (1 << 14) != 0
             && let Some(font) = values.font
         {
             self.validate_font_access(namespace, font)?;
         }
         self.graphics_contexts
             .change(namespace, gc, mask, values)
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
         self.last_cpu_buffer_update = Some(buffer);
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
         if self.validate_pixmap_access(namespace, destination).is_ok() {
             return XAuthorityResponsePacket::accepted(transaction);
         }
         let Some(record) = self.windows.get(destination) else {
             return XAuthorityResponsePacket::rejected(
                 transaction,
                 XAuthorityRuntimeError::UnknownResource,
             );
         };
         let damage = Region::single(Rect {
             x: i32::from(dst_x),
             y: i32::from(dst_y),
             width: i32::from(width),
             height: i32::from(height),
         });
         let Some(update) = self.software_buffers.copy_area(
             source,
             destination,
             Size {
                 width: record.geometry.width,
                 height: record.geometry.height,
             },
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
             return self.apply_core_draw_with_gc(transaction, namespace, destination, damage, gc);
         };
         let handle = update.handle();
         self.last_cpu_buffer_update = Some(update);
         self.finish_drawing_update(XDrawingUpdate::core_draw(
             transaction,
             namespace,
             destination,
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
         self.last_cpu_buffer_update = Some(update);
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
 
     pub fn apply_put_image(
         &mut self,
         transaction: TransactionId,
         namespace: NamespaceId,
         drawable: crate::XResourceId,
         damage: Region,
         data: Option<&[u8]>,
     ) -> XAuthorityResponsePacket {
         if let Err(error) = self.validate_drawable_access(namespace, drawable) {
             return XAuthorityResponsePacket::rejected(transaction, error);
         }
         if let Ok(size) = self.pixmap_size(namespace, drawable) {
             let wrote_image = data.and_then(|data| {
                 damage
                     .rects
                     .first()
                     .and_then(|rect| self.software_buffers.put_image(drawable, size, *rect, data))
             });
             if wrote_image.is_none() {
                 return XAuthorityResponsePacket::rejected(
                     transaction,
                     XAuthorityRuntimeError::InvalidResource,
                 );
             }
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
         let Some(buffer) = data
             .and_then(|data| {
                 damage
                     .rects
                     .first()
                     .and_then(|rect| self.software_buffers.put_image(drawable, size, *rect, data))
             })
             .or_else(|| {
                 self.software_buffers.paint_damage(
                     drawable,
                     size,
                     &damage.rects,
                     &XGraphicsContextValues::default(),
                 )
             })
         else {
             return XAuthorityResponsePacket::rejected(
                 transaction,
                 XAuthorityRuntimeError::InvalidResource,
             );
         };
         let handle = buffer.handle();
         self.last_cpu_buffer_update = Some(buffer);
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
 
     pub(crate) fn apply_text_draw(
         &mut self,
         transaction: TransactionId,
         namespace: NamespaceId,
         window: crate::XResourceId,
         draw: XTextDraw<'_>,
         gc: &XGraphicsContextValues,
     ) -> XAuthorityResponsePacket {
         let Some(record) = self.windows.get(window) else {
             return XAuthorityResponsePacket::rejected(
                 transaction,
                 XAuthorityRuntimeError::UnknownResource,
             );
         };
         let damage = Region::single(Rect {
             x: i32::from(draw.x),
             y: i32::from(draw.baseline).saturating_sub(10),
             width: i32::try_from(draw.text.len().saturating_mul(8))
                 .unwrap_or(i32::MAX)
                 .max(1),
             height: 12,
         });
         let Some(buffer) = self.software_buffers.draw_text(
             window,
             Size {
                 width: record.geometry.width,
                 height: record.geometry.height,
             },
             draw,
             gc,
         ) else {
             return XAuthorityResponsePacket::rejected(
                 transaction,
                 XAuthorityRuntimeError::InvalidResource,
             );
         };
         let handle = buffer.handle();
         self.last_cpu_buffer_update = Some(buffer);
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
         self.last_cpu_buffer_update = Some(buffer);
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
         if matches!(update.buffer, sophia_protocol::BufferSource::CpuBuffer { .. })
             && update.kind != crate::XDrawingUpdateKind::PresentPixmap
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
             update.target_content_size = Some(presentation_update.size());
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
             self.last_cpu_buffer_update = Some(presentation_update);
         }
         let window = update.target_window;
         let previous_generation = update.previous_committed_generation;
         let transaction = match surface_transaction_from_drawing_update(&self.windows, update) {
             Ok(transaction) => transaction,
             Err(error) => {
                 return XAuthorityResponsePacket::rejected(transaction_id, error.into());
             }
         };
         if let Err(error) = self.windows.advance_generation(window, previous_generation) {
             return XAuthorityResponsePacket::rejected(transaction_id, error.into());
         }
         let mut response = XAuthorityResponsePacket::accepted(transaction_id);
         response.transactions.push(transaction);
         response
     }
}
