use crate::HeadlessOutput;
use crate::prelude::*;

/// Physical heads admitted behind one logical output. This bound is its own
/// named capacity: the logical-output table and native connector tables have
/// their own limits, and none of the three may borrow another's constant.
pub const MAX_HEADS_PER_OUTPUT: usize = 4;

/// Session-scoped opaque identity for one physical presentation target.
///
/// The live backend mints these and privately retains the card, connector,
/// CRTC, and plane mapping. A `RenderHeadId` may cross the Engine/backend
/// boundary for planning and observations; it must never cross the WM,
/// portal, metadata, shell-policy, or application-protocol boundary, so it
/// lives in `sophia-engine` rather than `sophia-protocol`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RenderHeadId(u64);

impl RenderHeadId {
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

/// Mints session-unique head identities. Engine defines the allocator so the
/// ID type stays private to this crate's boundary, but only the live backend
/// (or a headless backend stand-in) may hold an instance: Engine planning
/// code cannot invent heads it was never told about.
#[derive(Debug)]
pub struct RenderHeadAllocator {
    next: u64,
}

impl RenderHeadAllocator {
    pub const fn new() -> Self {
        Self { next: 1 }
    }

    pub fn mint(&mut self) -> RenderHeadId {
        let id = self.next;
        self.next = self
            .next
            .checked_add(1)
            .expect("Sophia render-head counter overflow");
        RenderHeadId::from_raw(id)
    }
}

impl Default for RenderHeadAllocator {
    fn default() -> Self {
        Self::new()
    }
}

/// Reduced description of one current native presentation target.
///
/// A mode, scale, or refresh change is a new target generation; work prepared
/// against an older generation is stale and cannot be relabelled. The record
/// deliberately carries no connector, CRTC, card, framebuffer, or renderer
/// identity — those stay behind the backend boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadRenderTarget {
    pub head: RenderHeadId,
    pub output: OutputId,
    pub target_generation: u64,
    pub native_size: Size,
    pub scale: u32,
    pub refresh_millihz: u32,
    pub transform: OutputTransform,
    pub mapping: OutputHeadMapping,
}

impl HeadRenderTarget {
    /// Shape equality that ignores the generation stamp: two targets with the
    /// same shape may differ only in generation (device or capability churn).
    fn same_shape(&self, other: &Self) -> bool {
        self.same_logical_shape(other)
            && self.refresh_millihz == other.refresh_millihz
            && self.transform == other.transform
            && self.mapping == other.mapping
    }

