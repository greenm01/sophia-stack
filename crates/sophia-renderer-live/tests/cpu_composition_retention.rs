use std::sync::Arc;

use sophia_engine::{CompositorDisplayCommand, CompositorDisplayList, HeadlessOutput};
use sophia_protocol::{
    BufferSource, CommittedSurfaceState, OutputId, Rect, Region, Size, SurfaceId,
};
use sophia_renderer_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveCpuBufferSource, LiveCpuBufferUpdate,
    LiveProductionCpuScene,
};

#[test]
fn production_scene_reuses_a_retired_frame_while_latest_pixels_are_shared() {
    let output = HeadlessOutput {
        id: OutputId::from_raw(1),
        size: Size {
            width: 4,
            height: 1,
        },
        scale: 1,
    };
    let surface = SurfaceId::new(1, 1);
    let mut committed = [CommittedSurfaceState {
        surface,
        committed_generation: 1,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::CpuBuffer { handle: 1 },
            sophia_protocol::Size {
                width: 1,
                height: 1,
            },
        ),
        damage: Region::empty(),
    }];
    let display_list = CompositorDisplayList {
        output: output.id,
        commands: vec![CompositorDisplayCommand::Surface { surface }],
    };
    let mut scene = LiveProductionCpuScene::new(output.size);
    scene
        .apply_updates([LiveCpuBufferUpdate::Replace(LiveCpuBufferSource {
            handle: 1,
            size: Size {
                width: 1,
                height: 1,
            },
            stride: 4,
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            generation: 1,
            bytes: Arc::new(vec![0x11; 4]),
        })])
        .unwrap();
    let first = scene
        .compose_display_list(output, &committed, &display_list, None)
        .unwrap()
        .frame
        .bytes
        .clone();
    let first_allocation = first.as_ptr();

    committed[0].committed_generation = 2;
    scene
        .apply_updates([LiveCpuBufferUpdate::Replace(LiveCpuBufferSource {
            handle: 1,
            size: Size {
                width: 1,
                height: 1,
            },
            stride: 4,
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            generation: 2,
            bytes: Arc::new(vec![0x22; 4]),
        })])
        .unwrap();
    let second = scene
        .compose_display_list(output, &committed, &display_list, None)
        .unwrap()
        .frame
        .bytes
        .clone();
    assert_ne!(second.as_ptr(), first_allocation);
    drop(first);

    committed[0].committed_generation = 3;
    scene
        .apply_updates([LiveCpuBufferUpdate::Replace(LiveCpuBufferSource {
            handle: 1,
            size: Size {
                width: 1,
                height: 1,
            },
            stride: 4,
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            generation: 3,
            bytes: Arc::new(vec![0x33; 4]),
        })])
        .unwrap();
    let third = scene
        .compose_display_list(output, &committed, &display_list, None)
        .unwrap();

    assert_eq!(third.frame.bytes.as_ptr(), first_allocation);
    assert_eq!(&third.frame.bytes[..4], &[0x33; 4]);
    assert_eq!(&third.frame.bytes[4..], &[0; 12]);
    assert_eq!(second.as_ref()[..4], [0x22; 4]);
}

#[test]
fn production_scene_reuses_shared_latest_pixels_for_an_unchanged_snapshot() {
    let output = HeadlessOutput {
        id: OutputId::from_raw(1),
        size: Size {
            width: 4,
            height: 1,
        },
        scale: 1,
    };
    let display_list = CompositorDisplayList::empty(output.id);
    let mut scene = LiveProductionCpuScene::new(output.size);
    let observer = scene
        .compose_display_list(output, &[], &display_list, None)
        .unwrap()
        .frame
        .bytes
        .clone();

    let unchanged = scene
        .compose_display_list(output, &[], &display_list, None)
        .unwrap();

    assert!(Arc::ptr_eq(&observer, &unchanged.frame.bytes));
}
