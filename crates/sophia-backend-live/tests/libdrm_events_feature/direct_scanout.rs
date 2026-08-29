// The direct scanout path and its fallback ladder.
//
// These exercise the exporter alone, with no render device: a direct frame
// never opens a renderer context, which is both the point of the row and what
// makes it testable here. The submit-side atomic test and the Present
// settlement are proven where their devices live.
//
// Model: `validation/tla/PresentFlipOwnership.tla`.

#[cfg(feature = "gbm-probe")]
fn direct_scanout_head() -> Size {
    Size {
        width: 640,
        height: 480,
    }
}

/// A lowered frame Engine proved needs no composition.
#[cfg(feature = "gbm-probe")]
fn proven_direct_frame() -> sophia_renderer_live::LiveOwnedMixedCompositionFrame {
    let fd: OwnedFd = std::fs::File::open("/dev/null")
        .expect("test plane descriptor")
        .into();
    sophia_renderer_live::LiveOwnedMixedCompositionFrame {
        trace: Some(sophia_renderer_live::LiveCompositionTrace {
            output: OutputId::from_raw(1),
            head: sophia_engine::RenderHeadId::from_raw(1),
            scene_generation: 91,
        }),
        layers: vec![sophia_renderer_live::LiveOwnedMixedCompositionLayer::DmaBuf {
            image_id: sophia_renderer_live::LiveRendererImageId::from_raw(11),
            frame: sophia_renderer_live::LiveOwnedMultiPlaneDmaBufFrame {
                width: 640,
                height: 480,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                modifier: 0,
                plane_count: 1,
                planes: [
                    Some(sophia_renderer_live::LiveOwnedDmaBufPlane {
                        fd,
                        offset: 0,
                        stride: 2_560,
                    }),
                    None,
                    None,
                    None,
                ],
            },
            placement: sophia_renderer_live::LiveCompositionPlacement {
                target: Rect {
                    x: 0,
                    y: 0,
                    width: 640,
                    height: 480,
                },
                clip: None,
                transform: sophia_protocol::Transform::IDENTITY,
                alpha: 1.0,
                sampling: sophia_engine::HeadSamplingClass::Exact,
            },
        }],
        output_damage_snapshot: None,
        direct_scanout: sophia_engine::DirectScanoutVerdict::Eligible,
    }
}

#[cfg(feature = "gbm-probe")]
#[test]
fn a_disabled_exporter_never_derives_a_direct_candidate() {
    // Off is the pre-row behaviour exactly: the frame stays pending for the
    // ordinary composed path, not a different path that happens to compose.
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    assert!(!exporter.direct_scanout_enabled());
    exporter.set_pending_mixed_frame(proven_direct_frame());

    let export = exporter.export_rendered_scanout_buffer(LiveGbmEglFrameTargetRecord::new(
        direct_scanout_head(),
    ));

    assert_eq!(exporter.direct_scanout_attempts(), 0);
    assert!(export.owner.is_none());
    assert!(!exporter.direct_scanout_outstanding());
}

#[cfg(feature = "gbm-probe")]
#[test]
fn a_proven_frame_exports_the_clients_buffer_and_keeps_its_composed_form() {
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    exporter.set_direct_scanout_enabled(true);
    exporter.set_pending_mixed_frame(proven_direct_frame());

    let export = exporter.export_rendered_scanout_buffer(LiveGbmEglFrameTargetRecord::new(
        direct_scanout_head(),
    ));

    assert_eq!(
        export.status,
        LiveRendererScanoutBufferExportStatus::Exported
    );
    let owner = export.owner.expect("a direct export owns the client buffer");
    assert!(owner.is_direct_client_buffer());
    // A client's buffer was allocated against the client's device, so it takes
    // the PRIME transport rather than a handle only the renderer's file knows.
    assert!(!owner.shares_kms_drm_file());
    let fds = owner
        .export_scanout_dma_buf_fds()
        .expect("plane descriptors duplicate")
        .expect("a direct owner always has descriptors");
    assert_eq!(fds.plane_count(), 1);

    let descriptor = export.descriptor.expect("a direct export has a descriptor");
    assert_eq!(descriptor.format, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888);
    assert_eq!(descriptor.pitch, 2_560);
    assert!(descriptor.is_valid_scanout_buffer());

    assert_eq!(exporter.direct_scanout_attempts(), 1);
    assert_eq!(exporter.direct_scanout_exports(), 1);
    // Nothing has reached a screen yet -- the driver has not been asked -- so
    // the composed form is still held against a refusal.
    assert!(exporter.direct_scanout_outstanding());
    assert!(!exporter.pending_frame());
}

