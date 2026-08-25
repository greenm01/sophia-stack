//! The GLX framebuffer configurations Sophia offers, and the bounds that go with
//! them.
//!
//! One owner for facts that were derived in two places and were about to be
//! derived in a third: the catalog answers `GetFBConfigs`, the runtime resolves a
//! drawable's depth from the same rows, and a pbuffer's refusal threshold is the
//! maximum this module advertises. Two copies of a fact are a drift waiting to
//! happen; three are one that already has.

/// One framebuffer configuration, as a passive row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XGlxFbConfig {
    pub id: u32,
    pub visual: u32,
    /// Alpha bits: zero for the opaque visual, eight for ARGB.
    pub alpha: u32,
    /// Whether the configuration advertises sRGB framebuffer capability.
    pub srgb: u32,
}

impl XGlxFbConfig {
    /// The X depth a drawable of this configuration reports.
    ///
    /// A pure conversion of the row rather than a second table, so a drawable's
    /// depth and the depth advertised for its configuration cannot disagree.
    pub const fn depth(self) -> u8 {
        24 + self.alpha as u8
    }
}

/// The configurations Sophia offers, in reply order.
pub const X_GLX_FB_CONFIGS: [XGlxFbConfig; 3] = [
    XGlxFbConfig {
        id: 1,
        visual: crate::X_SETUP_DEFAULT_VISUAL,
        alpha: 0,
        srgb: 0,
    },
    XGlxFbConfig {
        id: 2,
        visual: crate::X_SETUP_ARGB_VISUAL,
        alpha: 8,
        srgb: 0,
    },
    XGlxFbConfig {
        id: 3,
        visual: crate::X_SETUP_ARGB_VISUAL,
        alpha: 8,
        srgb: 1,
    },
];

/// The largest offscreen surface Sophia will record.
///
/// Sophia allocates nothing for a pbuffer, so this is a refusal threshold rather
/// than a capability claim. It is published as `GLX_MAX_PBUFFER_*` from the same
/// constants that enforce it, so the advertisement cannot drift from the answer.
pub const X_GLX_MAX_PBUFFER_WIDTH: u32 = 4096;
pub const X_GLX_MAX_PBUFFER_HEIGHT: u32 = 4096;
pub const X_GLX_MAX_PBUFFER_PIXELS: u32 = X_GLX_MAX_PBUFFER_WIDTH * X_GLX_MAX_PBUFFER_HEIGHT;

/// `GLX_DRAWABLE_TYPE`: the drawable kinds these configurations support.
///
/// Window and pbuffer, and exactly the kinds Sophia implements.
pub const X_GLX_DRAWABLE_TYPE_MASK: u32 = 0x5;

/// The configuration a client named, if Sophia offers it.
pub fn x_glx_fb_config(id: u32) -> Option<XGlxFbConfig> {
    X_GLX_FB_CONFIGS
        .iter()
        .copied()
        .find(|config| config.id == id)
}
