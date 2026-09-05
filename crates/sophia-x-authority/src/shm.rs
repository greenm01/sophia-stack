use std::collections::BTreeMap;

use sophia_protocol::NamespaceId;

use crate::{XAuthorityAccessError, XResourceId};

/// How a client named the memory behind a segment.
///
/// The mapping itself is not held here: this record is compared and cloned,
/// and a live mapping is neither. The runtime keeps the mapping beside the
/// table, keyed by the same segment id.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XShmBacking {
    /// MIT-SHM 1.1, named by SysV id.
    Sysv(u32),
    /// MIT-SHM 1.2, named by a descriptor the client passed or the server
    /// allocated.
    Descriptor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XShmSegmentRecord {
    pub id: XResourceId,
    pub namespace: NamespaceId,
    pub backing: XShmBacking,
    pub read_only: bool,
    pub generation: u64,
}

impl XShmSegmentRecord {
    /// The SysV id, when that is how this segment was named.
    pub const fn sysv_shmid(&self) -> Option<u32> {
        match self.backing {
            XShmBacking::Sysv(shmid) => Some(shmid),
            XShmBacking::Descriptor => None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XShmSegmentTable {
    records: BTreeMap<XResourceId, XShmSegmentRecord>,
}

impl XShmSegmentTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attach(
        &mut self,
        namespace: NamespaceId,
        id: XResourceId,
        backing: XShmBacking,
        read_only: bool,
        generation: u64,
    ) -> Result<(), XAuthorityAccessError> {
        if !namespace.is_valid() {
            return Err(XAuthorityAccessError::InvalidNamespace);
        }
        if !id.is_valid() {
            return Err(XAuthorityAccessError::InvalidResource);
        }

        self.records.insert(
            id,
            XShmSegmentRecord {
                id,
                namespace,
                backing,
                read_only,
                generation,
            },
        );
        Ok(())
    }

    pub fn lookup(
        &self,
        namespace: NamespaceId,
        id: XResourceId,
    ) -> Result<&XShmSegmentRecord, XAuthorityAccessError> {
        if !namespace.is_valid() {
            return Err(XAuthorityAccessError::InvalidNamespace);
        }
        if !id.is_valid() {
            return Err(XAuthorityAccessError::InvalidResource);
        }

        let record = self
            .records
            .get(&id)
            .ok_or(XAuthorityAccessError::UnknownResource)?;
        if record.namespace != namespace {
            return Err(XAuthorityAccessError::CrossNamespaceDenied);
        }
        Ok(record)
    }

    pub fn detach(
        &mut self,
        namespace: NamespaceId,
        id: XResourceId,
    ) -> Result<(), XAuthorityAccessError> {
        self.lookup(namespace, id)?;
        self.records.remove(&id);
        Ok(())
    }

    pub fn ids_for_namespace_in_client_range(
        &self,
        namespace: NamespaceId,
        range: crate::XWireClientResourceRange,
    ) -> Vec<XResourceId> {
        self.records
            .values()
            .filter(|record| {
                record.namespace == namespace
                    && u32::try_from(record.id.local.raw())
                        .is_ok_and(|raw| range.owns_new_resource(raw))
            })
            .map(|record| record.id)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
