#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;

use sophia_backend_live::{
    LiveProductionHeadCompositionFrame, live_present_head_frames_capture_image,
    live_surface_routes_to_output,
};
use sophia_engine::{
    HeadRenderTarget, RenderHeadId, SurfaceChromeStyle, applicable_output_retirement_set,
    build_output_head_plans, output_scene_snapshot_from_committed_in_view,
    surface_chrome_display_list,
};
use sophia_protocol::{
    BufferSource, CommittedSurfaceState, DRM_FORMAT_MOD_INVALID, OutputHeadMapping, OutputId,
    OutputTransform, Rect, Region, Size, SurfaceId,
};
use sophia_renderer_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveOwnedDmaBufPlane, LiveOwnedHeadCompositionSource,
    LiveOwnedHeadCompositionSourceKind, LiveOwnedMixedCompositionLayer,
    LiveOwnedMultiPlaneDmaBufFrame, LiveRendererImageId, lower_head_composition_plan,
};

// The live failure moved an older Kitty from x=1919 to x=3187 on output 1.
// Output 2 begins at x=2560, but policy confines this scrolling strip to 1.
const OUTPUT: OutputId = OutputId::from_raw(1);
const SURFACE: SurfaceId = SurfaceId::new(6291470, 1);
const IMAGE: LiveRendererImageId = LiveRendererImageId::from_raw(1636);
const VIEW: Rect = Rect {
    x: 0,
    y: 0,
    width: 2560,
    height: 1440,
};
const PREVIOUS: Rect = Rect {
    x: 1919,
    y: 41,
    width: 1258,
    height: 1390,
};

fn lowered_frames(x: i32) -> Vec<(OutputId, Vec<LiveProductionHeadCompositionFrame>)> {
    let geometry = Rect { x, ..PREVIOUS };
    let size = Size {
        width: geometry.width,
        height: geometry.height,
    };
    let source = BufferSource::DmaBuf { handle: 28 };
    let viewports = [
        (OUTPUT, VIEW),
        (
            OutputId::from_raw(2),
            Rect {
                x: 2560,
                y: 0,
                width: 1920,
                height: 1080,
            },
        ),
    ];
    let owners = BTreeMap::from([(SURFACE, OUTPUT)]);
    let mut outputs =
        applicable_output_retirement_set(&viewports, Some(PREVIOUS), geometry).unwrap();
    outputs.retain(|output| {
        live_surface_routes_to_output(SURFACE, &owners, &BTreeSet::new(), *output)
    });
    assert_eq!(
        outputs,
        vec![OUTPUT],
        "old bounds owe a clearing repaint even when new pixels are offscreen"
    );
    let committed = [CommittedSurfaceState::with_source(
        SURFACE,
        2,
        geometry,
        source,
        size,
        Region::single(geometry),
    )];
    let display_list = surface_chrome_display_list(
        OUTPUT,
        &[SURFACE],
        &committed,
        None,
        SurfaceChromeStyle::default(),
    )
    .unwrap();
    let snapshot = output_scene_snapshot_from_committed_in_view(
        OUTPUT,
        1636,
        VIEW,
        &committed,
        display_list,
        None,
    )
    .unwrap();
    let targets = [2560, 1280].map(|width| HeadRenderTarget {
        head: RenderHeadId::from_raw(width as u64),
        output: OUTPUT,
        target_generation: 2,
        native_size: Size {
            width,
            height: width * 9 / 16,
        },
        scale: 1,
        refresh_millihz: 60_000,
        transform: OutputTransform::Normal,
        mapping: OutputHeadMapping::Fit,
    });
    let sources = [LiveOwnedHeadCompositionSource {
        surface: SURFACE,
        source,
        kind: LiveOwnedHeadCompositionSourceKind::DmaBuf {
            image_id: IMAGE,
            frame: LiveOwnedMultiPlaneDmaBufFrame {
                width: size.width as u32,
                height: size.height as u32,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                modifier: DRM_FORMAT_MOD_INVALID,
                plane_count: 1,
                planes: [
                    Some(LiveOwnedDmaBufPlane {
                        fd: File::open("/dev/null").unwrap().into(),
                        offset: 0,
                        stride: size.width as u32 * 4,
                    }),
                    None,
                    None,
                    None,
                ],
            },
        },
    }];
    let heads = build_output_head_plans(&snapshot, &targets)
        .unwrap()
        .iter()
        .map(|plan| LiveProductionHeadCompositionFrame {
            head: plan.head,
            scene_generation: plan.scene_generation,
            target_generation: plan.target_generation,
            mapping: plan.mapping,
            logical_content_checksum: plan.logical_content_checksum,
            frame: lower_head_composition_plan(plan, &sources).unwrap(),
        })
        .collect();
    vec![(OUTPUT, heads)]
}

#[test]
fn old_visible_bounds_do_not_make_offscreen_pixels_a_copy_present() {
    let frames = lowered_frames(3187);
    assert!(
        frames[0].1[0].frame.output_damage_snapshot.is_some(),
        "the source head still has a real repaint"
    );
    assert!(!live_present_head_frames_capture_image(&frames, IMAGE));
    assert!(
        frames.iter().flat_map(|(_, heads)| heads).all(|head| head
            .frame
            .layers
            .iter()
            .all(|layer| !matches!(layer, LiveOwnedMixedCompositionLayer::DmaBuf { .. }))),
        "neither mirror head can capture the invisible candidate"
    );
}

#[test]
fn a_partially_visible_scrolling_window_still_owns_its_copy_present() {
    for x in [1919, 2559] {
        let frames = lowered_frames(x);
        assert!(live_present_head_frames_capture_image(&frames, IMAGE));
        assert!(
            !live_present_head_frames_capture_image(&frames, LiveRendererImageId::from_raw(1632)),
            "another frame's renderer image cannot satisfy capture ownership"
        );
    }
}
