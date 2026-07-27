#[derive(Default)]
struct RecordingOutputRuntimeAdapter {
    output_count: usize,
    generations: Vec<(usize, u64)>,
}

impl ProductionOutputRuntimeAdapter for RecordingOutputRuntimeAdapter {
    type Report = usize;
    type Error = String;

    fn output_count(&self) -> usize {
        self.output_count
    }

    fn run_output(
        &mut self,
        output_index: usize,
        committed: &[CommittedSurfaceState],
    ) -> Result<Self::Report, Self::Error> {
        self.generations
            .push((output_index, committed[0].committed_generation));
        Ok(output_index)
    }
}

#[test]
fn production_coordinator_projects_one_snapshot_through_output_runtime_adapter() {
    let engine = HeadlessEngine::default();
    let committed = vec![engine.committed_state_from_layer(&test_layer(0, 0, 0, Region::empty()))];
    let coordinator = ProductionSessionCoordinator::new(engine).with_committed_surfaces(committed);
    let mut adapter = RecordingOutputRuntimeAdapter {
        output_count: 2,
        ..RecordingOutputRuntimeAdapter::default()
    };

    let reports = coordinator.run_outputs(&mut adapter).unwrap();

    assert_eq!(reports, [0, 1]);
    assert_eq!(adapter.generations, [(0, 1), (1, 1)]);
}

#[derive(Default)]
struct RecordingProductionAdapter {
    calls: Vec<&'static str>,
    fail_at: Option<&'static str>,
    pending: Vec<(u64, usize)>,
    withhold_retirement: bool,
    feedback_cycles: Vec<u64>,
}

impl ProductionPresentationAdapter for RecordingProductionAdapter {
    type Frame = usize;
    type Submission = usize;
    type Retirement = usize;
    type Evidence = usize;
    type Error = &'static str;

    fn compose(
        &mut self,
        _cycle: u64,
        committed: &[CommittedSurfaceState],
        _authority_commits: &[TransactionCommit],
    ) -> Result<Self::Frame, Self::Error> {
        self.calls.push("compose");
        if self.fail_at == Some("compose") {
            return Err("compose");
        }
        Ok(committed.len())
    }

    fn submit_frame(
        &mut self,
        cycle: u64,
        frame: Self::Frame,
    ) -> Result<Self::Submission, Self::Error> {
        self.calls.push("submit");
        if self.fail_at == Some("submit") {
            return Err("submit");
        }
        self.pending.push((cycle, frame));
        Ok(frame)
    }

    fn poll_retirements(
        &mut self,
    ) -> Result<Vec<ProductionRetirement<Self::Retirement>>, Self::Error> {
        self.calls.push("retire");
        if self.withhold_retirement {
            return Ok(Vec::new());
        }
        if self.fail_at == Some("retire") {
            return Err("retire");
        }
        Ok(self
            .pending
            .drain(..)
            .map(|(cycle, retirement)| ProductionRetirement { cycle, retirement })
            .collect())
    }

    fn route_protocol_feedback(
        &mut self,
        cycle: u64,
        retirement: Self::Retirement,
    ) -> Result<Self::Evidence, Self::Error> {
        self.calls.push("feedback");
        self.feedback_cycles.push(cycle);
        if self.fail_at == Some("feedback") {
            return Err("feedback");
        }
        assert_eq!(retirement, 1);
        Ok(retirement)
    }
}

fn production_surface_batch(transaction: u64) -> AuthorityTransactionIntake {
    let surface = SurfaceId::new(44, 1);
    AuthorityTransactionIntake::new(
        TransactionId::from_raw(transaction),
        vec![SurfaceTransaction {
            transaction: TransactionId::from_raw(transaction),
            authority: AuthorityKind::SophiaX,
            surface,
            namespace: Some(NamespaceId::from_raw(2)),
            target_geometry: Rect {
                x: 10,
                y: 20,
                width: 320,
                height: 200,
            },
            target_buffer: BufferSource::CpuBuffer { handle: 900 },
            damage: Region::single(Rect {
                x: 0,
                y: 0,
                width: 320,
                height: 200,
            }),
            readiness: SurfaceTransactionReadiness::Ready,
            timeout_msec: 250,
            previous_committed_generation: 0,
        }],
    )
}

