use super::super::{LiveOutputTopologyOwner, LiveOutputTopologyPhase, LiveOutputTopologyRebuild};
use sophia_protocol::{OutputId, Size};

/// Rebuild with one head per output: the ordinary unmirrored desktop, and every
/// case here except the group that loses a connector.
fn observe_unmirrored(
    owner: &mut LiveOutputTopologyOwner,
    outputs: Vec<sophia_engine::HeadlessOutput>,
) -> Result<LiveOutputTopologyRebuild, &'static str> {
    let heads = outputs.iter().map(|output| (output.id, 1)).collect();
    owner.observe_rebuild(outputs, heads)
}

fn output(raw: u64, width: i32) -> sophia_engine::HeadlessOutput {
    sophia_engine::HeadlessOutput {
        id: OutputId::from_raw(raw),
        size: Size { width, height: 720 },
        scale: 1,
    }
}

fn owner() -> LiveOutputTopologyOwner {
    LiveOutputTopologyOwner::new_at_generation(
        vec![output(1, 1280)],
        vec![(OutputId::from_raw(1), 1)],
        1,
    )
    .unwrap()
}

#[test]
fn changed_topology_advances_public_identity_once() {
    let mut owner = owner();
    assert_eq!(owner.begin_rescan(1), Ok(true));
    assert_eq!(
        observe_unmirrored(&mut owner, vec![output(1, 1920), output(2, 1280)]),
        Ok(LiveOutputTopologyRebuild::TopologyChanged)
    );
    assert_eq!(owner.topology_epoch, 2);
    assert_eq!(owner.publication_generation, 2);
    owner.mark_published(7, true).unwrap();
    assert!(!owner.observe_presentation(8));
    owner.mark_policy_committed(8).unwrap();
    assert!(!owner.observe_presentation(7));
    assert!(!owner.observe_presentation(8));
    assert!(owner.observe_presentation(9));
    assert!(!owner.input_quarantined());
}

#[test]
fn losing_one_head_of_a_mirror_group_is_a_new_candidate() {
    // A group that loses a connector keeps its logical output, so the output list
    // is byte-for-byte what it was. Comparing that alone would republish nothing
    // and leave consumers holding an epoch computed for the wider group -- the
    // case `PublishedHeadsAreCurrent` exists to forbid.
    let outputs = vec![output(1, 1280)];
    let mut owner = LiveOutputTopologyOwner::new_at_generation(
        outputs.clone(),
        vec![(OutputId::from_raw(1), 2)],
        1,
    )
    .unwrap();

    assert!(owner.begin_rescan(1).unwrap());
    assert_eq!(
        owner.observe_rebuild(outputs.clone(), vec![(OutputId::from_raw(1), 1)]),
        Ok(LiveOutputTopologyRebuild::TopologyChanged)
    );
    assert_eq!(owner.topology_epoch, 2);
    assert_eq!(owner.publication_generation, 2);

    // And an unchanged group is still no change, so the head count cannot make
    // every rescan look like a new topology.
    owner.mark_published(1, false).unwrap();
    assert!(owner.observe_presentation(2));
    assert!(owner.begin_rescan(2).unwrap());
    assert_eq!(
        owner.observe_rebuild(outputs, vec![(OutputId::from_raw(1), 1)]),
        Ok(LiveOutputTopologyRebuild::TransportReplaced)
    );
    assert_eq!(owner.topology_epoch, 2);
}

#[test]
fn configured_initial_publication_generation_advances_from_its_baseline() {
    let mut owner = LiveOutputTopologyOwner::new_at_generation(
        vec![output(1, 1280)],
        vec![(OutputId::from_raw(1), 1)],
        2,
    )
    .unwrap();
    owner.begin_rescan(1).unwrap();
    observe_unmirrored(&mut owner, vec![output(1, 1920)]).unwrap();
    assert_eq!(owner.publication_generation, 3);
}

#[test]
fn retry_does_not_consume_another_security_or_public_epoch() {
    let mut owner = owner();
    assert_eq!(owner.begin_rescan(4), Ok(true));
    assert_eq!(
        observe_unmirrored(&mut owner, Vec::new()),
        Ok(LiveOutputTopologyRebuild::Unavailable)
    );
    assert_eq!(owner.begin_rescan(5), Ok(false));
    assert_eq!(owner.topology_epoch, 1);
    assert_eq!(owner.publication_generation, 1);
    assert_eq!(
        observe_unmirrored(&mut owner, vec![output(1, 1280)]),
        Ok(LiveOutputTopologyRebuild::TransportReplaced)
    );
}

#[test]
fn transport_replacement_still_waits_for_new_presentation() {
    let mut owner = owner();
    owner.begin_rescan(1).unwrap();
    observe_unmirrored(&mut owner, vec![output(1, 1280)]).unwrap();
    owner.mark_published(11, false).unwrap();
    assert!(!owner.observe_presentation(11));
    assert!(owner.observe_presentation(12));
}

#[test]
fn newer_notice_restarts_publication_without_reconsuming_security_epoch() {
    let mut owner = owner();
    assert!(owner.begin_rescan(1).unwrap());
    observe_unmirrored(&mut owner, vec![output(1, 1920)]).unwrap();
    owner.mark_published(3, true).unwrap();
    assert!(!owner.begin_rescan(2).unwrap());
    assert_eq!(owner.phase, LiveOutputTopologyPhase::Quarantined);
    assert_eq!(owner.transition, 2);
}

#[test]
fn redundant_notice_cannot_bypass_pending_policy_settlement() {
    let mut owner = owner();
    owner.begin_rescan(1).unwrap();
    observe_unmirrored(&mut owner, vec![output(1, 1920)]).unwrap();
    owner.mark_published(3, true).unwrap();
    owner.begin_rescan(2).unwrap();
    observe_unmirrored(&mut owner, vec![output(1, 1920)]).unwrap();
    owner.mark_published(4, false).unwrap();
    assert_eq!(owner.phase, LiveOutputTopologyPhase::Published);
    assert!(owner.policy_settlement_pending);
}

#[test]
fn duplicate_coalescer_token_does_not_restart_a_transition() {
    let mut owner = owner();
    assert!(owner.begin_rescan(3).unwrap());
    assert!(!owner.begin_rescan(3).unwrap());
    assert_eq!(owner.transition, 1);
    assert_eq!(owner.phase, LiveOutputTopologyPhase::Quarantined);
}
