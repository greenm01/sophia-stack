fn public_live_proposal(
    layout: &PersistentLiveLayout,
    active_output: sophia_protocol::OutputId,
    projections: Vec<sophia_protocol::PolicyOutputProjection>,
    transaction: TransactionId,
    source: LiveWmProposalSource,
    settlement: LivePolicySettlementIdentity,
) -> Result<LiveWmProposal, Box<dyn std::error::Error>> {
    let mut layers = layout
        .layers
        .values()
        .filter(|layer| !layout.is_policy_managed(layer.surface))
        .cloned()
        .collect::<Vec<_>>();
    let mut requested_sizes = BTreeMap::new();
    let mut applied_surfaces = Vec::new();
    let focus = projections
        .iter()
        .find(|projection| projection.output == active_output)
        .and_then(|projection| projection.focus);
    for projection in projections {
        for placement in projection.placements {
            applied_surfaces.push(placement.surface);
            if placement.presentation.minimized {
                continue;
            }
            let mut layer = if let Some(layer) = layout.layers.get(&placement.surface) {
                layer.clone()
            } else {
                let facts = layout
                    .layout_facts(placement.surface)
                    .ok_or("public WM projection names a missing planning surface")?;
                LayerSnapshot {
                    surface: facts.surface,
                    authority_local_id: None,
                    namespace: None,
                    stack_rank: facts.stack_rank,
                    geometry: facts.geometry,
                    source: BufferSource::None,
                    damage: Region::empty(),
                    opacity: 1.0,
                    crop: None,
                    transform: Transform::IDENTITY,
                    generation: facts.generation,
                    resize_sync: ResizeSyncCapability::ImplicitOnly,
                }
            };
            layer.geometry = placement.geometry;
            layer.stack_rank = u32::try_from(layers.len()).unwrap_or(u32::MAX - 1);
            if let Some(size) = placement.requested_size {
                requested_sizes.insert(placement.surface, size);
            }
            layers.push(layer);
        }
    }
    let moved_surfaces = layers
        .iter()
        .filter(|layer| {
            layout
                .layers
                .get(&layer.surface)
                .is_some_and(|current| current.geometry != layer.geometry)
        })
        .count();
    Ok(LiveWmProposal {
        transaction,
        layers,
        requested_sizes,
        configure_deliveries: 0,
        focus,
        timeout: Duration::from_millis(SESSION_WM_TRANSPORT_RESPONSE_TIMEOUT_MSEC),
        update: WmTransactionUpdate {
            commit: TransactionCommit {
                transaction,
                outcome: TransactionOutcome::Committed,
                applied_surfaces,
            },
            ipc_error: None,
        },
        moved_surfaces,
        source: Some(source),
        effects: None,
        policy_settlement: Some(settlement),
    })
}

fn public_operation_proposal(
    layout: &PersistentLiveLayout,
    transaction: TransactionId,
    settlement: LivePolicySettlementIdentity,
) -> LiveWmProposal {
    LiveWmProposal {
        transaction,
        layers: layout.layers.values().cloned().collect(),
        requested_sizes: BTreeMap::new(),
        configure_deliveries: 0,
        focus: None,
        timeout: Duration::from_secs(1),
        update: WmTransactionUpdate {
            commit: TransactionCommit {
                transaction,
                outcome: TransactionOutcome::Committed,
                applied_surfaces: Vec::new(),
            },
            ipc_error: None,
        },
        moved_surfaces: 0,
        source: None,
        effects: None,
        policy_settlement: Some(settlement),
    }
}
