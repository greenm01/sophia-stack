use std::collections::BTreeMap;

use sophia_protocol::{
    AuthorityKind, AuthoritySurface, NamespaceId, Rect, SurfaceConstraints, SurfaceId,
    SurfacePresentationRole,
};

use crate::{XAuthorityAccessError, XMapState, XResourceId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XWindowRecord {
    pub id: XResourceId,
    pub parent: XResourceId,
    pub surface: SurfaceId,
    pub namespace: NamespaceId,
    pub override_redirect: bool,
    pub map_state: XMapState,
    pub geometry: Rect,
    pub constraints: SurfaceConstraints,
    pub generation: u64,
}

impl XWindowRecord {
    pub fn presentation_role(&self) -> SurfacePresentationRole {
        let is_root_child = self.parent.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT);
        if self.override_redirect || !is_root_child {
            SurfacePresentationRole::ClientPositioned
        } else {
            SurfacePresentationRole::PolicyManaged
        }
    }

    pub fn authority_surface(&self) -> AuthoritySurface {
        AuthoritySurface {
            authority: AuthorityKind::SophiaX,
            local_id: self.id.local,
            surface: self.surface,
            namespace: Some(self.namespace),
            presentation: self.presentation_role(),
            mapped: self.map_state == XMapState::Mapped,
            geometry: self.geometry,
            constraints: self.constraints,
            generation: self.generation,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XWindowLifecycleEvent {
    Created {
        id: XResourceId,
        surface: SurfaceId,
        namespace: NamespaceId,
        geometry: Rect,
        constraints: SurfaceConstraints,
        generation: u64,
    },
    Mapped {
        id: XResourceId,
        generation: u64,
    },
    PolicyPending {
        id: XResourceId,
        generation: u64,
    },
    Unmapped {
        id: XResourceId,
        generation: u64,
    },
    Configured {
        id: XResourceId,
        x: Option<i16>,
        y: Option<i16>,
        width: Option<u16>,
        height: Option<u16>,
        generation: u64,
    },
    Destroyed {
        id: XResourceId,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XWindowTable {
    windows: BTreeMap<XResourceId, XWindowRecord>,
}

impl XWindowTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(
        &mut self,
        event: XWindowLifecycleEvent,
    ) -> Result<Option<AuthoritySurface>, XAuthorityAccessError> {
        match event {
            XWindowLifecycleEvent::Created {
                id,
                surface,
                namespace,
                geometry,
                constraints,
                generation,
            } => {
                if !id.is_valid() {
                    return Err(XAuthorityAccessError::InvalidResource);
                }
                if !surface.is_valid() {
                    return Err(XAuthorityAccessError::InvalidResource);
                }
                if !namespace.is_valid() {
                    return Err(XAuthorityAccessError::InvalidNamespace);
                }

                let record = XWindowRecord {
                    id,
                    parent: XResourceId::new(u64::from(crate::X_SETUP_DEFAULT_ROOT), 1),
                    surface,
                    namespace,
                    override_redirect: false,
                    map_state: XMapState::Unmapped,
                    geometry,
                    constraints,
                    generation,
                };
                let authority_surface = record.authority_surface();
                self.windows.insert(id, record);
                Ok(Some(authority_surface))
            }
            // Mapping changes the X11 lifecycle state but does not create a
            // compositor transaction.  In particular, its X11 request
            // sequence must not overwrite the generation used as the
            // predecessor of the next pixel transaction.
            XWindowLifecycleEvent::Mapped { id, generation: _ } => {
                let record = self
                    .windows
                    .get_mut(&id)
                    .ok_or(XAuthorityAccessError::UnknownResource)?;
                if record.map_state == XMapState::Mapped {
                    return Ok(None);
                }
                record.map_state = XMapState::Mapped;
                Ok(Some(record.authority_surface()))
            }
            XWindowLifecycleEvent::PolicyPending { id, generation: _ } => {
                let record = self
                    .windows
                    .get_mut(&id)
                    .ok_or(XAuthorityAccessError::UnknownResource)?;
                if record.map_state != XMapState::Unmapped {
                    return Ok(None);
                }
                record.map_state = XMapState::PolicyPending;
                Ok(Some(record.authority_surface()))
            }
            XWindowLifecycleEvent::Unmapped { id, generation: _ } => {
                let record = self
                    .windows
                    .get_mut(&id)
                    .ok_or(XAuthorityAccessError::UnknownResource)?;
                if record.map_state == XMapState::Unmapped {
                    return Ok(None);
                }
                record.map_state = XMapState::Unmapped;
                Ok(Some(record.authority_surface()))
            }
            XWindowLifecycleEvent::Configured {
                id,
                x,
                y,
                width,
                height,
                generation: _,
            } => {
                let record = self
                    .windows
                    .get_mut(&id)
                    .ok_or(XAuthorityAccessError::UnknownResource)?;
                if let Some(x) = x {
                    record.geometry.x = i32::from(x);
                }
                if let Some(y) = y {
                    record.geometry.y = i32::from(y);
                }
                if let Some(width) = width {
                    record.geometry.width = i32::from(width);
                }
                if let Some(height) = height {
                    record.geometry.height = i32::from(height);
                }
                Ok(Some(record.authority_surface()))
            }
            XWindowLifecycleEvent::Destroyed { id } => {
                self.windows.remove(&id);
                let root = XResourceId::new(u64::from(crate::X_SETUP_DEFAULT_ROOT), 1);
                for record in self.windows.values_mut() {
                    if record.parent == id {
                        record.parent = root;
                    }
                }
                Ok(None)
            }
        }
    }

    pub fn get(&self, id: XResourceId) -> Option<&XWindowRecord> {
        self.windows.get(&id)
    }

    pub fn set_override_redirect(
        &mut self,
        id: XResourceId,
        override_redirect: bool,
    ) -> Result<AuthoritySurface, XAuthorityAccessError> {
        let record = self
            .windows
            .get_mut(&id)
            .ok_or(XAuthorityAccessError::UnknownResource)?;
        record.override_redirect = override_redirect;
        Ok(record.authority_surface())
    }

    pub fn set_constraints(
        &mut self,
        id: XResourceId,
        constraints: SurfaceConstraints,
    ) -> Result<AuthoritySurface, XAuthorityAccessError> {
        let record = self
            .windows
            .get_mut(&id)
            .ok_or(XAuthorityAccessError::UnknownResource)?;
        record.constraints = constraints;
        Ok(record.authority_surface())
    }

    pub fn set_parent(
        &mut self,
        id: XResourceId,
        parent: XResourceId,
    ) -> Result<(), XAuthorityAccessError> {
        if id == parent {
            return Err(XAuthorityAccessError::InvalidResource);
        }
        let mut ancestor = parent;
        while let Some(record) = self.windows.get(&ancestor) {
            if record.parent == id {
                return Err(XAuthorityAccessError::InvalidResource);
            }
            ancestor = record.parent;
        }
        self.windows
            .get_mut(&id)
            .ok_or(XAuthorityAccessError::UnknownResource)?
            .parent = parent;
        Ok(())
    }

    pub fn direct_children(&self, namespace: NamespaceId, parent: XResourceId) -> Vec<XResourceId> {
        self.windows
            .values()
            .filter(|record| record.namespace == namespace && record.parent == parent)
            .map(|record| record.id)
            .collect()
    }

    pub fn presentation_root_and_offset(
        &self,
        id: XResourceId,
    ) -> Result<(XResourceId, i32, i32), XAuthorityAccessError> {
        let mut current = id;
        let mut x = 0i32;
        let mut y = 0i32;
        loop {
            let record = self
                .windows
                .get(&current)
                .ok_or(XAuthorityAccessError::UnknownResource)?;
            if record.parent.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT) {
                return Ok((current, x, y));
            }
            x = x.saturating_add(record.geometry.x);
            y = y.saturating_add(record.geometry.y);
            current = record.parent;
        }
    }

    pub fn advance_generation(
        &mut self,
        id: XResourceId,
        expected: u64,
    ) -> Result<u64, XAuthorityAccessError> {
        let record = self
            .windows
            .get_mut(&id)
            .ok_or(XAuthorityAccessError::UnknownResource)?;
        if record.generation != expected {
            return Err(XAuthorityAccessError::StaleGeneration);
        }
        let next = expected
            .checked_add(1)
            .ok_or(XAuthorityAccessError::InvalidResource)?;
        record.generation = next;
        Ok(next)
    }

    pub fn ids_for_namespace(&self, namespace: NamespaceId) -> Vec<XResourceId> {
        self.windows
            .values()
            .filter(|record| record.namespace == namespace)
            .map(|record| record.id)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
}
