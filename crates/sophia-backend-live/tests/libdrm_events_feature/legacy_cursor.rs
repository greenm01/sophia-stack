#[cfg(feature = "gbm-probe")]
mod legacy_cursor {
    use std::io;

    use sophia_backend_live::{
        ClassicHardwareCursorUpdate, HardwareCursorPath, LEGACY_HARDWARE_CURSOR_FALLBACK_EDGE,
        LegacyHardwareCursorAdmission, LegacyHardwareCursorController, LegacyHardwareCursorDevice,
        LegacyHardwareCursorTarget, hardware_cursor_admission, legacy_hardware_cursor_admission,
        resolve_legacy_hardware_cursor_dimensions,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Call {
        Hide(u32),
        Install(u32),
        Move(u32, i32, i32),
    }

    #[derive(Debug, Default)]
    struct FakeCursorDevice {
        calls: Vec<Call>,
        fail_at: Option<usize>,
    }

    impl FakeCursorDevice {
        fn record(&mut self, call: Call) -> io::Result<()> {
            self.calls.push(call);
            if self.fail_at == Some(self.calls.len()) {
                Err(io::Error::other("injected cursor ioctl failure"))
            } else {
                Ok(())
            }
        }
    }

    impl LegacyHardwareCursorDevice for FakeCursorDevice {
        type Crtc = u32;

        fn hide_cursor(&mut self, crtc: Self::Crtc) -> io::Result<()> {
            self.record(Call::Hide(crtc))
        }

        fn install_cursor(&mut self, crtc: Self::Crtc) -> io::Result<()> {
            self.record(Call::Install(crtc))
        }

        fn move_cursor(&mut self, crtc: Self::Crtc, x: i32, y: i32) -> io::Result<()> {
            self.record(Call::Move(crtc, x, y))
        }
    }

    fn target(crtc: u32, x: i32, y: i32) -> LegacyHardwareCursorTarget<u32> {
        LegacyHardwareCursorTarget { crtc, x, y }
    }

    #[test]
    fn cursor_dimensions_prefer_valid_driver_caps_and_fallback_per_axis() {
        assert_eq!(
            resolve_legacy_hardware_cursor_dimensions(Some(256), Some(128)),
            sophia_backend_live::LegacyHardwareCursorDimensions {
                width: 256,
                height: 128,
            }
        );
        assert_eq!(
            resolve_legacy_hardware_cursor_dimensions(Some(0), None),
            sophia_backend_live::LegacyHardwareCursorDimensions {
                width: LEGACY_HARDWARE_CURSOR_FALLBACK_EDGE,
                height: LEGACY_HARDWARE_CURSOR_FALLBACK_EDGE,
            }
        );
        assert_eq!(
            resolve_legacy_hardware_cursor_dimensions(Some(u64::MAX), Some(32)),
            sophia_backend_live::LegacyHardwareCursorDimensions {
                width: LEGACY_HARDWARE_CURSOR_FALLBACK_EDGE,
                height: 32,
            }
        );
    }

    #[test]
    fn legacy_cursor_orders_initialization_moves_crossing_and_hide() {
        let mut device = FakeCursorDevice::default();
        let mut controller = LegacyHardwareCursorController::default();
        controller.initialize(&mut device, &[7, 9]).unwrap();
        assert_eq!(
            controller
                .update(&mut device, Some(target(7, 10, 11)))
                .unwrap(),
            ClassicHardwareCursorUpdate::Visible
        );
        assert_eq!(
            controller
                .update(&mut device, Some(target(7, 12, 13)))
                .unwrap(),
            ClassicHardwareCursorUpdate::Visible
        );
        assert_eq!(
            controller
                .update(&mut device, Some(target(9, 2, 3)))
                .unwrap(),
            ClassicHardwareCursorUpdate::Visible
        );
        assert_eq!(
            controller.update(&mut device, None).unwrap(),
            ClassicHardwareCursorUpdate::Hidden
        );
        assert_eq!(
            device.calls,
            vec![
                Call::Hide(7),
                Call::Hide(9),
                Call::Install(7),
                Call::Move(7, 10, 11),
                Call::Move(7, 12, 13),
                Call::Hide(7),
                Call::Install(9),
                Call::Move(9, 2, 3),
                Call::Hide(9),
            ]
        );
        assert_eq!(controller.active_crtc(), None);
    }

    #[test]
    fn legacy_cursor_records_only_successful_ioctl_state_transitions() {
        let mut device = FakeCursorDevice::default();
        let mut controller = LegacyHardwareCursorController::default();
        controller.initialize(&mut device, &[7, 9]).unwrap();
        controller
            .update(&mut device, Some(target(7, 10, 11)))
            .unwrap();

        device.fail_at = Some(device.calls.len() + 1);
        assert!(
            controller
                .update(&mut device, Some(target(9, 2, 3)))
                .is_err()
        );
        assert_eq!(controller.active_crtc(), Some(7));

        device.fail_at = Some(device.calls.len() + 2);
        assert!(
            controller
                .update(&mut device, Some(target(9, 2, 3)))
                .is_err()
        );
        assert_eq!(controller.active_crtc(), None);

        device.fail_at = Some(device.calls.len() + 2);
        assert!(
            controller
                .update(&mut device, Some(target(9, 2, 3)))
                .is_err()
        );
        assert_eq!(controller.active_crtc(), Some(9));
    }

    #[test]
    fn mirrored_cursor_remains_active_on_every_target_crtc() {
        let mut device = FakeCursorDevice::default();
        let mut controller = LegacyHardwareCursorController::default();
        controller.initialize(&mut device, &[7, 9]).unwrap();

        assert_eq!(
            controller
                .update_many(&mut device, &[target(7, 10, 11), target(9, 8, 9)])
                .unwrap(),
            ClassicHardwareCursorUpdate::Visible
        );
        assert_eq!(controller.active_crtcs(), &[7, 9]);
        assert_eq!(
            controller
                .update_many(&mut device, &[target(7, 12, 13), target(9, 10, 11)])
                .unwrap(),
            ClassicHardwareCursorUpdate::Visible
        );
        assert_eq!(controller.active_crtcs(), &[7, 9]);
        assert_eq!(
            &device.calls[2..],
            &[
                Call::Install(7),
                Call::Move(7, 10, 11),
                Call::Install(9),
                Call::Move(9, 8, 9),
                Call::Move(7, 12, 13),
                Call::Move(9, 10, 11),
            ]
        );
    }

    #[test]
    fn failed_initialization_is_retryable_and_never_marks_ready() {
        let mut device = FakeCursorDevice {
            fail_at: Some(2),
            ..FakeCursorDevice::default()
        };
        let mut controller = LegacyHardwareCursorController::default();
        assert!(controller.initialize(&mut device, &[7, 9]).is_err());
        assert!(!controller.is_initialized());
        assert!(
            controller
                .update(&mut device, Some(target(7, 1, 1)))
                .is_err()
        );

        device.fail_at = None;
        controller.initialize(&mut device, &[7, 9]).unwrap();
        assert!(controller.is_initialized());
    }

    #[test]
    fn primary_flip_only_defers_one_time_legacy_cursor_initialization() {
        assert_eq!(
            legacy_hardware_cursor_admission(false, true),
            LegacyHardwareCursorAdmission::DeferredInitialization
        );
        assert_eq!(
            legacy_hardware_cursor_admission(false, false),
            LegacyHardwareCursorAdmission::InitializeThenUpdate
        );
        assert_eq!(
            legacy_hardware_cursor_admission(true, true),
            LegacyHardwareCursorAdmission::Update
        );
    }

    /// The one row the two paths answer differently.
    ///
    /// An ioctl moves a cursor whenever it likes; archive `0004` counted
    /// fifteen updates issued while a page flip was outstanding, with no
    /// failures. The kernel serializes atomic commits per CRTC, so the same
    /// move has to wait for the commit in flight -- which is the whole
    /// substance of the transaction-owner row, reduced to one case of a pure
    /// function.
    #[test]
    fn only_the_atomic_path_defers_a_cursor_update_behind_a_flip() {
        assert_eq!(
            hardware_cursor_admission(HardwareCursorPath::AtomicPlane, true, true),
            LegacyHardwareCursorAdmission::DeferredUpdate
        );
        assert_eq!(
            hardware_cursor_admission(HardwareCursorPath::LegacyIoctl, true, true),
            LegacyHardwareCursorAdmission::Update,
            "the legacy path is what archive 0004 proved and does not change"
        );
    }

    /// Everything except that one row is shared, including the rule that
    /// initialization -- which touches every CRTC -- waits for a quiet CRTC
    /// on either path.
    #[test]
    fn both_paths_agree_on_every_other_case() {
        for path in [
            HardwareCursorPath::LegacyIoctl,
            HardwareCursorPath::AtomicPlane,
        ] {
            assert_eq!(
                hardware_cursor_admission(path, false, true),
                LegacyHardwareCursorAdmission::DeferredInitialization
            );
            assert_eq!(
                hardware_cursor_admission(path, false, false),
                LegacyHardwareCursorAdmission::InitializeThenUpdate
            );
            assert_eq!(
                hardware_cursor_admission(path, true, false),
                LegacyHardwareCursorAdmission::Update,
                "a quiet CRTC takes a cursor update on either path"
            );
        }
    }

    #[test]
    fn teardown_hides_the_active_cursor_before_buffer_release() {
        let mut device = FakeCursorDevice::default();
        let mut controller = LegacyHardwareCursorController::default();
        controller.initialize(&mut device, &[7]).unwrap();
        controller
            .update(&mut device, Some(target(7, 4, 5)))
            .unwrap();

        controller.hide_for_teardown(&mut device).unwrap();

        assert_eq!(controller.active_crtc(), None);
        assert_eq!(device.calls.last(), Some(&Call::Hide(7)));
    }
}
