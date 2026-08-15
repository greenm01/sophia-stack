fn software_pixmap_byte_len(size: Size) -> Option<usize> {
    let width = usize::try_from(size.width).ok()?;
    let height = usize::try_from(size.height).ok()?;
    let bytes = width.checked_mul(height)?.checked_mul(4)?;
    (width != 0 && height != 0 && bytes <= crate::X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES)
        .then_some(bytes)
}

const X_AUTHORITY_PRESENT_REGION_MAX_RECTS: usize = 2_048;

fn clipped_present_rect(size: Size, rect: Rect) -> Option<Rect> {
    let left = rect.x.max(0).min(size.width);
    let top = rect.y.max(0).min(size.height);
    let right = rect.x.saturating_add(rect.width).max(0).min(size.width);
    let bottom = rect.y.saturating_add(rect.height).max(0).min(size.height);
    (right > left && bottom > top).then_some(Rect {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    })
}

fn present_source_damage(size: Size, update: Option<&Region>) -> Option<Vec<Rect>> {
    if update.is_some_and(|region| region.rects.len() > X_AUTHORITY_PRESENT_REGION_MAX_RECTS) {
        return None;
    }
    Some(match update {
        Some(region) => region
            .rects
            .iter()
            .filter_map(|rect| clipped_present_rect(size, *rect))
            .collect(),
        None => vec![Rect {
            x: 0,
            y: 0,
            width: size.width,
            height: size.height,
        }],
    })
}

fn translated_present_damage(source: &[Rect], x_offset: i16, y_offset: i16) -> Region {
    Region {
        rects: source
            .iter()
            .map(|rect| Rect {
                x: rect.x.saturating_add(i32::from(x_offset)),
                y: rect.y.saturating_add(i32::from(y_offset)),
                width: rect.width,
                height: rect.height,
            })
            .collect(),
    }
}

