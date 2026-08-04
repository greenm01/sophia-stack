#[cfg(feature = "gbm-probe")]
mod legacy_cursor {
    use std::io;

    use sophia_backend_live::{
        ClassicHardwareCursorUpdate, LEGACY_HARDWARE_CURSOR_FALLBACK_EDGE,
        LegacyHardwareCursorAdmission, LegacyHardwareCursorController, LegacyHardwareCursorDevice,
        LegacyHardwareCursorTarget, legacy_hardware_cursor_admission,
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
