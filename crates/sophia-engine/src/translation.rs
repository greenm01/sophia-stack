use sophia_protocol::{LayerSnapshot, OutputId, PolicyProjectionProposal, Rect, SurfaceId};
use std::collections::{BTreeMap, BTreeSet};

/// Translation hints never add a surface, cross an output, or alter authority.
pub fn policy_translation_groups_valid(proposal: &PolicyProjectionProposal) -> bool {
    if proposal.translation_groups.len() > sophia_protocol::POLICY_MAX_OUTPUTS {
        return false;
    }
    let mut groups = BTreeSet::new();
    let mut members = BTreeSet::new();
    for group in &proposal.translation_groups {
        if group.group == 0
            || group.members.is_empty()
            || !groups.insert((group.output, group.group))
        {
            return false;
        }
        let Some(output) = proposal.outputs.iter().find(|o| o.output == group.output) else {
            return false;
        };
        for surface in &group.members {
            let Some(placement) = output.placements.iter().find(|p| p.surface == *surface) else {
                return false;
            };
            if !members.insert(*surface)
                || members.len() > sophia_protocol::POLICY_MAX_SURFACES
                || placement.presentation.fullscreen
                || placement.presentation.minimized
            {
                return false;
            }
        }
    }
    true
}

type GroupKey = (u64, OutputId, u64);

#[derive(Clone, Copy, Debug)]
struct Spring {
    from: [f64; 2],
    target: [f64; 2],
    started: f64,
}

impl Spring {
    fn stationary(target: [f64; 2], now: f64) -> Self {
        Self {
            from: target,
            target,
            started: now,
        }
    }
    fn sample(self, now: f64) -> [f64; 2] {
        let elapsed = (now - self.started).max(0.0);
        // Niri's movement default: unit mass, critical damping, stiffness 800.
        let t = elapsed * 800.0_f64.sqrt();
        let decay = (1.0 + t) * (-t).exp();
        let value =
            std::array::from_fn(|i| self.target[i] + (self.from[i] - self.target[i]) * decay);
        if elapsed >= 2.0 || (0..2).all(|i| (value[i] - self.target[i]).abs() < 0.0001) {
            self.target
        } else {
            value
        }
    }
    fn retarget(&mut self, target: [f64; 2], now: f64) {
        if self.target != target {
            *self = Self {
                from: self.sample(now),
                target,
                started: now,
            };
        }
    }
    fn active(self, now: f64) -> bool {
        self.sample(now) != self.target
    }
}

#[derive(Clone, Debug)]
struct Member {
    group: GroupKey,
    position: Spring,
    size: (i32, i32),
}

/// Engine-owned presentation state. The caller supplies monotonic frame time;
/// no clock, callbacks, application pixels, or WM policy live in this reducer.
#[derive(Clone, Debug, Default)]
pub struct TranslationTimeline {
    groups: BTreeMap<GroupKey, Spring>,
    members: BTreeMap<SurfaceId, Member>,
    disabled: bool,
}

impl TranslationTimeline {
    pub fn set_enabled(&mut self, enabled: bool) {
        self.disabled = !enabled;
        if !enabled {
            self.settle();
        }
    }

    pub fn settle(&mut self) {
        for spring in self
            .groups
            .values_mut()
            .chain(self.members.values_mut().map(|m| &mut m.position))
        {
            spring.from = spring.target;
        }
    }

