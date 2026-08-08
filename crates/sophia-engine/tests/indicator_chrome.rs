use sophia_engine::{
    IndicatorChromeAction, PolicyIndicatorPublication, activate_indicator_at,
    layout_indicator_strip, reserve_indicator_strip,
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
