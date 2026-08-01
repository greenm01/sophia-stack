impl XAuthorityRuntime {
     pub fn resource_count(&self) -> usize {
         self.resources.len()
     }
 
     pub fn window_count(&self) -> usize {
         self.windows.len()
     }
 
     pub fn shm_segment_count(&self) -> usize {
         self.shm_segments.len()
     }
 
     pub fn attach_shm_segment(
         &mut self,
         namespace: NamespaceId,
         segment: crate::XResourceId,
         shmid: u32,
         read_only: bool,
         generation: u64,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.shm_segments
             .attach(namespace, segment, shmid, read_only, generation)
             .map_err(Into::into)
     }
 
     pub fn detach_shm_segment(
         &mut self,
         namespace: NamespaceId,
         segment: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.shm_segments
             .detach(namespace, segment)
             .map_err(Into::into)
     }
 
     pub fn validate_shm_segment_access(
         &self,
         namespace: NamespaceId,
         segment: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.shm_segments
             .lookup(namespace, segment)
             .map(|_| ())
             .map_err(Into::into)
     }
 
     pub fn shm_segment_shmid(
         &self,
         namespace: NamespaceId,
         segment: crate::XResourceId,
     ) -> Result<u32, XAuthorityRuntimeError> {
         self.shm_segments
             .lookup(namespace, segment)
             .map(|record| record.shmid)
             .map_err(Into::into)
     }
 
     pub fn validate_window_access(
         &self,
         namespace: NamespaceId,
         window: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         if self.is_clipboard_proxy(namespace, window) {
             return Ok(());
         }
         self.resources
             .lookup(namespace, window, XResourceKind::Window)
             .map(|_| ())
             .map_err(Into::into)
     }
 
     pub fn window_geometry(
         &self,
         namespace: NamespaceId,
         window: crate::XResourceId,
     ) -> Result<Rect, XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, window, XResourceKind::Window)?;
         self.windows
             .get(window)
             .map(|record| record.geometry)
             .ok_or(XAuthorityRuntimeError::UnknownResource)
     }

     pub fn window_override_redirect(
         &self,
         namespace: NamespaceId,
         window: crate::XResourceId,
     ) -> Result<bool, XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, window, XResourceKind::Window)?;
         self.windows
             .get(window)
             .map(|record| record.override_redirect)
             .ok_or(XAuthorityRuntimeError::UnknownResource)
     }

     pub fn set_window_override_redirect(
         &mut self,
         namespace: NamespaceId,
         window: crate::XResourceId,
         override_redirect: bool,
     ) -> Result<AuthoritySurface, XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, window, XResourceKind::Window)?;
         self.windows
             .set_override_redirect(window, override_redirect)
             .map_err(Into::into)
     }

     pub fn set_window_constraints(
         &mut self,
         namespace: NamespaceId,
         window: crate::XResourceId,
         constraints: sophia_protocol::SurfaceConstraints,
     ) -> Result<AuthoritySurface, XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, window, XResourceKind::Window)?;
         self.windows
             .set_constraints(window, constraints)
             .map_err(Into::into)
     }

    pub fn set_window_transient_for(
        &mut self,
        namespace: NamespaceId,
        window: crate::XResourceId,
        owner: Option<crate::XResourceId>,
    ) -> Result<AuthoritySurface, XAuthorityRuntimeError> {
        self.resources
            .lookup(namespace, window, XResourceKind::Window)?;
        let owner_surface = owner.and_then(|owner| {
            self.resources
                .lookup(namespace, owner, XResourceKind::Window)
                .ok()?;
            let (root, _, _) = self.windows.presentation_root_and_offset(owner).ok()?;
            self.windows.get(root).map(|record| record.surface)
        });
        self.windows
            .set_presentation_owner(window, owner_surface)
            .map_err(Into::into)
    }

    pub fn set_window_parent(
         &mut self,
         namespace: NamespaceId,
         window: crate::XResourceId,
         parent: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, window, XResourceKind::Window)?;
         if parent.local.raw() != u64::from(crate::X_SETUP_DEFAULT_ROOT) {
             self.resources
                 .lookup(namespace, parent, XResourceKind::Window)?;
         }
        self.windows.set_parent(window, parent).map_err(Into::into)
    }

    pub fn window_presentation_root_and_offset(
        &self,
        namespace: NamespaceId,
        window: crate::XResourceId,
    ) -> Result<(crate::XResourceId, sophia_protocol::SurfaceId, i32, i32), XAuthorityRuntimeError>
    {
        self.resources
            .lookup(namespace, window, XResourceKind::Window)?;
        let (root, x, y) = self.windows.presentation_root_and_offset(window)?;
        let record = self
            .windows
            .get(root)
            .ok_or(XAuthorityRuntimeError::UnknownResource)?;
        Ok((root, record.surface, x, y))
    }

     pub fn window_parent_and_children(
         &self,
         namespace: NamespaceId,
         window: crate::XResourceId,
     ) -> Result<(crate::XResourceId, Vec<crate::XResourceId>), XAuthorityRuntimeError> {
         if window.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT) {
             return Ok((
                 crate::XResourceId::NONE,
                 self.windows.direct_children(namespace, window),
             ));
         }
         self.resources
             .lookup(namespace, window, XResourceKind::Window)?;
         let parent = self
             .windows
             .get(window)
             .ok_or(XAuthorityRuntimeError::UnknownResource)?
             .parent;
         Ok((parent, self.windows.direct_children(namespace, window)))
     }
 
     pub fn set_window_visual(
         &mut self,
         window: crate::XResourceId,
         depth: u8,
         visual: u32,
         colormap: crate::XResourceId,
     ) {
         self.window_visuals
             .insert(window, (depth, visual, colormap));
     }
 
     pub fn window_visual(&self, window: crate::XResourceId) -> (u8, u32, crate::XResourceId) {
         self.window_visuals.get(&window).copied().unwrap_or((
             24,
             crate::X_SETUP_DEFAULT_VISUAL,
             crate::XResourceId::new(u64::from(crate::X_SETUP_DEFAULT_COLORMAP), 1),
         ))
     }

     pub fn window_map_state(
         &self,
         namespace: NamespaceId,
         window: crate::XResourceId,
     ) -> Result<crate::XMapState, XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, window, XResourceKind::Window)?;
         self.windows
             .get(window)
             .map(|record| record.map_state)
             .ok_or(XAuthorityRuntimeError::UnknownResource)
     }

     /// Whether an X client may directly mutate this window's geometry.
     /// Once a policy-managed toplevel is mapped, geometry belongs to the
     /// Engine/WM control path; children and override-redirect windows retain
     /// normal client geometry authority.
     pub fn client_controls_window_geometry(
         &self,
         namespace: NamespaceId,
         window: crate::XResourceId,
     ) -> Result<bool, XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, window, XResourceKind::Window)?;
         let record = self
             .windows
             .get(window)
             .ok_or(XAuthorityRuntimeError::UnknownResource)?;
         Ok(record.map_state != crate::XMapState::Mapped
             || record.presentation_role()
                 != sophia_protocol::SurfacePresentationRole::PolicyManaged)
     }
 
     pub fn create_glx_context(
         &mut self,
         namespace: NamespaceId,
         context: crate::XResourceId,
         fbconfig: u32,
         direct: bool,
     ) -> Result<(), XAuthorityRuntimeError> {
         if self.resources.get(context).is_some()
             || self.glx_contexts.contains_key(&context)
             || self.glx_windows.contains_key(&context)
         {
             return Err(XAuthorityRuntimeError::InvalidResource);
         }
         self.glx_contexts
             .insert(context, (namespace, fbconfig, direct));
         Ok(())
     }
 
     pub fn glx_context(
         &self,
         namespace: NamespaceId,
         context: crate::XResourceId,
     ) -> Result<(u32, bool), XAuthorityRuntimeError> {
         self.glx_contexts
             .get(&context)
             .filter(|(owner, _, _)| *owner == namespace)
             .map(|(_, config, direct)| (*config, *direct))
             .ok_or(XAuthorityRuntimeError::UnknownResource)
     }
 
     pub fn destroy_glx_context(
         &mut self,
         namespace: NamespaceId,
         context: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.glx_context(namespace, context)?;
         self.glx_contexts.remove(&context);
         Ok(())
     }
 
     pub fn create_glx_window(
         &mut self,
         namespace: NamespaceId,
         glx_window: crate::XResourceId,
         window: crate::XResourceId,
         fbconfig: u32,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.validate_window_access(namespace, window)?;
         if self.resources.get(glx_window).is_some()
             || self.glx_contexts.contains_key(&glx_window)
             || self.glx_windows.contains_key(&glx_window)
         {
             return Err(XAuthorityRuntimeError::InvalidResource);
         }
         self.glx_windows
             .insert(glx_window, (namespace, window, fbconfig));
         Ok(())
     }
 
     pub fn glx_drawable(
         &self,
         namespace: NamespaceId,
         drawable: crate::XResourceId,
     ) -> Result<(crate::XResourceId, u32), XAuthorityRuntimeError> {
         if let Some((owner, window, config)) = self.glx_windows.get(&drawable) {
             return (*owner == namespace)
                 .then_some((*window, *config))
                 .ok_or(XAuthorityRuntimeError::UnknownResource);
         }
         self.validate_window_access(namespace, drawable)?;
         let config = if self.window_visual(drawable).1 == crate::X_SETUP_ARGB_VISUAL {
             3
         } else {
             1
         };
         Ok((drawable, config))
     }
 
     pub fn destroy_glx_window(
         &mut self,
         namespace: NamespaceId,
         glx_window: crate::XResourceId,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.glx_drawable(namespace, glx_window)?;
         self.glx_windows.remove(&glx_window);
         Ok(())
     }
 
     pub fn configure_window_geometry(
         &mut self,
         namespace: NamespaceId,
         window: crate::XResourceId,
         update: XWindowGeometryUpdate,
     ) -> Result<(), XAuthorityRuntimeError> {
         self.configure_window_geometry_observed(namespace, window, update)
             .map(drop)
     }

     pub(crate) fn configure_window_geometry_observed(
         &mut self,
         namespace: NamespaceId,
         window: crate::XResourceId,
         update: XWindowGeometryUpdate,
     ) -> Result<AuthoritySurface, XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, window, XResourceKind::Window)?;
         self.windows.apply(XWindowLifecycleEvent::Configured {
             id: window,
             x: update.x,
             y: update.y,
             width: update.width,
             height: update.height,
             generation: update.generation,
         })?
         .ok_or(XAuthorityRuntimeError::UnknownResource)
     }
 
     /// Ends an X11 window's lifetime and returns the Sophia surface that the
     /// Engine must remove from its committed snapshot.
     pub fn destroy_window(
         &mut self,
         namespace: NamespaceId,
         window: crate::XResourceId,
     ) -> Result<sophia_protocol::SurfaceId, XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, window, XResourceKind::Window)?;
         let surface = self
             .windows
             .get(window)
             .ok_or(XAuthorityRuntimeError::UnknownResource)?
             .surface;
         self.selections.clear_window_owner(
             window,
             &self.windows,
             crate::XSelectionChangeKind::SelectionWindowDestroyed,
         );
         self.windows
             .apply(XWindowLifecycleEvent::Destroyed { id: window })?;
         self.resources.remove(window);
         self.software_buffers.remove(window);
         self.window_background_pixels.remove(&window);
         self.window_visuals.remove(&window);
         self.glx_windows
             .retain(|_, (_, underlying, _)| *underlying != window);
         if self
             .input_focus
             .get(&namespace)
             .is_some_and(|(focus, _)| *focus == window)
         {
             self.input_focus.remove(&namespace);
         }
         Ok(surface)
     }
 
     /// Reclaims every supported resource created from a disconnected client's
     /// XID range. Existing-resource references are intentionally not used as
     /// ownership evidence: classic shared-X clients may refer to one another's
     /// resources, while allocation is constrained by the setup range.
     pub fn release_client_resource_range(
         &mut self,
         namespace: NamespaceId,
         range: crate::XWireClientResourceRange,
     ) -> Result<XAuthorityClientResourceRelease, XAuthorityRuntimeError> {
         if !namespace.is_valid() {
             return Err(XAuthorityRuntimeError::InvalidNamespace);
         }
 
         let mut release = XAuthorityClientResourceRelease::default();
         self.glx_contexts.retain(|id, (owner, _, _)| {
             let owned = *owner == namespace
                 && u32::try_from(id.local.raw()).is_ok_and(|raw| range.owns_new_resource(raw));
             if owned {
                 release.released_glx_contexts = release.released_glx_contexts.saturating_add(1);
             }
             !owned
         });
         self.glx_windows.retain(|id, (owner, _, _)| {
             let owned = *owner == namespace
                 && u32::try_from(id.local.raw()).is_ok_and(|raw| range.owns_new_resource(raw));
             if owned {
                 release.released_glx_windows = release.released_glx_windows.saturating_add(1);
             }
             !owned
         });
         for gc in self
             .graphics_contexts
             .ids_for_namespace_in_client_range(namespace, range)
         {
             self.free_graphics_context(namespace, gc)?;
             release.released_graphics_contexts =
                 release.released_graphics_contexts.saturating_add(1);
         }
         for segment in self
             .shm_segments
             .ids_for_namespace_in_client_range(namespace, range)
         {
             self.detach_shm_segment(namespace, segment)?;
             release.released_shm_segments = release.released_shm_segments.saturating_add(1);
         }
 
         for record in self
             .resources
             .records_for_namespace_in_client_range(namespace, range)
         {
             match record.kind {
                 XResourceKind::Window => {
                     let surface = self.destroy_window(namespace, record.id)?;
                     release.destroyed_windows.push(record.id);
                     release.removed_surfaces.push(surface);
                 }
                 XResourceKind::Pixmap => {
                     if let Some(handle) = self.free_pixmap(namespace, record.id)? {
                         release.released_dma_bufs.push(handle);
                     }
                     release.released_pixmaps = release.released_pixmaps.saturating_add(1);
                 }
                 XResourceKind::Font => {
                     self.close_font(namespace, record.id)?;
                     release.released_fonts = release.released_fonts.saturating_add(1);
                 }
                 XResourceKind::Cursor => {
                     self.free_cursor(namespace, record.id)?;
                     release.released_cursors = release.released_cursors.saturating_add(1);
                 }
                 // The reduced frontend does not currently persist client atoms,
                 // colormaps, or GCs in the resource table. Remove any future
                 // record in this range rather than retaining a disconnect leak.
                 XResourceKind::Atom | XResourceKind::Property | XResourceKind::GraphicsContext => {
                     self.resources.remove(record.id);
                 }
                 XResourceKind::Fence => {
                     self.resources.remove(record.id);
                     if let Some(handle) = self.dri3_fences.remove(&record.id) {
                         release.released_fences.push(handle);
                     }
                 }
                 XResourceKind::Region => {
                     self.resources.remove(record.id);
                     self.xfixes_regions.remove(&record.id);
                 }
                 XResourceKind::SyncCounter => {
                     self.resources.remove(record.id);
                     self.sync_counters.remove(&record.id);
                 }
             }
         }
         Ok(release)
     }
 
     pub fn configure_window_size_from_engine(
         &mut self,
         namespace: NamespaceId,
         window: crate::XResourceId,
         size: Size,
     ) -> Result<Rect, XAuthorityRuntimeError> {
         if size.width <= 0
             || size.height <= 0
             || size.width > i32::from(u16::MAX)
             || size.height > i32::from(u16::MAX)
         {
             return Err(XAuthorityRuntimeError::InvalidResource);
         }
         let current = self.window_geometry(namespace, window)?;
         let generation = self
             .windows
             .get(window)
             .ok_or(XAuthorityRuntimeError::UnknownResource)?
             .generation;
         self.configure_window_geometry(
             namespace,
             window,
             XWindowGeometryUpdate {
                 width: Some(u16::try_from(size.width).expect("validated above")),
                 height: Some(u16::try_from(size.height).expect("validated above")),
                 generation,
                 ..XWindowGeometryUpdate::default()
             },
         )?;
         Ok(Rect {
             width: size.width,
             height: size.height,
             ..current
         })
     }

     pub fn admit_window_from_engine(
         &mut self,
         namespace: NamespaceId,
         window: crate::XResourceId,
         geometry: Rect,
     ) -> Result<Rect, XAuthorityRuntimeError> {
         if geometry.is_empty()
             || geometry.width > i32::from(u16::MAX)
             || geometry.height > i32::from(u16::MAX)
             || geometry.x < i32::from(i16::MIN)
             || geometry.x > i32::from(i16::MAX)
             || geometry.y < i32::from(i16::MIN)
             || geometry.y > i32::from(i16::MAX)
         {
             return Err(XAuthorityRuntimeError::InvalidResource);
         }
         self.resources
             .lookup(namespace, window, XResourceKind::Window)?;
         let record = self
             .windows
             .get(window)
             .ok_or(XAuthorityRuntimeError::UnknownResource)?;
         if record.map_state != crate::XMapState::PolicyPending {
             return Err(XAuthorityRuntimeError::InvalidResource);
         }
         let generation = record.generation;
         self.configure_window_geometry(
             namespace,
             window,
             XWindowGeometryUpdate {
                 x: Some(i16::try_from(geometry.x).expect("validated above")),
                 y: Some(i16::try_from(geometry.y).expect("validated above")),
                 width: Some(u16::try_from(geometry.width).expect("validated above")),
                 height: Some(u16::try_from(geometry.height).expect("validated above")),
                 generation,
             },
         )?;
         self.windows.apply(XWindowLifecycleEvent::Mapped {
             id: window,
             generation,
         })?;
         Ok(geometry)
     }

    pub fn unmap_window(
        &mut self,
        namespace: NamespaceId,
        window: crate::XResourceId,
    ) -> Result<Option<AuthoritySurface>, XAuthorityRuntimeError> {
         self.resources
             .lookup(namespace, window, XResourceKind::Window)?;
         let record = self
             .windows
             .get(window)
             .ok_or(XAuthorityRuntimeError::UnknownResource)?;
         let generation = record.generation;
         self.windows.apply(XWindowLifecycleEvent::Unmapped {
             id: window,
             generation,
         }).map_err(Into::into)
     }
 
     pub fn map_direct_subwindows(
         &mut self,
         namespace: NamespaceId,
         parent: crate::XResourceId,
         generation: u64,
     ) -> Result<Vec<AuthoritySurface>, XAuthorityRuntimeError> {
         if parent.local.raw() != u64::from(crate::X_SETUP_DEFAULT_ROOT) {
             self.resources
                 .lookup(namespace, parent, XResourceKind::Window)?;
         }
         let mut surfaces = Vec::new();
         for window in self.windows.direct_children(namespace, parent) {
             let role = self
                 .windows
                 .get(window)
                 .ok_or(XAuthorityRuntimeError::UnknownResource)?
                 .presentation_role();
             let event = if role
                 == sophia_protocol::SurfacePresentationRole::ClientPositioned
                 || !self.defer_policy_maps
             {
                 XWindowLifecycleEvent::Mapped {
                     id: window,
                     generation,
                 }
             } else {
                 XWindowLifecycleEvent::PolicyPending {
                     id: window,
                     generation,
                 }
             };
             if let Some(surface) = self.windows.apply(event)? {
                 surfaces.push(surface);
             }
         }
         Ok(surfaces)
     }
 
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XWindowGeometryUpdate {
    pub x: Option<i16>,
    pub y: Option<i16>,
    pub width: Option<u16>,
    pub height: Option<u16>,
    pub generation: u64,
}
