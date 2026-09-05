use super::*;

#[test]
fn policy_placement_assigns_existing_layers_and_reassigns_them_on_output_moves() {
    let first = OutputId::from_raw(1);
    let second = OutputId::from_raw(2);
    let surface = SurfaceId::new(93, 1);
    for (previous, destination) in [(None, first), (Some(first), second)] {
        let mut layout = PersistentLiveLayout::default();
        let mut layer = test_layer(
            surface,
            Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            },
        );
        // Startup has already observed a raster before the first WM placement.
        // A later move clones the same cached layer with its former output.
        layer.output = previous;
        layout.layers.insert(surface, layer);
        let projection = policy_projection(destination, surface);
        let content = BTreeMap::from([(surface, projection.placements[0].clone())]);
        let transaction = TransactionId::from_raw(930);
        let proposal = public_live_proposal(
            &layout,
            destination,
            vec![projection],
            transaction,
            LiveWmProposalSource::Manage(surface),
            LivePolicySettlementIdentity {
                connection_epoch: 1,
                request_id: 1,
                scene_generation: 1,
                transaction,
                expect_session_operation: false,
                session_operation: false,
            },
            &content,
        )
        .unwrap();
        let placed = proposal
            .layers
            .iter()
            .find(|layer| layer.surface == surface)
            .unwrap();
        assert_eq!(placed.output, Some(destination));
        assert_eq!(
            layout.layers[&surface].output, previous,
            "a proposal cannot mutate committed placement"
        );
        let owners = BTreeMap::from([(surface, placed.output.unwrap())]);
        assert_eq!(
            sophia_backend_live::live_surfaces_owned_by_output(&[surface], &owners, destination),
            vec![surface]
        );
        let other = if destination == first { second } else { first };
        assert!(
            sophia_backend_live::live_surfaces_owned_by_output(&[surface], &owners, other)
                .is_empty()
        );
    }
}