#[cfg(feature = "gbm-probe")]
#[test]
fn a_refused_direct_attempt_re_offers_the_same_content_for_composition() {
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    exporter.set_direct_scanout_enabled(true);
    exporter.set_pending_mixed_frame(proven_direct_frame());
    let _ = exporter.export_rendered_scanout_buffer(LiveGbmEglFrameTargetRecord::new(
        direct_scanout_head(),
    ));
    assert!(!exporter.pending_frame());

    assert!(LiveRenderedScanoutBufferExporter::fall_back_from_direct(
        &mut exporter
    ));

    // The frame is back, and back without its proof: the retry composes rather
    // than being refused again for the same reason, which is what keeps a
    // refusal from becoming a loop. `PresentFlipOwnership.tla`, `CommitRefused`.
    assert!(exporter.pending_mixed_frame());
    assert_eq!(exporter.direct_scanout_fallbacks(), 1);
    let second = exporter.export_rendered_scanout_buffer(LiveGbmEglFrameTargetRecord::new(
        direct_scanout_head(),
    ));
    assert_eq!(exporter.direct_scanout_attempts(), 1);
    assert!(second.owner.is_none());
    assert!(!exporter.direct_scanout_outstanding());

    // Nothing left to fall back to once it has been re-offered.
    assert!(!LiveRenderedScanoutBufferExporter::fall_back_from_direct(
        &mut exporter
    ));
}

#[cfg(feature = "gbm-probe")]
#[test]
fn committing_a_direct_flip_drops_the_composed_form_and_counts_the_flip() {
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    exporter.set_direct_scanout_enabled(true);
    exporter.set_pending_mixed_frame(proven_direct_frame());
    let _ = exporter.export_rendered_scanout_buffer(LiveGbmEglFrameTargetRecord::new(
        direct_scanout_head(),
    ));

    assert_eq!(exporter.direct_scanout_flips(), 0);
    LiveRenderedScanoutBufferExporter::commit_direct_scanout(&mut exporter);

    assert_eq!(exporter.direct_scanout_flips(), 1);
    assert!(!exporter.direct_scanout_outstanding());
    // The composed copy is gone; the client's buffer is not, because it is what
    // the screen is scanning until a successor retires it.
    // `PresentFlipOwnership.tla`, `DisplayedClientBufferIsNeverReleased`.
    assert!(!exporter.pending_frame());
    assert!(!LiveRenderedScanoutBufferExporter::fall_back_from_direct(
        &mut exporter
    ));
}

#[cfg(feature = "gbm-probe")]
#[test]
fn one_composed_frame_between_direct_ones_costs_a_fresh_validating_commit() {
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    exporter.set_direct_scanout_enabled(true);
    let target = LiveGbmEglFrameTargetRecord::new(direct_scanout_head());

    // First direct frame: the edge into direct scanout, so a test is owed.
    exporter.set_pending_mixed_frame(proven_direct_frame());
    let _ = exporter.export_rendered_scanout_buffer(target);
    assert!(LiveRenderedScanoutBufferExporter::direct_scanout_test_required(&exporter));
    LiveRenderedScanoutBufferExporter::record_direct_scanout_test(&mut exporter, true);
    LiveRenderedScanoutBufferExporter::commit_direct_scanout(&mut exporter);

    // A run of direct frames continues the episode: no further ioctl.
    exporter.set_pending_mixed_frame(proven_direct_frame());
    let _ = exporter.export_rendered_scanout_buffer(target);
    assert!(!LiveRenderedScanoutBufferExporter::direct_scanout_test_required(&exporter));
    LiveRenderedScanoutBufferExporter::commit_direct_scanout(&mut exporter);

    // An overlay opens: one composed frame. That alone ends the episode, so the
    // next direct frame is validated afresh rather than riding a test taken
    // when the scene was something else.
    // `PresentFlipOwnership.tla`, `ReProveAfterEpisodeChange`.
    let mut composed = proven_direct_frame();
    composed.direct_scanout = sophia_engine::DirectScanoutVerdict::CompositionRequired;
    exporter.set_pending_mixed_frame(composed);
    let _ = exporter.export_rendered_scanout_buffer(target);

    exporter.set_pending_mixed_frame(proven_direct_frame());
    let _ = exporter.export_rendered_scanout_buffer(target);
    assert!(LiveRenderedScanoutBufferExporter::direct_scanout_test_required(&exporter));
    assert_eq!(exporter.direct_scanout_tests(), 1);
}

