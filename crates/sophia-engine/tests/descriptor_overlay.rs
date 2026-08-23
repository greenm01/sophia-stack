use sophia_engine::*;
use sophia_protocol::*;

const OUTPUT: OutputId = OutputId::from_raw(1);
const FIRST: SurfaceId = SurfaceId::new(10, 1);
const SECOND: SurfaceId = SurfaceId::new(11, 1);

fn descriptor(
    surface: SurfaceId,
    generation: u64,
    label: Option<&str>,
    trust_level: TrustLevel,
    attention: AttentionState,
) -> ChromeDescriptor {
    ChromeDescriptor {
        surface,
        label: label.map(|text| DisplayLabel {
            text: text.to_owned(),
            redacted: true,
        }),
        icon: Some(IconTokenId::from_raw(u64::from(surface.index()) + 1)),
        trust_level,
        attention,
        generation,
    }
}

fn action(slot: u16, generation: u64, token: u64) -> ToplevelActionCapabilityRef {
    ToplevelActionCapabilityRef {
        token,
        issuer_epoch: 3,
        issuer_revocation_epoch: 4,
        recipient_epoch: 5,
        target_slot: slot,
        target_generation: generation,
    }
}

fn candidate(selected_slot: u16, generation: u64) -> DescriptorOverlayCandidate {
    DescriptorOverlayCandidate {
        projection: 7,
        generation,
        output: OUTPUT,
        broker_epoch: 3,
        broker_revocation_epoch: 4,
        shell_session_epoch: 5,
        selected_slot: Some(selected_slot),
        entries: vec![
            DescriptorOverlayEntry {
                slot: 1,
                surface: FIRST,
                descriptor_generation: 8,
                action: action(1, 8, 101),
            },
            DescriptorOverlayEntry {
                slot: 2,
                surface: SECOND,
                descriptor_generation: 9,
                action: action(2, 9, 102),
            },
        ],
    }
}

fn table() -> ChromeDescriptorTable {
    let mut table = ChromeDescriptorTable::default();
    table.upsert(descriptor(
        FIRST,
        8,
        Some("Browser"),
        TrustLevel::Isolated,
        AttentionState::Critical,
    ));
    table.upsert(descriptor(
        SECOND,
        9,
        None,
        TrustLevel::Trusted,
        AttentionState::None,
    ));
    table
}

fn bounds() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: 1_920,
        height: 1_080,
    }
}

