use super::*;

fn runtime() -> LiveProductionVisualRuntime {
    LiveProductionVisualRuntime::new(
        &[
            HeadlessOutput {
                id: OutputId::from_raw(1),
                size: Size {
                    width: 2560,
                    height: 1440,
                },
                scale: 1,
            },
            HeadlessOutput {
                id: OutputId::from_raw(2),
                size: Size {
                    width: 1920,
                    height: 1080,
                },
                scale: 1,
            },
        ],
        None,
    )
    .unwrap()
}

fn layer(surface: SurfaceId) -> LayerSnapshot {
    LayerSnapshot {
        surface,
        authority_local_id: None,
        output: None,
        namespace: None,
        stack_rank: 0,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 2560,
            height: 32,
        },
        source_size: Size {
            width: 2560,
            height: 32,
        },
        source: BufferSource::CpuBuffer { handle: 1 },
        damage: Region::empty(),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 1,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
        translation: None,
    }
}

#[test]
fn panel_route_reaches_display_list_and_is_withdrawn_with_session_visibility() {
    let panel = SurfaceId::new(1, 1);
    let unplaced = SurfaceId::new(2, 1);
    let mut runtime = runtime();
    let layers = [layer(panel), layer(unplaced)];
    let committed = layers
        .iter()
        .map(|l| {
            CommittedSurfaceState::with_source(
                l.surface,
                1,
                l.geometry,
                l.source,
                l.source_size,
                Region::empty(),
            )
        })
        .collect::<Vec<_>>();
    assert!(runtime.apply_presentation_layout(&layers, &[panel]));
    for output in [OutputId::from_raw(1), OutputId::from_raw(2)] {
        let list = runtime
            .display_list_for_output(
                output,
                runtime.outputs.logical_viewport(output).unwrap(),
                &committed,
                &[panel, unplaced],
            )
            .unwrap();
        assert!(
            list.commands
                .contains(&CompositorDisplayCommand::Surface { surface: panel })
        );
        assert!(
            !list
                .commands
                .contains(&CompositorDisplayCommand::Surface { surface: unplaced })
        );
    }
    assert!(
        runtime.apply_presentation_layout(&layers, &[]),
        "routing withdrawal invalidates the projection even with the same layer order"
    );
    assert!(runtime.geometry_routed_surfaces.is_empty());
    assert!(!runtime.apply_presentation_layout(&layers, &[]));
    runtime.apply_presentation_layout(&[], &[panel]);
    assert!(
        runtime.geometry_routed_surfaces.is_empty(),
        "stale route cannot resurrect an unmapped panel"
    );
}

#[test]
fn retained_repaints_wait_for_the_exact_first_present_to_retire() {
    let mut runtime = runtime();
    let surface = SurfaceId::new(3, 1);
    let transaction = TransactionId::from_raw(23649);
    let output = OutputId::from_raw(1);
    let geometry = Rect {
        x: 1919,
        y: 41,
        width: 1258,
        height: 1390,
    };
    let size = Size {
        width: geometry.width,
        height: geometry.height,
    };
    let candidate = SurfaceTransaction {
        transaction,
        surface,
        authority: AuthorityKind::SophiaX,
        namespace: None,
        target_geometry: geometry,
        presentation_extent: size,
        content: SurfaceContentSet::singleton(BufferSource::DmaBuf { handle: 28 }, size),
        damage: Region::empty(),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let prepared = runtime.production.prepare_present_transaction(&candidate);
    assert!(prepared.is_ready());
    let present = LiveProductionSubmittedPresent::new(
        BTreeMap::from([(output, LiveProductionNativeFrameId::from_raw(3045))]),
        output,
        candidate.key(),
        transaction,
        surface,
        prepared,
        LiveRetainedRendererImageLayer {
            image_id: LiveRendererImageId::from_raw(28),
            size,
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            placement: LiveCompositionPlacement {
                target: geometry,
                clip: None,
                transform: Transform::IDENTITY,
                alpha: 1.0,
                sampling: HeadSamplingClass::Exact,
            },
        },
    )
    .unwrap();
    assert!(!runtime.retained_projection_blocked());
    runtime.present_scheduler.mark_rendering(present);
    assert!(
        runtime.retained_projection_blocked(),
        "first-frame render owns its retirement proof before KMS submission"
    );
    runtime
        .present_scheduler
        .mark_output_submitted(output)
        .unwrap();
    assert!(runtime.retained_projection_blocked());
    runtime
        .present_scheduler
        .mark_output_retired(LiveProductionPageFlipRetirement {
            output,
            ust: 1000,
            msc: 1,
        })
        .unwrap();
    assert!(
        runtime.retained_projection_blocked(),
        "retirement must be settled before repainting"
    );
    runtime.present_scheduler.take_submitted().unwrap();
    assert!(!runtime.retained_projection_blocked());
}
