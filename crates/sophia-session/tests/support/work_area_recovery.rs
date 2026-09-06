use super::*;

#[test]
fn work_area_target_supersedes_aborted_launch_size_before_and_after_retirement() {
    let surface = SurfaceId::new(94, 1);
    let old = Size {
        width: 1258,
        height: 1408,
    };
    let target = Size {
        width: 1258,
        height: 1390,
    };
    for recovery in [None, Some(old)] {
        let mut layout = PersistentLiveLayout::default();
        register_test_routes(&mut layout, &[surface]);
        layout.layout_epochs.record_committed(surface, old);
        if let Some(recovery) = recovery {
            layout.layout_epochs.set_recovery_extent(surface, recovery);
        }
        // A timeout can leave an obligation while its first frame is in flight.
        layout.layout_epochs.record_committed(
            surface,
            Size {
                width: 800,
                height: 600,
            },
        );
        layout.layout_epochs.set_pending_target(surface, old);
        let geometry = Rect {
            x: 651,
            y: 41,
            width: target.width,
            height: target.height,
        };
        layout.layers.insert(
            surface,
            test_layer(
                surface,
                Rect {
                    height: old.height,
                    ..geometry
                },
            ),
        );
        let transaction = TransactionId::from_raw(940);
        let proposal = LiveWmProposal {
            transaction,
            layers: vec![test_layer(surface, geometry)],
            requested_sizes: BTreeMap::from([(surface, target)]),
            presentation_states: BTreeMap::new(),
            configure_deliveries: 0,
            focus: Some(surface),
            timeout: Duration::from_secs(1),
            update: sophia_engine::WmTransactionUpdate {
                commit: TransactionCommit {
                    transaction,
                    outcome: TransactionOutcome::Committed,
                    applied_surfaces: vec![surface],
                },
            },
            moved_surfaces: 0,
            source: Some(LiveWmProposalSource::Relayout),
            policy_settlement: None,
        };
        let mut controls = crate::session_control::SessionControlQueue::default();
        assert!(layout.stage(proposal, &mut controls).unwrap().is_none());
        assert_eq!(layout.layout_epochs.pending_target(surface), Some(target));
        let pending = layout.pending.as_ref().unwrap();
        assert_eq!(pending.layers[0].geometry, geometry);
        assert_eq!(pending.requested_sizes[&surface], target);
        layout.layout_epochs.record_committed(surface, target);
        layout.release_recovery_extent(surface, "test_work_area_frame_retired");
        assert_eq!(layout.layout_epochs.pending_target(surface), None);
    }
}
