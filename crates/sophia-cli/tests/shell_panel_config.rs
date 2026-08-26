//! The panel ceiling has two owners, and this is where they are made to agree.
//!
//! `sophia-config` depends on nothing else in the stack, so it cannot read the
//! wire's own maximum and carries its own copy. This crate sees both, so a
//! drift between them fails here rather than in a session that accepts a
//! profile the wire will refuse.

#[test]
fn the_profile_ceiling_is_the_wire_ceiling() {
    assert_eq!(
        sophia_config::SHELL_PANEL_MAX_THICKNESS_PX,
        sophia_protocol::SOPHIA_SHELL_MAX_RESERVATION_THICKNESS_PX,
        "a profile may promise exactly the panel the wire can carry",
    );
}

#[test]
fn the_compiled_profile_asks_for_a_panel_the_wire_accepts() {
    let profile =
        sophia_config::load_desktop_profile(None, sophia_config::ConfigGeneration::INITIAL)
            .expect("the compiled profile loads");
    let thickness = sophia_config::desktop_profile_shell_panel_thickness(&profile)
        .expect("the compiled profile reserves a panel");

    // The claim the session will make on this profile, encoded and decoded as
    // the shell would send it, so the compiled default cannot be a value the
    // candidate codec rejects.
    let candidate = sophia_protocol::ShellV1Candidate {
        connection_epoch: 1,
        snapshot_generation: 1,
        candidate_generation: 1,
        output: sophia_protocol::OutputId::from_raw(1),
        visible: true,
        selected_slot: Some(1),
        reservation: Some(sophia_protocol::ShellV1WorkAreaReservation {
            edge: sophia_protocol::ShellV1ReservationEdge::Bottom,
            thickness_px: thickness,
        }),
        entries: vec![sophia_protocol::ShellV1CandidateEntry {
            slot: 1,
            generation: 1,
        }],
    };
    let frame = sophia_protocol::encode_shell_v1_candidate_frame(
        sophia_protocol::TransactionId::from_raw(1),
        &candidate,
    )
    .expect("the compiled panel encodes");
    let (_, decoded) =
        sophia_protocol::decode_shell_v1_candidate_frame(&frame).expect("and decodes back");
    assert_eq!(decoded.reservation, candidate.reservation);
}
