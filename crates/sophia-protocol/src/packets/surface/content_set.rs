use super::*;

/// Raster realizations one authority transaction may assert for one surface
/// generation. This bound is its own named capacity; logical-output and
/// connector-table capacities are not this bound.
pub const MAX_SURFACE_CONTENT_VARIANTS: usize = 4;

/// Density is carried in thousandths of the logical scale, so exact-match and
/// least-scale-error selection stay integer arithmetic.
pub const SURFACE_CONTENT_DENSITY_1X_MILLIS: u32 = 1_000;

/// One authority-asserted realization of a surface's content.
///
/// The authority alone asserts that variants of one set are semantically
/// equal pixels at different densities; Engine can validate identity and
/// bounds but cannot prove opaque pixels equal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceContentVariant {
    /// Stable identity within one content generation; nonzero.
    pub variant: u32,
    pub source: BufferSource,
    pub pixel_size: Size,
    /// Raster density relative to the logical extent, in thousandths.
    pub density_millis: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceContentSetError {
    EmptyVariantSet,
    VariantCapacityExceeded { count: usize },
    InvalidVariantIdentity,
    DuplicateVariantIdentity { variant: u32 },
    InvalidDensity { variant: u32 },
    DuplicateDensityClass { density_millis: u32 },
}

impl core::fmt::Display for SurfaceContentSetError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyVariantSet => write!(formatter, "surface content set has no variants"),
            Self::VariantCapacityExceeded { count } => write!(
                formatter,
                "surface content set holds {count} variants, more than {MAX_SURFACE_CONTENT_VARIANTS}"
            ),
            Self::InvalidVariantIdentity => {
                write!(formatter, "surface content variant identity is zero")
            }
            Self::DuplicateVariantIdentity { variant } => {
                write!(formatter, "surface content variant {variant} appears twice")
            }
            Self::InvalidDensity { variant } => {
                write!(
                    formatter,
                    "surface content variant {variant} has zero density"
                )
            }
            Self::DuplicateDensityClass { density_millis } => write!(
                formatter,
                "surface content set repeats density class {density_millis}"
            ),
        }
    }
}

impl std::error::Error for SurfaceContentSetError {}

/// The bounded raster content admitted for one committed surface generation.
///
/// The set is immutable after construction and its invariants are enforced
/// here rather than revalidated downstream: it is never empty, never exceeds
/// its named capacity, and never repeats a variant identity or density class.
/// A replacement or additional variant is a new authority transaction, not a
/// mutation. The set lives inside its owning transaction or committed state,
/// so a variant cannot name a different surface, transaction, or generation
/// than its envelope — that identity is structural, not validated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceContentSet {
    logical_extent: Size,
    variants: Vec<SurfaceContentVariant>,
}

impl SurfaceContentSet {
    pub fn new(
        logical_extent: Size,
        variants: Vec<SurfaceContentVariant>,
    ) -> Result<Self, SurfaceContentSetError> {
        if variants.is_empty() {
            return Err(SurfaceContentSetError::EmptyVariantSet);
        }
        if variants.len() > MAX_SURFACE_CONTENT_VARIANTS {
            return Err(SurfaceContentSetError::VariantCapacityExceeded {
                count: variants.len(),
            });
        }
        for (index, variant) in variants.iter().enumerate() {
            if variant.variant == 0 {
                return Err(SurfaceContentSetError::InvalidVariantIdentity);
            }
            if variant.density_millis == 0 {
                return Err(SurfaceContentSetError::InvalidDensity {
                    variant: variant.variant,
                });
            }
            for earlier in &variants[..index] {
                if earlier.variant == variant.variant {
                    return Err(SurfaceContentSetError::DuplicateVariantIdentity {
                        variant: variant.variant,
                    });
                }
                if earlier.density_millis == variant.density_millis {
                    return Err(SurfaceContentSetError::DuplicateDensityClass {
                        density_millis: variant.density_millis,
                    });
                }
            }
        }
        Ok(Self {
            logical_extent,
            variants,
        })
    }

    /// The one-variant normalization every current producer uses: a single
    /// raster at identity density whose pixels span the logical extent.
    pub fn singleton(source: BufferSource, logical_extent: Size) -> Self {
        Self {
            logical_extent,
            variants: vec![SurfaceContentVariant {
                variant: 1,
                source,
                pixel_size: logical_extent,
                density_millis: SURFACE_CONTENT_DENSITY_1X_MILLIS,
            }],
        }
    }

    pub const fn logical_extent(&self) -> Size {
        self.logical_extent
    }

    pub fn variants(&self) -> &[SurfaceContentVariant] {
        &self.variants
    }

    /// The identity-density variant, or the nearest to it. Until per-head
    /// selection lands this is the single content value threaded through the
    /// pipeline; ties break on stable variant identity.
    pub fn canonical_variant(&self) -> &SurfaceContentVariant {
        self.variants
            .iter()
            .min_by_key(|variant| {
                (
                    variant
                        .density_millis
                        .abs_diff(SURFACE_CONTENT_DENSITY_1X_MILLIS),
                    variant.variant,
                )
            })
            .expect("a surface content set is never empty")
    }

    pub fn canonical_source(&self) -> BufferSource {
        self.canonical_variant().source
    }
}
