use sophia_engine::{
    CompositorDisplayCommand, CompositorDisplayList, IndicatorChromeAction,
    PolicyIndicatorPublication, activate_indicator_at, compositor_display_list_damage,
    indicator_strip_display_command, layout_indicator_strip, project_indicator_chrome_targets,
    reserve_indicator_strip,
};
use sophia_protocol::{
    OutputId, Point, PolicyProjectionIndicator, PolicyProjectionOutputStatus, Rect, WmActionId,
};

fn publication(generation: u64) -> PolicyIndicatorPublication {
    PolicyIndicatorPublication {
        generation,
        connection_epoch: Some(4),
        projection_commit_serial: 7,
        indicators: vec![PolicyProjectionIndicator {
            output: OutputId::from_raw(1),
            slot: 0,
            indicator: 9,
            action: Some(WmActionId::from_raw(11)),
            state_bits: 1,
            label: "1".into(),
        }],
        output_statuses: vec![PolicyProjectionOutputStatus {
            output: OutputId::from_raw(1),
            focus_bits: 1,
            layout: "Scroller".into(),
        }],
    }
}

fn display_list(publication: &PolicyIndicatorPublication) -> CompositorDisplayList {
    CompositorDisplayList {
        output: OutputId::from_raw(1),
        commands: vec![
            indicator_strip_display_command(
                publication,
                OutputId::from_raw(1),
                Rect {
                    x: 0,
                    y: 0,
                    width: 300,
                    height: 200,
                },
            )
            .unwrap(),
        ],
    }
}

#[test]
fn strip_layout_and_action_share_one_publication_identity() {
    let current = publication(3);
    let strip = layout_indicator_strip(
        &current,
        OutputId::from_raw(1),
        Rect {
            x: 0,
            y: 0,
            width: 300,
            height: 200,
        },
    )
    .unwrap();
    assert_eq!(strip.geometry.height, 14);
    assert_eq!(
        reserve_indicator_strip(Rect {
            x: 0,
            y: 0,
            width: 300,
            height: 200
        }),
        Some(Rect {
            x: 0,
            y: 14,
            width: 300,
            height: 186
        })
    );
    assert_eq!(strip.labels[0].1, "1");
    assert_eq!(strip.status.as_ref().unwrap().1, "Scroller");
    assert_eq!(
        activate_indicator_at(&current, &strip.hit_targets, Point { x: 2.0, y: 2.0 }),
        IndicatorChromeAction::Activated {
            output: OutputId::from_raw(1),
            action: WmActionId::from_raw(11),
        }
    );
    assert_eq!(
        activate_indicator_at(
            &publication(4),
            &strip.hit_targets,
            Point { x: 2.0, y: 2.0 }
        ),
        IndicatorChromeAction::Stale
    );
}

#[test]
fn disconnected_policy_retains_an_opaque_actionless_strip() {
    let mut disconnected = publication(4);
    disconnected.connection_epoch = None;
    let command = indicator_strip_display_command(
        &disconnected,
        OutputId::from_raw(1),
        Rect {
            x: 0,
            y: 0,
            width: 300,
            height: 200,
        },
    )
    .unwrap();
    let CompositorDisplayCommand::IndicatorStrip(strip) = command else {
        panic!("policy-loss reservation was not an indicator strip")
    };
    assert_eq!(strip.strip.geometry.height, 14);
    assert!(strip.strip.labels.is_empty());
    assert!(strip.strip.hit_targets.is_empty());
}

#[test]
fn semantic_changes_damage_the_retained_strip_and_stable_state_does_not() {
    let before = display_list(&publication(3));
    let stable = display_list(&publication(3));
    let after = display_list(&publication(4));

    assert!(compositor_display_list_damage(&before, &stable).is_empty());
    assert_eq!(
        compositor_display_list_damage(&before, &after).rects,
        [
            Rect {
                x: 0,
                y: 0,
                width: 300,
                height: 14,
            },
            Rect {
                x: 0,
                y: 0,
                width: 300,
                height: 14,
            },
        ]
    );
}

#[test]
fn head_native_hit_targets_project_back_to_the_logical_strip() {
    let source = Rect {
        x: 100,
        y: 40,
        width: 1_000,
        height: 28,
    };
    let destination = Rect {
        x: 20,
        y: 30,
        width: 500,
        height: 14,
    };
    let target = sophia_engine::IndicatorChromeHitTarget {
        publication_generation: 7,
        connection_epoch: 8,
        projection_commit_serial: 9,
        output: OutputId::from_raw(1),
        indicator: 10,
        action: Some(WmActionId::from_raw(11)),
        geometry: Rect {
            x: 100,
            y: 40,
            width: 200,
            height: 28,
        },
    };

    let projected = project_indicator_chrome_targets(&[target], source, destination);

    assert_eq!(
        projected[0].geometry,
        Rect {
            x: 20,
            y: 30,
            width: 100,
            height: 14,
        }
    );
    assert!(project_indicator_chrome_targets(&projected, Rect::default(), destination).is_empty());
}
