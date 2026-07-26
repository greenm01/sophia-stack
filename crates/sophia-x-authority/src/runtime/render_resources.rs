impl XAuthorityRuntime {
     pub fn create_pixmap(
         &mut self,
         namespace: NamespaceId,
         pixmap: crate::XResourceId,
         size: Size,
         generation: u64,
     ) -> Result<(), XAuthorityRuntimeError> {
         if size.width <= 0 || size.height <= 0 {
             return Err(XAuthorityRuntimeError::InvalidResource);
         }
         self.resources
             .insert(pixmap, XResourceKind::Pixmap, namespace, generation)
             .map_err(XAuthorityRuntimeError::from)?;
         self.pixmap_sizes.insert(pixmap, size);
         Ok(())
     }
 
     pub fn free_pixmap(
         &mut self,
         namespace: NamespaceId,
         pixmap: crate::XResourceId,
     ) -> Result<Option<sophia_protocol::BufferHandle>, XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, pixmap, XResourceKind::Pixmap)?;
         self.resources.remove(pixmap);
         self.pixmap_sizes.remove(&pixmap);
         self.software_buffers.remove(pixmap);
         Ok(self
             .dri3_pixmaps
             .remove(&pixmap)
             .map(|descriptor| descriptor.handle))
     }
 
     pub fn validate_pixmap_access(
         &self,
         namespace: NamespaceId,
         pixmap: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, pixmap, XResourceKind::Pixmap)
             .map(|_| ())
             .map_err(Into::into)
     }

     pub fn pixmap_size(
         &self,
         namespace: NamespaceId,
         pixmap: crate::XResourceId,
     ) -> Result<Size, XAuthorityRuntimeError> {
         self.validate_pixmap_access(namespace, pixmap)?;
         self.pixmap_sizes
             .get(&pixmap)
             .copied()
             .ok_or(XAuthorityRuntimeError::UnknownResource)
     }
 
     #[allow(clippy::too_many_arguments)]
     pub fn create_dri3_pixmap(
         &mut self,
         namespace: NamespaceId,
         pixmap: crate::XResourceId,
         generation: u64,
         size_bytes: u32,
         width: u16,
         height: u16,
         stride: u16,
         depth: u8,
         bits_per_pixel: u8,
     ) -> Result<sophia_protocol::DmaBufDescriptor, XAuthorityRuntimeError> {
         let format = match (depth, bits_per_pixel) {
             (24, 32) => sophia_protocol::DRM_FORMAT_XRGB8888,
             (32, 32) => sophia_protocol::DRM_FORMAT_ARGB8888,
             _ => return Err(XAuthorityRuntimeError::InvalidResource),
         };
         let handle = self.next_dma_buf_handle.max(1);
         let descriptor = sophia_protocol::DmaBufDescriptor {
             handle: sophia_protocol::BufferHandle::from_raw(handle),
             size: Size {
                 width: i32::from(width),
                 height: i32::from(height),
             },
             format,
             modifier: sophia_protocol::DRM_FORMAT_MOD_INVALID,
             plane_count: 1,
             planes: [
                 Some(sophia_protocol::DmaBufPlaneDescriptor {
                     offset: 0,
                     stride: u32::from(stride),
                 }),
                 None,
                 None,
                 None,
             ],
         };
         descriptor
             .validate()
             .map_err(|_| XAuthorityRuntimeError::InvalidResource)?;
         if u64::from(stride).saturating_mul(u64::from(height)) > u64::from(size_bytes) {
             return Err(XAuthorityRuntimeError::InvalidResource);
         }
         self.create_pixmap(namespace, pixmap, descriptor.size, generation)?;
         self.next_dma_buf_handle = handle.saturating_add(1).max(1);
         self.dri3_pixmaps.insert(pixmap, descriptor);
         Ok(descriptor)
     }
 
     #[allow(clippy::too_many_arguments)]
     pub fn create_dri3_pixmap_from_buffers(
         &mut self,
         namespace: NamespaceId,
         pixmap: crate::XResourceId,
         generation: u64,
         num_buffers: u8,
         width: u16,
         height: u16,
         strides: [u32; sophia_protocol::DMA_BUF_MAX_PLANES],
         offsets: [u32; sophia_protocol::DMA_BUF_MAX_PLANES],
         depth: u8,
         bits_per_pixel: u8,
         modifier: u64,
     ) -> Result<sophia_protocol::DmaBufDescriptor, XAuthorityRuntimeError> {
         let format = match (depth, bits_per_pixel) {
             (24, 32) => sophia_protocol::DRM_FORMAT_XRGB8888,
             (32, 32) => sophia_protocol::DRM_FORMAT_ARGB8888,
             _ => return Err(XAuthorityRuntimeError::InvalidResource),
         };
         if num_buffers == 0 || usize::from(num_buffers) > sophia_protocol::DMA_BUF_MAX_PLANES {
             return Err(XAuthorityRuntimeError::InvalidResource);
         }
         let handle = self.next_dma_buf_handle.max(1);
         let mut planes = [None; sophia_protocol::DMA_BUF_MAX_PLANES];
         for index in 0..usize::from(num_buffers) {
             planes[index] = Some(sophia_protocol::DmaBufPlaneDescriptor {
                 offset: offsets[index],
                 stride: strides[index],
             });
         }
         let descriptor = sophia_protocol::DmaBufDescriptor {
             handle: sophia_protocol::BufferHandle::from_raw(handle),
             size: Size {
                 width: i32::from(width),
                 height: i32::from(height),
             },
             format,
             modifier,
             plane_count: num_buffers,
             planes,
         };
         descriptor
             .validate()
             .map_err(|_| XAuthorityRuntimeError::InvalidResource)?;
         self.create_pixmap(namespace, pixmap, descriptor.size, generation)?;
         self.next_dma_buf_handle = handle.saturating_add(1).max(1);
         self.dri3_pixmaps.insert(pixmap, descriptor);
         Ok(descriptor)
     }
 
     pub fn dri3_pixmap_descriptor(
         &self,
         namespace: NamespaceId,
         pixmap: crate::XResourceId,
     ) -> Result<sophia_protocol::DmaBufDescriptor, XAuthorityRuntimeError> {
         self.validate_pixmap_access(namespace, pixmap)?;
         self.dri3_pixmaps
             .get(&pixmap)
             .copied()
             .ok_or(XAuthorityRuntimeError::UnknownResource)
     }
 
     pub fn present_standard_pixmap(
         &mut self,
         transaction: TransactionId,
         namespace: NamespaceId,
         window: crate::XResourceId,
         pixmap: crate::XResourceId,
     ) -> XAuthorityResponsePacket {
         let record = match self.windows.get(window) {
             Some(record) if record.namespace == namespace => record.clone(),
             _ => {
                 return XAuthorityResponsePacket::rejected(
                     transaction,
                     XAuthorityRuntimeError::UnknownResource,
                 );
             }
         };
         if let Err(error) = self.validate_pixmap_access(namespace, pixmap) {
             return XAuthorityResponsePacket::rejected(transaction, error);
         }
         let buffer = self.dri3_pixmaps.get(&pixmap).map_or(
             sophia_protocol::BufferSource::XPixmap {
                 pixmap: u32::try_from(pixmap.local.raw()).unwrap_or(0),
             },
             |descriptor| sophia_protocol::BufferSource::DmaBuf {
                 handle: descriptor.handle.raw(),
             },
         );
         self.finish_drawing_update(XDrawingUpdate::present_buffer(
             transaction,
             namespace,
             window,
             buffer,
             Region::single(Rect {
                 x: 0,
                 y: 0,
                 width: record.geometry.width,
                 height: record.geometry.height,
             }),
             record.generation,
             250,
         ))
     }
 
     pub fn create_xfixes_region(
         &mut self,
         namespace: NamespaceId,
         region: crate::XResourceId,
         rectangles: Vec<Rect>,
         generation: u64,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.resources
             .insert(region, XResourceKind::Region, namespace, generation)?;
         self.xfixes_regions
             .insert(region, Region { rects: rectangles });
         Ok(())
     }
 
     pub fn set_xfixes_region(
         &mut self,
         namespace: NamespaceId,
         region: crate::XResourceId,
         rectangles: Vec<Rect>,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.validate_xfixes_region_access(namespace, region)?;
         self.xfixes_regions
             .insert(region, Region { rects: rectangles });
         Ok(())
     }
 
     pub fn destroy_xfixes_region(
         &mut self,
         namespace: NamespaceId,
         region: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.validate_xfixes_region_access(namespace, region)?;
         self.resources.remove(region);
         self.xfixes_regions.remove(&region);
         Ok(())
     }
 
     pub fn validate_xfixes_region_access(
         &self,
         namespace: NamespaceId,
         region: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, region, XResourceKind::Region)
             .map(|_| ())
             .map_err(Into::into)
     }
 
     pub fn create_dri3_fence(
         &mut self,
         namespace: NamespaceId,
         fence: crate::XResourceId,
         generation: u64,
     ) -> Result<sophia_protocol::FenceHandle, XAuthorityRuntimeError> {
         self.resources
             .insert(fence, XResourceKind::Fence, namespace, generation)
             .map_err(XAuthorityRuntimeError::from)?;
         let handle = sophia_protocol::FenceHandle::from_raw(self.next_fence_handle.max(1));
         self.next_fence_handle = handle.raw().saturating_add(1).max(1);
         self.dri3_fences.insert(fence, handle);
         Ok(handle)
     }
 
     pub fn validate_dri3_fence_access(
         &self,
         namespace: NamespaceId,
         fence: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, fence, XResourceKind::Fence)
             .map(|_| ())
             .map_err(Into::into)
     }
 
     pub fn dri3_fence_handle(
         &self,
         namespace: NamespaceId,
         fence: crate::XResourceId,
     ) -> Result<sophia_protocol::FenceHandle, XAuthorityRuntimeError> {
         self.validate_dri3_fence_access(namespace, fence)?;
         self.dri3_fences
             .get(&fence)
             .copied()
             .ok_or(XAuthorityRuntimeError::UnknownResource)
     }
 
     pub fn destroy_dri3_fence(
         &mut self,
         namespace: NamespaceId,
         fence: crate::XResourceId,
     ) -> Result<sophia_protocol::FenceHandle, XAuthorityRuntimeError> {
         self.validate_dri3_fence_access(namespace, fence)?;
         self.resources.remove(fence);
         self.dri3_fences
             .remove(&fence)
             .ok_or(XAuthorityRuntimeError::UnknownResource)
     }
 
     pub fn open_font(
         &mut self,
         namespace: NamespaceId,
         font: crate::XResourceId,
         generation: u64,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.resources
             .insert(font, XResourceKind::Font, namespace, generation)
             .map_err(Into::into)
     }
 
     pub fn close_font(
         &mut self,
         namespace: NamespaceId,
         font: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, font, XResourceKind::Font)?;
         self.resources.remove(font);
         Ok(())
     }
 
     pub fn validate_font_access(
         &self,
         namespace: NamespaceId,
         font: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, font, XResourceKind::Font)
             .map(|_| ())
             .map_err(Into::into)
     }
 
     pub fn create_cursor(
         &mut self,
         namespace: NamespaceId,
         cursor: crate::XResourceId,
         generation: u64,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.resources
             .insert(cursor, XResourceKind::Cursor, namespace, generation)
             .map_err(Into::into)
     }
 
     pub fn free_cursor(
         &mut self,
         namespace: NamespaceId,
         cursor: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, cursor, XResourceKind::Cursor)?;
         self.resources.remove(cursor);
         Ok(())
     }
 
     pub fn validate_cursor_access(
         &self,
         namespace: NamespaceId,
         cursor: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, cursor, XResourceKind::Cursor)
             .map(|_| ())
             .map_err(Into::into)
     }
 
}
