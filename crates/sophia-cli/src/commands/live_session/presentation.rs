struct XPresentSessionObserver {
    router: XServerFrontendProtocolRouter,
    complete_flip: usize,
    complete_skip: usize,
    idle: usize,
    complete_routed: usize,
    idle_routed: usize,
    route_failures: usize,
    idle_fence_triggers: usize,
    disconnect_sources: usize,
    disconnect_fences: usize,
    disconnect_failures: usize,
}

impl XPresentSessionObserver {
    fn new(router: XServerFrontendProtocolRouter) -> Self {
        Self {
            router,
            complete_flip: 0,
            complete_skip: 0,
            idle: 0,
            complete_routed: 0,
            idle_routed: 0,
            route_failures: 0,
            idle_fence_triggers: 0,
            disconnect_sources: 0,
            disconnect_fences: 0,
            disconnect_failures: 0,
        }
    }

    fn observe_feedback(&mut self, outcome: sophia_backend_live::LivePresentFeedbackOutcome) {
        if outcome.idle_fence_triggered {
            self.idle_fence_triggers = self.idle_fence_triggers.saturating_add(1);
        }
        for feedback in outcome.feedback {
            match feedback {
                sophia_backend_live::LivePresentProtocolFeedback::Complete {
                    transaction,
                    ust,
                    msc,
                    mode,
                } => {
                    let mode = match mode {
                        sophia_backend_live::LivePresentCompletionMode::Flip => {
                            self.complete_flip = self.complete_flip.saturating_add(1);
                            XPresentCompletionMode::Flip
                        }
                        sophia_backend_live::LivePresentCompletionMode::Skip => {
                            self.complete_skip = self.complete_skip.saturating_add(1);
                            XPresentCompletionMode::Skip
                        }
                    };
                    match self
                        .router
                        .route_present_complete(transaction, ust, msc, mode)
                    {
                        Ok(routed) => {
                            self.complete_routed =
                                self.complete_routed.saturating_add(usize::from(routed));
                            if std::env::var_os("SOPHIA_LIVE_SESSION_DIAGNOSTIC").is_some() {
                                eprintln!(
                                    "sophia_live_session_present_feedback schema=1 kind=complete transaction={} routed={routed} mode={mode:?} ust={ust} msc={msc}",
                                    transaction.raw(),
                                );
                            }
                        }
                        Err(error) => {
                            self.route_failures = self.route_failures.saturating_add(1);
                            eprintln!(
                                "sophia_live_session_present_feedback schema=1 kind=complete transaction={} routed=false error={error}",
                                transaction.raw(),
                            );
                        }
                    }
                }
                sophia_backend_live::LivePresentProtocolFeedback::Idle { transaction } => {
                    self.idle = self.idle.saturating_add(1);
                    match self.router.route_present_idle(transaction) {
                        Ok(routed) => {
                            self.idle_routed = self.idle_routed.saturating_add(usize::from(routed));
                            if std::env::var_os("SOPHIA_LIVE_SESSION_DIAGNOSTIC").is_some() {
                                eprintln!(
                                    "sophia_live_session_present_feedback schema=1 kind=idle transaction={} routed={routed}",
                                    transaction.raw(),
                                );
                            }
                        }
                        Err(error) => {
                            self.route_failures = self.route_failures.saturating_add(1);
                            eprintln!(
                                "sophia_live_session_present_feedback schema=1 kind=idle transaction={} routed=false error={error}",
                                transaction.raw(),
                            );
                        }
                    }
                }
            }
        }
    }

    fn observe_disconnect(
        &mut self,
        report: sophia_backend_live::LivePresentationDisconnectReport,
    ) {
        self.idle_fence_triggers = self
            .idle_fence_triggers
            .saturating_add(report.triggered_idle_fences);
        self.disconnect_sources = self
            .disconnect_sources
            .saturating_add(report.released_sources.len());
        self.disconnect_fences = self
            .disconnect_fences
            .saturating_add(report.released_fences.len());
        self.disconnect_failures = self
            .disconnect_failures
            .saturating_add(report.failed_idle_fences);
    }
}

