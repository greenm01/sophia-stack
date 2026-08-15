/// Logical outputs Engine will track in one session. This is the
/// logical-output bound; the per-output head bound is `MAX_HEADS_PER_OUTPUT`
/// and native connector tables carry their own limits.
pub const MAX_DRM_KMS_OUTPUTS: usize = 16;

use crate::prelude::*;

/// One display timing in reduced form. The mode is a shape-and-refresh fact;
/// connector, CRTC, and mode-object identity stay behind the backend boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrmKmsMode {
    pub size: Size,
    pub refresh_millihz: u32,
}

impl DrmKmsMode {
    pub const fn new(width: i32, height: i32, refresh_millihz: u32) -> Self {
        Self {
            size: Size { width, height },
            refresh_millihz,
        }
    }
}