#[cfg(feature = "gbm-probe")]
#[test]
fn a_refused_validating_commit_ends_the_episode_and_is_counted() {
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    exporter.set_direct_scanout_enabled(true);
    exporter.set_pending_mixed_frame(proven_direct_frame());
    let _ = exporter.export_rendered_scanout_buffer(LiveGbmEglFrameTargetRecord::new(
        direct_scanout_head(),
    ));

    LiveRenderedScanoutBufferExporter::record_direct_scanout_test(&mut exporter, false);

    assert_eq!(exporter.direct_scanout_tests(), 1);
    assert_eq!(exporter.direct_scanout_test_rejections(), 1);
    assert!(LiveRenderedScanoutBufferExporter::direct_scanout_test_required(&exporter));
    assert!(LiveRenderedScanoutBufferExporter::fall_back_from_direct(
        &mut exporter
    ));
    assert!(exporter.pending_mixed_frame());
}

#[cfg(feature = "gbm-probe")]
#[test]
fn losing_the_direct_path_ends_any_episode_in_progress() {
    // A head that joins a mirror group loses the direct path. When it leaves
    // one, a test taken while it was somebody else's clone proves nothing.
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    exporter.set_direct_scanout_enabled(true);
    exporter.set_pending_mixed_frame(proven_direct_frame());
    let _ = exporter.export_rendered_scanout_buffer(LiveGbmEglFrameTargetRecord::new(
        direct_scanout_head(),
    ));
    LiveRenderedScanoutBufferExporter::record_direct_scanout_test(&mut exporter, true);
    assert!(!LiveRenderedScanoutBufferExporter::direct_scanout_test_required(&exporter));

    exporter.set_direct_scanout_enabled(false);
    exporter.set_direct_scanout_enabled(true);

    assert!(LiveRenderedScanoutBufferExporter::direct_scanout_test_required(&exporter));
}

#[cfg(feature = "gbm-probe")]
#[test]
fn a_structurally_ineligible_frame_falls_through_to_composition_without_a_second_attempt() {
    // Engine proved it, the layers disagree. The frame is not lost: it is
    // re-offered with its proof cleared, so it composes on this same pass and
    // is not re-derived and re-refused on every frame after it.
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    exporter.set_direct_scanout_enabled(true);
    let mut frame = proven_direct_frame();
    frame
        .layers
        .push(sophia_renderer_live::LiveOwnedMixedCompositionLayer::Solid {
            geometry: Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 24,
            },
            color: sophia_engine::CompositorRgb8 {
                red: 0,
                green: 0,
                blue: 0,
            },
        });
    exporter.set_pending_mixed_frame(frame);

    let export = exporter.export_rendered_scanout_buffer(LiveGbmEglFrameTargetRecord::new(
        direct_scanout_head(),
    ));

    assert_eq!(exporter.direct_scanout_attempts(), 1);
    assert_eq!(exporter.direct_scanout_refusals(), 1);
    assert_eq!(
        exporter.last_direct_scanout_refusal().map(
            sophia_renderer_live::LiveDirectScanoutRefusal::reduced_name
        ),
        Some("layer_count")
    );
    assert!(export.owner.is_none());
    assert!(!exporter.direct_scanout_outstanding());

    // Re-offered without its proof: a second pass makes no second attempt.
    exporter.set_direct_scanout_enabled(true);
    let _ = exporter.export_rendered_scanout_buffer(LiveGbmEglFrameTargetRecord::new(
        direct_scanout_head(),
    ));
    assert_eq!(exporter.direct_scanout_attempts(), 1);
}

#[cfg(feature = "gbm-probe")]
#[test]
fn every_step_of_an_episode_names_the_scene_it_belongs_to() {
    // The identity is read from the composed form held against a refusal, so
    // it survives past the export that produced it. Without that, a flip and
    // its test could only be correlated by their order in the log, which says
    // nothing when two heads interleave.
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    exporter.set_direct_scanout_enabled(true);
    exporter.set_pending_mixed_frame(proven_direct_frame());
    let _ = exporter.export_rendered_scanout_buffer(LiveGbmEglFrameTargetRecord::new(
        direct_scanout_head(),
    ));

    assert_eq!(exporter.outstanding_direct_scene_generation(), Some(91));
    LiveRenderedScanoutBufferExporter::record_direct_scanout_test(&mut exporter, true);
    assert_eq!(exporter.outstanding_direct_scene_generation(), Some(91));
    LiveRenderedScanoutBufferExporter::commit_direct_scanout(&mut exporter);
    assert_eq!(exporter.outstanding_direct_scene_generation(), None);
}