#[test]
fn sanitized_descriptors_become_one_bounded_title_only_projection() {
    let projection = descriptor_overlay_projection(&candidate(1, 10), &table(), bounds()).unwrap();

    assert_eq!(projection.geometry.width, DESCRIPTOR_OVERLAY_MAX_WIDTH);
    assert_eq!(projection.geometry.height, 80);
    assert_eq!(projection.targets.len(), 2);
    assert_eq!(projection.targets[0].action.token, 101);
    assert_eq!(projection.targets[0].id.authority_session_epoch, 5);
    assert_eq!(projection.targets[0].geometry.height, 32);
    assert_eq!(
        projection.targets[1].geometry.y - projection.targets[0].geometry.y,
        32
    );

    let labels = projection
        .commands
        .iter()
        .filter_map(|command| match command {
            CompositorDisplayCommand::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(labels, ["Browser", "Window"]);
    assert!(projection.commands.iter().any(|command| matches!(
        command,
        CompositorDisplayCommand::Rect(CompositorRect {
            node: CompositorNodeId::DescriptorOverlay {
                slot: 1,
                role: DescriptorOverlayNodeRole::Attention,
                ..
            },
            ..
        })
    )));
}

#[test]
fn projection_rejects_stale_duplicate_excessive_and_misscoped_candidates() {
    let descriptors = table();
    let mut stale = candidate(1, 10);
    stale.entries[0].descriptor_generation += 1;
    stale.entries[0].action.target_generation += 1;
    assert_eq!(
        descriptor_overlay_projection(&stale, &descriptors, bounds()),
        Err(DescriptorOverlayError::StaleDescriptor)
    );

    let mut duplicate = candidate(1, 10);
    duplicate.entries[1].slot = 1;
    duplicate.entries[1].action.target_slot = 1;
    assert_eq!(
        descriptor_overlay_projection(&duplicate, &descriptors, bounds()),
        Err(DescriptorOverlayError::DuplicateEntry)
    );

    let mut wrong_epoch = candidate(1, 10);
    wrong_epoch.entries[0].action.recipient_epoch += 1;
    assert_eq!(
        descriptor_overlay_projection(&wrong_epoch, &descriptors, bounds()),
        Err(DescriptorOverlayError::InvalidAction)
    );

    let mut invalid_descriptors = descriptors.clone();
    invalid_descriptors.upsert(descriptor(
        FIRST,
        8,
        Some("bad\nlabel"),
        TrustLevel::Isolated,
        AttentionState::None,
    ));
    assert_eq!(
        descriptor_overlay_projection(&candidate(1, 10), &invalid_descriptors, bounds()),
        Err(DescriptorOverlayError::InvalidDescriptor)
    );

    let mut excessive = candidate(1, 10);
    excessive.entries = (1..=MAX_DESCRIPTOR_OVERLAY_ENTRIES + 1)
        .map(|index| DescriptorOverlayEntry {
            slot: u16::try_from(index).unwrap(),
            surface: SurfaceId::new(u32::try_from(index).unwrap(), 1),
            descriptor_generation: 1,
            action: action(
                u16::try_from(index).unwrap(),
                1,
                u64::try_from(index + 1).unwrap(),
            ),
        })
        .collect();
    assert_eq!(
        descriptor_overlay_projection(&excessive, &descriptors, bounds()),
        Err(DescriptorOverlayError::CapacityExceeded)
    );
}

#[test]
fn stable_nodes_limit_selection_damage_to_the_old_and_new_markers() {
    let descriptors = table();
    let before = descriptor_overlay_projection(&candidate(1, 10), &descriptors, bounds()).unwrap();
    let stable = descriptor_overlay_projection(&candidate(1, 10), &descriptors, bounds()).unwrap();
    let after = descriptor_overlay_projection(&candidate(2, 11), &descriptors, bounds()).unwrap();
    let before = CompositorDisplayList {
        output: OUTPUT,
        commands: before.commands,
    };
    let stable = CompositorDisplayList {
        output: OUTPUT,
        commands: stable.commands,
    };
    let after = CompositorDisplayList {
        output: OUTPUT,
        commands: after.commands,
    };

    assert!(compositor_display_list_damage(&before, &stable).is_empty());
    let damage = compositor_display_list_damage(&before, &after);
    assert_eq!(damage.rects.len(), 2);
    assert!(
        damage
            .rects
            .iter()
            .all(|rect| rect.width == 3 && rect.height == 32)
    );
}

#[test]
fn duplicate_generic_node_identity_is_rejected_before_head_lowering() {
    let projection = descriptor_overlay_projection(&candidate(1, 10), &table(), bounds()).unwrap();
    let mut commands = projection.commands;
    commands.push(commands[0].clone());
    assert_eq!(
        output_scene_snapshot_from_committed_in_view(
            OUTPUT,
            1,
            bounds(),
            &[],
            CompositorDisplayList {
                output: OUTPUT,
                commands,
            },
            None,
        )
        .unwrap_err(),
        HeadCompositionPlanError::InvalidSnapshot
    );
}

fn button(pressed: bool) -> InputEventKind {
    InputEventKind::PointerButton {
        button: CHROME_PRIMARY_BUTTON,
        pressed,
    }
}

#[test]
fn only_the_last_presented_exact_target_activates_once() {
    let projection = descriptor_overlay_projection(&candidate(1, 10), &table(), bounds()).unwrap();
    let target = projection.targets[0].clone();
    let point = Point {
        x: f64::from(target.geometry.x + 10),
        y: f64::from(target.geometry.y + 10),
    };
    let seat = SeatId::from_raw(1);
    let device = DeviceId::from_raw(2);
    let mut state = PresentedChromeCaptureState::default();

    assert_eq!(
        resolve_presented_chrome_pointer_event(
            &mut state,
            seat,
            device,
            button(true),
            Some(point),
            Some(OUTPUT),
            12,
            &[],
            None,
            false,
        )
        .unwrap(),
        PresentedChromePointerDisposition::Pass
    );
    assert_eq!(
        resolve_presented_chrome_pointer_event(
            &mut state,
            seat,
            device,
            button(true),
            Some(point),
            Some(OUTPUT),
            12,
            &projection.targets,
            Some(projection.geometry),
            false,
        )
        .unwrap(),
        PresentedChromePointerDisposition::Captured
    );
    assert_eq!(
        resolve_presented_chrome_pointer_event(
            &mut state,
            seat,
            device,
            button(false),
            Some(point),
            Some(OUTPUT),
            12,
            &projection.targets,
            Some(projection.geometry),
            false,
        )
        .unwrap(),
        PresentedChromePointerDisposition::Activated {
            action: target.action,
            activation: 1,
        }
    );
    assert!(state.capture(seat).is_none());

    assert_eq!(
        resolve_presented_chrome_pointer_event(
            &mut state,
            seat,
            device,
            button(true),
            Some(point),
            Some(OUTPUT),
            12,
            &projection.targets,
            Some(projection.geometry),
            true,
        )
        .unwrap(),
        PresentedChromePointerDisposition::Pass
    );
}

#[test]
fn presentation_target_and_device_changes_cancel_or_retain_exact_capture() {
    let projection = descriptor_overlay_projection(&candidate(1, 10), &table(), bounds()).unwrap();
    let target = projection.targets[0].clone();
    let point = Point {
        x: f64::from(target.geometry.x + 10),
        y: f64::from(target.geometry.y + 10),
    };
    let seat = SeatId::from_raw(1);
    let device = DeviceId::from_raw(2);
    let mut state = PresentedChromeCaptureState::default();
    let route = |state: &mut PresentedChromeCaptureState,
                 device,
                 kind,
                 epoch,
                 targets: &[PresentedChromeTarget]| {
        resolve_presented_chrome_pointer_event(
            state,
            seat,
            device,
            kind,
            Some(point),
            Some(OUTPUT),
            epoch,
            targets,
            Some(projection.geometry),
            false,
        )
        .unwrap()
    };

    assert_eq!(
        route(&mut state, device, button(true), 12, &projection.targets),
        PresentedChromePointerDisposition::Captured
    );
    assert_eq!(
        route(
            &mut state,
            DeviceId::from_raw(3),
            button(false),
            12,
            &projection.targets,
        ),
        PresentedChromePointerDisposition::Consumed
    );
    assert!(state.capture(seat).is_some());
    assert_eq!(
        route(&mut state, device, button(false), 13, &projection.targets),
        PresentedChromePointerDisposition::Cancelled
    );

    assert_eq!(
        route(&mut state, device, button(true), 12, &projection.targets),
        PresentedChromePointerDisposition::Captured
    );
    let mut changed = projection.targets.clone();
    changed[0].id.generation += 1;
    changed[0].action.target_generation += 1;
    assert_eq!(
        route(&mut state, device, button(false), 12, &changed),
        PresentedChromePointerDisposition::Cancelled
    );
}
