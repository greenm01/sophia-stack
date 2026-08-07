impl XAuthorityRuntime {
    pub fn create_colormap(
        &mut self,
        namespace: NamespaceId,
        colormap: crate::XResourceId,
        visual: u32,
        generation: u64,
    ) -> Result<(), crate::XColormapError> {
        if crate::x_true_color_visual(visual).is_none() {
            return Err(crate::XColormapError::UnknownVisual);
        }
        if self.resources.get(colormap).is_some() {
            return Err(crate::XColormapError::DuplicateId);
        }
        self.resources
            .insert(colormap, XResourceKind::Colormap, namespace, generation)
            .map_err(crate::XColormapError::Access)?;
        self.colormaps.insert(colormap, visual);
        Ok(())
    }

    pub fn colormap_visual(
        &self,
        namespace: NamespaceId,
        colormap: crate::XResourceId,
    ) -> Result<u32, crate::XColormapError> {
        if !namespace.is_valid() {
            return Err(crate::XColormapError::Access(
                crate::XAuthorityAccessError::InvalidNamespace,
            ));
        }
        if colormap.local.raw() == u64::from(crate::X_SETUP_DEFAULT_COLORMAP) {
            return Ok(crate::X_SETUP_DEFAULT_VISUAL);
        }
        self.resources
            .lookup(namespace, colormap, XResourceKind::Colormap)
            .map_err(crate::XColormapError::Access)?;
        self.colormaps
            .get(&colormap)
            .copied()
            .ok_or(crate::XColormapError::Access(
                crate::XAuthorityAccessError::UnknownResource,
            ))
    }

    pub fn free_colormap(
        &mut self,
        namespace: NamespaceId,
        colormap: crate::XResourceId,
    ) -> Result<(), crate::XColormapError> {
        if colormap.local.raw() == u64::from(crate::X_SETUP_DEFAULT_COLORMAP) {
            if namespace.is_valid() {
                return Ok(());
            }
            return Err(crate::XColormapError::Access(
                crate::XAuthorityAccessError::InvalidNamespace,
            ));
        }
        self.colormap_visual(namespace, colormap)?;
        self.resources.remove(colormap);
        self.colormaps.remove(&colormap);
        Ok(())
    }
}
