use std::sync::Arc;

use sophia_engine::{
    CompositorNodeId, HeadCompositorIndicatorStrip, IndicatorChromeHitTarget, IndicatorChromeStrip,
};
use sophia_protocol::{
    OutputId, POLICY_INDICATOR_STATE_ACTIVE, POLICY_INDICATOR_STATE_OCCUPIED,
    POLICY_INDICATOR_STATE_URGENT, Rect, WmActionId,
};
use sophia_renderer_live::{
    INDICATOR_STRIP_CACHE_MAX_BYTES, INDICATOR_STRIP_CACHE_MAX_ENTRIES,
    INDICATOR_STRIP_FONT_RELEASE, INDICATOR_STRIP_FONT_SHA256, IndicatorStripRasterCache,
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
};

fn strip(generation: u64) -> HeadCompositorIndicatorStrip {
    let output = OutputId::from_raw(1);
    let cell = Rect {
        x: 0,
        y: 0,
        width: 48,
        height: 14,
    };
    HeadCompositorIndicatorStrip {
        node: CompositorNodeId::IndicatorStrip { output },
        generation,
        strip: IndicatorChromeStrip {
            output,
            geometry: Rect {
                x: 0,
                y: 0,
                width: 200,
                height: 14,
            },
            labels: vec![(
                cell,
                "main".into(),
                POLICY_INDICATOR_STATE_OCCUPIED
                    | POLICY_INDICATOR_STATE_ACTIVE
                    | POLICY_INDICATOR_STATE_URGENT,
            )],
            status: Some((
                Rect {
                    x: 100,
                    y: 0,
                    width: 100,
                    height: 14,
                },
                "Scroller".into(),
                0,
            )),
            hit_targets: vec![IndicatorChromeHitTarget {
                publication_generation: generation,
                connection_epoch: 2,
                output,
                indicator: 4,
                action: Some(WmActionId::from_raw(5)),
                geometry: cell,
            }],
        },
    }
}

#[test]
fn included_font_raster_is_deterministic_xrgb_and_contains_semantic_markers() {
    assert_eq!(INDICATOR_STRIP_FONT_RELEASE, "JetBrains Mono 2.304");
    assert_eq!(
        INDICATOR_STRIP_FONT_SHA256,
        "fb3b2575d7b0657359707993288f12a7360344d39387bb26050e276d61f6bd2a"
    );
    let mut first_cache = IndicatorStripRasterCache::default();
    let mut second_cache = IndicatorStripRasterCache::default();
    let first = first_cache.raster_for(&strip(7)).unwrap();
    let second = second_cache.raster_for(&strip(7)).unwrap();

    assert_eq!(first.size.width, 200);
    assert_eq!(first.size.height, 14);
    assert_eq!(first.stride, 800);
    assert_eq!(first.format, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888);
    assert_eq!(first.bytes.as_ref(), second.bytes.as_ref());
    assert!(first.bytes.chunks_exact(4).all(|pixel| pixel[3] == 0xff));
    assert!(
        first
            .bytes
            .chunks_exact(4)
            .any(|pixel| pixel == [0xff, 0xb7, 0x70, 0xff])
    );
    assert!(
        first
            .bytes
            .chunks_exact(4)
            .any(|pixel| pixel == [0xb0, 0xb6, 0xff, 0xff])
    );
}

#[test]
fn cache_reuses_exact_semantics_and_evicts_without_invalidating_frames() {
    let mut cache = IndicatorStripRasterCache::default();
    let first = cache.raster_for(&strip(1)).unwrap();
    let shared_pixels = Arc::clone(&first.bytes);
    let repeated = cache.raster_for(&strip(1)).unwrap();
    assert_eq!(first.handle, repeated.handle);
    assert!(Arc::ptr_eq(&first.bytes, &repeated.bytes));

    for generation in 2..=u64::try_from(INDICATOR_STRIP_CACHE_MAX_ENTRIES + 2).unwrap() {
        cache.raster_for(&strip(generation)).unwrap();
    }
    let stats = cache.stats();
    assert_eq!(stats.entries, INDICATOR_STRIP_CACHE_MAX_ENTRIES);
    assert!(stats.bytes <= INDICATOR_STRIP_CACHE_MAX_BYTES);
    assert!(stats.hits >= 1);
    assert!(stats.evictions >= 2);
    assert!(!shared_pixels.is_empty());
}
