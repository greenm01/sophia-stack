use sophia_engine::*;
use sophia_protocol::*;
#[path = "../../sophia-protocol/tests/support/reference_fixture.rs"]
mod fixture;
fn event(keycode: u32, pressed: bool) -> InputEventPacket {
    InputEventPacket {
        serial: 1,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: 1,
        kind: InputEventKind::Key { keycode, pressed },
        global_position: None,
        target_surface: None,
        local_position: None,
    }
}
#[test]
fn pages_cover_every_binding_and_keep_geometry_without_actions() {
    let catalog = fixture::catalog(256);
    let mut c = fixture::candidate(256);
    let bounds = Rect {
        x: 100,
        y: 20,
        width: 1920,
        height: 1080,
    };
    let measure = |text: &str, size: u16| {
        (
            (text.len() as i32 * i32::from(size) * 3 + 4) / 5,
            (i32::from(size) * 6 + 4) / 5,
        )
    };
    let (first, _, pages) = reference_sheet_projection(&c, &catalog, 1, bounds, measure).unwrap();
    assert!(pages > 1);
    assert!(first.targets.is_empty());
    let mut seen = std::collections::BTreeSet::new();
    for page in 0..pages {
        c.page = page;
        let (projection, actual, all) =
            reference_sheet_projection(&c, &catalog, u64::from(page) + 1, bounds, measure).unwrap();
        assert_eq!((actual, all), (page, pages));
        assert_eq!(projection.geometry, first.geometry);
        for command in projection.commands {
            if let CompositorDisplayCommand::Text(t) = command
                && let CompositorNodeId::DescriptorOverlay {
                    slot,
                    role: DescriptorOverlayNodeRole::Label,
                    ..
                } = t.node
                && slot != u16::MAX
            {
                assert!(seen.insert(slot));
            }
        }
    }
    assert_eq!(seen.len(), 256);
    c.catalog_generation += 1;
    assert!(reference_sheet_projection(&c, &catalog, 1, bounds, measure).is_err());
}
#[test]
fn only_presented_sheet_captures_and_consumes_its_own_releases() {
    let mut capture = ReferenceSheetCapture::default();
    assert_eq!(capture.route(&event(30, true)), (false, None));
    capture.present(Some((OutputId::from_raw(1), 12)));
    assert_eq!(capture.route(&event(42, false)), (false, None));
    assert_eq!(capture.route(&event(30, false)), (false, None));
    let (consumed, op) = capture.route(&event(109, true));
    assert!(consumed);
    assert_eq!(op.unwrap().2, ShellReferenceOperation::Next);
    assert_eq!(capture.route(&event(109, true)), (true, None));
    assert_eq!(capture.route(&event(109, false)), (true, None));
    assert_eq!(
        capture.route(&event(31, true)).1.unwrap().2,
        ShellReferenceOperation::Dismiss
    );
    capture.present(None);
    assert_eq!(capture.route(&event(31, false)), (true, None));
    assert_eq!(capture.route(&event(32, true)), (false, None));
}
#[test]
fn wheel_accumulates_high_resolution_steps_and_cannot_page_after_dismissal() {
    let mut capture = ReferenceSheetCapture::default();
    capture.present(Some((OutputId::from_raw(1), 1)));
    let mut wheel = event(1, true);
    wheel.kind = InputEventKind::PointerAxis {
        horizontal_v120: 0,
        vertical_v120: 60,
    };
    assert_eq!(capture.route(&wheel), (true, None));
    assert_eq!(
        capture.route(&wheel).1.unwrap().2,
        ShellReferenceOperation::Next
    );
    capture.route(&event(30, true));
    assert_eq!(capture.route(&wheel), (true, None));
}

#[test]
fn maximum_labels_cannot_push_binding_columns_off_screen() {
    let catalog = fixture::catalog(256);
    let mut candidate = fixture::candidate(256);
    for row in &mut candidate.entries {
        row.label = "a".repeat(128);
    }
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut page = 0;
    loop {
        candidate.page = page;
        let (projection, _, pages) = reference_sheet_projection(
            &candidate,
            &catalog,
            u64::from(page) + 1,
            bounds,
            |s, n| {
                (
                    s.len() as i32 * i32::from(n) * 3 / 5,
                    i32::from(n) * 6 / 5 + 1,
                )
            },
        )
        .unwrap();
        for c in projection.commands {
            if let CompositorDisplayCommand::Text(t) = c
                && let CompositorNodeId::DescriptorOverlay {
                    slot,
                    role: DescriptorOverlayNodeRole::Label,
                    ..
                } = t.node
                && slot != u16::MAX
            {
                assert!(
                    t.geometry.x >= projection.geometry.x
                        && t.geometry.x + t.geometry.width
                            <= projection.geometry.x + projection.geometry.width
                );
                seen.insert(slot);
            }
        }
        page += 1;
        if page == pages {
            break;
        }
    }
    assert_eq!(seen.len(), 256);
}
