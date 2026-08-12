fn head_objects(
    connector: u32,
    crtc: u32,
    plane: u32,
    framebuffer: u32,
    size: Size,
) -> LibdrmNativePrimaryPlaneObjects {
    LibdrmNativePrimaryPlaneObjects::new(
        drm::control::from_u32(connector).expect("connector handle should be nonzero"),
        drm::control::from_u32(crtc).expect("crtc handle should be nonzero"),
        drm::control::from_u32(plane).expect("plane handle should be nonzero"),
        drm::control::from_u32(framebuffer).expect("framebuffer handle should be nonzero"),
        15,
        size,
    )
}

fn head(
    connector: u32,
    crtc: u32,
    plane: u32,
    framebuffer: u32,
    size: Size,
) -> LibdrmNativeAtomicHead {
    LibdrmNativeAtomicHead::new(
        head_objects(connector, crtc, plane, framebuffer, size),
        primary_plane_properties(),
    )
}

fn scanout_size(width: i32, height: i32) -> Size {
    Size { width, height }
}

#[test]
fn independent_heads_combine_into_one_atomic_request() {
    let first = head(21, 41, 51, 61, scanout_size(1920, 1080));
    let second = head(22, 42, 52, 62, scanout_size(2560, 1440));

    let build = build_native_multi_head_atomic_request(
        &[first, second],
        LibdrmNativeAtomicCommitRequestScope::Modeset,
    );

    assert_eq!(
        build.status,
        LibdrmNativeMultiHeadRequestBuildStatus::Built
    );
    assert_eq!(build.heads, 2);
    assert!(build.request.is_some());
}

#[test]
fn a_mirror_group_shares_one_framebuffer_across_heads() {
    // Two connectors, two CRTCs, two planes, one framebuffer: this is mirroring.
    let primary = head(21, 41, 51, 61, scanout_size(1920, 1080));
    let mirrored = head(22, 42, 52, 61, scanout_size(1920, 1080));

    let build = build_native_multi_head_atomic_request(
        &[primary, mirrored],
        LibdrmNativeAtomicCommitRequestScope::Modeset,
    );

    assert_eq!(
        build.status,
        LibdrmNativeMultiHeadRequestBuildStatus::Built
    );
    assert_eq!(build.heads, 2);
}

#[test]
fn a_mirror_group_with_mismatched_sizes_fails_closed() {
    // Same framebuffer, different scanout size. No primary plane scaling exists,
    // so this must be rejected rather than letterboxed or sent to the kernel.
    let primary = head(21, 41, 51, 61, scanout_size(1920, 1080));
    let mirrored = head(22, 42, 52, 61, scanout_size(1280, 720));

    let build = build_native_multi_head_atomic_request(
        &[primary, mirrored],
        LibdrmNativeAtomicCommitRequestScope::Modeset,
    );

    assert_eq!(
        build.status,
        LibdrmNativeMultiHeadRequestBuildStatus::MismatchedMirrorSize
    );
    assert!(build.request.is_none());
}

#[test]
fn differing_sizes_are_allowed_when_heads_do_not_share_a_framebuffer() {
    // The same-size rule is a mirror-group rule, not a desktop-wide one.
    let first = head(21, 41, 51, 61, scanout_size(1920, 1080));
    let second = head(22, 42, 52, 62, scanout_size(1280, 720));

    let build = build_native_multi_head_atomic_request(
        &[first, second],
        LibdrmNativeAtomicCommitRequestScope::Modeset,
    );

    assert_eq!(
        build.status,
        LibdrmNativeMultiHeadRequestBuildStatus::Built
    );
}

#[test]
fn heads_sharing_a_kms_object_fail_closed() {
    let size = scanout_size(1920, 1080);
    for (label, second) in [
        ("connector", head(21, 42, 52, 62, size)),
        ("crtc", head(22, 41, 52, 62, size)),
        ("plane", head(22, 42, 51, 62, size)),
    ] {
        let build = build_native_multi_head_atomic_request(
            &[head(21, 41, 51, 61, size), second],
            LibdrmNativeAtomicCommitRequestScope::Modeset,
        );
        assert_eq!(
            build.status,
            LibdrmNativeMultiHeadRequestBuildStatus::OverlappingObjects,
            "a shared {label} must be rejected"
        );
        assert!(build.request.is_none());
    }
}

#[test]
fn an_empty_head_set_is_rejected() {
    let build = build_native_multi_head_atomic_request(
        &[],
        LibdrmNativeAtomicCommitRequestScope::Modeset,
    );

    assert_eq!(build.status, LibdrmNativeMultiHeadRequestBuildStatus::NoHeads);
    assert!(build.request.is_none());
}

#[test]
fn an_invalid_size_on_any_head_rejects_the_whole_request() {
    let build = build_native_multi_head_atomic_request(
        &[
            head(21, 41, 51, 61, scanout_size(1920, 1080)),
            head(22, 42, 52, 62, scanout_size(0, 1080)),
        ],
        LibdrmNativeAtomicCommitRequestScope::Modeset,
    );

    assert_eq!(
        build.status,
        LibdrmNativeMultiHeadRequestBuildStatus::InvalidSize
    );
    assert!(build.request.is_none());
}

