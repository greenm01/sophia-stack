#![cfg(test)]

use super::*;
use std::fs::File;
use std::os::fd::OwnedFd;

fn fd() -> OwnedFd {
    File::open("/dev/null").unwrap().into()
}

fn layer(width: u32, target: Rect) -> LiveRetainedDmaBufLayer {
    LiveRetainedDmaBufLayer {
        frame: LiveOwnedMultiPlaneDmaBufFrame {
            width,
            height: 48,
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            modifier: DRM_FORMAT_MOD_INVALID,
            plane_count: 1,
            planes: [
                Some(LiveOwnedDmaBufPlane {
                    fd: fd(),
                    offset: 0,
                    stride: width * 4,
                }),
                None,
                None,
                None,
            ],
        },
        placement: LiveCompositionPlacement {
            target,
            clip: None,
            transform: Transform::IDENTITY,
            alpha: 1.0,
        },
    }
}

#[test]
fn displayed_surface_keeps_frame_and_placement_in_one_snapshot() {
    let surface = SurfaceId::new(1, 1);
    let first = TransactionId::from_raw(2);
    let second = TransactionId::from_raw(3);
    let third = TransactionId::from_raw(4);
    let half = Rect {
        x: 0,
        y: 0,
        width: 64,
        height: 48,
    };
    let full = Rect {
        x: 0,
        y: 0,
        width: 128,
        height: 48,
    };
    let mut displayed = BTreeMap::new();

    assert_eq!(
        replace_displayed_surface(&mut displayed, surface, first, layer(64, half)),
        None
    );
    assert_eq!(displayed[&surface].layer.placement.target, half);
    assert_eq!(
        replace_displayed_surface(&mut displayed, surface, second, layer(64, half)),
        Some(first)
    );
    assert_eq!(
        replace_displayed_surface(&mut displayed, surface, third, layer(128, full)),
        Some(second)
    );
    assert_eq!(displayed[&surface].layer.frame.width, 128);
    assert_eq!(displayed[&surface].layer.placement.target, full);
    assert_eq!(displayed[&surface].retained_transaction, Some(third));
}

#[test]
fn displayed_layer_reports_scaling_and_clones_exact_placement() {
    let target = Rect {
        x: 64,
        y: 0,
        width: 128,
        height: 48,
    };
    let scaled = layer(64, target);
    assert!(!scaled.has_unit_scale());

    let coherent = layer(128, target);
    assert!(coherent.has_unit_scale());
    let cloned = coherent.try_clone().unwrap();
    assert_eq!(cloned.frame.width, coherent.frame.width);
    assert_eq!(cloned.placement.target, coherent.placement.target);
    assert_eq!(cloned.placement.clip, coherent.placement.clip);
    assert_eq!(cloned.placement.transform, coherent.placement.transform);
    assert_eq!(cloned.placement.alpha, coherent.placement.alpha);
}
