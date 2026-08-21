/// Texture sampling policy for a composed layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeCompositionSampling {
    /// Preserve exact source bytes when source and destination pixels correspond.
    ExactNearest,
    /// Reconstruct a reduction with the Catmull-Rom shader, in linear light.
    SharpDownscale,
    /// Reconstruct an enlargement with the same shader, in linear light.
    ///
    /// Catmull-Rom is an interpolating kernel -- it passes through its samples --
    /// so it is the textbook bicubic upsample as well as a reduction filter, and
    /// one program serving both directions means one place where light is
    /// decoded and re-encoded rather than two that could drift apart.
    SharpUpscale,
    /// Hardware bilinear, reached only when the reconstruction shader is absent.
    ///
    /// Its own variant rather than a reuse of the upscale name, because a
    /// degraded draw and an enlargement are different facts and the counter that
    /// used to hold both could not tell anyone which had happened. It filters in
    /// gamma-encoded space, which is the defect the shader exists to avoid, so a
    /// run reporting these is reporting that the correction did not reach the
    /// screen.
    LinearFallback,
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
    pub sharp_upscale_draws: usize,
    /// Draws that fell back to hardware bilinear, in either direction.
    ///
    /// This replaces a pair that could not agree: the old upscale counter was
    /// incremented both by a real enlargement and by a degraded reduction,
    /// because the fallback reported itself as an upscale, while a separate
    /// counter tallied the fallbacks alongside it. One number now means one
    /// thing, and any value above zero says the reconstruction shader is not
    /// running.
    pub linear_fallback_draws: usize,
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
            sharp_upscale_draws: self
                .sharp_upscale_draws
                .saturating_add(other.sharp_upscale_draws),
            linear_fallback_draws: self
                .linear_fallback_draws
                .saturating_add(other.linear_fallback_draws),
        }
    }
}

impl NativeCompositionSampling {
    pub const fn reduced_name(self) -> &'static str {
        match self {
            Self::ExactNearest => "exact_nearest",
            Self::SharpDownscale => "sharp_downscale",
            Self::SharpUpscale => "sharp_upscale",
            Self::LinearFallback => "linear_fallback",
        }
    }

    /// Whether this draw is served by the reconstruction shader.
    pub const fn is_reconstructed(self) -> bool {
        matches!(self, Self::SharpDownscale | Self::SharpUpscale)
    }
}

/// Selects sampling without consulting renderer state.
///
/// A reduction on either axis needs reconstruction. Mirror projection is
/// uniform today, but treating a future mixed-axis transform as a downscale is
/// the conservative choice because it prevents source rows from being dropped.
/// The physical rig already produces one: a 1280x1440 raster drawn at 1920x1080
/// enlarges in x while reducing in y.
///
/// Never returns `LinearFallback`. Nothing about a rectangle asks for a degraded
/// draw -- that is a fact about whether a shader compiled, which this function
/// deliberately cannot see.
pub const fn native_composition_sampling(
    source: (u32, u32),
    target: (u32, u32),
) -> NativeCompositionSampling {
    if source.0 == target.0 && source.1 == target.1 {
        NativeCompositionSampling::ExactNearest
    } else if target.0 < source.0 || target.1 < source.1 {
        NativeCompositionSampling::SharpDownscale
    } else {
        NativeCompositionSampling::SharpUpscale
    }
}