#[test]
fn a_modeset_requires_a_mode_blob_on_every_head() {
    let size = scanout_size(1920, 1080);
    let without_blob = LibdrmNativeAtomicHead::new(
        LibdrmNativePrimaryPlaneObjects::new_with_optional_mode_blob(
            drm::control::from_u32(22).expect("connector handle should be nonzero"),
            drm::control::from_u32(42).expect("crtc handle should be nonzero"),
            drm::control::from_u32(52).expect("plane handle should be nonzero"),
            drm::control::from_u32(62).expect("framebuffer handle should be nonzero"),
            None,
            size,
        ),
        primary_plane_properties(),
    );

    let modeset = build_native_multi_head_atomic_request(
        &[head(21, 41, 51, 61, size), without_blob],
        LibdrmNativeAtomicCommitRequestScope::Modeset,
    );
    assert_eq!(
        modeset.status,
        LibdrmNativeMultiHeadRequestBuildStatus::MissingModeBlob
    );

    // A page flip changes no mode, so it needs no blob.
    let page_flip = build_native_multi_head_atomic_request(
        &[head(21, 41, 51, 61, size), without_blob],
        LibdrmNativeAtomicCommitRequestScope::PageFlip,
    );
    assert_eq!(
        page_flip.status,
        LibdrmNativeMultiHeadRequestBuildStatus::Built
    );
}

#[test]
fn a_combined_request_can_be_tested_without_touching_hardware() {
    let build = build_native_multi_head_atomic_request(
        &[
            head(21, 41, 51, 61, scanout_size(1920, 1080)),
            head(22, 42, 52, 61, scanout_size(1920, 1080)),
        ],
        LibdrmNativeAtomicCommitRequestScope::Modeset,
    );
    let request = build
        .request
        .expect("a valid mirror group should build")
        .test_only()
        .allow_modeset();

    let flags = request.reduced_flags();
    assert!(flags.test_only);
    assert!(flags.allow_modeset);
}

fn timing(width: u32, height: u32, refresh_millihz: u32) -> LibdrmNativeOutputTiming {
    LibdrmNativeOutputTiming::new(width, height, refresh_millihz)
}

#[test]
fn a_planned_timing_resolves_to_its_position_in_the_connector_mode_list() {
    let modes = [
        timing(3840, 2160, 60_000),
        timing(2560, 1440, 144_000),
        timing(1920, 1080, 60_000),
    ];

    let resolution = resolve_native_output_mode_index(&modes, timing(2560, 1440, 144_000));

    assert_eq!(
        resolution.status,
        LibdrmNativeModeResolutionStatus::Resolved
    );
    assert_eq!(resolution.index, Some(1));
}

#[test]
fn a_timing_the_connector_does_not_offer_fails_closed() {
    let modes = [timing(1920, 1080, 60_000)];

    // Right resolution, wrong refresh: still not a mode this connector has.
    let resolution = resolve_native_output_mode_index(&modes, timing(1920, 1080, 75_000));

    assert_eq!(
        resolution.status,
        LibdrmNativeModeResolutionStatus::UnknownTiming
    );
    assert!(resolution.index.is_none());
}

#[test]
fn an_invalid_request_is_rejected_before_matching() {
    let modes = [timing(1920, 1080, 60_000)];

    for requested in [
        timing(0, 1080, 60_000),
        timing(1920, 0, 60_000),
        timing(1920, 1080, 0),
    ] {
        let resolution = resolve_native_output_mode_index(&modes, requested);
        assert_eq!(
            resolution.status,
            LibdrmNativeModeResolutionStatus::InvalidTiming
        );
    }
}

#[test]
fn resolution_picks_the_first_of_several_modes_sharing_one_reduced_timing() {
    // Reducing a DRM mode to width, height, and integer refresh is lossy, so two
    // distinct modes can collide here. The capability reader dedupes and keeps the
    // first, so resolution must make the same choice or advertisement and commit
    // would disagree about which mode a timing names.
    let modes = [
        timing(1920, 1080, 60_000),
        timing(1920, 1080, 60_000),
        timing(1280, 720, 60_000),
    ];

    let resolution = resolve_native_output_mode_index(&modes, timing(1920, 1080, 60_000));

    assert_eq!(resolution.index, Some(0));
}

#[test]
fn an_invalid_advertised_mode_is_skipped_rather_than_matched() {
    // A connector can report a degenerate mode. It must never be selected, even
    // when a caller asks for something that would compare equal after reduction.
    let modes = [timing(0, 0, 0), timing(1920, 1080, 60_000)];

    let resolution = resolve_native_output_mode_index(&modes, timing(1920, 1080, 60_000));

    assert_eq!(resolution.index, Some(1));
}

#[test]
fn an_empty_mode_list_resolves_nothing() {
    let resolution = resolve_native_output_mode_index(&[], timing(1920, 1080, 60_000));

    assert_eq!(
        resolution.status,
        LibdrmNativeModeResolutionStatus::UnknownTiming
    );
}

