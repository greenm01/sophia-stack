#![cfg(feature = "atomic-scanout-live")]

use std::cell::RefCell;
use std::collections::BTreeMap;

use sophia_backend_live::{
    LibdrmNativeOutputCapability, LibdrmNativeOutputTiming, LibdrmNativeVrrPropertyDiscoveryStatus,
};
use sophia_cli::desktop_output_heads::{
    NativeOutputComposedHead, NativeOutputHeadResolveError, NativeOutputHeadUnavailable,
    NativeOutputTopologyHardware, resolve_native_output_topology_heads,
};
use sophia_cli::desktop_output_topology::{
    NativeOutputActivationPlan, prepare_native_output_activation_plan,
    project_native_output_topology,
};
use sophia_config::{
    ConfigDigest, ConfigGeneration, DesktopNamedOutputCandidate, DesktopOutputCandidate,
    reconcile_desktop_output_candidate,
};
use sophia_engine::HeadlessOutput;
use sophia_protocol::{OutputId, Size};

/// One head as this test sees it. The real composer yields DRM handles; the
/// resolver never inspects a head, so a pair of numbers proves the same behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TestHead {
    output: u64,
    timing: LibdrmNativeOutputTiming,
}

#[derive(Debug, Default)]
struct FakeHardware {
    /// Outputs that fail, and how.
    failures: BTreeMap<u64, NativeOutputHeadUnavailable>,
    next_blob: RefCell<u64>,
    created: RefCell<Vec<u64>>,
    released: RefCell<Vec<u64>>,
}

impl FakeHardware {
    fn new() -> Self {
        Self {
            next_blob: RefCell::new(700),
            ..Self::default()
        }
    }

    fn failing(output: u64, cause: NativeOutputHeadUnavailable) -> Self {
        let mut hardware = Self::new();
        hardware.failures.insert(output, cause);
        hardware
    }

    fn live_blobs(&self) -> Vec<u64> {
        let released = self.released.borrow();
        self.created
            .borrow()
            .iter()
            .copied()
            .filter(|blob| !released.contains(blob))
            .collect()
    }
}

impl NativeOutputTopologyHardware for FakeHardware {
    type Head = TestHead;

    fn compose_head(
        &self,
        output: OutputId,
        timing: LibdrmNativeOutputTiming,
    ) -> Result<NativeOutputComposedHead<Self::Head>, NativeOutputHeadUnavailable> {
        if let Some(cause) = self.failures.get(&output.raw()) {
            return Err(*cause);
        }
        let mut next = self.next_blob.borrow_mut();
        let mode_blob = *next;
        *next += 1;
        self.created.borrow_mut().push(mode_blob);
        Ok(NativeOutputComposedHead {
            head: TestHead {
                output: output.raw(),
                timing,
            },
            mode_blob,
        })
    }

    fn release_mode_blob(&self, blob: u64) {
        self.released.borrow_mut().push(blob);
    }
}

fn timing() -> LibdrmNativeOutputTiming {
    LibdrmNativeOutputTiming::new(1920, 1080, 60_000)
}

fn capability(output: u64, connector: &str) -> LibdrmNativeOutputCapability {
    LibdrmNativeOutputCapability::new(
        OutputId::from_raw(output),
        u32::try_from(output).expect("test output ids are small"),
        connector,
        [timing()],
        Some(timing()),
        timing(),
        LibdrmNativeVrrPropertyDiscoveryStatus::Unsupported,
    )
    .expect("capability fixture is valid")
}

fn headless(output: u64) -> HeadlessOutput {
    HeadlessOutput {
        id: OutputId::from_raw(output),
        size: Size {
            width: 1920,
            height: 1080,
        },
        scale: 1,
    }
}

fn plan_for(
    outputs: &[u64],
) -> (
    NativeOutputActivationPlan,
    Vec<LibdrmNativeOutputCapability>,
) {
    plan_with_disabled(outputs, &[])
}

fn plan_with_disabled(
    outputs: &[u64],
    disabled: &[u64],
) -> (
    NativeOutputActivationPlan,
    Vec<LibdrmNativeOutputCapability>,
) {
    let capabilities = outputs
        .iter()
        .map(|output| capability(*output, &format!("DP-{output}")))
        .collect::<Vec<_>>();
    let headless = outputs.iter().copied().map(headless).collect::<Vec<_>>();
    let topology =
        project_native_output_topology(&capabilities, &headless).expect("topology projects");
    let candidate = DesktopOutputCandidate {
        generation: ConfigGeneration::from_raw(7),
        digest: ConfigDigest::new([9; 32]),
        inherit_sophia: true,
        named: disabled
            .iter()
            .map(|output| DesktopNamedOutputCandidate {
                connector: format!("DP-{output}"),
                mode: None,
                scale: None,
                position: None,
                transform: None,
                enabled: Some(false),
                focus_at_startup: None,
                vrr: None,
            })
            .collect(),
    };
    let reconciliation =
        reconcile_desktop_output_candidate(&candidate, &topology).expect("candidate reconciles");
    let plan = prepare_native_output_activation_plan(&capabilities, &topology, &reconciliation)
        .expect("plan prepares");
    (plan, capabilities)
}

