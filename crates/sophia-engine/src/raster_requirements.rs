use crate::prelude::*;
use crate::{HeadRenderTarget, OutputSceneSnapshot};
use sophia_protocol::{
    MAX_SURFACE_CONTENT_VARIANTS, SurfaceContentFidelity, SurfaceRasterClass,
    SurfaceRasterRequirements, SurfaceRasterResponseIdentity, SurfaceRasterTransform,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrackedRasterRequirement {
    content_generation: u64,
    logical_extent: Size,
    classes: Vec<SurfaceRasterClass>,
    requirement_generation: u64,
}

/// Engine-owned reducer for protocol-neutral raster demand.
///
/// Demand is derived from physical head targets but leaves Engine keyed only
/// by `SurfaceId` and density class. Connector and protocol object identities
/// never cross this boundary. Reconciliation is edge-triggered: unchanged or
/// already-exact demand emits no work.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SurfaceRasterRequirementTracker {
    next_generation: u64,
    tracked: BTreeMap<SurfaceId, TrackedRasterRequirement>,
}

#[derive(Clone, Debug)]
struct Demand {
    content_generation: u64,
    logical_extent: Size,
    counts: BTreeMap<SurfaceRasterClass, usize>,
    authority_exact: BTreeSet<SurfaceRasterClass>,
    canonical: SurfaceRasterClass,
}

impl SurfaceRasterRequirementTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reconciles all visible output snapshots in one pass so a surface shared
    /// by extended outputs receives one unioned, bounded requirement.
    pub fn reconcile(
        &mut self,
        snapshots: &[OutputSceneSnapshot],
        targets: &[HeadRenderTarget],
    ) -> Result<Vec<SurfaceRasterRequirements>, &'static str> {
        let mut demand = BTreeMap::<SurfaceId, Demand>::new();
        for snapshot in snapshots {
            let output_targets = targets
                .iter()
                .filter(|target| target.output == snapshot.output);
            for target in output_targets {
                let density_millis = target_density_millis(snapshot, *target)?;
                let class = SurfaceRasterClass {
                    density_millis,
                    transform: SurfaceRasterTransform::Normal,
                };
                for surface in &snapshot.surfaces {
                    let entry = demand.entry(surface.surface).or_insert_with(|| Demand {
                        content_generation: surface.committed_generation,
                        logical_extent: surface.content.logical_extent(),
                        counts: BTreeMap::new(),
                        authority_exact: surface
                            .content
                            .variants()
                            .iter()
                            .filter(|variant| {
                                variant.fidelity == SurfaceContentFidelity::AuthorityRaster
                            })
                            .map(|variant| SurfaceRasterClass {
                                density_millis: variant.density_millis,
                                transform: variant.transform,
                            })
                            .collect(),
                        canonical: SurfaceRasterClass {
                            density_millis: surface.content.canonical_variant().density_millis,
                            transform: surface.content.canonical_variant().transform,
                        },
                    });
                    if entry.content_generation != surface.committed_generation
                        || entry.logical_extent != surface.content.logical_extent()
                    {
                        return Err("surface raster demand saw inconsistent committed content");
                    }
                    *entry.counts.entry(class).or_default() += 1;
                }
            }
        }

        self.tracked
            .retain(|surface, _| demand.contains_key(surface));
        let mut requirements = Vec::new();
        for (surface, demand) in demand {
            let previous = self.tracked.get(&surface);
            let canonical_demanded = demand.counts.contains_key(&demand.canonical);
            let mut ranked = demand.counts.into_iter().collect::<Vec<_>>();
            ranked.sort_by_key(|(class, count)| {
                (
                    !demand.authority_exact.contains(class),
                    core::cmp::Reverse(*count),
                    !previous.is_some_and(|tracked| tracked.classes.contains(class)),
                    *class,
                )
            });
            // Every replacement set retains one canonical compatibility
            // variant. When no head demands its class, reserve that slot so
            // the advisory request remains satisfiable instead of asking an
            // authority for MAX additional variants.
            let requested_capacity = if canonical_demanded {
                MAX_SURFACE_CONTENT_VARIANTS
            } else {
                MAX_SURFACE_CONTENT_VARIANTS.saturating_sub(1)
            };
            let mut classes = ranked
                .into_iter()
                .take(requested_capacity)
                .map(|(class, _)| class)
                .collect::<Vec<_>>();
            classes.sort();
            let satisfied = classes
                .iter()
                .all(|class| demand.authority_exact.contains(class));
            if satisfied {
                self.tracked.remove(&surface);
                continue;
            }
            if previous.is_some_and(|tracked| {
                tracked.content_generation == demand.content_generation
                    && tracked.logical_extent == demand.logical_extent
                    && tracked.classes == classes
            }) {
                continue;
            }
            self.next_generation = self.next_generation.saturating_add(1).max(1);
            let tracked = TrackedRasterRequirement {
                content_generation: demand.content_generation,
                logical_extent: demand.logical_extent,
                classes: classes.clone(),
                requirement_generation: self.next_generation,
            };
            requirements.push(SurfaceRasterRequirements {
                surface,
                committed_content_generation: tracked.content_generation,
                requirement_generation: tracked.requirement_generation,
                logical_extent: tracked.logical_extent,
                classes,
            });
            self.tracked.insert(surface, tracked);
        }
        Ok(requirements)
    }

    /// Rejects a late or cross-surface authority response before transaction
    /// admission. A valid response consumes only its exact outstanding edge.
    pub fn accept_response(&mut self, response: SurfaceRasterResponseIdentity) -> bool {
        if !response.is_valid() {
            return false;
        }
        let Some(tracked) = self.tracked.get(&response.surface) else {
            return false;
        };
        if tracked.content_generation != response.source_content_generation
            || tracked.requirement_generation != response.requirement_generation
        {
            return false;
        }
        self.tracked.remove(&response.surface);
        true
    }
}

fn target_density_millis(
    snapshot: &OutputSceneSnapshot,
    target: HeadRenderTarget,
) -> Result<u32, &'static str> {
    if target.output != snapshot.output
        || target.native_size.width <= 0
        || target.native_size.height <= 0
        || snapshot.logical_viewport.width <= 0
        || snapshot.logical_viewport.height <= 0
    {
        return Err("invalid raster requirement target");
    }
    let source_width = i64::from(snapshot.logical_viewport.width);
    let source_height = i64::from(snapshot.logical_viewport.height);
    let destination_width = i64::from(target.native_size.width);
    let destination_height = i64::from(target.native_size.height);
    let (projected_width, projected_height) = match target.mapping {
        sophia_protocol::OutputHeadMapping::Exact => (source_width, source_height),
        sophia_protocol::OutputHeadMapping::Fit | sophia_protocol::OutputHeadMapping::Cover => {
            let use_width = if target.mapping == sophia_protocol::OutputHeadMapping::Fit {
                destination_width.saturating_mul(source_height)
                    <= destination_height.saturating_mul(source_width)
            } else {
                destination_width.saturating_mul(source_height)
                    >= destination_height.saturating_mul(source_width)
            };
            if use_width {
                (
                    destination_width,
                    destination_width.saturating_mul(source_height) / source_width,
                )
            } else {
                (
                    destination_height.saturating_mul(source_width) / source_height,
                    destination_height,
                )
            }
        }
    };
    let x = projected_width.saturating_mul(1_000) / source_width;
    let y = projected_height.saturating_mul(1_000) / source_height;
    Ok(u32::try_from(x.min(y).max(1)).unwrap_or(u32::MAX))
}
