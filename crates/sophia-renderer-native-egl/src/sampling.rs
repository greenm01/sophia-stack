/// Texture sampling policy for a composed layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeCompositionSampling {
    /// Preserve exact source bytes when source and destination pixels correspond.
    ExactNearest,
    /// Use the contrast-preserving reconstruction shader for reductions.
    SharpDownscale,
    /// Use hardware bilinear sampling when enlarging content.
    LinearUpscale,
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeCompositionAlphaMode {
    Opaque,
    Premultiplied,
}

#[cfg(feature = "gbm-platform")]
impl NativeCompositionAlphaMode {
    pub(crate) const fn is_premultiplied(self) -> bool {
        matches!(self, Self::Premultiplied)
    }

    pub(crate) const fn reduced_name(self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::Premultiplied => "premultiplied",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeCompositionSamplingStats {
    pub exact_nearest_draws: usize,
    pub sharp_downscale_draws: usize,
    pub linear_upscale_draws: usize,
    pub sharp_downscale_fallbacks: usize,
}

#[cfg(feature = "gbm-platform")]
impl NativeCompositionSamplingStats {
    pub(crate) fn saturating_add(self, other: Self) -> Self {
        Self {
            exact_nearest_draws: self
                .exact_nearest_draws
                .saturating_add(other.exact_nearest_draws),
            sharp_downscale_draws: self
                .sharp_downscale_draws
                .saturating_add(other.sharp_downscale_draws),
            linear_upscale_draws: self
                .linear_upscale_draws
                .saturating_add(other.linear_upscale_draws),
            sharp_downscale_fallbacks: self
                .sharp_downscale_fallbacks
                .saturating_add(other.sharp_downscale_fallbacks),
        }
    }
}

impl NativeCompositionSampling {
    pub const fn reduced_name(self) -> &'static str {
        match self {
            Self::ExactNearest => "exact_nearest",
            Self::SharpDownscale => "sharp_downscale",
            Self::LinearUpscale => "linear_upscale",
        }
    }
}

/// Selects sampling without consulting renderer state.
///
/// A reduction on either axis needs reconstruction. Mirror projection is
/// uniform today, but treating a future mixed-axis transform as a downscale is
/// the conservative choice because it prevents source rows from being dropped.
pub const fn native_composition_sampling(
    source: (u32, u32),
    target: (u32, u32),
) -> NativeCompositionSampling {
    if source.0 == target.0 && source.1 == target.1 {
        NativeCompositionSampling::ExactNearest
    } else if target.0 < source.0 || target.1 < source.1 {
        NativeCompositionSampling::SharpDownscale
    } else {
        NativeCompositionSampling::LinearUpscale
    }
}
