use sophia_protocol::{
    OutputId, POLICY_MAX_TAB_GROUPS, POLICY_MAX_TAB_MEMBERS, PolicyOutputProjection,
    PolicyProjectionProposal, PolicySceneSnapshot, PolicyTabGroup,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabGroupError {
    Capacity,
    Identity,
    Geometry,
    Member,
    Selection,
}

/// Replaces groups on exactly the proposal's affected outputs. Hidden members
/// are permitted only when the complete authoritative scene still owns them.
pub fn validate_policy_tab_groups(
    scene: &PolicySceneSnapshot,
    projections: &BTreeMap<OutputId, PolicyOutputProjection>,
    previous: &[PolicyTabGroup],
    proposal: &PolicyProjectionProposal,
) -> Result<Vec<PolicyTabGroup>, TabGroupError> {
    let affected: BTreeSet<_> = proposal.outputs.iter().map(|o| o.output).collect();
    let mut result: Vec<_> = previous
        .iter()
        .filter(|g| {
            !affected.contains(&g.output) && scene.outputs.iter().any(|o| o.output == g.output)
        })
        .cloned()
        .collect();
    result.extend(proposal.tab_groups.iter().cloned());
    if result.len() > POLICY_MAX_TAB_GROUPS
        || result.iter().map(|g| g.members.len()).sum::<usize>() > POLICY_MAX_TAB_MEMBERS
    {
        return Err(TabGroupError::Capacity);
    }
    let mut ids = BTreeSet::new();
    for g in &result {
        if g.group == 0 || !ids.insert((g.output, g.group)) {
            return Err(TabGroupError::Identity);
        }
        let output = scene
            .outputs
            .iter()
            .find(|o| o.output == g.output)
            .ok_or(TabGroupError::Identity)?;
        let contains = |outer: sophia_protocol::Rect, inner: sophia_protocol::Rect| {
            inner.width > 0
                && inner.height > 0
                && i64::from(inner.x) >= i64::from(outer.x)
                && i64::from(inner.y) >= i64::from(outer.y)
                && i64::from(inner.x) + i64::from(inner.width)
                    <= i64::from(outer.x) + i64::from(outer.width)
                && i64::from(inner.y) + i64::from(inner.height)
                    <= i64::from(outer.y) + i64::from(outer.height)
        };
        if !contains(output.work_area, g.geometry) {
            return Err(TabGroupError::Geometry);
        }
        let projection = projections.get(&g.output).ok_or(TabGroupError::Identity)?;
        if projection
            .placements
            .iter()
            .any(|p| p.presentation.fullscreen)
        {
            return Err(TabGroupError::Geometry);
        }
        if result.iter().any(|other| {
            other.output == g.output
                && other.group != g.group
                && crate::tab_rects_overlap(g.geometry, other.geometry)
        }) {
            return Err(TabGroupError::Geometry);
        }
        let mut members = BTreeSet::new();
        for member in &g.members {
            if !members.insert(*member)
                || !scene.surfaces.iter().any(|s| {
                    s.surface == *member
                        && s.capabilities.focusable
                        && (!s.current_state.minimized
                            || projection
                                .placements
                                .iter()
                                .any(|p| p.surface == *member && !p.presentation.minimized))
                })
            {
                return Err(TabGroupError::Member);
            }
            if projections
                .iter()
                .any(|(o, p)| *o != g.output && p.placements.iter().any(|p| p.surface == *member))
            {
                return Err(TabGroupError::Member);
            }
        }
        match g.selected {
            Some(s)
                if members.contains(&s)
                    && projection
                        .placements
                        .iter()
                        .any(|p| p.surface == s && !p.presentation.minimized) => {}
            None if members.is_empty() => {}
            _ => return Err(TabGroupError::Selection),
        }
    }
    if proposal
        .tab_groups
        .iter()
        .any(|g| !affected.contains(&g.output))
    {
        return Err(TabGroupError::Identity);
    }
    Ok(result)
}
