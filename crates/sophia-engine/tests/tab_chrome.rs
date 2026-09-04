use sophia_engine::*;
use sophia_protocol::*;
#[test]
fn neutral_tabs_are_inert_bounded_and_use_standard_gpu_primitives() {
    let group = PolicyTabGroup {
        output: OutputId::from_raw(1),
        group: 4,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 101,
            height: 24,
        },
        focused: true,
        selected: Some(SurfaceId::new(1, 1)),
        members: vec![
            SurfaceId::new(1, 1),
            SurfaceId::new(2, 1),
            SurfaceId::new(3, 1),
        ],
    };
    let bar = tab_bar_projection(&group, 1, None);
    assert!(bar.targets.is_empty());
    let rects = bar
        .commands
        .iter()
        .filter_map(|c| {
            if let CompositorDisplayCommand::Rect(r) = c {
                Some(r.geometry)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(rects.iter().map(|r| r.width).sum::<i32>(), 101);
    assert_eq!(rects[1].x, rects[0].width);
    assert!(bar.commands.iter().all(|c| matches!(
        c,
        CompositorDisplayCommand::Rect(_) | CompositorDisplayCommand::Text(_)
    )));
    let mut changed = group.clone();
    changed.selected = Some(SurfaceId::new(2, 1));
    let mut commands = Vec::new();
    append_tab_bars(&mut commands, &[changed.clone()], 2, &[bar], group.output);
    assert_eq!(commands, tab_bar_projection(&changed, 2, None).commands);
}
