use sophia_engine::{OutputPowerLevel, OutputPowerState, OutputPowerTransition};
use sophia_protocol::OutputId;

fn output(raw: u64) -> OutputId {
    OutputId::from_raw(raw)
}

fn admitted(outputs: &[u64]) -> OutputPowerState {
    let mut state = OutputPowerState::default();
    state.admit_outputs(outputs.iter().copied().map(output));
    state
}

#[test]
fn an_admitted_output_starts_lit() {
    let state = admitted(&[1, 2]);

    assert_eq!(state.level(output(1)), Some(OutputPowerLevel::On));
    assert_eq!(state.level(output(2)), Some(OutputPowerLevel::On));
    assert_eq!(state.scanning_out(), vec![output(1), output(2)]);
    assert!(!state.fully_dark());
}

#[test]
fn powering_an_output_down_leaves_it_in_the_desktop() {
    // The whole point of a separate authority. A dark output still has an id, still
    // holds its surfaces, and is still one of the outputs policy lays out on.
    // Enablement is what removes an output, and enablement is expressed elsewhere,
    // by omission from the complete snapshot.
    let mut state = admitted(&[1, 2]);

    assert_eq!(
        state.request(output(2), OutputPowerLevel::Off),
        OutputPowerTransition::Changed {
            from: OutputPowerLevel::On,
            to: OutputPowerLevel::Off,
        }
    );

    assert_eq!(state.len(), 2, "a dark output is still an output");
    assert_eq!(state.level(output(2)), Some(OutputPowerLevel::Off));
    assert_eq!(state.scanning_out(), vec![output(1)]);
    assert!(!state.fully_dark());
}

#[test]
fn a_repeated_request_is_not_a_transition() {
    // The backend is told only when something changed. A power request that repeats
    // the current level must not produce a commit, because a commit is how a dark
    // screen briefly relights.
    let mut state = admitted(&[1]);

    assert_eq!(
        state.request(output(1), OutputPowerLevel::Off),
        OutputPowerTransition::Changed {
            from: OutputPowerLevel::On,
            to: OutputPowerLevel::Off,
        }
    );
    assert_eq!(
        state.request(output(1), OutputPowerLevel::Off),
        OutputPowerTransition::Unchanged
    );
}

#[test]
fn a_topology_change_keeps_the_level_of_every_surviving_output() {
    // A mode change on one output must not relight another that the operator or an
    // idle policy powered down. Reconciling against the new set preserves what
    // survives without inventing state for what did not.
    let mut state = admitted(&[1, 2]);
    state.request(output(2), OutputPowerLevel::Off);

    state.admit_outputs([output(1), output(2), output(3)]);

    assert_eq!(state.level(output(1)), Some(OutputPowerLevel::On));
    assert_eq!(
        state.level(output(2)),
        Some(OutputPowerLevel::Off),
        "a topology change is not a reason to relight"
    );
    assert_eq!(
        state.level(output(3)),
        Some(OutputPowerLevel::On),
        "a newly admitted output arrives lit"
    );
}

#[test]
fn an_output_that_leaves_the_topology_keeps_no_power_state() {
    // Retaining a level for a departed output would apply it to whatever later
    // claimed that id, which is how a reconnected monitor comes back dark for no
    // reason anyone can trace.
    let mut state = admitted(&[1, 2]);
    state.request(output(2), OutputPowerLevel::Off);

    state.admit_outputs([output(1)]);
    assert_eq!(state.level(output(2)), None);
    assert_eq!(state.len(), 1);

    state.admit_outputs([output(1), output(2)]);
    assert_eq!(
        state.level(output(2)),
        Some(OutputPowerLevel::On),
        "a returning output is a new one"
    );
}

#[test]
fn an_unknown_output_is_refused_rather_than_recorded() {
    let mut state = admitted(&[1]);

    assert_eq!(
        state.request(output(9), OutputPowerLevel::Off),
        OutputPowerTransition::UnknownOutput
    );
    assert_eq!(state.level(output(9)), None);
    assert_eq!(state.len(), 1);
}

#[test]
fn a_fully_dark_desktop_is_still_a_desktop() {
    // fully_dark gates frame production, not teardown. An empty state is not dark;
    // it is a session with no outputs, which is a different condition entirely.
    let mut state = admitted(&[1, 2]);
    assert!(!OutputPowerState::default().fully_dark());

    state.request(output(1), OutputPowerLevel::Off);
    assert!(!state.fully_dark());

    state.request(output(2), OutputPowerLevel::Standby);
    assert!(state.fully_dark());
    assert!(state.scanning_out().is_empty());
    assert_eq!(state.len(), 2);
}

#[test]
fn only_the_lit_level_scans_out() {
    for level in [
        OutputPowerLevel::Standby,
        OutputPowerLevel::Suspend,
        OutputPowerLevel::Off,
    ] {
        assert!(!level.scans_out(), "{} must not scan out", level.label());
    }
    assert!(OutputPowerLevel::On.scans_out());
}