impl XAuthorityRuntime {
    pub fn create_pixmap(
        &mut self,
        namespace: NamespaceId,
        pixmap: crate::XResourceId,
        size: Size,
        depth: u8,
        generation: u64,
    ) -> Result<(), XAuthorityRuntimeError> {
        if size.width <= 0 || size.height <= 0 || crate::x11_pixmap_format(depth).is_none() {
            return Err(XAuthorityRuntimeError::InvalidResource);
        }
        self.resources
            .insert(pixmap, XResourceKind::Pixmap, namespace, generation)
            .map_err(XAuthorityRuntimeError::from)?;
        self.pixmaps.insert(pixmap, XPixmapRecord { size, depth });
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
        self.pixmaps.remove(&pixmap);
        self.shm_pixmaps.remove(&pixmap);
        self.shm_mappings
            .retain(|_, mapping| mapping.strong_count() != 0);
        self.software_buffers.remove(pixmap);
        Ok(self
            .dri3_pixmaps
            .remove(&pixmap)
            .map(|descriptor| descriptor.handle))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_shm_pixmap(
        &mut self,
        namespace: NamespaceId,
        pixmap: crate::XResourceId,
        size: Size,
        depth: u8,
        generation: u64,
        segment: crate::XResourceId,
        offset: u32,
    ) -> Result<(), XAuthorityRuntimeError> {
        let shmid = self.shm_segment_shmid(namespace, segment)?;
        let byte_len =
            software_pixmap_byte_len(size).ok_or(XAuthorityRuntimeError::InvalidResource)?;
        let end = usize::try_from(offset)
            .ok()
            .and_then(|offset| offset.checked_add(byte_len))
            .ok_or(XAuthorityRuntimeError::InvalidResource)?;
        if end > crate::X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES {
            return Err(XAuthorityRuntimeError::InvalidResource);
        }
        let mapping = self
            .shm_mappings
            .get(&shmid)
            .and_then(Weak::upgrade)
            .map_or_else(
                || {
                    sophia_sysv_shm::ReadOnlyMapping::attach(shmid)
                        .map(Arc::new)
                        .map_err(|_| XAuthorityRuntimeError::InvalidResource)
                },
                Ok,
            )?;
        if end > mapping.len() {
            return Err(XAuthorityRuntimeError::InvalidResource);
        }
        self.create_pixmap(namespace, pixmap, size, depth, generation)?;
        self.shm_mappings.insert(shmid, Arc::downgrade(&mapping));
        self.shm_pixmaps.insert(
            pixmap,
            XShmPixmapBinding {
                offset,
                size,
                mapping,
            },
        );
        Ok(())
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
        self.pixmap_geometry(namespace, pixmap)
            .map(|(size, _)| size)
    }

    pub fn pixmap_geometry(
        &self,
        namespace: NamespaceId,
        pixmap: crate::XResourceId,
    ) -> Result<(Size, u8), XAuthorityRuntimeError> {
        self.validate_pixmap_access(namespace, pixmap)?;
        self.pixmaps
            .get(&pixmap)
            .map(|record| (record.size, record.depth))
            .ok_or(XAuthorityRuntimeError::UnknownResource)
    }

    pub fn pixmap_depth(
        &self,
        namespace: NamespaceId,
        pixmap: crate::XResourceId,
    ) -> Result<u8, XAuthorityRuntimeError> {
        self.pixmap_geometry(namespace, pixmap)
            .map(|(_, depth)| depth)
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
        self.create_pixmap(namespace, pixmap, descriptor.size, depth, generation)?;
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
        self.create_pixmap(namespace, pixmap, descriptor.size, depth, generation)?;
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
        x_offset: i16,
        y_offset: i16,
        valid_region: Option<Region>,
        update_region: Option<Region>,
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
        if valid_region
            .as_ref()
            .is_some_and(|region| region.rects.len() > X_AUTHORITY_PRESENT_REGION_MAX_RECTS)
        {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        }
        let Some(pixmap_size) = self.pixmaps.get(&pixmap).map(|record| record.size) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::UnknownResource,
            );
        };
        let Some(source_damage) = present_source_damage(pixmap_size, update_region.as_ref()) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let damage = translated_present_damage(&source_damage, x_offset, y_offset);
        let (target_window, target_generation, buffer, damage) =
            if let Some(descriptor) = self.dri3_pixmaps.get(&pixmap) {
                let (presentation_window, _, child_x, child_y) =
                    match self.window_presentation_root_and_offset(namespace, window) {
                        Ok(presentation) => presentation,
                        Err(error) => {
                            return XAuthorityResponsePacket::rejected(transaction, error);
                        }
                    };
                let Some(presentation_record) = self.windows.get(presentation_window) else {
                    return XAuthorityResponsePacket::rejected(
                        transaction,
                        XAuthorityRuntimeError::UnknownResource,
                    );
                };
                (
                    presentation_window,
                    presentation_record.generation,
                    sophia_protocol::BufferSource::DmaBuf {
                        handle: descriptor.handle.raw(),
                    },
                    Region {
                        rects: damage
                            .rects
                            .into_iter()
                            .map(|rect| Rect {
                                x: rect.x.saturating_add(child_x),
                                y: rect.y.saturating_add(child_y),
                                ..rect
                            })
                            .collect(),
                    },
                )
            } else {
                if let Some(binding) = self.shm_pixmaps.get(&pixmap).cloned() {
                    let Some(stride) = usize::try_from(binding.size.width)
                        .ok()
                        .and_then(|width| width.checked_mul(4))
                    else {
                        return XAuthorityResponsePacket::rejected(
                            transaction,
                            XAuthorityRuntimeError::InvalidResource,
                        );
                    };
                    if self
                        .software_buffers
                        .ensure_image_backing(pixmap, binding.size)
                        .is_none()
                    {
                        return XAuthorityResponsePacket::rejected(
                            transaction,
                            XAuthorityRuntimeError::InvalidResource,
                        );
                    }
                    for rect in &source_damage {
                        let packed = usize::try_from(binding.offset).ok().and_then(|offset| {
                            let row_offset = usize::try_from(rect.x).ok()?.checked_mul(4)?;
                            let row_bytes = usize::try_from(rect.width).ok()?.checked_mul(4)?;
                            let rows = usize::try_from(rect.height).ok()?;
                            let source_y = usize::try_from(rect.y).ok()?.checked_mul(stride)?;
                            binding
                                .mapping
                                .copy_rows(
                                    offset.checked_add(source_y)?,
                                    stride,
                                    row_offset,
                                    row_bytes,
                                    rows,
                                )
                                .ok()
                        });
                        if packed.as_ref().is_none_or(|bytes| {
                            self.software_buffers
                                .put_image_backing(pixmap, binding.size, *rect, bytes)
                                .is_none()
                        }) {
                            return XAuthorityResponsePacket::rejected(
                                transaction,
                                XAuthorityRuntimeError::InvalidResource,
                            );
                        }
                    }
                }
                let Some(update) = self.software_buffers.present_window_damage(
                    window,
                    Size {
                        width: record.geometry.width,
                        height: record.geometry.height,
                    },
                    pixmap,
                    i32::from(x_offset),
                    i32::from(y_offset),
                    &source_damage,
                ) else {
                    return XAuthorityResponsePacket::rejected(
                        transaction,
                        XAuthorityRuntimeError::InvalidResource,
                    );
                };
                let handle = update.handle();
                self.last_cpu_buffer_update = Some(update);
                (
                    window,
                    record.generation,
                    sophia_protocol::BufferSource::CpuBuffer { handle },
                    damage,
                )
            };
        self.finish_drawing_update(XDrawingUpdate::present_buffer(
            transaction,
            namespace,
            target_window,
            buffer,
            Size {
                width: record.geometry.width,
                height: record.geometry.height,
            },
            damage,
            target_generation,
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

    pub fn xfixes_region_snapshot(
        &self,
        namespace: NamespaceId,
        region: crate::XResourceId,
    ) -> Result<Region, XAuthorityRuntimeError> {
        self.validate_xfixes_region_access(namespace, region)?;
        self.xfixes_regions
            .get(&region)
            .cloned()
            .ok_or(XAuthorityRuntimeError::UnknownResource)
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
        self.open_font_face(namespace, font, crate::XFontFace::default(), generation)
    }

    pub(crate) fn open_font_face(
        &mut self,
        namespace: NamespaceId,
        font: crate::XResourceId,
        face: crate::XFontFace,
        generation: u64,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .insert(font, XResourceKind::Font, namespace, generation)
            .map_err(XAuthorityRuntimeError::from)?;
        self.fonts.insert(font, XFontRecord { face });
        Ok(())
    }

    pub fn close_font(
        &mut self,
        namespace: NamespaceId,
        font: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .lookup(namespace, font, XResourceKind::Font)?;
        self.resources.remove(font);
        self.fonts.remove(&font);
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

    pub(crate) fn font_face(
        &self,
        namespace: NamespaceId,
        font: crate::XResourceId,
    ) -> Result<crate::XFontFace, XAuthorityRuntimeError> {
        self.validate_font_access(namespace, font)?;
        self.fonts
            .get(&font)
            .map(|record| record.face)
            .ok_or(XAuthorityRuntimeError::UnknownResource)
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