    /// The part of the shape a logical view depends on. Refresh is excluded:
    /// a mirror group's heads legitimately run near-but-not-equal rates, and
    /// pacing is reduced separately at the slowest head.
    const fn same_logical_shape(&self, other: &Self) -> bool {
        self.native_size.width == other.native_size.width
            && self.native_size.height == other.native_size.height
            && self.scale == other.scale
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineHeadRegistryUpdate {
    Inserted,
    Replaced,
    InvalidHead,
    InvalidOutput,
    HeadOwnedByOtherOutput,
    StaleTargetGeneration,
    HeadCapacityExceeded,
    OutputCapacityExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineLogicalOutputUpdate {
    Updated,
    UnknownOutput,
    InvalidOutput,
}

impl EngineHeadRegistryUpdate {
    pub const fn is_admitted(self) -> bool {
        matches!(self, Self::Inserted | Self::Replaced)
    }
}

/// Engine's view of the admitted physical heads, grouped by logical output.
///
/// The registry owns no native resources; it is the passive fact table the
/// frame clock, presentation registry, and topology derive per-output logical
/// facts from. Mirroring is several heads behind one output.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EngineHeadRegistry {
    heads: BTreeMap<OutputId, Vec<HeadRenderTarget>>,
    logical_outputs: BTreeMap<OutputId, HeadlessOutput>,
}

impl EngineHeadRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Admits or replaces one head target, fail closed.
    ///
    /// A head readmitted with a different shape must advance its target
    /// generation: silently mutating a target under an unchanged generation
    /// would let stale prepared work be relabelled as current.
    pub fn admit(&mut self, target: HeadRenderTarget) -> EngineHeadRegistryUpdate {
        if !target.head.is_valid() {
            return EngineHeadRegistryUpdate::InvalidHead;
        }
        if !target.output.is_valid() {
            return EngineHeadRegistryUpdate::InvalidOutput;
        }
        if self.heads.iter().any(|(output, heads)| {
            *output != target.output && heads.iter().any(|head| head.head == target.head)
        }) {
            return EngineHeadRegistryUpdate::HeadOwnedByOtherOutput;
        }

        let is_new_output = !self.heads.contains_key(&target.output);
        if is_new_output && self.heads.len() >= crate::MAX_DRM_KMS_OUTPUTS {
            return EngineHeadRegistryUpdate::OutputCapacityExceeded;
        }

        let heads = self.heads.entry(target.output).or_default();
        if let Some(existing) = heads.iter_mut().find(|head| head.head == target.head) {
            let update = if existing.same_shape(&target) {
                if target.target_generation < existing.target_generation {
                    return EngineHeadRegistryUpdate::StaleTargetGeneration;
                }
                EngineHeadRegistryUpdate::Replaced
            } else if target.target_generation > existing.target_generation {
                EngineHeadRegistryUpdate::Replaced
            } else {
                return EngineHeadRegistryUpdate::StaleTargetGeneration;
            };
            *existing = target;
            return update;
        }

        if heads.len() >= MAX_HEADS_PER_OUTPUT {
            if is_new_output {
                self.heads.remove(&target.output);
            }
            return EngineHeadRegistryUpdate::HeadCapacityExceeded;
        }
        heads.push(target);
        self.logical_outputs
            .entry(target.output)
            .or_insert(HeadlessOutput {
                id: target.output,
                size: target.native_size,
                scale: target.scale,
            });
        EngineHeadRegistryUpdate::Inserted
    }

    pub fn remove_head(&mut self, head: RenderHeadId) -> Option<HeadRenderTarget> {
        let mut removed = None;
        self.heads.retain(|_, heads| {
            if let Some(index) = heads.iter().position(|target| target.head == head) {
                removed = Some(heads.remove(index));
            }
            !heads.is_empty()
        });
        if let Some(target) = removed
            && !self.heads.contains_key(&target.output)
        {
            self.logical_outputs.remove(&target.output);
        }
        removed
    }

    pub fn remove_output(&mut self, output: OutputId) -> usize {
        self.logical_outputs.remove(&output);
        self.heads
            .remove(&output)
            .map(|heads| heads.len())
            .unwrap_or(0)
    }

    pub fn head(&self, head: RenderHeadId) -> Option<&HeadRenderTarget> {
        self.heads
            .values()
            .flatten()
            .find(|target| target.head == head)
    }

    pub fn output_heads(&self, output: OutputId) -> &[HeadRenderTarget] {
        self.heads
            .get(&output)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn outputs(&self) -> impl Iterator<Item = OutputId> + '_ {
        self.heads.keys().copied()
    }

    pub fn heads(&self) -> impl Iterator<Item = &HeadRenderTarget> {
        self.heads.values().flatten()
    }

    pub fn output_count(&self) -> usize {
        self.heads.len()
    }

    pub fn head_count(&self) -> usize {
        self.heads.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.heads.is_empty()
    }

    /// Replaces the Engine-owned logical viewport without changing a physical
    /// target. Mode and scale changes on individual heads are deliberately
    /// independent of this fact.
    pub fn set_logical_output(&mut self, output: HeadlessOutput) -> EngineLogicalOutputUpdate {
        if !output.id.is_valid()
            || output.size.width <= 0
            || output.size.height <= 0
            || output.scale == 0
        {
            return EngineLogicalOutputUpdate::InvalidOutput;
        }
        if !self.heads.contains_key(&output.id) {
            return EngineLogicalOutputUpdate::UnknownOutput;
        }
        self.logical_outputs.insert(output.id, output);
        EngineLogicalOutputUpdate::Updated
    }

    /// Logical policy/scene view. It is stored independently from every native
    /// head shape, so a 2560x1440 and 1920x1080 mirror remains one output.
    pub fn logical_output(&self, output: OutputId) -> Option<HeadlessOutput> {
        self.logical_outputs.get(&output).copied()
    }

    /// Refresh pacing for one logical output: the slowest head's rate. A
    /// mirror group advances at its slowest required head, so a faster
    /// sibling must not shorten the logical frame interval. Heads reporting
    /// an unknown (zero) rate are ignored; zero means no head knows.
    pub fn logical_refresh_millihz(&self, output: OutputId) -> u32 {
        self.output_heads(output)
            .iter()
            .map(|head| head.refresh_millihz)
            .filter(|refresh| *refresh != 0)
            .min()
            .unwrap_or(0)
    }

    pub fn logical_outputs(&self) -> impl Iterator<Item = HeadlessOutput> + '_ {
        self.logical_outputs.values().copied()
    }

    /// Transitional single-output selection: the lowest logical output. The
    /// distinguished primary disappears with per-head composition planning.
    pub fn primary_engine_output(&self) -> Option<HeadlessOutput> {
        self.logical_outputs.values().next().copied()
    }
}
