fn native_output_capability() -> LibdrmNativeOutputCapability {
    let mode = LibdrmNativeOutputTiming::new(2560, 1440, 120_000);
    LibdrmNativeOutputCapability::new(
        OutputId::from_raw(1),
        10,
        "DP-1",
        [mode],
        Some(mode),
        mode,
        LibdrmNativeVrrPropertyDiscoveryStatus::Discovered,
    )
    .unwrap()
}

#[test]
fn native_output_capability_requires_consistent_bounded_identity_and_modes() {
    let capability = native_output_capability();
    assert_eq!(capability.output(), OutputId::from_raw(1));
    assert_eq!(capability.connector_id(), 10);
    assert_eq!(capability.connector_name(), "DP-1");
    assert_eq!(capability.modes(), [capability.selected_mode()]);
    assert_eq!(
        capability.preferred_mode(),
        Some(capability.selected_mode())
    );

    let mode = capability.selected_mode();
    assert!(
        LibdrmNativeOutputCapability::new(
            OutputId::from_raw(1),
            10,
            "../card0",
            [mode],
            Some(mode),
            mode,
            LibdrmNativeVrrPropertyDiscoveryStatus::Unsupported,
        )
        .is_err()
    );
    assert!(
        LibdrmNativeOutputCapability::new(
            OutputId::from_raw(1),
            10,
            "DP-1",
            [mode],
            Some(mode),
            LibdrmNativeOutputTiming::new(1920, 1080, 60_000),
            LibdrmNativeVrrPropertyDiscoveryStatus::Unsupported,
        )
        .is_err()
    );
}

#[test]
fn native_output_vrr_requires_complete_properties() {
    assert!(native_output_capability().vrr_configurable());
    let mode = LibdrmNativeOutputTiming::new(2560, 1440, 120_000);
    let capability = LibdrmNativeOutputCapability::new(
        OutputId::from_raw(1),
        10,
        "DP-1",
        [mode],
        Some(mode),
        mode,
        LibdrmNativeVrrPropertyDiscoveryStatus::MissingEnableProperty,
    )
    .unwrap();
    assert!(!capability.vrr_configurable());
    assert_eq!(
        capability.vrr_status(),
        LibdrmNativeVrrPropertyDiscoveryStatus::MissingEnableProperty
    );
}
