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
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SurfaceRasterTransform {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

impl SurfaceRasterTransform {
    pub const fn swaps_axes(self) -> bool {
        matches!(
            self,
            Self::Rotate90 | Self::Rotate270 | Self::Flipped90 | Self::Flipped270
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceContentFidelity {
    /// The authority produced this raster directly for the declared class.
    AuthorityRaster,
    /// The authority retained a sampled compatibility realization. It remains
    /// usable, but Engine must not report it as native content.
    SampledFallback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceContentVariant {
    /// Stable identity within one content generation; nonzero.
    pub variant: u32,
    pub source: BufferSource,
    pub pixel_size: Size,
    /// Raster density relative to the logical extent, in thousandths.
    pub density_millis: u32,
    pub transform: SurfaceRasterTransform,
    pub fidelity: SurfaceContentFidelity,
    /// Damage in this variant's pixel coordinates. Published variants are
    /// ready; pending authority work is never placed in a committed set.
    pub damage: Region,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceContentSetError {
    EmptyVariantSet,
    InvalidLogicalExtent,
    VariantCapacityExceeded {
        count: usize,
    },
    InvalidVariantIdentity,
    DuplicateVariantIdentity {
        variant: u32,
    },
    InvalidDensity {
        variant: u32,
    },
    InvalidPixelSize {
        variant: u32,
    },
    InvalidDamage {
        variant: u32,
    },
    DuplicateRasterClass {
        density_millis: u32,
        transform: SurfaceRasterTransform,
    },
}

impl core::fmt::Display for SurfaceContentSetError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyVariantSet => write!(formatter, "surface content set has no variants"),
            Self::InvalidLogicalExtent => {
                write!(
                    formatter,
                    "surface content set has an invalid logical extent"
                )
            }
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
            Self::InvalidPixelSize { variant } => {
                write!(
                    formatter,
                    "surface content variant {variant} has an invalid pixel size"
                )
            }
            Self::InvalidDamage { variant } => {
                write!(
                    formatter,
                    "surface content variant {variant} has invalid damage"
                )
            }
            Self::DuplicateRasterClass {
                density_millis,
                transform,
            } => write!(
                formatter,
                "surface content set repeats raster class {density_millis}/{transform:?}"
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
        if logical_extent.width <= 0 || logical_extent.height <= 0 {
            return Err(SurfaceContentSetError::InvalidLogicalExtent);
        }
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
            if expected_pixel_size(logical_extent, variant.density_millis, variant.transform)
                != Some(variant.pixel_size)
            {
                return Err(SurfaceContentSetError::InvalidPixelSize {
                    variant: variant.variant,
                });
            }
            if variant.damage.rects.iter().any(|rect| {
                rect.is_empty()
                    || rect.x < 0
                    || rect.y < 0
                    || rect
                        .x
                        .checked_add(rect.width)
                        .is_none_or(|right| right > variant.pixel_size.width)
                    || rect
                        .y
                        .checked_add(rect.height)
                        .is_none_or(|bottom| bottom > variant.pixel_size.height)
            }) {
                return Err(SurfaceContentSetError::InvalidDamage {
                    variant: variant.variant,
                });
            }
            for earlier in &variants[..index] {
                if earlier.variant == variant.variant {
                    return Err(SurfaceContentSetError::DuplicateVariantIdentity {
                        variant: variant.variant,
                    });
                }
                if earlier.density_millis == variant.density_millis
                    && earlier.transform == variant.transform
                {
                    return Err(SurfaceContentSetError::DuplicateRasterClass {
                        density_millis: variant.density_millis,
                        transform: variant.transform,
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
                transform: SurfaceRasterTransform::Normal,
                fidelity: SurfaceContentFidelity::AuthorityRaster,
                damage: Region::single(Rect {
                    x: 0,
                    y: 0,
                    width: logical_extent.width,
                    height: logical_extent.height,
                }),
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
                    variant.transform != SurfaceRasterTransform::Normal,
                    variant.variant,
                )
            })
            .expect("a surface content set is never empty")
    }

    pub fn canonical_source(&self) -> BufferSource {
        self.canonical_variant().source
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SurfaceRasterClass {
    pub density_millis: u32,
    pub transform: SurfaceRasterTransform,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceRasterRequirements {
    pub surface: SurfaceId,
    pub committed_content_generation: u64,
    pub requirement_generation: u64,
    pub logical_extent: Size,
    pub classes: Vec<SurfaceRasterClass>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceRasterRequirementsError {
    InvalidIdentity,
    InvalidLogicalExtent,
    Empty,
    CapacityExceeded,
    InvalidDensity,
    DuplicateClass,
}

impl SurfaceRasterRequirements {
    pub fn validate(&self) -> Result<(), SurfaceRasterRequirementsError> {
        if !self.surface.is_valid()
            || self.committed_content_generation == 0
            || self.requirement_generation == 0
        {
            return Err(SurfaceRasterRequirementsError::InvalidIdentity);
        }
        if self.logical_extent.width <= 0 || self.logical_extent.height <= 0 {
            return Err(SurfaceRasterRequirementsError::InvalidLogicalExtent);
        }
        if self.classes.is_empty() {
            return Err(SurfaceRasterRequirementsError::Empty);
        }
        if self.classes.len() > MAX_SURFACE_CONTENT_VARIANTS {
            return Err(SurfaceRasterRequirementsError::CapacityExceeded);
        }
        let mut classes = std::collections::BTreeSet::new();
        for class in &self.classes {
            if class.density_millis == 0 {
                return Err(SurfaceRasterRequirementsError::InvalidDensity);
            }
            if !classes.insert(*class) {
                return Err(SurfaceRasterRequirementsError::DuplicateClass);
            }
        }
        Ok(())
    }
}

fn expected_pixel_size(
    logical: Size,
    density_millis: u32,
    transform: SurfaceRasterTransform,
) -> Option<Size> {
    let project = |extent: i32| {
        u64::try_from(extent)
            .ok()?
            .checked_mul(u64::from(density_millis))?
            .checked_add(999)?
            .checked_div(1_000)
            .and_then(|extent| i32::try_from(extent).ok())
    };
    let width = project(logical.width)?;
    let height = project(logical.height)?;
    Some(if transform.swaps_axes() {
        Size {
            width: height,
            height: width,
        }
    } else {
        Size { width, height }
    })
}