    pub fn replace_targets(&mut self, layers: &[LayerSnapshot], now: f64) -> bool {
        let mut changed = false;
        let mut keys = BTreeSet::new();
        let mut surfaces = BTreeSet::new();
        for layer in layers {
            let (Some(hint), Some(output)) = (layer.translation, layer.output) else {
                continue;
            };
            let key = (hint.connection_epoch, output, hint.group);
            if keys.insert(key) {
                let target = [f64::from(hint.x), f64::from(hint.y)];
                changed |= self.groups.get(&key).is_none_or(|s| s.target != target);
                self.groups
                    .entry(key)
                    .and_modify(|s| s.retarget(target, now))
                    .or_insert_with(|| Spring::stationary(target, now));
            }
            surfaces.insert(layer.surface);
            let target = [
                f64::from(layer.geometry.x) - f64::from(hint.x),
                f64::from(layer.geometry.y) - f64::from(hint.y),
            ];
            let size = (layer.geometry.width, layer.geometry.height);
            changed |= self
                .members
                .get(&layer.surface)
                .is_none_or(|m| m.group != key || m.size != size || m.position.target != target);
            self.members
                .entry(layer.surface)
                .and_modify(|member| {
                    if member.group == key && member.size == size {
                        member.position.retarget(target, now);
                    } else {
                        member.position = Spring::stationary(target, now);
                    }
                    member.group = key;
                    member.size = size;
                })
                .or_insert(Member {
                    group: key,
                    position: Spring::stationary(target, now),
                    size,
                });
        }
        changed |= self.groups.len() != keys.len() || self.members.len() != surfaces.len();
        self.groups.retain(|key, _| keys.contains(key));
        self.members.retain(|surface, _| surfaces.contains(surface));
        if self.disabled {
            self.settle();
        }
        changed
    }

    pub fn active(&self, now: f64) -> bool {
        self.groups.values().any(|s| s.active(now))
            || self.members.values().any(|m| m.position.active(now))
    }

    pub fn active_on(&self, output: OutputId, now: f64) -> bool {
        self.groups
            .iter()
            .any(|(key, s)| key.1 == output && s.active(now))
            || self
                .members
                .values()
                .any(|m| m.group.1 == output && m.position.active(now))
    }

    /// Produces presentation-only geometry. Committed client sizes and generations
    /// remain authoritative; chrome is translated by exactly the surface delta.
    pub fn project(
        &self,
        output: OutputId,
        committed: &[sophia_protocol::CommittedSurfaceState],
        mut display_list: crate::CompositorDisplayList,
        now: f64,
    ) -> (
        Vec<sophia_protocol::CommittedSurfaceState>,
        crate::CompositorDisplayList,
    ) {
        let mut surfaces = committed.to_vec();
        let mut deltas = BTreeMap::new();
        for state in &mut surfaces {
            let before = state.geometry;
            state.geometry = self.geometry(state.surface, output, before, now);
            deltas.insert(
                state.surface,
                (
                    i64::from(state.geometry.x) - i64::from(before.x),
                    i64::from(state.geometry.y) - i64::from(before.y),
                ),
            );
        }
        let translate = |node: crate::CompositorNodeId, rect: &mut Rect| {
            if let crate::CompositorNodeId::SurfaceChrome { surface, .. } = node
                && let Some((x, y)) = deltas.get(&surface)
            {
                rect.x =
                    (i64::from(rect.x) + x).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
                rect.y =
                    (i64::from(rect.y) + y).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
            }
        };
        for command in &mut display_list.commands {
            match command {
                crate::CompositorDisplayCommand::Border(b) => {
                    translate(b.node, &mut b.outer);
                    translate(b.node, &mut b.inner);
                }
                crate::CompositorDisplayCommand::Rect(r) => translate(r.node, &mut r.geometry),
                crate::CompositorDisplayCommand::Text(t) => translate(t.node, &mut t.geometry),
                _ => {}
            }
        }
        (surfaces, display_list)
    }

    pub fn geometry(&self, surface: SurfaceId, output: OutputId, target: Rect, now: f64) -> Rect {
        let Some(member) = self
            .members
            .get(&surface)
            .filter(|m| m.group.1 == output && m.size == (target.width, target.height))
        else {
            return target;
        };
        let Some(group) = self.groups.get(&member.group) else {
            return target;
        };
        let position = member.position.sample(now);
        let translation = group.sample(now);
        Rect {
            x: (position[0] + translation[0]).round() as i32,
            y: (position[1] + translation[1]).round() as i32,
            ..target
        }
    }
}