#[test]
fn every_enabled_output_contributes_one_head() {
    let (plan, capabilities) = plan_for(&[1, 2]);
    let hardware = FakeHardware::new();

    let resolved = resolve_native_output_topology_heads(&plan, &capabilities, &hardware)
        .expect("a complete plan resolves");

    assert_eq!(resolved.len(), 2);
    assert_eq!(
        resolved.heads(),
        [
            TestHead {
                output: 1,
                timing: timing()
            },
            TestHead {
                output: 2,
                timing: timing()
            },
        ]
    );
    assert_eq!(hardware.live_blobs().len(), 2);
}

#[test]
fn dropping_a_resolution_releases_every_blob_it_created() {
    let (plan, capabilities) = plan_for(&[1, 2]);
    let hardware = FakeHardware::new();

    {
        let resolved = resolve_native_output_topology_heads(&plan, &capabilities, &hardware)
            .expect("a complete plan resolves");
        assert_eq!(resolved.len(), 2);
        assert!(hardware.released.borrow().is_empty());
    }

    // The blobs belong to the resolution, so leaving its scope returns them.
    assert!(hardware.live_blobs().is_empty());
    assert_eq!(hardware.released.borrow().len(), 2);
}

#[test]
fn one_unavailable_output_fails_the_whole_topology_and_leaks_nothing() {
    // The second output fails after the first has already created a blob. A
    // partial topology is a different desktop, so the plan fails closed -- and the
    // blob the first output created must not survive the failure.
    let (plan, capabilities) = plan_for(&[1, 2]);
    let hardware = FakeHardware::failing(2, NativeOutputHeadUnavailable::MissingProperties);

    let error = resolve_native_output_topology_heads(&plan, &capabilities, &hardware)
        .expect_err("an unavailable head fails the plan");

    assert_eq!(
        error,
        NativeOutputHeadResolveError::Unavailable {
            output: 2,
            cause: NativeOutputHeadUnavailable::MissingProperties,
        }
    );
    assert_eq!(hardware.created.borrow().len(), 1);
    assert!(
        hardware.live_blobs().is_empty(),
        "a rejected resolution releases what it created"
    );
}

#[test]
fn each_unavailable_cause_survives_into_the_error() {
    // The causes are kept apart because they send an operator somewhere different.
    for cause in [
        NativeOutputHeadUnavailable::MissingSelection,
        NativeOutputHeadUnavailable::MissingProperties,
        NativeOutputHeadUnavailable::UnknownTiming,
        NativeOutputHeadUnavailable::ModeBlobRefused,
    ] {
        let (plan, capabilities) = plan_for(&[1]);
        let hardware = FakeHardware::failing(1, cause);

        assert_eq!(
            resolve_native_output_topology_heads(&plan, &capabilities, &hardware)
                .expect_err("the only head is unavailable"),
            NativeOutputHeadResolveError::Unavailable { output: 1, cause }
        );
    }
}

#[test]
fn a_disabled_output_contributes_no_head() {
    // Disablement is expressed by omission, not by an inactive head. That is the
    // same rule the output logical-space contract states for policy snapshots.
    let (plan, capabilities) = plan_with_disabled(&[1, 2], &[2]);
    let hardware = FakeHardware::new();

    let resolved = resolve_native_output_topology_heads(&plan, &capabilities, &hardware)
        .expect("one enabled output is a topology");

    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved.heads()[0].output, 1);
    assert_eq!(hardware.created.borrow().len(), 1);
}

#[test]
fn a_candidate_that_enables_nothing_never_becomes_a_plan() {
    // The resolver carries a NoEnabledOutputs arm, but production never reaches it:
    // reconciliation refuses an all-disabled candidate first. Pinning that here
    // records where the invariant actually lives, so a later change that relaxes
    // reconciliation fails a test instead of quietly making the backstop load-bearing.
    let capabilities = [capability(1, "DP-1")];
    let topology =
        project_native_output_topology(&capabilities, &[headless(1)]).expect("topology projects");
    let candidate = DesktopOutputCandidate {
        generation: ConfigGeneration::from_raw(7),
        digest: ConfigDigest::new([9; 32]),
        inherit_sophia: true,
        named: vec![DesktopNamedOutputCandidate {
            connector: "DP-1".to_owned(),
            mode: None,
            scale: None,
            position: None,
            transform: None,
            enabled: Some(false),
            focus_at_startup: None,
            vrr: None,
        }],
    };

    assert!(
        reconcile_desktop_output_candidate(&candidate, &topology).is_err(),
        "a desktop with no enabled output is refused before a plan exists"
    );
}

#[test]
fn a_plan_whose_outputs_have_no_capability_resolves_nothing() {
    let (plan, _) = plan_for(&[1]);
    let hardware = FakeHardware::new();

    let error = resolve_native_output_topology_heads(&plan, &[], &hardware)
        .expect_err("an output with no capability cannot be composed");

    assert_eq!(error, NativeOutputHeadResolveError::MissingCapability(1));
    assert!(
        hardware.created.borrow().is_empty(),
        "a missing capability is caught before any kernel resource exists"
    );
}
