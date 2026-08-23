fn reconcile_live_layout_progress(
    layout: &mut PersistentLiveLayout,
    update_slot_available: bool,
) -> LiveLayoutProgress {
    if !layout.pending_is_ready() {
        return LiveLayoutProgress::Blocked;
    }
    if !update_slot_available {
        return LiveLayoutProgress::DeferredReady;
    }
    LiveLayoutProgress::Committed(
        layout
            .resolve_pending()
            .expect("ready pending layout resolves when its output slot is available"),
    )
}

fn wm_update_coordinator_batch(
    transaction: TransactionId,
) -> XAuthorityObservedTransactionBatch {
    XAuthorityObservedTransactionBatch {
        client: None,
        admission: None,
        surface_routes: Vec::new(),
        transaction,
        transactions: Vec::new(),
        surface_presentations: Vec::new(),
        presentation_intents: Vec::new(),
        removed_surfaces: Vec::new(),
        surface_output_reservations: Vec::new(),
        cpu_buffer_updates: Vec::new(),
        raster_responses: Vec::new(),
        dma_buf_registrations: Vec::new(),
        fence_registrations: Vec::new(),
        present_submissions: Vec::new(),
        software_present_submissions: Vec::new(),
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
        protocol_errors: Vec::new(),
        expected_protocol_errors: Vec::new(),
        metadata: Vec::new(),
        selection_owner_change: false,
        selection_conversion: false,
    }
}

fn center_geometry_without_scaling(mut geometry: Rect, output: Size) -> Rect {
    geometry.x = output.width.saturating_sub(geometry.width).max(0) / 2;
    geometry.y = output.height.saturating_sub(geometry.height).max(0) / 2;
    geometry
}

fn validate_live_wm_transaction(
    transaction: &sophia_protocol::LayoutTransaction,
    layout: &PersistentLiveLayout,
    bounds: Rect,
) -> Result<(), Box<dyn std::error::Error>> {
    for placement in &transaction.render_positions {
        let known = layout.knows_surface(placement.surface);
        let policy_managed = layout.is_policy_managed(placement.surface);
        let empty = placement.geometry.is_empty();
        let within = rect_is_within(bounds, placement.geometry);
        if !known || !policy_managed || empty || !within {
            return Err(format!(
                "live WM returned invalid placement: known={known} policy_managed={policy_managed} empty={empty} within={within} geometry={:?} bounds={bounds:?}",
                placement.geometry
            )
            .into());
        }
    }
    for request in &transaction.requested_sizes {
        if !layout.knows_surface(request.surface)
            || !layout.is_policy_managed(request.surface)
            || request.size.width <= 0
            || request.size.height <= 0
            || request.size.width > i32::from(u16::MAX)
            || request.size.height > i32::from(u16::MAX)
        {
            return Err("live WM returned an invalid surface size request".into());
        }
    }
    if transaction
        .focus
        .is_some_and(|surface| {
            !layout.knows_surface(surface) || !layout.is_policy_managed(surface)
        })
    {
        return Err("live WM returned an unknown focus surface".into());
    }
    Ok(())
}

fn rect_is_within(bounds: Rect, geometry: Rect) -> bool {
    let Some(bounds_right) = bounds.x.checked_add(bounds.width) else {
        return false;
    };
    let Some(bounds_bottom) = bounds.y.checked_add(bounds.height) else {
        return false;
    };
    let Some(right) = geometry.x.checked_add(geometry.width) else {
        return false;
    };
    let Some(bottom) = geometry.y.checked_add(geometry.height) else {
        return false;
    };
    geometry.x >= bounds.x
        && geometry.y >= bounds.y
        && right <= bounds_right
        && bottom <= bounds_bottom
}

fn successful_primary_exit_ends_session(input_proof_requested: bool) -> bool {
    !input_proof_requested
}

fn global_runtime_deadline_ends_session(input_proof_requested: bool) -> bool {
    !input_proof_requested
}
