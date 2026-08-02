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
    /// The client published `WM_TRANSIENT_FOR`, even when its owner is the
    /// root window or cannot be reduced to an Engine surface.
    pub transient_for: bool,
    /// The first recognized EWMH functional type requests popup-like window
    /// management rather than normal blind-WM tiling.
    pub client_positioned_window_type: bool,
    pub presentation_owner: Option<SurfaceId>,
    /// Engine admission is pending for a redirected policy-managed root child.
    /// This is not an X11 map state: the window remains `Unmapped` until the
    /// Engine applies the configure/map decision.
    pub policy_map_pending: bool,
    pub map_state: XMapState,
    pub geometry: Rect,
    pub constraints: SurfaceConstraints,
    pub generation: u64,
}

impl XWindowRecord {
    pub fn presentation_role(&self) -> SurfacePresentationRole {
        let is_root_child = self.parent.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT);
        if self.override_redirect
            || self.transient_for
            || self.client_positioned_window_type
            || !is_root_child
        {
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
            presentation_owner: self.presentation_owner,
            mapped: self.map_state == XMapState::Viewable,
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
                    transient_for: false,
                    client_positioned_window_type: false,
                    presentation_owner: None,
                    policy_map_pending: false,
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
                let parent = self
                    .windows
                    .get(&id)
                    .ok_or(XAuthorityAccessError::UnknownResource)?
                    .parent;
                let parent_viewable = parent.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT)
                    || self
                        .windows
                        .get(&parent)
                        .is_some_and(|parent| parent.map_state == XMapState::Viewable);
                let record = self.windows.get_mut(&id).expect("window checked above");
                if record.map_state != XMapState::Unmapped {
                    return Ok(None);
                }
                record.policy_map_pending = false;
                record.map_state = if parent_viewable {
                    XMapState::Viewable
                } else {
                    XMapState::Unviewable
                };
                let surface = record.authority_surface();
                if parent_viewable {
                    self.promote_unviewable_descendants(id);
                }
                Ok(Some(surface))
            }
            XWindowLifecycleEvent::PolicyPending { id, generation: _ } => {
                let record = self
                    .windows
                    .get_mut(&id)
                    .ok_or(XAuthorityAccessError::UnknownResource)?;
                if record.map_state != XMapState::Unmapped || record.policy_map_pending {
                    return Ok(None);
                }
                record.policy_map_pending = true;
                Ok(Some(record.authority_surface()))
            }
            XWindowLifecycleEvent::Unmapped { id, generation: _ } => {
                let record = self
                    .windows
                    .get_mut(&id)
                    .ok_or(XAuthorityAccessError::UnknownResource)?;
                if record.map_state == XMapState::Unmapped && !record.policy_map_pending {
                    return Ok(None);
                }
                record.policy_map_pending = false;
                record.map_state = XMapState::Unmapped;
                let surface = record.authority_surface();
                self.demote_viewable_descendants(id);
                Ok(Some(surface))
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

    pub fn set_transient_for(
        &mut self,
        id: XResourceId,
        transient_for: bool,
        owner: Option<SurfaceId>,
    ) -> Result<AuthoritySurface, XAuthorityAccessError> {
        let record = self
            .windows
            .get_mut(&id)
            .ok_or(XAuthorityAccessError::UnknownResource)?;
        record.transient_for = transient_for;
        record.presentation_owner = transient_for
            .then_some(owner)
            .flatten()
            .filter(|owner| *owner != record.surface);
        Ok(record.authority_surface())
    }

    pub fn set_client_positioned_window_type(
        &mut self,
        id: XResourceId,
        client_positioned: bool,
    ) -> Result<AuthoritySurface, XAuthorityAccessError> {
        let record = self
            .windows
            .get_mut(&id)
            .ok_or(XAuthorityAccessError::UnknownResource)?;
        record.client_positioned_window_type = client_positioned;
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
        self.recompute_subtree_viewability(id);
        Ok(())
    }

    fn promote_unviewable_descendants(&mut self, parent: XResourceId) {
        let children = self.direct_children_any_namespace(parent);
        for child in children {
            let promoted = self.windows.get_mut(&child).is_some_and(|record| {
                if record.map_state == XMapState::Unviewable {
                    record.map_state = XMapState::Viewable;
                    true
                } else {
                    false
                }
            });
            if promoted
                || self
                    .windows
                    .get(&child)
                    .is_some_and(|record| record.map_state == XMapState::Viewable)
            {
                self.promote_unviewable_descendants(child);
            }
        }
    }

    fn demote_viewable_descendants(&mut self, parent: XResourceId) {
        let children = self.direct_children_any_namespace(parent);
        for child in children {
            let demoted = self.windows.get_mut(&child).is_some_and(|record| {
                if record.map_state == XMapState::Viewable {
                    record.map_state = XMapState::Unviewable;
                    true
                } else {
                    false
                }
            });
            if demoted {
                self.demote_viewable_descendants(child);
            }
        }
    }

    fn recompute_subtree_viewability(&mut self, id: XResourceId) {
        let Some(record) = self.windows.get(&id) else {
            return;
        };
        if record.map_state == XMapState::Unmapped {
            self.demote_viewable_descendants(id);
            return;
        }
        let parent = record.parent;
        let parent_viewable = parent.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT)
            || self
                .windows
                .get(&parent)
                .is_some_and(|parent| parent.map_state == XMapState::Viewable);
        if let Some(record) = self.windows.get_mut(&id) {
            record.map_state = if parent_viewable {
                XMapState::Viewable
            } else {
                XMapState::Unviewable
            };
        }
        if parent_viewable {
            self.promote_unviewable_descendants(id);
        } else {
            self.demote_viewable_descendants(id);
        }
    }

    fn direct_children_any_namespace(&self, parent: XResourceId) -> Vec<XResourceId> {
        self.windows
            .values()
            .filter(|record| record.parent == parent)
            .map(|record| record.id)
            .collect()
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