#[test]
fn production_coordinator_applies_prepared_present_to_its_owned_snapshot() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let committed = vec![engine.committed_state_from_layer(&old_layer)];
    let mut coordinator =
        ProductionSessionCoordinator::new(engine).with_committed_surfaces(committed);
    let mut next_layer = old_layer;
    next_layer.geometry.width = 640;
    next_layer.source = BufferSource::DmaBuf { handle: 77 };
    let mut transaction = next_layer.to_surface_transaction(
        TransactionId::from_raw(205),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        1,
    );
    transaction.previous_committed_generation = 99;
    let prepared = coordinator.prepare_present_transaction(&transaction);

    let commit = coordinator.apply_prepared_surface_commit(prepared);

    assert_eq!(commit.outcome, TransactionOutcome::Committed);
    assert_eq!(coordinator.committed_surfaces()[0].geometry.width, 640);
    assert_eq!(
        coordinator.committed_surfaces()[0].buffer,
        BufferSource::DmaBuf { handle: 77 }
    );
}

#[test]
fn present_candidate_preserves_unrelated_committed_surface_identity() {
    let engine = HeadlessEngine::default();
    let bar = engine.committed_state_from_layer(&test_layer(1, 0, 0, Region::empty()));
    let bar_generation = bar.committed_generation;
    let kitty_surface = SurfaceId::new(2, 1);
    let kitty_transaction = SurfaceTransaction {
        transaction: TransactionId::from_raw(403),
        authority: AuthorityKind::SophiaX,
        surface: kitty_surface,
        namespace: None,
        target_geometry: Rect {
            x: 0,
            y: 14,
            width: 1280,
            height: 1426,
        },
        target_buffer: BufferSource::DmaBuf { handle: 77 },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 1426,
        }),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 99,
    };
    let mut coordinator =
        ProductionSessionCoordinator::new(engine).with_committed_surfaces(vec![bar.clone()]);

    let prepared = coordinator.prepare_present_transaction(&kitty_transaction);

    assert!(prepared.is_ready());
    assert_eq!(prepared.commit().applied_surfaces, [kitty_surface]);
    assert_eq!(
        prepared
            .candidate()
            .iter()
            .find(|state| state.surface == bar.surface)
            .map(|state| state.committed_generation),
        Some(bar_generation)
    );
    assert_eq!(coordinator.committed_surfaces(), [bar.clone()]);
    let commit = coordinator.apply_prepared_surface_commit(prepared);
    assert_eq!(commit.outcome, TransactionOutcome::Committed);
    assert_eq!(coordinator.committed_surfaces().len(), 2);
    assert_eq!(
        coordinator
            .committed_surfaces()
            .iter()
            .find(|state| state.surface == bar.surface)
            .map(|state| state.committed_generation),
        Some(bar_generation)
    );
    assert_eq!(
        coordinator
            .committed_surfaces()
            .iter()
            .find(|state| state.surface == kitty_surface)
            .map(|state| state.committed_generation),
        Some(1)
    );
}

#[test]
fn production_coordinator_rejects_stale_prepared_baseline_before_backend_feedback() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let committed = vec![engine.committed_state_from_layer(&old_layer)];
    let mut coordinator =
        ProductionSessionCoordinator::new(engine).with_committed_surfaces(committed);
    let transaction = old_layer.to_surface_transaction(
        TransactionId::from_raw(206),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        1,
    );
    let prepared = coordinator.engine().prepare_surface_transactions(
        TransactionId::from_raw(206),
        &[transaction],
        coordinator.committed_surfaces(),
    );
    let mut changed = coordinator.committed_surfaces().to_vec();
    changed[0].committed_generation = 9;
    coordinator.replace_committed_surfaces(changed);
    let commit = coordinator.apply_prepared_surface_commit(prepared);

    assert_eq!(commit.outcome, TransactionOutcome::RejectedStaleSurface);
    assert_eq!(coordinator.committed_surfaces()[0].committed_generation, 9);
}

