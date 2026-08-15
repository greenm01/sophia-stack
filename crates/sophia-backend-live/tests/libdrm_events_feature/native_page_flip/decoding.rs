use super::*;

#[test]
fn native_libdrm_poller_constructs_routes_from_discovered_outputs_without_kms_identity() {
    let root = multi_output_drm_sysfs_fixture("native-libdrm-discovered-routes");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let authority =
        LibdrmBackendFdAuthority::new(22).expect("nonzero generation should mint authority token");
    let routes = report.native_libdrm_output_routes();
    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].slot.raw(), 1);
    assert_eq!(routes[0].output, OutputId::from_raw(1));
    assert_eq!(routes[1].slot.raw(), 2);
    assert_eq!(routes[1].output, OutputId::from_raw(2));
    // The route carries the minted opaque head, which is what tells two heads
    // of one logical output apart once mirroring puts them there; the
    // connector integer stays behind the backend's head table.
    assert_eq!(routes[1].head, sophia_engine::RenderHeadId::from_raw(2));
    assert_eq!(
        report.native_libdrm_poller_startup_report(),
        LiveLibdrmPollerStartupReport {
            status: LiveLibdrmPollerStartupStatus::Ready,
            route_count: 2,
        }
    );
    let mut poller = report
        .native_libdrm_poller_from_authority(authority)
        .expect("ready discovery should construct native poller");
    let (sender, receiver) = mpsc::sync_channel(2);
    assert_eq!(poller.diagnostics().route_count, 2);
    poller.inject_callbacks([LibdrmNativePageFlipCallback::new(routes[1].slot, 90)]);
    let poll_report = poller.poll_page_flip_events(&sender, 4);
    assert_eq!(poll_report.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(
        receiver.try_recv().expect("callback should map"),
        LivePageFlipCallback {
            output: OutputId::from_raw(2),
            head: sophia_engine::RenderHeadId::from_raw(2),
            frame_serial: 90,
        }
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_libdrm_poller_construction_fails_closed_without_outputs() {
    let root = std::env::temp_dir().join("sophia-backend-live-native-libdrm-no-routes");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let authority =
        LibdrmBackendFdAuthority::new(23).expect("nonzero generation should mint authority token");
    assert!(report.native_libdrm_output_routes().is_empty());
    assert_eq!(
        report.native_libdrm_poller_startup_report(),
        LiveLibdrmPollerStartupReport {
            status: LiveLibdrmPollerStartupStatus::NoOutputs,
            route_count: 0,
        }
    );
    assert!(
        report
            .native_libdrm_poller_from_authority(authority)
            .is_none()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_libdrm_page_flip_callback_decodes_without_native_resource_identity() {
    assert_eq!(LibdrmNativeOutputSlot::new(0), None);
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    assert_eq!(slot.raw(), 2);
    let routes = [LibdrmNativeOutputRoute {
        slot,
        output: OutputId::from_raw(7),
        head: sophia_engine::RenderHeadId::from_raw(1),
    }];
    assert_eq!(
        LibdrmNativePageFlipCallback::new(slot, 81).decode(&routes),
        LibdrmNativePageFlipDecodeReport {
            status: LibdrmNativePageFlipDecodeStatus::Decoded,
            callback: Some(LivePageFlipCallback {
                output: OutputId::from_raw(7),
                head: sophia_engine::RenderHeadId::from_raw(1),
                frame_serial: 81,
            }),
        }
    );
    let unknown_slot = LibdrmNativeOutputSlot::new(3).expect("nonzero slot should be valid");
    assert_eq!(
        LibdrmNativePageFlipCallback::new(unknown_slot, 82).decode(&routes),
        LibdrmNativePageFlipDecodeReport {
            status: LibdrmNativePageFlipDecodeStatus::UnknownOutputSlot,
            callback: None,
        }
    );
    assert_eq!(
        LibdrmNativePageFlipCallback::new(slot, 0).decode(&routes),
        LibdrmNativePageFlipDecodeReport {
            status: LibdrmNativePageFlipDecodeStatus::InvalidFrameSerial,
            callback: None,
        }
    );
}

#[test]
fn native_libdrm_page_flip_decode_batch_is_bounded_and_reduced() {
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let unknown_slot = LibdrmNativeOutputSlot::new(3).expect("nonzero slot should be valid");
    let routes = [LibdrmNativeOutputRoute {
        slot,
        output: OutputId::from_raw(7),
        head: sophia_engine::RenderHeadId::from_raw(1),
    }];
    let callbacks = [
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 0),
        LibdrmNativePageFlipCallback::new(unknown_slot, 82),
        LibdrmNativePageFlipCallback::new(slot, 83),
    ];
    let (sender, receiver) = mpsc::sync_channel(4);
    let report = decode_native_page_flip_batch(&callbacks, &routes, &sender, 4);
    assert_eq!(
        report.read_loop.status,
        LibdrmNativeReadLoopStatus::CallbackDecoded
    );
    assert_eq!(report.read_loop.decoded_callbacks, 2);
    assert_eq!(report.read_loop.rejected_callbacks, 2);
    assert_eq!(report.poll.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(report.poll.callbacks.emitted, 2);
    for frame_serial in [81, 83] {
        assert_eq!(
            receiver.try_recv().expect("callback should be queued"),
            LivePageFlipCallback {
                output: OutputId::from_raw(7),
                head: sophia_engine::RenderHeadId::from_raw(1),
                frame_serial,
            }
        );
    }
    assert!(receiver.try_recv().is_err());
    let (sender, _receiver) = mpsc::sync_channel(4);
    let limited = decode_native_page_flip_batch(&callbacks, &routes, &sender, 1);
    assert_eq!(limited.read_loop.decoded_callbacks, 1);
    assert_eq!(limited.poll.callbacks.emitted, 1);
    assert!(limited.poll.callbacks.max_reached);
    assert_eq!(limited.poll.callbacks.queued_remaining, 3);
}

#[test]
fn native_libdrm_page_flip_decode_batch_reports_backpressure_without_native_identity() {
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let routes = [LibdrmNativeOutputRoute {
        slot,
        output: OutputId::from_raw(7),
        head: sophia_engine::RenderHeadId::from_raw(1),
    }];
    let callbacks = [
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 82),
    ];
    let (sender, _receiver) = mpsc::sync_channel(1);
    let report = decode_native_page_flip_batch(&callbacks, &routes, &sender, 4);
    assert_eq!(report.read_loop.decoded_callbacks, 2);
    assert_eq!(report.read_loop.rejected_callbacks, 0);
    assert_eq!(
        report.poll.status,
        LibdrmPageFlipEventPollStatus::Backpressure
    );
    assert_eq!(report.poll.callbacks.emitted, 1);
    assert_eq!(report.poll.callbacks.queued_remaining, 1);
}

#[test]
fn libdrm_event_poll_report_projects_source_state_without_native_identity() {
    for (source, expected) in [
        (
            LivePageFlipCallbackSourceReport {
                emitted: 0,
                queued_remaining: 0,
                backpressure: false,
                disconnected: false,
                max_reached: false,
            },
            LibdrmPageFlipEventPollStatus::Idle,
        ),
        (
            LivePageFlipCallbackSourceReport {
                emitted: 2,
                queued_remaining: 0,
                backpressure: false,
                disconnected: false,
                max_reached: false,
            },
            LibdrmPageFlipEventPollStatus::Emitted,
        ),
        (
            LivePageFlipCallbackSourceReport {
                emitted: 1,
                queued_remaining: 1,
                backpressure: true,
                disconnected: false,
                max_reached: false,
            },
            LibdrmPageFlipEventPollStatus::Backpressure,
        ),
    ] {
        assert_eq!(
            LibdrmPageFlipEventPollReport::from_source_report(source).status,
            expected
        );
    }
}

#[test]
fn fake_libdrm_page_flip_poller_feeds_runtime_queue() {
    let root = ready_drm_sysfs_fixture("fake-libdrm-page-flip-poller");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let (sender, receiver) = mpsc::sync_channel(1);
    let mut poller = FakeLibdrmPageFlipEventPoller::new([
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            head: sophia_engine::RenderHeadId::from_raw(1),
            frame_serial: 61,
        },
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            head: sophia_engine::RenderHeadId::from_raw(1),
            frame_serial: 62,
        },
    ]);
    let poll = poller.poll_page_flip_events(&sender, 4);
    assert_eq!(poll.status, LibdrmPageFlipEventPollStatus::Backpressure);
    assert_eq!(poll.callbacks.emitted, 1);
    assert_eq!(poller.queued_len(), 1);
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 4));
    let first_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain first callback");
    assert_eq!(
        first_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(61),
        }
    );
    let poll = poller.poll_page_flip_events(&sender, 4);
    assert_eq!(poll.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(poller.queued_len(), 0);
    let second_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain second callback");
    assert_eq!(
        second_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(62),
        }
    );
    std::fs::remove_dir_all(root).unwrap();
}