/// Records the flags of every submission, so a test can prove the validation pass
/// really carried TEST_ONLY and that nothing else reached the device.
#[derive(Default)]
struct RecordingCommitDevice {
    submissions: std::cell::RefCell<Vec<drm::control::AtomicCommitFlags>>,
    results: std::cell::RefCell<Vec<Result<(), std::io::ErrorKind>>>,
}

impl RecordingCommitDevice {
    fn with_results(results: Vec<Result<(), std::io::ErrorKind>>) -> Self {
        Self {
            submissions: std::cell::RefCell::new(Vec::new()),
            results: std::cell::RefCell::new(results),
        }
    }
}

impl LibdrmNativeAtomicCommitDevice for RecordingCommitDevice {
    fn submit_atomic_commit(
        &self,
        flags: drm::control::AtomicCommitFlags,
        _request: drm::control::atomic::AtomicModeReq,
    ) -> io::Result<()> {
        self.submissions.borrow_mut().push(flags);
        let mut results = self.results.borrow_mut();
        if results.is_empty() {
            return Ok(());
        }
        match results.remove(0) {
            Ok(()) => Ok(()),
            Err(kind) => Err(io::Error::new(kind, "synthetic commit failure")),
        }
    }
}

fn submitted_flags(
    committer: &NativeLibdrmAtomicScanoutCommitter<RecordingCommitDevice>,
) -> Vec<drm::control::AtomicCommitFlags> {
    committer.device().submissions.borrow().clone()
}

#[test]
fn validating_a_topology_sets_test_only_and_allows_modeset() {
    let mut committer = NativeLibdrmAtomicScanoutCommitter::new(RecordingCommitDevice::default());
    let heads = [
        head(21, 41, 51, 61, scanout_size(1920, 1080)),
        head(22, 42, 52, 61, scanout_size(1920, 1080)),
    ];

    let outcome = submit_native_multi_head_topology(
        &mut committer,
        &heads,
        NativeTopologySubmitIntent::Validate,
    );

    assert_eq!(outcome, NativeTopologySubmitOutcome::Accepted);
    let flags = submitted_flags(&committer);
    assert_eq!(flags.len(), 1);
    // The safety property: validation must not be able to change anything.
    assert!(flags[0].contains(drm::control::AtomicCommitFlags::TEST_ONLY));
    assert!(flags[0].contains(drm::control::AtomicCommitFlags::ALLOW_MODESET));
}

#[test]
fn activating_a_topology_does_not_set_test_only() {
    let mut committer = NativeLibdrmAtomicScanoutCommitter::new(RecordingCommitDevice::default());
    let heads = [head(21, 41, 51, 61, scanout_size(1920, 1080))];

    let outcome = submit_native_multi_head_topology(
        &mut committer,
        &heads,
        NativeTopologySubmitIntent::Activate,
    );

    assert_eq!(outcome, NativeTopologySubmitOutcome::Accepted);
    let flags = submitted_flags(&committer);
    assert!(!flags[0].contains(drm::control::AtomicCommitFlags::TEST_ONLY));
    assert!(flags[0].contains(drm::control::AtomicCommitFlags::ALLOW_MODESET));
}

#[test]
fn an_unbuildable_topology_never_reaches_the_device() {
    let mut committer = NativeLibdrmAtomicScanoutCommitter::new(RecordingCommitDevice::default());
    // One framebuffer, two sizes: a mirror group that cannot be expressed.
    let heads = [
        head(21, 41, 51, 61, scanout_size(1920, 1080)),
        head(22, 42, 52, 61, scanout_size(1280, 720)),
    ];

    let outcome = submit_native_multi_head_topology(
        &mut committer,
        &heads,
        NativeTopologySubmitIntent::Activate,
    );

    assert_eq!(
        outcome,
        NativeTopologySubmitOutcome::Unbuildable(
            LibdrmNativeMultiHeadRequestBuildStatus::MismatchedMirrorSize
        )
    );
    assert!(submitted_flags(&committer).is_empty());
}

#[test]
fn a_kernel_refusal_and_a_busy_device_are_reported_apart() {
    let heads = [head(21, 41, 51, 61, scanout_size(1920, 1080))];

    let mut refusing = NativeLibdrmAtomicScanoutCommitter::new(RecordingCommitDevice::with_results(
        vec![Err(io::ErrorKind::InvalidInput)],
    ));
    assert_eq!(
        submit_native_multi_head_topology(
            &mut refusing,
            &heads,
            NativeTopologySubmitIntent::Validate
        ),
        NativeTopologySubmitOutcome::Rejected
    );

    let mut busy = NativeLibdrmAtomicScanoutCommitter::new(RecordingCommitDevice::with_results(
        vec![Err(io::ErrorKind::WouldBlock)],
    ));
    assert_eq!(
        submit_native_multi_head_topology(&mut busy, &heads, NativeTopologySubmitIntent::Validate),
        NativeTopologySubmitOutcome::Busy
    );
}