#[test]
fn production_coordinator_settles_a_stale_prepared_retirement_without_replacing_current_state() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let committed = vec![engine.committed_state_from_layer(&old_layer)];
    let mut coordinator =
        ProductionSessionCoordinator::new(engine).with_committed_surfaces(committed);
    let transaction = old_layer.to_surface_transaction(
        TransactionId::from_raw(207),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        1,
    );
    let prepared = coordinator.prepare_present_transaction(&transaction);
    coordinator.replace_committed_surfaces(Vec::new());
    let mut settled = None;

    let report = coordinator
        .settle_prepared_retirement(prepared, |commit| {
            settled = Some(commit.outcome);
            Ok::<_, &'static str>("skip")
        })
        .expect("a stale retirement remains a controlled settlement");

    assert_eq!(settled, Some(TransactionOutcome::RejectedStaleSurface));
    assert_eq!(
        report.commit.outcome,
        TransactionOutcome::RejectedStaleSurface
    );
    assert_eq!(report.evidence, "skip");
    assert!(report.committed_surfaces.is_empty());
    assert!(coordinator.committed_surfaces().is_empty());
}

#[test]
fn production_coordinator_orders_commit_composition_retirement_and_feedback() {
    let mut coordinator = ProductionSessionCoordinator::new(HeadlessEngine::default());
    let mut adapter = RecordingProductionAdapter::default();

    let report = coordinator
        .run_cycle(&[production_surface_batch(201)], &mut adapter)
        .expect("production cycle should complete");

    assert_eq!(adapter.calls, ["compose", "submit", "retire", "feedback"]);
    assert_eq!(report.cycle, 1);
    assert_eq!(
        report.authority_commits[0].outcome,
        TransactionOutcome::Committed
    );
    assert_eq!(report.committed_surfaces.len(), 1);
    assert_eq!(report.submission, 1);
    assert_eq!(report.evidence, [1]);
}

#[test]
fn production_coordinator_routes_delayed_feedback_only_after_a_later_retirement_poll() {
    let mut coordinator = ProductionSessionCoordinator::new(HeadlessEngine::default());
    let mut adapter = RecordingProductionAdapter {
        withhold_retirement: true,
        ..RecordingProductionAdapter::default()
    };

    let first = coordinator
        .run_cycle(&[production_surface_batch(204)], &mut adapter)
        .expect("submission without a page flip remains in flight");
    assert!(first.evidence.is_empty());
    assert!(adapter.feedback_cycles.is_empty());

    adapter.withhold_retirement = false;
    let second = coordinator
        .run_cycle(&[], &mut adapter)
        .expect("later page flip poll should retire queued submissions");
    assert_eq!(second.evidence, [1, 1]);
    assert_eq!(adapter.feedback_cycles, [1, 2]);
}

#[test]
fn production_coordinator_never_routes_feedback_before_retirement() {
    let mut coordinator = ProductionSessionCoordinator::new(HeadlessEngine::default());
    let mut adapter = RecordingProductionAdapter {
        calls: Vec::new(),
        fail_at: Some("retire"),
        ..RecordingProductionAdapter::default()
    };

    let error = coordinator
        .run_cycle(&[production_surface_batch(202)], &mut adapter)
        .expect_err("missing retirement must fail the cycle");

    assert_eq!(adapter.calls, ["compose", "submit", "retire"]);
    assert_eq!(
        error,
        ProductionSessionCycleError {
            cycle: 1,
            phase: ProductionSessionPhase::KmsRetire,
            source: "retire",
        }
    );
    assert_eq!(coordinator.committed_surfaces().len(), 1);
}

#[test]
fn production_coordinator_reports_feedback_failure_after_retirement() {
    let mut coordinator = ProductionSessionCoordinator::new(HeadlessEngine::default());
    let mut adapter = RecordingProductionAdapter {
        calls: Vec::new(),
        fail_at: Some("feedback"),
        ..RecordingProductionAdapter::default()
    };

    let error = coordinator
        .run_cycle(&[production_surface_batch(203)], &mut adapter)
        .expect_err("feedback failure must remain explicit");

    assert_eq!(adapter.calls, ["compose", "submit", "retire", "feedback"]);
    assert_eq!(error.phase, ProductionSessionPhase::ProtocolFeedback);
    assert_eq!(coordinator.committed_surfaces().len(), 1);
}
