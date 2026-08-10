#![cfg(feature = "atomic-scanout-live")]

use sophia_backend_live::{
    LibdrmNativeOutputCapability, LibdrmNativeOutputTiming, LibdrmNativeVrrPropertyDiscoveryStatus,
};
use sophia_cli::desktop_output_topology::{
    NativeOutputTopologyProjectionError, project_native_output_topology,
};
use sophia_config::{
    ConfigDigest, ConfigGeneration, DesktopNamedOutputCandidate, DesktopOutputCandidate,
    DesktopOutputMode, DesktopOutputScale, DesktopOutputTransformSet, DesktopOutputVrrMode,
    reconcile_desktop_output_candidate,
};
use sophia_engine::HeadlessOutput;
use sophia_protocol::{OutputId, Size};

fn capability(
    output: u64,
    connector: &str,
    width: u32,
    height: u32,
    vrr: bool,
) -> LibdrmNativeOutputCapability {
    let selected = LibdrmNativeOutputTiming::new(width, height, 60_000);
    let alternate = LibdrmNativeOutputTiming::new(width, height, 120_000);
    LibdrmNativeOutputCapability::new(
        OutputId::from_raw(output),
        u32::try_from(output).unwrap(),
        connector,
        [selected, alternate],
        Some(alternate),
        selected,
        if vrr {
            LibdrmNativeVrrPropertyDiscoveryStatus::Discovered
        } else {
            LibdrmNativeVrrPropertyDiscoveryStatus::Unsupported
        },
    )
    .unwrap()
}

fn output(output: u64, width: i32, height: i32, scale: u32) -> HeadlessOutput {
    HeadlessOutput {
        id: OutputId::from_raw(output),
        size: Size { width, height },
        scale,
    }
}

#[test]
fn native_capabilities_project_in_engine_semantic_order() {
    let capabilities = [
        capability(1, "DP-1", 2560, 1440, true),
        capability(2, "DP-2", 1920, 1080, false),
    ];
    let outputs = [output(2, 1920, 1080, 2), output(1, 2560, 1440, 1)];

    let topology = project_native_output_topology(&capabilities, &outputs).unwrap();

    assert_eq!(topology.connectors.len(), 2);
    assert_eq!(topology.connectors[0].connector, "DP-2");
    assert_eq!(topology.connectors[0].current.position, (0, 0));
    assert_eq!(topology.connectors[0].current.scale_milli, 2_000);
    assert_eq!(topology.connectors[1].connector, "DP-1");
    assert_eq!(topology.connectors[1].current.position, (960, 0));
    assert!(topology.connectors[1].vrr_capable);
    assert_eq!(
        topology.connectors[1].transforms,
        DesktopOutputTransformSet::NORMAL
    );
    assert_eq!(
        topology.connectors[1].current.vrr,
        DesktopOutputVrrMode::Disabled
    );
}

#[test]
fn native_projection_rejects_cross_owner_inconsistency() {
    let capability = capability(1, "DP-1", 2560, 1440, true);
    assert_eq!(
        project_native_output_topology(&[capability.clone()], &[output(1, 1920, 1080, 1)]),
        Err(NativeOutputTopologyProjectionError::PixelSizeMismatch(1))
    );
    assert_eq!(
        project_native_output_topology(&[capability.clone()], &[output(2, 2560, 1440, 1)]),
        Err(NativeOutputTopologyProjectionError::MissingCapability(2))
    );
    assert_eq!(
        project_native_output_topology(
            &[capability.clone()],
            &[output(1, 2560, 1440, 1), output(1, 2560, 1440, 1),]
        ),
        Err(NativeOutputTopologyProjectionError::DuplicateOutput(1))
    );
    assert_eq!(
        project_native_output_topology(&[capability], &[output(1, 2560, 1440, 9)]),
        Err(NativeOutputTopologyProjectionError::ScaleUnsupported(1))
    );
}

#[test]
fn migrated_output_candidate_reconciles_against_native_projection() {
    let capabilities = [
        capability(1, "DP-1", 2560, 1440, true),
        capability(2, "DP-2", 1920, 1080, false),
    ];
    let outputs = [output(1, 2560, 1440, 1), output(2, 1920, 1080, 1)];
    let topology = project_native_output_topology(&capabilities, &outputs).unwrap();
    let candidate = DesktopOutputCandidate {
        generation: ConfigGeneration::INITIAL,
        digest: ConfigDigest::new([7; 32]),
        inherit_sophia: true,
        named: vec![
            DesktopNamedOutputCandidate {
                connector: "DP-1".to_owned(),
                mode: Some(DesktopOutputMode::Exact {
                    width: 2560,
                    height: 1440,
                    refresh_millihz: 120_000,
                }),
                scale: Some(DesktopOutputScale::Automatic),
                position: Some((0, 0)),
                transform: None,
                enabled: Some(true),
                focus_at_startup: Some(true),
                vrr: Some(DesktopOutputVrrMode::Automatic),
            },
            DesktopNamedOutputCandidate {
                connector: "DP-2".to_owned(),
                mode: Some(DesktopOutputMode::Exact {
                    width: 1920,
                    height: 1080,
                    refresh_millihz: 60_000,
                }),
                scale: Some(DesktopOutputScale::Automatic),
                position: Some((2560, 0)),
                transform: None,
                enabled: Some(true),
                focus_at_startup: None,
                vrr: None,
            },
        ],
    };

    let reconciled = reconcile_desktop_output_candidate(&candidate, &topology).unwrap();

    assert_eq!(reconciled.outputs.len(), 2);
    assert_eq!(reconciled.outputs[0].mode.refresh_millihz, 120_000);
    assert_eq!(reconciled.focused_connector.as_deref(), Some("DP-1"));
}
