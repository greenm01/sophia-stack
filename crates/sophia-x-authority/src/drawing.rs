use sophia_protocol::{
    AuthorityKind, BufferSource, NamespaceId, Region, Size, SurfaceTransaction,
    SurfaceTransactionReadiness, TransactionId,
};

use crate::{XAuthorityAccessError, XResourceId, XWindowTable};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XDrawingUpdateKind {
    PresentPixmap,
    ShmPutImage,
    CoreDraw,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XDrawingUpdate {
    pub transaction: TransactionId,
    pub requester_namespace: NamespaceId,
    pub target_window: XResourceId,
    pub kind: XDrawingUpdateKind,
    pub buffer: BufferSource,
    /// Pixel extent of the drawing window before any descendant-to-toplevel
    /// presentation projection.
    pub target_content_size: Option<Size>,
    pub damage: Region,
    pub previous_committed_generation: u64,
    pub timeout_msec: u32,
}

impl XDrawingUpdate {
    pub fn present_pixmap(
        transaction: TransactionId,
        requester_namespace: NamespaceId,
        target_window: XResourceId,
        pixmap: u32,
        damage: Region,
        previous_committed_generation: u64,
        timeout_msec: u32,
    ) -> Self {
        Self {
            transaction,
            requester_namespace,
            target_window,
            kind: XDrawingUpdateKind::PresentPixmap,
            buffer: BufferSource::XPixmap { pixmap },
            target_content_size: None,
            damage,
            previous_committed_generation,
            timeout_msec,
        }
    }

    pub fn present_buffer(
        transaction: TransactionId,
        requester_namespace: NamespaceId,
        target_window: XResourceId,
        buffer: BufferSource,
        target_content_size: Size,
        damage: Region,
        previous_committed_generation: u64,
        timeout_msec: u32,
    ) -> Self {
        Self {
            transaction,
            requester_namespace,
            target_window,
            kind: XDrawingUpdateKind::PresentPixmap,
            buffer,
            target_content_size: Some(target_content_size),
            damage,
            previous_committed_generation,
            timeout_msec,
        }
    }

    pub fn shm_put_image(
        transaction: TransactionId,
        requester_namespace: NamespaceId,
        target_window: XResourceId,
        handle: u64,
        damage: Region,
        previous_committed_generation: u64,
        timeout_msec: u32,
    ) -> Self {
        Self {
            transaction,
            requester_namespace,
            target_window,
            kind: XDrawingUpdateKind::ShmPutImage,
            buffer: BufferSource::CpuBuffer { handle },
            target_content_size: None,
            damage,
            previous_committed_generation,
            timeout_msec,
        }
    }

    pub fn core_draw(
        transaction: TransactionId,
        requester_namespace: NamespaceId,
        target_window: XResourceId,
        handle: u64,
        damage: Region,
        previous_committed_generation: u64,
        timeout_msec: u32,
    ) -> Self {
        Self {
            transaction,
            requester_namespace,
            target_window,
            kind: XDrawingUpdateKind::CoreDraw,
            buffer: BufferSource::CpuBuffer { handle },
            target_content_size: None,
            damage,
            previous_committed_generation,
            timeout_msec,
        }
    }
}

pub fn surface_transaction_from_drawing_update(
    windows: &XWindowTable,
    update: XDrawingUpdate,
) -> Result<SurfaceTransaction, XAuthorityAccessError> {
    if !update.transaction.is_valid() {
        return Err(XAuthorityAccessError::InvalidResource);
    }
    if !update.requester_namespace.is_valid() {
        return Err(XAuthorityAccessError::InvalidNamespace);
    }
    if !update.target_window.is_valid() {
        return Err(XAuthorityAccessError::InvalidResource);
    }
    if matches!(update.buffer, BufferSource::None) {
        return Err(XAuthorityAccessError::InvalidResource);
    }

    let window = windows
        .get(update.target_window)
        .ok_or(XAuthorityAccessError::UnknownResource)?;

    if window.namespace != update.requester_namespace {
        return Err(XAuthorityAccessError::CrossNamespaceDenied);
    }
    if !window.surface.is_valid() {
        return Err(XAuthorityAccessError::InvalidSurface);
    }

    let target_content_size = update.target_content_size.unwrap_or(Size {
        width: window.geometry.width,
        height: window.geometry.height,
    });
    if target_content_size.width <= 0 || target_content_size.height <= 0 {
        return Err(XAuthorityAccessError::InvalidResource);
    }

    Ok(SurfaceTransaction {
        transaction: update.transaction,
        authority: AuthorityKind::SophiaX,
        surface: window.surface,
        namespace: Some(window.namespace),
        target_geometry: window.geometry,
        target_content_size,
        target_buffer: update.buffer,
        damage: update.damage,
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: update.timeout_msec,
        previous_committed_generation: update.previous_committed_generation,
    })
}