fn production_authority_batch(
    batch: &XAuthorityObservedTransactionBatch,
    released_admission_groups: &[LiveAdmissionAuthorityGroup],
    layout: &PersistentLiveLayout,
) -> LiveProductionAuthorityBatch {
    let mut groups = Vec::<sophia_backend_live::LiveProductionAuthorityGroup>::new();
    for transaction in &batch.transactions {
        let index = production_authority_group_index(&mut groups, transaction.transaction);
        groups[index].transactions.push(transaction.clone());
    }
    if !batch.removed_surfaces.is_empty() {
        let index = production_authority_group_index(&mut groups, batch.transaction);
        groups[index]
            .removed_surfaces
            .extend(batch.removed_surfaces.iter().copied());
    }
    for submission in &batch.present_submissions {
        let index = production_authority_group_index(&mut groups, submission.transaction);
        groups[index].present_submissions.push(
            sophia_backend_live::LiveProductionPresentSubmission {
                transaction: submission.transaction,
                surface: submission.surface,
                buffer: submission.buffer,
                x_offset: submission.x_offset,
                y_offset: submission.y_offset,
                acquire_fence: submission.acquire_fence,
                idle_fence: submission.idle_fence,
                layout_disposition: layout
                    .present_layout_disposition(submission.surface, submission.buffer),
            },
        );
    }
    for released in released_admission_groups {
        let index = production_authority_group_index(&mut groups, released.transaction);
        groups[index]
            .transactions
            .extend(released.transactions.iter().cloned());
        groups[index].present_submissions.extend(
            released
                .present_submissions
                .iter()
                .map(|submission| {
                    sophia_backend_live::LiveProductionPresentSubmission {
                        transaction: submission.transaction,
                        surface: submission.surface,
                        buffer: submission.buffer,
                        x_offset: submission.x_offset,
                        y_offset: submission.y_offset,
                        acquire_fence: submission.acquire_fence,
                        idle_fence: submission.idle_fence,
                        layout_disposition: layout
                            .present_layout_disposition(submission.surface, submission.buffer),
                    }
                }),
        );
    }
    LiveProductionAuthorityBatch {
        groups,
        dma_buf_registrations: batch
            .dma_buf_registrations
            .iter()
            .map(|registration| LiveProductionDmaBufRegistration {
                descriptor: registration.descriptor,
                plane_fds: registration.plane_fds.clone(),
            })
            .collect(),
        fence_registrations: batch
            .fence_registrations
            .iter()
            .map(|registration| LiveProductionFenceRegistration {
                handle: registration.handle,
                initially_triggered: registration.initially_triggered,
                fd: Arc::clone(&registration.fd),
            })
            .collect(),
        released_dma_bufs: batch.released_dma_bufs.clone(),
        released_fences: batch.released_fences.clone(),
    }
}

fn production_authority_group_index(
    groups: &mut Vec<sophia_backend_live::LiveProductionAuthorityGroup>,
    transaction: TransactionId,
) -> usize {
    if let Some(index) = groups
        .iter()
        .position(|group| group.transaction == transaction)
    {
        return index;
    }
    groups.push(sophia_backend_live::LiveProductionAuthorityGroup {
        transaction,
        transactions: Vec::new(),
        removed_surfaces: Vec::new(),
        present_submissions: Vec::new(),
    });
    groups.len() - 1
}

fn renderer_cpu_buffer_update(
    update: &sophia_x_authority::XAuthorityCpuBufferUpdate,
) -> sophia_backend_live::LiveCpuBufferUpdate {
    match update {
        sophia_x_authority::XAuthorityCpuBufferUpdate::Replace(buffer) => {
            sophia_backend_live::LiveCpuBufferUpdate::Replace(
                sophia_backend_live::LiveCpuBufferSource {
                    handle: buffer.handle,
                    size: buffer.size,
                    stride: buffer.stride,
                    format: buffer.format,
                    generation: buffer.generation,
                    bytes: buffer.bytes.clone(),
                },
            )
        }
        sophia_x_authority::XAuthorityCpuBufferUpdate::Patch(patch) => {
            sophia_backend_live::LiveCpuBufferUpdate::Patch(
                sophia_backend_live::LiveCpuBufferPatch {
                    handle: patch.handle,
                    size: patch.size,
                    stride: patch.stride,
                    format: patch.format,
                    generation: patch.generation,
                    rect: patch.rect,
                    bytes: patch.bytes.clone(),
                },
            )
        }
    }
}

fn synthetic_text_input_events(
    text: &str,
) -> Result<Vec<sophia_protocol::InputEventPacket>, Box<dyn std::error::Error>> {
    let mut serial = 1u64;
    let mut events = Vec::with_capacity((text.len() + 1).saturating_mul(2));
    for x_keycode in text
        .bytes()
        .map(super::x_authority::x11_keycode_for_ascii)
        .chain(std::iter::once(Some(36)))
    {
        let x_keycode = x_keycode.ok_or("test input has no core X keycode")?;
        let keycode = u32::from(
            x_keycode
                .checked_sub(8)
                .ok_or("test input has no evdev keycode")?,
        );
        for pressed in [true, false] {
            events.push(sophia_protocol::InputEventPacket {
                serial,
                seat: SeatId::from_raw(SESSION_SEAT_RAW),
                device: DeviceId::from_raw(SESSION_KEYBOARD_DEVICE_RAW),
                time_msec: serial,
                kind: sophia_protocol::InputEventKind::Key { keycode, pressed },
                global_position: None,
                target_surface: None,
                local_position: None,
            });
            serial = serial.saturating_add(1);
        }
    }
    Ok(events)
}
