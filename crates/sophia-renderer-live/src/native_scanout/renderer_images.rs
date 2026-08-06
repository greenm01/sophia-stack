#[derive(Debug)]
pub struct LiveRendererImageSnapshot {
    pub(super) image_id: LiveRendererImageId,
    pub(super) inner: sophia_renderer_native_egl::NativeRendererImageSnapshot,
}

impl LiveRendererImageSnapshot {
    pub const fn image_id(&self) -> LiveRendererImageId {
        self.image_id
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LiveRendererImageId(u64);

impl LiveRendererImageId {
    pub const INVALID: Self = Self(0);

    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}
