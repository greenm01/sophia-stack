struct ReconciledPublicPolicyProposal {
    policy: sophia_protocol::PolicyProjectionProposal,
    content: BTreeMap<SurfaceId, sophia_protocol::PolicySurfacePlacement>,
    adjusted_surfaces: usize,
}

fn reconcile_public_policy_proposal(
    layout: &PersistentLiveLayout,
    proposal: &sophia_protocol::PolicyProjectionProposal,
    work_areas: &BTreeMap<sophia_protocol::OutputId, Rect>,
    chrome: sophia_engine::SurfaceChromeStyle,
) -> Result<ReconciledPublicPolicyProposal, Box<dyn std::error::Error>> {
    let mut reconciled = proposal.clone();
    let mut content = BTreeMap::new();
    let mut adjusted_surfaces = BTreeSet::new();
    for output in &mut reconciled.outputs {
        let bounds = work_areas
            .get(&output.output)
            .copied()
            .ok_or("public WM projection has no Engine work area")?;
        let transaction = sophia_protocol::LayoutTransaction {
            transaction: proposal.transaction,
            requested_sizes: output
                .placements
                .iter()
                .filter_map(|placement| {
                    placement
                        .requested_size
                        .map(|size| sophia_protocol::SurfaceSizeRequest {
                            surface: placement.surface,
                            size,
                        })
                })
                .collect(),
            focus: output.focus,
            render_positions: output
                .placements
                .iter()
                .enumerate()
                .map(|(index, placement)| {
                    Ok(sophia_protocol::SurfacePlacement {
                        surface: placement.surface,
                        geometry: placement.geometry,
                        z_index: i32::try_from(index)
                            .map_err(|_| "public WM projection stack is excessive")?,
                        crop: placement.crop,
                        transform: match placement.transform {
                            sophia_protocol::PolicyTransform::Identity => Transform::IDENTITY,
                        },
                    })
                })
                .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?,
            timeout_msec: u32::try_from(SESSION_WM_TRANSPORT_RESPONSE_TIMEOUT_MSEC)
                .unwrap_or(u32::MAX),
        };
        let transaction = sophia_engine::apply_surface_chrome_clearance(&transaction, chrome)?;
        let reconciliation = layout
            .layout_epochs
            .reconcile_transaction(&transaction, bounds)?;
        let geometries = reconciliation
            .transaction
            .render_positions
            .into_iter()
            .map(|placement| (placement.surface, placement.geometry))
            .collect::<BTreeMap<_, _>>();
        let requested_sizes = reconciliation
            .transaction
            .requested_sizes
            .into_iter()
            .map(|request| (request.surface, request.size))
            .collect::<BTreeMap<_, _>>();
        for placement in &mut output.placements {
            let content_geometry = geometries[&placement.surface];
            let content_requested_size = requested_sizes.get(&placement.surface).copied();
            let outer_geometry = sophia_engine::outer_surface_geometry(content_geometry, chrome)?;
            let outer_requested_size = placement.requested_size.map(|_| sophia_protocol::Size {
                width: outer_geometry.width,
                height: outer_geometry.height,
            });
            if placement.geometry != outer_geometry
                || placement.requested_size != outer_requested_size
            {
                adjusted_surfaces.insert(placement.surface);
            }
            let mut content_placement = placement.clone();
            content_placement.geometry = content_geometry;
            content_placement.requested_size = content_requested_size;
            content.insert(placement.surface, content_placement);
            // The reducer remains an outer-allocation model. Only the proposal
            // materialized into Engine layers and configure requests crosses
            // the chrome boundary into content coordinates.
            placement.geometry = outer_geometry;
            placement.requested_size = outer_requested_size;
            println!(
                "sophia_live_wm_chrome schema=2 status=reconciled transaction={} output={} surface={} clearance={} outer={}x{}_{}_{} content={}x{}_{}_{} request={} adjusted={}",
                proposal.transaction.raw(),
                output.output.raw(),
                placement.surface.index(),
                chrome.clearance(),
                placement.geometry.width,
                placement.geometry.height,
                placement.geometry.x,
                placement.geometry.y,
                content_geometry.width,
                content_geometry.height,
                content_geometry.x,
                content_geometry.y,
                content_requested_size
                    .map(|size| format!("{}x{}", size.width, size.height))
                    .unwrap_or_else(|| "none".to_owned()),
                adjusted_surfaces.contains(&placement.surface),
            );
        }
    }
    Ok(ReconciledPublicPolicyProposal {
        policy: reconciled,
        content,
        adjusted_surfaces: adjusted_surfaces.len(),
    })
}

fn public_live_proposal(
    layout: &PersistentLiveLayout,
    active_output: sophia_protocol::OutputId,
    projections: Vec<sophia_protocol::PolicyOutputProjection>,
    transaction: TransactionId,
    source: LiveWmProposalSource,
    settlement: LivePolicySettlementIdentity,
    content: &BTreeMap<SurfaceId, sophia_protocol::PolicySurfacePlacement>,
) -> Result<LiveWmProposal, Box<dyn std::error::Error>> {
    let mut layers = layout
        .layers
        .values()
        .filter(|layer| !layout.is_policy_managed(layer.surface))
        .cloned()
        .collect::<Vec<_>>();
    let mut requested_sizes = BTreeMap::new();
    let mut presentation_states = BTreeMap::new();
    let mut applied_surfaces = Vec::new();
    let focus = projections
        .iter()
        .find(|projection| projection.output == active_output)
        .and_then(|projection| projection.focus);
    for projection in projections {
        for placement in projection.placements {
            let materialized = content
                .get(&placement.surface)
                .ok_or("public WM projection has no reconciled content placement")?;
            applied_surfaces.push(placement.surface);
            presentation_states.insert(placement.surface, placement.presentation);
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
                    // A planning surface names no raster yet, so this is a placeholder.
                    source_size: sophia_protocol::Size {
                        width: facts.geometry.width,
                        height: facts.geometry.height,
                    },
                    damage: Region::empty(),
                    opacity: 1.0,
                    crop: None,
                    transform: Transform::IDENTITY,
                    generation: facts.generation,
                    resize_sync: ResizeSyncCapability::ImplicitOnly,
                }
            };
            layer.geometry = materialized.geometry;
            layer.stack_rank = u32::try_from(layers.len()).unwrap_or(u32::MAX - 1);
            if let Some(size) = materialized.requested_size {
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
        presentation_states,
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
        presentation_states: BTreeMap::new(),
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
